use crate::category::Category;
use crate::gpu::GpuMonitor;
use crate::memory::MemoryInfo;
use crate::process::{ProcessDetails, ProcessInfo, SortMode, fetch_process_details};
use crate::{DataCommand, DataUpdate};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::widgets::TableState;
use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

/// Which input mode the UI is in. These are mutually exclusive, which three
/// separate bools could not express: nothing stopped help and filter mode from
/// both being set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Filter,
    KillConfirmation,
    Help,
}

/// Whether the app is sampling, holding, or on its way out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Running,
    Paused,
    Quitting,
}

#[derive(Debug)]
pub struct App {
    // Data from background thread — latest values only, no history
    pub cpu_core_values: Vec<f32>,
    pub cpu_average: f32,
    pub gpu_value: f32,
    processes: Vec<ProcessInfo>,

    // Static info (doesn't change)
    pub gpu_monitor: GpuMonitor,         // For GPU availability check
    pub memory_info: Option<MemoryInfo>, // Updated from background thread

    // UI state
    /// CPU model name for the header (e.g. "Apple M4 Pro").
    pub cpu_brand: String,
    pub gpu_visible: bool,
    pub selected_process: usize,
    pub table_state: TableState,
    pub run_state: RunState,
    pub filter_input: String,
    pub filtered_indices: Vec<usize>,
    pub kill_target_pid: Option<u32>,
    pub kill_target_name: String,
    pub mode: Mode,
    /// Set on terminal resize; main loop must clear the terminal before the
    /// next draw. A shrink+grow that lands back on the old size is invisible
    /// to ratatui's autoresize, yet the emulator has already scrolled the
    /// alternate screen, so a diff-only redraw leaves stale rows behind.
    pub needs_clear: bool,
    pub pinned_pids: HashSet<u32>,
    sort_mode: SortMode,

    // Breakout / details panel state
    pub expanded_pid: Option<u32>,
    pub selected_details: Option<ProcessDetails>,
    details_tx: Sender<u32>,
    details_rx: Receiver<ProcessDetails>,
    details_last_fetched: Option<Instant>,

    // Channel to send commands to background thread
    command_tx: Sender<DataCommand>,
}

impl App {
    pub fn new(command_tx: Sender<DataCommand>) -> Self {
        let gpu_monitor = GpuMonitor::new();

        let mut table_state = TableState::default();
        table_state.select(Some(0));

        let (details_req_tx, details_req_rx) = mpsc::channel::<u32>();
        let (details_res_tx, details_res_rx) = mpsc::channel::<ProcessDetails>();
        thread::spawn(move || {
            while let Ok(pid) = details_req_rx.recv() {
                let details = fetch_process_details(pid);
                if details_res_tx.send(details).is_err() {
                    break;
                }
            }
        });

        let cpu_brand = std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "cpu".to_string());

