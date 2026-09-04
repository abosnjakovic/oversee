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

/// The words that identify a process: its name, the executable, and the first
/// real argument, each also reduced to its path basename so
/// `/opt/next/dist/bin/next` yields `next`.
///
/// Only the first two argv tokens are read, because later ones are the tool's
/// arguments rather than the tool: scanning the whole line tagged
/// `pip install flask` as a server and `brew install go` as a build tool.
fn words<'a>(name: &'a str, cmd: &'a str) -> impl Iterator<Item = &'a str> {
    std::iter::once(name)
        .chain(argv(cmd).take(2))
        .flat_map(|token| [token, basename(token)])
}

/// The command line's tokens, less its flags.
fn argv(cmd: &str) -> impl Iterator<Item = &str> {
    cmd.split_whitespace().filter(|t| !t.starts_with('-'))
}

/// Dispatchers that only mean a server under one subcommand. `php artisan`
/// serves under `serve` and opens a REPL under `tinker`; Django's `manage.py`
/// serves under `runserver` and runs a test suite under `test`. Matching the
/// dispatcher alone tagged both REPLs and migrations as servers.
const SERVER_SUBCOMMANDS: &[(&str, &str)] = &[("artisan", "serve"), ("manage.py", "runserver")];

/// True when `cmd` invokes `word` with `sub` as its next non-flag argument.
fn runs_subcommand(cmd: &str, word: &str, sub: &str) -> bool {
    let mut tokens = argv(cmd);
    while let Some(token) = tokens.next() {
        if basename(token).eq_ignore_ascii_case(word) {
            return tokens
                .next()
                .is_some_and(|next| next.eq_ignore_ascii_case(sub));
        }
    }
    false
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
    if SERVER_SUBCOMMANDS
        .iter()
        .any(|(word, sub)| runs_subcommand(cmd, word, sub))
    {
        return Some(Category::Server);
    }
    let words: Vec<&str> = words(name, cmd).collect();
    Category::ALL.into_iter().find(|category| {
        category
            .words()
            .iter()
            .any(|entry| words.iter().any(|word| word.eq_ignore_ascii_case(entry)))
    })
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
    "sublime text",
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
    "http.server",
    "live-server",
    "serve",
    "rails",
    "puma",
    "unicorn",
    "rack",
    "sinatra",
    "flask",
    "uvicorn",
    "gunicorn",
    "hypercorn",
    "daphne",
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
            assert_ne!(
                category.glyph(),
                "◆",
                "{category:?} collides with the pin marker"
            );
        }
    }

    /// A package manager's ARGUMENT is not the tool being run. Scanning the whole
    /// argv tagged `pip install flask` as a server and `brew install go` as a
    /// build tool — the installed package's name collided with a table entry.
    #[test]
    fn a_package_manager_argument_is_not_the_tool() {
        assert_eq!(classify("npm", "npm install eslint"), None);
        assert_eq!(classify("pip", "pip install flask"), None);
        assert_eq!(classify("pip3", "pip3 install uvicorn gunicorn"), None);
        assert_eq!(classify("brew", "brew install go"), None);
        assert_eq!(classify("cargo", "cargo add vite"), Some(Category::Build));
    }

    /// Restricting the word set must not cost a genuine detection: the tool is
    /// still identified when it is the first real argument of a runtime.
    #[test]
    fn the_first_real_argument_still_identifies_the_tool() {
        assert_eq!(
            classify("node", "node /opt/next/dist/bin/next dev"),
            Some(Category::Server)
        );
        assert_eq!(
            classify("python3", "python3 -m uvicorn main:app --reload"),
            Some(Category::Server)
        );
    }

    /// `artisan` and `manage.py` are dispatchers, not servers. Only the
    /// subcommand says whether one is serving traffic or opening a REPL.
    #[test]
    fn dispatchers_classify_on_their_subcommand() {
        assert_eq!(classify("php", "php artisan serve"), Some(Category::Server));
        assert_eq!(classify("php", "php artisan tinker"), None);
        assert_eq!(classify("php", "php artisan migrate"), None);
        assert_eq!(
            classify("python", "python manage.py runserver 0.0.0.0:8000"),
            Some(Category::Server)
        );
        assert_eq!(classify("python", "python manage.py test"), None);
        assert_eq!(classify("python", "python manage.py migrate"), None);
    }

    /// Python's stdlib server module is `http.server`; the npm package is the
    /// distinct word `http-server`. Both are common enough to want.
    #[test]
    fn the_python_stdlib_server_module_is_detected() {
        assert_eq!(
            classify("python3", "python3 -m http.server 8000"),
            Some(Category::Server)
        );
        assert_eq!(
            classify("node", "node http-server ."),
            Some(Category::Server)
        );
    }

    #[test]
    fn classifies_by_process_name() {
        assert_eq!(classify("nvim", "nvim src/main.rs"), Some(Category::Editor));
        assert_eq!(classify("claude", "claude"), Some(Category::Agent));
        assert_eq!(
            classify("postgres", "postgres -D /db"),
            Some(Category::Database)
        );
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
        assert_eq!(classify("php", "php artisan serve"), Some(Category::Server));
        assert_eq!(
            classify("node", "node"),
            None,
            "a bare runtime says nothing"
        );
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
            classify(
                "Code Helper (Renderer)",
                "/Applications/Code Helper --type=renderer"
            ),
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
        assert_eq!(
            classify("cursor-agent", "cursor-agent"),
            Some(Category::Agent)
        );
        assert_eq!(classify("cursor", "cursor ."), Some(Category::Editor));
    }
}
