# Dev Process Tier Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Group the processes a developer cares about — coding agents, editors, dev servers, databases, build tools and container runtimes — above the rest of the process table under the COMMAND sort, marked with a glyph.

**Architecture:** A new pure module classifies a process from its name and argv. `ProcessInfo` carries the resulting `Option<Category>`, computed in the collector thread's full refresh. `App::get_filtered_processes` gains a second stable-sort key that groups by category, inert under every sort mode but COMMAND. The UI prefixes the COMMAND cell with a coloured glyph.

**Tech Stack:** Rust 2024, ratatui 0.29, sysinfo 0.32. No new dependencies.

## Global Constraints

- Australian English in comments and documentation.
- Version control is GitButler (`but`), never raw `git` write commands. Every commit targets `feat/dev-process-tier`.
- `cargo clippy --all-targets` must stay clean. The repo denies `clippy::pedantic`, `clippy::nursery`, plus `unwrap_used`, `expect_used`, `indexing_slicing`, `arithmetic_side_effects`, `as_conversions`, `string_slice` and `panic`. `clippy.toml` exempts tests from the unwrap, expect, panic and indexing lints.
- No `as` casts outside `src/convert.rs`. Convert through `From`/`TryFrom`, or add a named method.
- `cargo fmt` before every commit.
- Category display order is fixed and total: Agent, Editor, Server, Database, Build, Container.

## Diagrams

**None.** The `diagram-design` skill was loaded and applied to the plan as a whole and to each of the five tasks.

- **Plan-level architecture schematic — rejected.** The pipeline is four stages in a straight line (`classify` → `ProcessInfo.category` → two-key stable sort → glyph render), one per file, with no branching, fan-in, shared state or failure path. The skill's §2 gate — *would the reader learn more from this than from a well-written paragraph?* — answers no, and its "don't use for" list names one-shape diagrams explicitly. The **Architecture** line above carries it.
- **Task 1 (classification) — rejected.** The content is the word-derivation rule and six lookup tables. The skill routes lists of things to a table; the worked-examples table in the spec already does this better than a schematic would.
- **Task 2 (plumbing) — rejected.** One field added to a struct.
- **Task 3 (tier sort) — rejected.** The content is a before/after ordering, which the skill routes explicitly to a table.
- **Task 4 (glyph render) — rejected.** The output is literally terminal text; the ASCII mock-up in the spec is a more faithful representation than an SVG of a terminal would be.
- **Task 5 (manual verification) — rejected.** A checklist.

---

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `src/category.rs` | **New.** `Category` enum, per-category word tables, `classify`. Pure — no system access, no `self`. | 1 |
| `src/main.rs` | `mod category;` declaration only. | 1 |
| `src/process.rs` | Carries `category` on `ProcessInfo`, classifies during full refresh, sorts by `cmd` under COMMAND, defaults to COMMAND. | 2 |
| `src/app.rs` | Two-key tier sort, COMMAND default, filter matches category name. | 3 |
| `src/theme.rs` | One new palette entry for the glyph colour. | 4 |
| `src/ui.rs` | Renders the glyph prefix, adjusts the breakout wrap width. | 4 |

`src/category.rs` is deliberately a module of its own rather than an addition to `process.rs`. `process.rs` is already 560 lines and owns system interaction; classification is a pure string function whose entire value is that it can be tested against fixed argv strings with no `System` in sight.

## Parallel Waves

Agents share one working tree (GitButler; linked worktrees unsupported). Safety comes from disjoint write-sets, not isolation.

**Shared write targets**

| File | Tasks | Collision |
|---|---|---|
| `src/main.rs` | 1 | none — only Task 1 touches it |

No file appears in two write-sets. There is nothing to hoist.

**Schedule**

| Wave | Tasks | Blocked on | Prep before dispatch |
|---|---|---|---|
| 1 | 1 | — | — |
| 2 | 2 | 1 | — |
| 3 | 3, 4 | 1, 2 | — |
| 4 | 5 | 1, 2, 3, 4 | — |

**Serial tail:** Task 5 last — it consumes nothing but verifies the behaviour Tasks 1–4 build.

**Not worth dispatching in parallel.** Only wave 3 holds two tasks, and both are under twenty lines of production code. The dispatch and review overhead exceeds the saving. Run all five in order.

---

### Task 1: Classification module

