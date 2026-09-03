# Dev process tier

Group the processes a developer actually cares about — coding agents, editors,
dev servers, databases, build tools and container runtimes — at the top of the
process table, subgrouped by category, and mark each with a glyph.

## Problem

The process table sorts by CPU, memory, name or PID. None of those answer the
question a developer opens a system monitor to ask: *is my dev server up, which
agent is running, what is my editor costing me?* Those processes are scattered
through several hundred rows, and the ones that matter are frequently idle, so
the CPU sort buries them.

## Behaviour

### Grouping is tied to the COMMAND sort

When the sort mode is COMMAND — which becomes the default — dev processes are
grouped above everything else, subgrouped by category in a fixed order:

```
◉ agent
▣ editor
▸ server
▤ database
◈ build
▩ container
  (everything else, untiered)
```

Within each subgroup, and within the untiered remainder, processes stay in the
sort mode's own order.

No separator rows are drawn between subgroups. Every row remains one process, so
selection, navigation, pin and kill continue to index the table directly. The
glyph and the grouping together carry the category.

Under the CPU, MEM and PID sorts the tier is inert: those sorts remain a pure
ranking, so a runaway process is still found where it is expected. The category
*glyph* still shows under every sort mode — it is a property of the process, not
of the sort — so a 400% CPU row is still visibly a build.

Pinned processes continue to float above everything, including the dev tier.

### Detection

Classification is a pure function of the process name and its full argv:

```rust
pub fn classify(name: &str, cmd: &str) -> Option<Category>
```

`None` means the process is not a dev tool, and it stays untiered.

**Helper processes are excluded first.** Electron and Chromium applications run
many child processes — one VS Code window is roughly eight — and tiering all of
them would push eight near-identical rows above the agents and servers the tier
exists to surface. They are identified by Electron's documented convention:

- a `--type=renderer`, `--type=gpu-process`, `--type=utility` or `--type=zygote`
  flag in argv, or
- `Helper` in the process name.

**Matching is word-exact, never substring.** A word set is built from:

- the process name,
- the basename of `argv[0]`,
- every argv token that does not begin with `-`, and the basename of each.

Each word is compared, case-insensitively and in full, against the category
tables. Categories are tested in declaration order and the first hit wins.

Substring matching is the trap this avoids. `"vim"` as a substring matches
`vimeo-downloader`; `"code"` matches `vscode-ripgrep` and much of
`/usr/local/lib`. Exact word matching costs nothing and eliminates that entire
class of false positive.

Worked examples:

| Command line | Words | Result |
|---|---|---|
| `node /opt/next/dist/bin/next dev` | node, next, dev | `next` → Server |
| `nvim src/main.rs` | nvim, src/main.rs | `nvim` → Editor |
| `uvicorn main:app --reload` | uvicorn, main:app | `uvicorn` → Server |
| `php artisan serve` | php, artisan, serve | `artisan` → Server |
| `vimeo-downloader` | vimeo-downloader | no match |
| `node` | node | no match |
| `Code Helper (Renderer) --type=renderer` | — | helper, excluded |

A bare runtime — `node`, `python3`, `ruby`, `deno` — deliberately matches
nothing. The runtime name carries no information; the argv does.

### Category tables

Case-insensitive, exact word match.

**Agent**: `claude`, `codex`, `pi`, `aider`, `cursor-agent`, `copilot`,
`copilot-language-server`, `gemini`, `goose`, `opencode`, `cline`, `devin`

**Editor**: `nvim`, `vim`, `vi`, `zed`, `code`, `cursor`, `windsurf`, `emacs`,
`helix`, `hx`, `subl`, `sublime_text`, `kak`, `kakoune`, `idea`, `pycharm`,
`webstorm`, `goland`, `clion`, `rustrover`, `rider`

**Server**: `next`, `vite`, `nuxt`, `astro`, `remix`, `nodemon`,
`webpack-dev-server`, `http-server`, `live-server`, `serve`, `rails`, `puma`,
`unicorn`, `rack`, `sinatra`, `manage.py`, `flask`, `uvicorn`, `gunicorn`,
`hypercorn`, `daphne`, `artisan`, `nginx`, `caddy`, `httpd`, `apache2`,
`wrangler`, `phoenix`

**Database**: `postgres`, `postgresql`, `mysqld`, `mariadbd`, `redis-server`,
`valkey-server`, `mongod`, `clickhouse`, `cockroach`, `influxd`, `cassandra`,
`elasticsearch`, `opensearch`, `etcd`, `memcached`, `neo4j`, `surreal`