        Self {
            // Data will be populated from background thread
            cpu_core_values: Vec::new(),
            cpu_average: 0.0,
            gpu_value: 0.0,
            processes: Vec::new(),

            gpu_monitor,
            memory_info: None,

            cpu_brand,
            gpu_visible: true,
            selected_process: 0,
            table_state,
            run_state: RunState::Running,
            mode: Mode::Normal,
            filter_input: String::new(),
            filtered_indices: Vec::new(),
            kill_target_pid: None,
            kill_target_name: String::new(),
            needs_clear: false,
            pinned_pids: HashSet::new(),
            sort_mode: SortMode::Name,

            expanded_pid: None,
            selected_details: None,
            details_tx: details_req_tx,
            details_rx: details_res_rx,
            details_last_fetched: None,

            command_tx,
        }
    }

    /// Process any pending data updates from the background thread.
    /// Returns true if any data was updated. Only the latest value of each
    /// metric is kept — the bar meters show current state, no history.
    pub fn process_updates(&mut self, rx: &Receiver<DataUpdate>) -> bool {
        let mut updated = false;

        // Drain all available updates (non-blocking)
        while let Ok(update) = rx.try_recv() {
            match update {
                DataUpdate::Cpu {
                    core_values,
                    average_value,
                } => {
                    self.cpu_core_values = core_values;
                    self.cpu_average = average_value;
                    updated = true;
                }
                DataUpdate::Gpu { overall_value } => {
                    self.gpu_value = overall_value;
                    updated = true;
                }
                DataUpdate::Memory {
                    usage_value: _,
                    info,
                } => {
                    self.memory_info = Some(info);
                    updated = true;
                }
                DataUpdate::Processes { processes } => {
                    self.processes = processes;
                    self.update_filtered_indices();

                    // Reset selection if out of bounds
                    let process_count = self.processes.len();
                    if self.selected_process >= process_count && process_count > 0 {
                        self.selected_process = process_count.saturating_sub(1);
                    }

                    // Clear breakout if expanded process exited
                    if let Some(pid) = self.expanded_pid
                        && !self.processes.iter().any(|p| p.pid == pid)
                    {
                        self.expanded_pid = None;
                        self.selected_details = None;
                        self.details_last_fetched = None;
                    }

                    updated = true;
                }
            }
        }

        // Drain detail responses; keep only if still matches expanded_pid
        while let Ok(details) = self.details_rx.try_recv() {
            if self.expanded_pid == Some(details.pid) {
                self.selected_details = Some(details);
                updated = true;
            }
        }

        // Periodic refresh of detail data while breakout is open
        if let Some(pid) = self.expanded_pid {
            let stale = self
                .details_last_fetched
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(2));
            if stale {
                self.details_last_fetched = Some(Instant::now());
                let _ = self.details_tx.send(pid);
            }
        }

        updated
    }

    pub fn handle_event(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        // Poll timeout sets the idle wakeup floor. Crossterm returns immediately
        // when an event arrives, so key latency is unaffected by this value.
        if event::poll(Duration::from_millis(100))? {
            return Ok(self.apply_event(&event::read()?));
        }
        Ok(false)
    }

    /// Apply one terminal event; returns true if a redraw is needed.
    fn apply_event(&mut self, ev: &Event) -> bool {
        match *ev {
            Event::Key(key) => {
                self.handle_key_event(key);
                true
            }
            Event::Resize(_, _) => {
                self.needs_clear = true;
                true
            }
            _ => false,
        }
    }

    /// Route a key to the handler for whichever mode is active.
    fn handle_key_event(&mut self, key: KeyEvent) {
        match self.mode {
            Mode::Help => self.handle_help_key(key),
            Mode::KillConfirmation => self.handle_kill_confirmation_key(key),
            Mode::Filter => self.handle_filter_key(key),
            Mode::Normal => self.handle_normal_key(key),
        }
    }

    const fn handle_help_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('?' | 'q') | KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            _ => {}
        }
    }

    fn handle_kill_confirmation_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y' | 'Y') => {
                if let Some(pid) = self.kill_target_pid {
                    Self::kill_process(pid);
                }
                self.mode = Mode::Normal;
                self.kill_target_pid = None;
                self.kill_target_name.clear();
            }
            KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.kill_target_pid = None;
                self.kill_target_name.clear();
            }
            _ => {}
        }
    }

    fn handle_filter_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.filter_input.clear();
                self.update_filtered_indices();
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                self.update_filtered_indices();
            }
            KeyCode::Backspace => {
                self.filter_input.pop();
                self.update_filtered_indices();
            }
            KeyCode::Char(c) => {
                self.filter_input.push(c);
                self.update_filtered_indices();
            }
            _ => {}
        }
    }

    /// Pin or unpin the selected process, expanding or collapsing its breakout
    /// with it, and request the detail fetch the expanded view needs.
    fn toggle_selected_pin(&mut self) {
        let processes = self.get_filtered_processes();
        let Some(pid) = processes.get(self.selected_process).map(|p| p.pid) else {
            return;
        };
        if self.pinned_pids.contains(&pid) {
            self.pinned_pids.remove(&pid);
        } else {
            self.pinned_pids.insert(pid);
        }
        self.selected_details = None;
        if self.expanded_pid == Some(pid) {
            self.expanded_pid = None;
            self.details_last_fetched = None;
        } else {
            self.expanded_pid = Some(pid);
            self.details_last_fetched = Some(Instant::now());
            let _ = self.details_tx.send(pid);
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.run_state = RunState::Quitting;
            }
            KeyCode::Enter => self.toggle_selected_pin(),
            KeyCode::Char('?') => {
                self.mode = Mode::Help;
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Filter;
            }
            KeyCode::Char(' ') => {
                self.run_state = if self.run_state == RunState::Paused {
                    RunState::Running
                } else {
                    RunState::Paused
                };
                if self.run_state == RunState::Paused {
                    let _ = self.command_tx.send(DataCommand::Pause);
                } else {
                    let _ = self.command_tx.send(DataCommand::Resume);
                }
            }
            KeyCode::Char('s') => {
                self.sort_mode = self.sort_mode.next();
                let _ = self.command_tx.send(DataCommand::ChangeSortMode);
            }
            KeyCode::Char('v') => {
                self.gpu_visible = !self.gpu_visible;
                let _ = self
                    .command_tx
                    .send(DataCommand::SetGpuActive(self.gpu_visible));
            }
            KeyCode::Char('K') => {
                let processes = self.get_filtered_processes();
                if let Some((pid, name)) = processes
                    .get(self.selected_process)
                    .map(|p| (p.pid, p.name.clone()))
                {
                    self.mode = Mode::KillConfirmation;
                    self.kill_target_pid = Some(pid);
                    self.kill_target_name = name;
                }
            }
            // Vim-style navigation
            KeyCode::Char('k') | KeyCode::Up if self.selected_process > 0 => {
                self.selected_process = self.selected_process.saturating_sub(1);
                self.table_state.select(Some(self.selected_process));
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let process_count = self.get_filtered_processes().len();
                if self.selected_process.saturating_add(1) < process_count {
                    self.selected_process = self.selected_process.saturating_add(1);
                    self.table_state.select(Some(self.selected_process));
                }
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.selected_process = 0;
                self.table_state.select(Some(self.selected_process));
            }
            KeyCode::Char('G') | KeyCode::End => {
                let process_count = self.get_filtered_processes().len();
                if process_count > 0 {
                    self.selected_process = process_count.saturating_sub(1);
                    self.table_state.select(Some(self.selected_process));
                }
            }
            KeyCode::PageUp => {
                self.selected_process = self.selected_process.saturating_sub(10);
                self.table_state.select(Some(self.selected_process));
            }
            KeyCode::PageDown => {
                let process_count = self.get_filtered_processes().len();
                if process_count > 0 {
                    self.selected_process = self
                        .selected_process
                        .saturating_add(10)
                        .min(process_count.saturating_sub(1));
                    self.table_state.select(Some(self.selected_process));
                }
            }
            _ => {}
        }
    }

    pub fn get_all_processes(&self) -> &[ProcessInfo] {
        &self.processes
    }

    pub const fn get_selected_process(&self) -> usize {
        self.selected_process
    }

    pub const fn is_running(&self) -> bool {
        !matches!(self.run_state, RunState::Quitting)
    }

    pub const fn is_paused(&self) -> bool {
        matches!(self.run_state, RunState::Paused)
    }

    pub const fn is_gpu_visible(&self) -> bool {
        self.gpu_visible && self.gpu_monitor.is_available()
    }

    pub const fn get_sort_mode(&self) -> SortMode {
        self.sort_mode
    }

    fn kill_process(pid: u32) {
        let pid = i32::try_from(pid).unwrap_or(i32::MAX);
        std::thread::spawn(move || {
            unsafe {
                // Send SIGTERM first to allow graceful shutdown
                libc::kill(pid, libc::SIGTERM);
            }
            // Wait briefly, then escalate to SIGKILL if still alive
            std::thread::sleep(std::time::Duration::from_secs(2));
            unsafe {
                // kill() with signal 0 checks if process exists without sending a signal
                if libc::kill(pid, 0) == 0 {
                    libc::kill(pid, libc::SIGKILL);
                }
            }
        });
    }

    pub fn update_filtered_indices(&mut self) {
        if self.filter_input.is_empty() {
            self.filtered_indices.clear();
        } else {
            let filter_lower = self.filter_input.to_lowercase();
            self.filtered_indices = self
                .processes
                .iter()
                .enumerate()
                .filter(|(_, proc)| Self::matches_filter(proc, &filter_lower))
                .map(|(i, _)| i)
                .collect();
        }

        if !self.filtered_indices.is_empty() && self.selected_process >= self.filtered_indices.len()
        {
            self.selected_process = 0;
            self.table_state.select(Some(0));
        }
    }

    /// Does this process match the filter?
    ///
    /// A leading colon searches categories only. A bare word searches every
    /// field, which meant `/agent` also matched the sixty-odd macOS daemons
    /// named `*Agent` — so `:agent` is how you ask for the dev tier itself.
    fn matches_filter(proc: &ProcessInfo, filter: &str) -> bool {
        if let Some(wanted) = filter.strip_prefix(':') {
            // An empty prefix would match every category, so a bare `:` must not.
            return !wanted.is_empty()
                && proc
                    .category
                    .is_some_and(|c| c.as_str().starts_with(wanted));
        }
        proc.name.to_lowercase().contains(filter)
            || proc.user.to_lowercase().contains(filter)
            || proc.pid.to_string().contains(filter)
            || proc.category.is_some_and(|c| c.as_str().contains(filter))
            || proc
                .ports
                .iter()
                .any(|port| port.port.to_string().contains(filter))
    }

    pub fn get_filtered_processes(&self) -> Vec<&ProcessInfo> {
        let mut processes: Vec<&ProcessInfo> =
            if self.filtered_indices.is_empty() && !self.filter_input.is_empty() {
                Vec::new()
            } else if self.filtered_indices.is_empty() {
                self.processes.iter().collect()
            } else {
                self.filtered_indices
                    .iter()
                    .filter_map(|&i| self.processes.get(i))
                    .collect()
            };

        // Pinning is explicit and outranks the automatic tier. The sort is
        // stable, so the collector's ordering survives inside every group.
        let tiering = matches!(self.sort_mode, SortMode::Name);
        if tiering || !self.pinned_pids.is_empty() {
            processes
                .sort_by_cached_key(|p| (!self.pinned_pids.contains(&p.pid), self.tier_rank(p)));
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_event_requests_clear_and_redraw() {
        let (tx, _rx) = mpsc::channel();
        let mut app = App::new(tx);
        assert!(!app.needs_clear);
        assert!(app.apply_event(&Event::Resize(80, 24)));
        assert!(app.needs_clear);
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::from(code))
    }

    /// The modes are mutually exclusive, and each one's own exit key returns to
    /// Normal. Three independent bools let help and filter both be set at once,
    /// and the dispatcher then silently preferred help.
    #[test]
    fn each_mode_is_entered_and_left_on_its_own_key() {
        let (tx, _rx) = mpsc::channel();
        let mut app = App::new(tx);
        assert_eq!(app.mode, Mode::Normal);

        app.apply_event(&key(KeyCode::Char('?')));
        assert_eq!(app.mode, Mode::Help);
        app.apply_event(&key(KeyCode::Char('/')));
        assert_eq!(app.mode, Mode::Help, "help must swallow the filter key");
        app.apply_event(&key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);

        app.apply_event(&key(KeyCode::Char('/')));
        assert_eq!(app.mode, Mode::Filter);
        app.apply_event(&key(KeyCode::Char('x')));
        assert_eq!(app.filter_input, "x", "filter mode types rather than binds");
        app.apply_event(&key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.filter_input.is_empty());
    }

    /// Space toggles sampling; q leaves for good. Quitting is not a pause.
    #[test]
    fn space_toggles_pause_and_q_quits() {
        let (tx, _rx) = mpsc::channel();
        let mut app = App::new(tx);
        assert!(app.is_running() && !app.is_paused());

        app.apply_event(&key(KeyCode::Char(' ')));
        assert!(app.is_paused());
        assert!(app.is_running(), "a paused app is still running");
        app.apply_event(&key(KeyCode::Char(' ')));
        assert!(!app.is_paused());

        app.apply_event(&key(KeyCode::Char('q')));
        assert!(!app.is_running());
    }

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
            vec![
                "claude",
                "nvim",
                "postgres",
                "docker",
                "WindowServer",
                "aardvark"
            ],
            "agent, editor, database, container, then the untiered rest"
        );
    }

    /// The tier must be inert under the other sorts, or the CPU ranking stops
    /// answering "what is eating my CPU".
    #[test]
    fn the_tier_is_inert_under_other_sorts() {
        let mut app = app_with(vec![
            dev_process(1, "WindowServer"),
            dev_process(2, "claude"),
        ]);
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
        let mut app = app_with(vec![
            dev_process(1, "WindowServer"),
            dev_process(2, "claude"),
        ]);
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

    /// A bare word searches everything, so /agent also matched the ~64 macOS
    /// daemons named *Agent. A leading colon searches categories only.
    #[test]
    fn a_colon_prefix_searches_categories_only() {
        let mut app = app_with(vec![
            dev_process(1, "claude"),
            dev_process(2, "PasswordBreachAgent"),
            dev_process(3, "nvim"),
        ]);

        app.filter_input = "agent".to_string();
        app.update_filtered_indices();
        assert_eq!(
            cmds(&app),
            vec!["claude", "PasswordBreachAgent"],
            "a bare word still searches names too"
        );

        app.filter_input = ":agent".to_string();
        app.update_filtered_indices();
        assert_eq!(cmds(&app), vec!["claude"], "the daemon is not an agent");
    }

    /// A colon on its own, or one naming no category, must not silently match
    /// everything — an empty prefix would.
    #[test]
    fn a_colon_matching_no_category_shows_nothing() {
        let mut app = app_with(vec![dev_process(1, "claude"), dev_process(2, "nvim")]);

        app.filter_input = ":".to_string();
        app.update_filtered_indices();
        assert!(cmds(&app).is_empty(), "a bare colon names no category");

        app.filter_input = ":nonsense".to_string();
        app.update_filtered_indices();
        assert!(cmds(&app).is_empty());
    }

    /// `tier_rank`'s early return is only reached when something is pinned AND
    /// the sort is not COMMAND — otherwise the outer gate short-circuits and
    /// the sort never runs, so `the_tier_is_inert_under_other_sorts` passes
    /// without ever calling it.
    #[test]
    fn a_pin_does_not_tier_the_rest_under_other_sorts() {
        let mut app = app_with(vec![
            dev_process(1, "WindowServer"),
            dev_process(2, "claude"),
            dev_process(3, "aardvark"),
        ]);
        app.sort_mode = SortMode::Cpu;
        app.pinned_pids.insert(3);

        assert_eq!(
            cmds(&app),
            vec!["aardvark", "WindowServer", "claude"],
            "the pin floats, but claude must not be tiered above WindowServer"
        );
    }

    /// The COMMAND sort is the only mode that groups, so it is the default.
    #[test]
    fn command_is_the_default_sort() {
        let (tx, _rx) = mpsc::channel();
        let app = App::new(tx);
        assert!(matches!(app.get_sort_mode(), SortMode::Name));
    }
}