**Files:**
- Create: `src/category.rs`
- Modify: `src/main.rs:1-9` (add `mod category;` in alphabetical position, after `mod app;`)
- Test: `src/category.rs` (`#[cfg(test)] mod tests` at the foot of the file — this codebase puts unit tests in the file under test, see `src/convert.rs:47`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub enum Category { Agent, Editor, Server, Database, Build, Container }` — derives `Debug, Clone, Copy, PartialEq, Eq`
  - `pub const fn Category::glyph(self) -> &'static str`
  - `pub const fn Category::as_str(self) -> &'static str`
  - `pub const fn Category::rank(self) -> usize`
  - `pub const Category::ALL: [Category; 6]`
  - `pub fn classify(name: &str, cmd: &str) -> Option<Category>`

- [ ] **Step 1: Create the module with the enum and its tables**

Create `src/category.rs`:

```rust
//! Classify a process as a developer tool from its name and command line.
//!
//! Every function here is a pure function of two `&str`. Nothing touches system
//! state, so the whole detection table is testable against fixed argv strings.

/// What kind of dev tool a process is. Declaration order is display order: the
/// process table groups by this order under the COMMAND sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Agent,
    Editor,
    Server,
    Database,
    Build,
    Container,
}

impl Category {
    /// Every category, in display order.
    pub const ALL: [Self; 6] = [
        Self::Agent,
        Self::Editor,
        Self::Server,
        Self::Database,
        Self::Build,
        Self::Container,
    ];

    /// Position in the display order. Written out rather than cast from the
    /// discriminant, because `as` is denied outside `convert.rs` and because an
    /// explicit number cannot drift from `ALL` unnoticed — `rank_matches_all`
    /// asserts the two agree.
    pub const fn rank(self) -> usize {
        match self {
            Self::Agent => 0,
            Self::Editor => 1,
            Self::Server => 2,
            Self::Database => 3,
            Self::Build => 4,
            Self::Container => 5,
        }
    }

    /// Glyph shown in the COMMAND column. Geometric Shapes and Block Elements
    /// only, so it renders single-width in any monospace font without a Nerd
    /// Font. `◆` is deliberately absent: it is already the pin marker.
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Agent => "◉",
            Self::Editor => "▣",
            Self::Server => "▸",
            Self::Database => "▤",
            Self::Build => "◈",
            Self::Container => "▩",
        }
    }

    /// Name the filter matches against.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Editor => "editor",
            Self::Server => "server",
            Self::Database => "database",
            Self::Build => "build",
            Self::Container => "container",
        }
    }

    /// The words that identify this category.
    const fn words(self) -> &'static [&'static str] {
        match self {
            Self::Agent => AGENT,
            Self::Editor => EDITOR,
            Self::Server => SERVER,
            Self::Database => DATABASE,
            Self::Build => BUILD,
            Self::Container => CONTAINER,
        }
    }
}

const AGENT: &[&str] = &[
    "claude",
    "codex",
    "pi",
    "aider",
    "cursor-agent",
    "copilot",
    "copilot-language-server",
    "gemini",
    "goose",
    "opencode",
    "cline",
    "devin",
];

const EDITOR: &[&str] = &[
    "nvim",
    "vim",
    "vi",
    "zed",
    "code",
    "cursor",
    "windsurf",
    "emacs",
    "helix",
    "hx",
    "subl",
    "sublime_text",
    "kak",
    "kakoune",
    "idea",
    "pycharm",
    "webstorm",
    "goland",
    "clion",
    "rustrover",
    "rider",
];

const SERVER: &[&str] = &[
    "next",
    "vite",
    "nuxt",
    "astro",
    "remix",
    "nodemon",
    "webpack-dev-server",
    "http-server",
    "live-server",
    "serve",
    "rails",
    "puma",
    "unicorn",
    "rack",
    "sinatra",
    "manage.py",
    "flask",
    "uvicorn",
    "gunicorn",
    "hypercorn",
    "daphne",
    "artisan",
    "nginx",
    "caddy",
    "httpd",
    "apache2",
    "wrangler",
    "phoenix",
];

const DATABASE: &[&str] = &[
    "postgres",
    "postgresql",
    "mysqld",
    "mariadbd",
    "redis-server",
    "valkey-server",
    "mongod",
    "clickhouse",
    "cockroach",
    "influxd",
    "cassandra",
    "elasticsearch",
    "opensearch",
    "etcd",
    "memcached",
    "neo4j",
    "surreal",
];

const BUILD: &[&str] = &[
    "cargo", "rustc", "tsc", "esbuild", "webpack", "rollup", "turbo", "jest", "vitest", "pytest",
    "gradle", "mvn", "maven", "make", "ninja", "cmake", "bazel", "go", "swiftc", "clang", "gcc",
    "sccache", "eslint", "prettier", "biome", "ruff", "mypy",
];

const CONTAINER: &[&str] = &[
    "docker",
    "dockerd",
    "containerd",
    "runc",
    "colima",
    "podman",
    "lima",
    "limactl",
    "orbstack",
    "qemu",
    "qemu-system-x86_64",
    "vfkit",
    "virtiofsd",
    "minikube",
];
```

- [ ] **Step 2: Write the failing tests**

Append to `src/category.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// `rank` is written out by hand so it cannot be an `as` cast. This is what
    /// stops it drifting from `ALL`, which is the order the table renders in.
    #[test]
    fn rank_matches_all() {
        for (i, category) in Category::ALL.into_iter().enumerate() {
            assert_eq!(category.rank(), i, "{category:?} is out of order");
        }
    }

    /// The glyph must be one column wide, or the command text below it stops
    /// aligning down the table. `◆` must never appear: it is the pin marker.
    #[test]
    fn glyphs_are_single_char_and_never_the_pin_marker() {
        for category in Category::ALL {
            assert_eq!(
                category.glyph().chars().count(),
                1,
                "{category:?} glyph is not one character"
            );
            assert_ne!(category.glyph(), "◆", "{category:?} collides with the pin marker");
        }
    }

    #[test]
    fn classifies_by_process_name() {
        assert_eq!(classify("nvim", "nvim src/main.rs"), Some(Category::Editor));
        assert_eq!(classify("claude", "claude"), Some(Category::Agent));
        assert_eq!(classify("postgres", "postgres -D /db"), Some(Category::Database));
    }

    /// The runtime name carries no information; the argv does. `node` is an
    /// agent, an editor, a server and a build tool depending on what follows it.
    #[test]
    fn classifies_by_argv_not_by_runtime() {
        assert_eq!(
            classify("node", "node /opt/next/dist/bin/next dev"),
            Some(Category::Server),
            "the next binary in argv identifies the server"
        );
        assert_eq!(
            classify("python3", "python3 -m uvicorn main:app --reload"),
            Some(Category::Server)
        );
        assert_eq!(
            classify("php", "php artisan serve"),
            Some(Category::Server)
        );
        assert_eq!(classify("node", "node"), None, "a bare runtime says nothing");
        assert_eq!(classify("python3", "python3"), None);
    }

    /// Substring matching would sweep these in. `vim` is inside
    /// `vimeo-downloader`, `code` is inside `vscode-ripgrep`, and `go` is inside
    /// most of `/usr/local`. Word-exact matching is what keeps them out.
    #[test]
    fn does_not_match_substrings() {
        assert_eq!(classify("vimeo-downloader", "vimeo-downloader"), None);
        assert_eq!(classify("rg", "/opt/vscode-ripgrep/bin/rg pattern"), None);
        assert_eq!(classify("google_drive", "google_drive --sync"), None);
        assert_eq!(classify("mongodb-compass", "mongodb-compass"), None);
    }

    /// One VS Code window is roughly eight processes. Tiering the helpers would
    /// push eight near-identical rows above the agents and servers the tier
    /// exists to surface.
    #[test]
    fn excludes_electron_helpers() {
        assert_eq!(
            classify("Code Helper (Renderer)", "/Applications/Code Helper --type=renderer"),
            None
        );
        assert_eq!(classify("Code Helper", "/Applications/Code Helper"), None);
        assert_eq!(
            classify("cursor", "/Applications/Cursor --type=gpu-process"),
            None
        );
        assert_eq!(
            classify("Code", "/Applications/Visual Studio Code/Code"),
            Some(Category::Editor),
            "the main process is still an editor"
        );
    }

    /// Process names on macOS are capitalised; argv paths are not.
    #[test]
    fn matching_ignores_case() {
        assert_eq!(classify("Docker", "Docker"), Some(Category::Container));
        assert_eq!(classify("OrbStack", "OrbStack"), Some(Category::Container));
    }

    /// Flags are not words. `--serve` must not make a process a server.
    #[test]
    fn flags_are_not_words() {
        assert_eq!(classify("myapp", "myapp --serve --next --code"), None);
    }

    /// Categories are tested in declaration order, so a word in two tables
    /// resolves to the earlier one. `cursor-agent` and `cursor` are distinct
    /// words, so the agent does not shadow the editor.
    #[test]
    fn cursor_agent_and_cursor_are_distinct() {
        assert_eq!(classify("cursor-agent", "cursor-agent"), Some(Category::Agent));
        assert_eq!(classify("cursor", "cursor ."), Some(Category::Editor));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test category`

Expected: FAIL — `cannot find function classify in this scope`. The enum tests (`rank_matches_all`, `glyphs_are_single_char_and_never_the_pin_marker`) will not compile either, because the module is not yet declared in `main.rs`.

- [ ] **Step 4: Declare the module**

Modify `src/main.rs`, adding the line after `mod app;`:

```rust
mod app;
mod category;
mod convert;
mod cpu;
```

- [ ] **Step 5: Implement `classify`**

Insert into `src/category.rs`, between the `impl Category` block and the `const AGENT` table:

```rust
/// Electron and Chromium applications run many child processes — one VS Code
/// window is roughly eight. They are excluded before any table lookup, on
/// Electron's own documented convention.
fn is_helper(name: &str, cmd: &str) -> bool {
    name.contains("Helper")
        || cmd.contains("--type=renderer")
        || cmd.contains("--type=gpu-process")
        || cmd.contains("--type=utility")
        || cmd.contains("--type=zygote")
}

/// The last path segment, or the whole string when there is no separator.
fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The words that identify a process: its name, plus every argv token that is
/// not a flag, each also reduced to its path basename so
/// `/opt/next/dist/bin/next` yields `next`.
fn words<'a>(name: &'a str, cmd: &'a str) -> impl Iterator<Item = &'a str> {
    std::iter::once(name)
        .chain(cmd.split_whitespace().filter(|t| !t.starts_with('-')))
        .flat_map(|token| [token, basename(token)])
}

/// Classify a process from its name and full argv. `None` for anything that is
/// not a dev tool, and for the helper processes of one.
///
/// Matching is word-exact rather than substring, because substrings misfire
/// badly here: `vim` is inside `vimeo-downloader`, `code` is inside
/// `vscode-ripgrep`. Categories are tested in display order, first hit wins.
pub fn classify(name: &str, cmd: &str) -> Option<Category> {
    if is_helper(name, cmd) {
        return None;
    }
    let words: Vec<&str> = words(name, cmd).collect();
    Category::ALL.into_iter().find(|category| {
        category
            .words()
            .iter()
            .any(|entry| words.iter().any(|word| word.eq_ignore_ascii_case(entry)))
    })
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test category`

Expected: PASS, 9 tests.

- [ ] **Step 7: Verify clippy and formatting**

Run: `cargo fmt && cargo clippy --all-targets`

Expected: no errors. If `clippy::missing_const_for_fn` fires on `basename`, `is_helper` or `classify`, add `const` as suggested — `words` cannot be `const` because it returns an iterator.

- [ ] **Step 8: Commit**

```bash
but diff
but commit -b feat/dev-process-tier -m "feat(category): classify processes as dev tools from argv

Word-exact matching against per-category tables, never substring: 'vim'
is inside vimeo-downloader and 'code' inside vscode-ripgrep. Words come
from the process name and every non-flag argv token, each also reduced to
its basename, so a runtime like node classifies by what it is running.

Electron helpers are excluded before any lookup; one VS Code window is
roughly eight processes and tiering them all would bury the agents and
servers the tier exists to surface." <ids from but diff>
```

---

### Task 2: Carry the category on `ProcessInfo`

**Files:**
- Modify: `src/process.rs:51-68` (struct), `src/process.rs:349` (default sort mode), `src/process.rs:440-490` (full build), `src/process.rs:510-527` (`sort_processes`)
- Test: `src/process.rs` (existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `crate::category::{classify, Category}` from Task 1.
- Produces: `ProcessInfo.category: Option<Category>` — a public field, populated on every full refresh and carried forward unchanged by CPU-only refreshes.

- [ ] **Step 1: Write the failing test**

Append inside the existing `mod tests` in `src/process.rs`:

```rust
    fn process_named(pid: u32, name: &str, cmd: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: name.to_string(),
            cmd: cmd.to_string(),
            user: "adam".to_string(),
            cpu_usage: 0.0,
            gpu_usage: None,
            memory: 0,
            ports: Vec::new(),
            category: crate::category::classify(name, cmd),
            cwd: None,
            exe: None,
            run_time: 0,
            thread_count: 0,
        }
    }

    /// The column is labelled COMMAND and displays `cmd`, so the sort must order
    /// by `cmd`. Sorting by `name` put every Node process under "node" while the
    /// table showed "next dev", "vite" and "claude".
    #[test]
    fn command_sort_orders_by_cmd_not_name() {
        let mut monitor = ProcessMonitor::new();
        monitor.sort_mode = SortMode::Name;
        monitor.processes = vec![
            process_named(1, "node", "vite"),
            process_named(2, "node", "claude"),
            process_named(3, "alpha", "next dev"),
        ];

        monitor.sort_processes();

        let order: Vec<&str> = monitor.processes.iter().map(|p| p.cmd.as_str()).collect();
        assert_eq!(order, vec!["claude", "next dev", "vite"]);
    }

    /// The collector defaults to the COMMAND sort, which is the only mode the
    /// dev tier groups under.
    #[test]
    fn command_is_the_default_sort() {
        let monitor = ProcessMonitor::new();
        assert!(matches!(monitor.sort_mode, SortMode::Name));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib process::tests::command`

Expected: FAIL to compile — `struct ProcessInfo has no field named category`.

- [ ] **Step 3: Add the field and the import**

In `src/process.rs`, add to the imports at the top of the file:

```rust
use crate::category::{Category, classify};
```

Add the field to `ProcessInfo`, after `ports`:

```rust
    pub ports: Vec<PortInfo>,
    /// Which kind of dev tool this is, if any. `None` means the process is not
    /// a dev tool and stays out of the tier at the top of the table.
    pub category: Option<Category>,
    pub cwd: Option<String>,
```

- [ ] **Step 4: Classify during the full build**

In `refresh`, the full-build path constructs `name` and `cmd` before the `ProcessInfo` literal. Add the classification immediately after `cmd` is built, and the field to the literal.

After the `let cmd = ...` block:

```rust
                // Classified here rather than in the UI: the CPU-only refresh
                // reuses the whole ProcessInfo, so this runs on the 10-second
                // full refresh and for new pids, not on every 2-second tick.
                let category = classify(&name, &cmd);
```

In the `ProcessInfo { .. }` literal, after `ports,`:

```rust
                    ports,
                    category,
                    cwd,
```

- [ ] **Step 5: Sort by cmd and default to COMMAND**

In `sort_processes`, replace the `SortMode::Name` arm:

```rust
            SortMode::Name => {
                // By cmd, not name: the column is labelled COMMAND and displays
                // cmd, so sorting by name filed every Node process under "node".
                self.processes.sort_by(|a, b| a.cmd.cmp(&b.cmd));
            }
```

In `ProcessMonitor::new`, change the sort mode:

```rust
            sort_mode: SortMode::Name,
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test`

Expected: PASS, all existing tests plus the two new ones. `ports_survive_a_refresh_that_skips_lsof` and `a_recycled_pid_does_not_inherit_the_previous_process_ports` must still pass — they exercise the same build path.

- [ ] **Step 7: Verify clippy and formatting**

Run: `cargo fmt && cargo clippy --all-targets`

Expected: no errors.

- [ ] **Step 8: Commit**

```bash
but diff
but commit -b feat/dev-process-tier -m "feat(process): carry a dev-tool category on ProcessInfo

Classification runs in the full refresh, so it costs nothing on the
2-second tick: the CPU-only path reuses the whole ProcessInfo.

The COMMAND sort now orders by cmd rather than name, matching what the
column displays, and becomes the collector's default." <ids from but diff>
```

---

### Task 3: Tier sort and category filtering

**Files:**
- Modify: `src/app.rs:124` (default sort mode), `src/app.rs:446-466` (filter), `src/app.rs:475-497` (`get_filtered_processes`)
- Test: `src/app.rs` (existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `Category::rank`, `Category::as_str` from Task 1; `ProcessInfo.category` from Task 2.
- Produces: nothing consumed by a later task.

- [ ] **Step 1: Write the failing test**

Append inside the existing `mod tests` in `src/app.rs`:

```rust
    fn dev_process(pid: u32, cmd: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: cmd.to_string(),
            cmd: cmd.to_string(),
            user: "adam".to_string(),
            cpu_usage: 0.0,
            gpu_usage: None,
            memory: 0,
            ports: Vec::new(),
            category: crate::category::classify(cmd, cmd),
            cwd: None,
            exe: None,
            run_time: 0,
            thread_count: 0,
        }
    }

    fn app_with(processes: Vec<ProcessInfo>) -> App {
        let (tx, _rx) = mpsc::channel();
        let mut app = App::new(tx);
        app.processes = processes;
        app
    }

    fn cmds(app: &App) -> Vec<String> {
        app.get_filtered_processes()
            .iter()
            .map(|p| p.cmd.clone())
            .collect()
    }

    /// The whole point: an idle agent outranks a busy unclassified process, and
    /// the categories keep their declared order regardless of input order.
    #[test]
    fn command_sort_groups_dev_processes_by_category() {
        let app = app_with(vec![
            dev_process(1, "WindowServer"),
            dev_process(2, "docker"),
            dev_process(3, "nvim"),
            dev_process(4, "claude"),
            dev_process(5, "postgres"),
            dev_process(6, "aardvark"),
        ]);

        assert_eq!(
            cmds(&app),
            vec!["claude", "nvim", "postgres", "docker", "WindowServer", "aardvark"],
            "agent, editor, database, container, then the untiered rest"
        );
    }

    /// The tier must be inert under the other sorts, or the CPU ranking stops
    /// answering "what is eating my CPU".
    #[test]
    fn the_tier_is_inert_under_other_sorts() {
        let mut app = app_with(vec![dev_process(1, "WindowServer"), dev_process(2, "claude")]);
        app.sort_mode = SortMode::Cpu;

        assert_eq!(
            cmds(&app),
            vec!["WindowServer", "claude"],
            "input order must survive; the collector owns CPU ordering"
        );
    }

    /// Pinning is an explicit act by the user and outranks the automatic tier.
    #[test]
    fn pinned_processes_float_above_the_dev_tier() {
        let mut app = app_with(vec![dev_process(1, "WindowServer"), dev_process(2, "claude")]);
        app.pinned_pids.insert(1);

        assert_eq!(cmds(&app), vec!["WindowServer", "claude"]);
    }

    /// Typing a category name is how the categories are discoverable without a
    /// legend on screen.
    #[test]
    fn filter_matches_the_category_name() {
        let mut app = app_with(vec![dev_process(1, "nvim"), dev_process(2, "claude")]);
        app.filter_input = "agent".to_string();
        app.update_filtered_indices();

        assert_eq!(cmds(&app), vec!["claude"]);
    }

    /// The COMMAND sort is the only mode that groups, so it is the default.
    #[test]
    fn command_is_the_default_sort() {
        let (tx, _rx) = mpsc::channel();
        let app = App::new(tx);
        assert!(matches!(app.get_sort_mode(), SortMode::Name));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib app::tests`

Expected: FAIL — `command_sort_groups_dev_processes_by_category` returns input order, and `command_is_the_default_sort` fails because the default is still `SortMode::Cpu`.

- [ ] **Step 3: Default to the COMMAND sort**

In `App::new`, `src/app.rs:124`:

```rust
            sort_mode: SortMode::Name,
```

- [ ] **Step 4: Add the tier rank and the two-key sort**

Replace the pinned-only sort at the end of `get_filtered_processes`:

```rust
        // Pinning is explicit and outranks the automatic tier. The sort is
        // stable, so the collector's ordering survives inside every group.
        let tiering = matches!(self.sort_mode, SortMode::Name);
        if tiering || !self.pinned_pids.is_empty() {
            processes.sort_by_cached_key(|p| {
                (!self.pinned_pids.contains(&p.pid), self.tier_rank(p))
            });
        }

        processes
    }

    /// Rank within the dev tier: categories in declaration order, everything
    /// else after them. A constant under every sort mode but COMMAND, so the
    /// CPU, MEM and PID sorts stay pure rankings.
    fn tier_rank(&self, proc: &ProcessInfo) -> usize {
        if !matches!(self.sort_mode, SortMode::Name) {
            return 0;
        }
        proc.category.map_or(usize::MAX, Category::rank)
    }
```

Add the import at the top of `src/app.rs`:

```rust
use crate::category::Category;
```

`sort_by_cached_key` rather than `sort_by_key`: the key involves a `HashSet` lookup, and `sort_by_key` recomputes it on every comparison. Cached computes it once per element.

- [ ] **Step 5: Match the category name in the filter**

In `update_filtered_indices`, add a clause to the predicate:

```rust
                    proc.name.to_lowercase().contains(&filter_lower)
                        || proc.user.to_lowercase().contains(&filter_lower)
                        || proc.pid.to_string().contains(&filter_lower)
                        || proc
                            .category
                            .is_some_and(|c| c.as_str().contains(&filter_lower))
                        || proc
                            .ports
                            .iter()
                            .any(|port| port.port.to_string().contains(&filter_lower))
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test`

Expected: PASS. The existing `each_mode_is_entered_and_left_on_its_own_key` and `space_toggles_pause_and_q_quits` must still pass.

- [ ] **Step 7: Verify clippy and formatting**

Run: `cargo fmt && cargo clippy --all-targets`

Expected: no errors.

- [ ] **Step 8: Commit**

```bash
but diff
but commit -b feat/dev-process-tier -m "feat(app): group dev processes at the top under the COMMAND sort

A second stable-sort key beside the existing pinned float, ranked by
category declaration order. It returns a constant under every other sort
mode, so the CPU, MEM and PID rankings are untouched.

COMMAND becomes the default sort, and the filter matches category names
so typing 'agent' narrows to agents." <ids from but diff>
```

---

### Task 4: Render the category glyph

**Files:**
- Modify: `src/theme.rs:7-36` (one field), `src/ui.rs:323-400` (`process_row`), `src/ui.rs:336` (`cmd_col_width`)
- Test: `src/ui.rs` (existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `Category::glyph` from Task 1; `ProcessInfo.category` from Task 2.
- Produces: nothing consumed by a later task.

- [ ] **Step 1: Write the failing test**

Append inside the existing `mod tests` in `src/ui.rs`:

`Category` is already in scope: the test module opens with `use super::*;` and Step 4 imports `Category` at the top of `src/ui.rs`. Do not import it again — that is an unused-import warning.

```rust
    /// The prefix must occupy the same width whether or not a process is a dev
    /// tool, or command text stops aligning down the table.
    #[test]
    fn cmd_line_prefix_is_two_columns_either_way() {
        let tiered = cmd_line(Some(Category::Agent), "claude".to_string());
        let plain = cmd_line(None, "claude".to_string());

        assert_eq!(tiered.width(), plain.width());
        assert_eq!(plain.width(), "  claude".chars().count());
    }

    /// The glyph identifies the category at a glance under every sort mode, not
    /// only the one that groups.
    #[test]
    fn cmd_line_shows_the_category_glyph() {
        let line = cmd_line(Some(Category::Server), "next dev".to_string());
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        assert_eq!(rendered, "▸ next dev");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib ui::tests::cmd_line`

Expected: FAIL — `cannot find function cmd_line in this scope`.

- [ ] **Step 3: Add the theme colour**

In `src/theme.rs`, add the field to the struct after `accent_crit`:

```rust
    pub accent_warn: Color,
    pub accent_crit: Color,

    /// Dev-tool category glyphs. One colour for all six, so they read as a
    /// single class of marker; the glyph shape says which category. Kept clear
    /// of cpu/gpu/mem, which already mean something in the bars above.
    pub category: Color,
```

And to the constant, in the same position:

```rust
    accent_warn: Color::Yellow,
    accent_crit: Color::Red,

    category: Color::Blue,
```

- [ ] **Step 4: Add `cmd_line` and use it in both row shapes**

In `src/ui.rs`, add the import at the top:

```rust
use crate::category::Category;
```

Add the function immediately before `fn process_row`:

```rust
/// The COMMAND cell's first line: a category glyph, or two columns of padding
/// so untiered rows keep their command text aligned with the tiered ones.
fn cmd_line(category: Option<Category>, text: String) -> Line<'static> {
    let prefix = category.map_or_else(
        || Span::raw("  "),
        |c| {
            Span::styled(
                format!("{} ", c.glyph()),
                Style::default().fg(THEME.category),
            )
        },
    );
    Line::from(vec![prefix, Span::raw(text)])
}
```

In `process_row`, replace the expanded branch's first line:

```rust
        let mut cmd_lines: Vec<Line> = vec![cmd_line(proc.category, cmd_display)];
```

And the collapsed branch's COMMAND cell:

```rust
            Cell::from(cmd_line(proc.category, cmd_display)),
```

- [ ] **Step 5: Widen the reserved width for the prefix**

In `render_process_list`, update `cmd_col_width` and its comment:

```rust
    // Width available for the Command column's wrapped breakout content.
    // Fixed cols total 8+8+6+6+12+7 = 47, plus 6 column spacings, plus 2 for the
    // highlight symbol, plus 2 for the category glyph prefix.
    let cmd_col_width = usize::from(table_and_title.width).saturating_sub(47 + 6 + 2 + 2);
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test`

Expected: PASS, all tests.

- [ ] **Step 7: Verify clippy and formatting**

Run: `cargo fmt && cargo clippy --all-targets`

Expected: no errors.

- [ ] **Step 8: Commit**

```bash
but diff
but commit -b feat/dev-process-tier -m "feat(ui): mark dev processes with a category glyph

One colour for all six glyphs so they read as a single class of marker;
the shape says which category. Untiered rows get two columns of padding
so command text stays aligned, and the breakout wrap width accounts for
the prefix." <ids from but diff>
```

---

### Task 5: Verify in the running application

The preceding four tasks are covered by unit tests, but none of them draws a terminal. This task confirms the feature in the real TUI, because a glyph that renders double-width or a colour that vanishes on a dark background will pass every test above.

**Files:**
- Modify: none.
- Test: manual, against a running instance.

**Interfaces:**
- Consumes: the behaviour built by Tasks 1–4.
- Produces: nothing.

- [ ] **Step 1: Start something in each category to observe**

In separate terminals, so the tier has something to show:

```bash
# server
python3 -m http.server 8000
# editor
nvim
# database, if installed — otherwise skip and note it
docker run --rm -p 5432:5432 -e POSTGRES_PASSWORD=x postgres
```

An agent is already running if this plan is being executed by one.

- [ ] **Step 2: Run oversee and check the tier**

```bash
cargo run
```

Confirm, in order:

- The table opens on the COMMAND sort — COMMAND is the underlined column heading, not CPU%.
- Dev processes sit at the top, subgrouped: agents, then editors, then servers, then databases, then build tools, then containers.
- Each carries a single-width blue glyph, and the command text of tiered and untiered rows starts in the same column. A glyph rendering double-width shows as a one-column misalignment down the whole table.

- [ ] **Step 3: Check the tier is inert under other sorts**

Press `s` to cycle to CPU%. Confirm the table becomes a pure CPU ranking with no grouping, and that the glyphs are still shown. Cycle back to COMMAND and confirm the grouping returns.

- [ ] **Step 4: Check pinning and filtering**

- Press `Enter` on an untiered process. Confirm it floats above the dev tier and shows the `◆` pin marker, and that the pin marker is visibly distinct from every category glyph.
- Press `/` and type `agent`. Confirm only agents remain.
- Press `Esc`, then `/` and type a port number of the running server. Confirm the port filter still works.

- [ ] **Step 5: Check the expanded breakout**

With a tiered process selected, press `Enter` to expand it. Confirm the breakout lines wrap inside the column rather than overflowing — the width was reduced by two for the glyph.

- [ ] **Step 6: Record the outcome**

If every check passes, say so explicitly. If any check fails, stop and report which — do not commit a fix without a test that reproduces it.

- [ ] **Step 7: Commit the plan's completion**

Nothing to commit unless Step 6 found a defect. If it did, fix it under a fresh failing test and commit that.

---

## Self-Review

**Spec coverage:**

| Spec requirement | Task |
|---|---|
| `Category` enum, six variants, declaration order = display order | 1 |
| `classify(name, cmd) -> Option<Category>` as a pure function | 1 |
| Helper exclusion by `--type=` flag or `Helper` in name | 1 |
| Word-exact matching from name, argv tokens and basenames | 1 |
| Six category tables, verbatim | 1 |
| `ProcessInfo.category`, computed in the full refresh | 2 |
| COMMAND sort orders by `cmd`, not `name` | 2 |
| `ProcessMonitor` defaults to COMMAND | 2 |
| Two-key stable sort, pinned above tier | 3 |
| Tier inert under CPU/MEM/PID | 3 |
| `App` defaults to COMMAND | 3 |
| Filter matches category name | 3 |
| Coloured glyph prefixing the COMMAND cell | 4 |
| Untiered rows padded to keep alignment | 4 |
| `cmd_col_width` reduced by two | 4 |
| One new `THEME.category` entry | 4 |
| No separator rows; one row per process | all — nothing in the plan adds a row |

No gaps.

**Placeholder scan:** No TBD, TODO, "handle edge cases", or "similar to Task N". Every code step carries the code. The one deferred value is `<ids from but diff>` in the commit steps, which is a runtime value the executor reads from the preceding `but diff` — not a placeholder for a decision.

**Type consistency:** `classify`, `Category::glyph`, `Category::as_str`, `Category::rank`, `Category::ALL`, `ProcessInfo.category`, `cmd_line`, `tier_rank`, `THEME.category` are each spelled identically in the task that defines them and in every task that consumes them. `SortMode::Name` is the COMMAND sort throughout — the variant is named `Name` in existing code and the column is labelled COMMAND; the plan uses `SortMode::Name` in code and "COMMAND" in prose, never a third spelling.

**Diagram check:** `## Diagrams` present, accounts for the plan as a whole and names all five tasks.
