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
    pub running: bool,
    pub paused: bool,
    pub filter_mode: bool,
    pub filter_input: String,
    pub filtered_indices: Vec<usize>,
    pub kill_confirmation_mode: bool,
    pub kill_target_pid: Option<u32>,
    pub kill_target_name: String,
    pub help_mode: bool,
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

        App {
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
            running: true,
            paused: false,
            filter_mode: false,
            filter_input: String::new(),
            filtered_indices: Vec::new(),
            kill_confirmation_mode: false,
            kill_target_pid: None,
            kill_target_name: String::new(),
            help_mode: false,
            needs_clear: false,
            pinned_pids: HashSet::new(),
            sort_mode: SortMode::Cpu,

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
                        self.selected_process = process_count - 1;
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
                .map(|t| t.elapsed() >= Duration::from_secs(2))
                .unwrap_or(true);
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
            return Ok(self.apply_event(event::read()?));
        }
        Ok(false)
    }

    /// Apply one terminal event; returns true if a redraw is needed.
    fn apply_event(&mut self, ev: Event) -> bool {
        match ev {
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

    fn handle_key_event(&mut self, key: KeyEvent) {
        // Handle help mode
        if self.help_mode {
            match key.code {
                KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => {
                    self.help_mode = false;
                }
                _ => {}
            }
            return;
        }

        // Handle kill confirmation mode
        if self.kill_confirmation_mode {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if let Some(pid) = self.kill_target_pid {
                        self.kill_process(pid);
                    }
                    self.kill_confirmation_mode = false;
                    self.kill_target_pid = None;
                    self.kill_target_name.clear();
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.kill_confirmation_mode = false;
                    self.kill_target_pid = None;
                    self.kill_target_name.clear();
                }
                _ => {}
            }
            return;
        }

        // Handle filter mode input
        if self.filter_mode {
            match key.code {
                KeyCode::Esc => {
                    self.filter_mode = false;
                    self.filter_input.clear();
                    self.update_filtered_indices();
                }
                KeyCode::Enter => {
                    self.filter_mode = false;
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
            return;
        }

        // Normal mode key handling
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.running = false;
            }
            KeyCode::Enter => {
                let processes = self.get_filtered_processes();
                if !processes.is_empty() && self.selected_process < processes.len() {
                    let pid = processes[self.selected_process].pid;
                    if self.pinned_pids.contains(&pid) {
                        self.pinned_pids.remove(&pid);
                    } else {
                        self.pinned_pids.insert(pid);
                    }
                    if self.expanded_pid == Some(pid) {
                        self.expanded_pid = None;
                        self.selected_details = None;
                        self.details_last_fetched = None;
                    } else {
                        self.expanded_pid = Some(pid);
                        self.selected_details = None;
                        self.details_last_fetched = Some(Instant::now());
                        let _ = self.details_tx.send(pid);
                    }
                }
            }
            KeyCode::Char('?') => {
                self.help_mode = true;
            }
            KeyCode::Char('/') => {
                self.filter_mode = true;
            }
            KeyCode::Char(' ') => {
                self.paused = !self.paused;
                if self.paused {
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
                if !processes.is_empty() && self.selected_process < processes.len() {
                    let pid = processes[self.selected_process].pid;
                    let name = processes[self.selected_process].name.clone();
                    self.kill_confirmation_mode = true;
                    self.kill_target_pid = Some(pid);
                    self.kill_target_name = name;
                }
            }
            // Vim-style navigation
            KeyCode::Char('k') | KeyCode::Up if self.selected_process > 0 => {
                self.selected_process -= 1;
                self.table_state.select(Some(self.selected_process));
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let process_count = self.get_filtered_processes().len();
                if process_count > 0 && self.selected_process < process_count - 1 {
                    self.selected_process += 1;
                    self.table_state.select(Some(self.selected_process));
                }
            }
            KeyCode::Char('g') => {
                self.selected_process = 0;
                self.table_state.select(Some(self.selected_process));
            }
            KeyCode::Char('G') => {
                let process_count = self.get_filtered_processes().len();
                if process_count > 0 {
                    self.selected_process = process_count - 1;
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
                    self.selected_process = (self.selected_process + 10).min(process_count - 1);
                    self.table_state.select(Some(self.selected_process));
                }
            }
            KeyCode::Home => {
                self.selected_process = 0;
                self.table_state.select(Some(self.selected_process));
            }
            KeyCode::End => {
                let process_count = self.get_filtered_processes().len();
                if process_count > 0 {
                    self.selected_process = process_count - 1;
                    self.table_state.select(Some(self.selected_process));
                }
            }
            _ => {}
        }
    }

    pub fn get_all_processes(&self) -> &[ProcessInfo] {
        &self.processes
    }

    pub fn get_selected_process(&self) -> usize {
        self.selected_process
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn is_gpu_visible(&self) -> bool {
        self.gpu_visible && self.gpu_monitor.is_available()
    }

    pub fn get_sort_mode(&self) -> SortMode {
        self.sort_mode
    }

    fn kill_process(&self, pid: u32) {
        let pid = pid as i32;
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
                .filter(|(_, proc)| {
                    proc.name.to_lowercase().contains(&filter_lower)
                        || proc.user.to_lowercase().contains(&filter_lower)
                        || proc.pid.to_string().contains(&filter_lower)
                        || proc
                            .ports
                            .iter()
                            .any(|port| port.port.to_string().contains(&filter_lower))
                })
                .map(|(i, _)| i)
                .collect();
        }

        if !self.filtered_indices.is_empty() && self.selected_process >= self.filtered_indices.len()
        {
            self.selected_process = 0;
            self.table_state.select(Some(0));
        }
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

        if !self.pinned_pids.is_empty() {
            processes.sort_by(|a, b| {
                let a_pinned = self.pinned_pids.contains(&a.pid);
                let b_pinned = self.pinned_pids.contains(&b.pid);
                b_pinned.cmp(&a_pinned)
            });
        }

        processes
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
        assert!(app.apply_event(Event::Resize(80, 24)));
        assert!(app.needs_clear);
    }
}