**Build**: `cargo`, `rustc`, `tsc`, `esbuild`, `webpack`, `rollup`, `turbo`,
`jest`, `vitest`, `pytest`, `gradle`, `mvn`, `maven`, `make`, `ninja`, `cmake`,
`bazel`, `go`, `swiftc`, `clang`, `gcc`, `sccache`, `eslint`, `prettier`,
`biome`, `ruff`, `mypy`

**Container**: `docker`, `dockerd`, `containerd`, `runc`, `colima`, `podman`,
`lima`, `limactl`, `orbstack`, `qemu`, `qemu-system-x86_64`, `vfkit`,
`virtiofsd`, `minikube`

### Display

A single glyph prefixes the COMMAND cell, coloured with one new palette entry
`THEME.category` so the glyphs read as one class of marker; the shape says which
category. All glyphs come from Geometric Shapes and Block Elements — no emoji,
no Nerd Font — and are single-width in any monospace font.

`◆` is deliberately excluded: it is already the pin marker.

Untiered rows are indented by the same two columns so the command text stays
aligned down the table.

The filter gains the category name as a searchable field, so `/agent` narrows to
agents. This makes the categories discoverable without a legend.

## Implementation

### New module: `src/category.rs`

```rust
/// What kind of dev tool a process is. Declaration order is display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category { Agent, Editor, Server, Database, Build, Container }

impl Category {
    /// The glyph shown in the COMMAND column.
    pub const fn glyph(self) -> &'static str;
    /// The name the filter matches against.
    pub const fn as_str(self) -> &'static str;
}

/// Classify from process name and full argv. `None` for anything that is not a
/// dev tool, and for the helper processes of one.
pub fn classify(name: &str, cmd: &str) -> Option<Category>;
```

Everything in this module is a pure function of two `&str`. It touches no
system state, so the whole detection table is testable against fixed argv
strings.

### `src/process.rs`

- `ProcessInfo` gains `category: Option<Category>`.
- `refresh` classifies during the full process build.

Classification rides the existing cheap path for free: the CPU-only refresh
reuses the whole `ProcessInfo`, so `classify` runs only on a full refresh (every
10 seconds) and for newly seen pids, not on every 2-second tick.

- `sort_processes` under `SortMode::Name` sorts by `cmd`, not `name`. The
  COMMAND sort should sort by what the COMMAND column displays; sorting `next
  dev` under "node" was an existing inconsistency that becomes prominent once
  this is the default sort.
- `ProcessMonitor::new` defaults to `SortMode::Name`.

### `src/app.rs`

- `App::new` defaults to `SortMode::Name`.
- `get_filtered_processes` already stable-sorts pinned rows to the top. It
  becomes a two-key sort:

```rust
processes.sort_by_key(|p| (
    !self.pinned_pids.contains(&p.pid),  // pinned first (false < true)
    self.tier_rank(p),                   // category order, then the rest
));
```

`tier_rank` returns `p.category.map_or(usize::MAX, |c| c as usize)` under
`SortMode::Name`, and a constant under every other sort mode. The sort is
stable, so the collector's ordering survives inside each group.

- `update_filtered_indices` also matches `category.as_str()`.

### `src/ui.rs`

- `process_row` prefixes the COMMAND cell with a styled glyph span, or two
  spaces of padding when the process is untiered.
- `cmd_col_width` shrinks by two to account for the prefix, so breakout content
  still wraps correctly.

### `src/theme.rs`

- One new field, `category: Color::Blue`.

## Testing

- `classify` table test over real argv strings, covering every worked example
  above including the false positives (`vimeo-downloader`, bare `node`) and the
  helper exclusion.
- Category order is total and matches the documented display order.
- Tier ordering holds under `SortMode::Name`: an idle agent sorts above a busy
  untiered process.
- The tier is inert under `SortMode::Cpu`: the same two processes sort by CPU.
- Pinned processes still float above the dev tier.

## Deliberate omissions

**Port-based server detection.** A process listening on a port is suggestive of
a server, but tagging on that alone would sweep in `rapportd`, `sshd` and
`mDNSResponder`. Restricting to a dev port range does not help — those are not
reserved. The name tables are the whole signal. If a framework is missed, the
fix is one entry in a table.

**Parent-based helper roll-up.** Dropping any process whose parent shares its
category would catch helpers we have not heard of, not just Electron's, but it
needs a parent pid on `ProcessInfo` and a second pass — and it would wrongly
hide a child that is the interesting one, such as the `next-server` child
actually holding port 3000 under its `next dev` parent.

**User-configurable tables.** The tables are compile-time constants. A config
file is worth adding when someone actually needs a tool that is not listed, not
before.
