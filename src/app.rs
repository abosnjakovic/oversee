use crate::gpu::GpuMonitor;
use crate::memory::MemoryInfo;
use crate::process::{ProcessInfo, SortMode};
use crate::{DataCommand, DataUpdate};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::widgets::TableState;
use std::collections::{HashSet, VecDeque};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

const MAX_TIMELINE_OFFSET: usize = 900; // Allow scrolling back 15 minutes

#[derive(Debug)]
pub struct App {
    // Data from background thread
    pub cpu_core_histories: Vec<VecDeque<f32>>,
    pub gpu_core_histories: Vec<VecDeque<f32>>,
    pub gpu_overall_history: VecDeque<f32>,
    pub memory_usage_history: VecDeque<f32>,
    cpu_average_history: VecDeque<f32>,
    processes: Vec<ProcessInfo>,

    // Static info (doesn't change)
    pub gpu_monitor: GpuMonitor,         // For GPU availability check
    pub memory_info: Option<MemoryInfo>, // Updated from background thread

    // UI state
    pub gpu_visible: bool,
    pub selected_process: usize,
    pub table_state: TableState,
    pub running: bool,
    pub paused: bool,
    pub timeline_offset: usize,
    pub filter_mode: bool,
    pub filter_input: String,
    pub filtered_indices: Vec<usize>,
    pub kill_confirmation_mode: bool,
    pub kill_target_pid: Option<u32>,
    pub kill_target_name: String,
    pub help_mode: bool,
    pub pinned_pids: HashSet<u32>,
    sort_mode: SortMode,

    // Channel to send commands to background thread
    command_tx: Sender<DataCommand>,
}

impl App {
    pub fn new(command_tx: Sender<DataCommand>) -> Self {
        let gpu_monitor = GpuMonitor::new();
        let gpu_core_count = gpu_monitor.get_core_count();

        let mut table_state = TableState::default();
        table_state.select(Some(0));

        App {
            // Data will be populated from background thread
            cpu_core_histories: Vec::new(),
            gpu_core_histories: (0..gpu_core_count).map(|_| VecDeque::new()).collect(),
            gpu_overall_history: VecDeque::new(),
            memory_usage_history: VecDeque::new(),
            cpu_average_history: VecDeque::new(),
            processes: Vec::new(),

            gpu_monitor,
            memory_info: None,

            gpu_visible: true,
            selected_process: 0,
            table_state,
            running: true,
            paused: false,
            timeline_offset: 0,
            filter_mode: false,
            filter_input: String::new(),
            filtered_indices: Vec::new(),
            kill_confirmation_mode: false,
            kill_target_pid: None,
            kill_target_name: String::new(),
            help_mode: false,
            pinned_pids: HashSet::new(),
            sort_mode: SortMode::Cpu,

            command_tx,
        }
    }

    /// Process any pending data updates from the background thread.
    /// Returns true if any data was updated.
    /// App maintains its own history buffers and appends incremental values.
    pub fn process_updates(&mut self, rx: &Receiver<DataUpdate>) -> bool {
        const MAX_HISTORY: usize = 1200;
        let mut updated = false;

        // Drain all available updates (non-blocking)
        while let Ok(update) = rx.try_recv() {
            match update {
                DataUpdate::Cpu {
                    core_values,
                    average_value,
                } => {
                    // Initialise history vectors if needed
                    if self.cpu_core_histories.len() != core_values.len() {
                        self.cpu_core_histories = (0..core_values.len())
                            .map(|_| VecDeque::with_capacity(MAX_HISTORY))
                            .collect();
                    }

                    // Append new values to histories
                    for (i, &value) in core_values.iter().enumerate() {
                        if i < self.cpu_core_histories.len() {
                            self.cpu_core_histories[i].push_back(value);
                            if self.cpu_core_histories[i].len() > MAX_HISTORY {
                                self.cpu_core_histories[i].pop_front();
                            }
                        }
                    }

                    self.cpu_average_history.push_back(average_value);
                    if self.cpu_average_history.len() > MAX_HISTORY {
                        self.cpu_average_history.pop_front();
                    }
                    updated = true;
                }
                DataUpdate::Gpu {
                    core_values,
                    overall_value,
                } => {
                    // Initialise history vectors if needed
                    if self.gpu_core_histories.len() != core_values.len() {
                        self.gpu_core_histories = (0..core_values.len())
                            .map(|_| VecDeque::with_capacity(MAX_HISTORY))
                            .collect();
                    }

                    // Append new values to histories
                    for (i, &value) in core_values.iter().enumerate() {
                        if i < self.gpu_core_histories.len() {
                            self.gpu_core_histories[i].push_back(value);
                            if self.gpu_core_histories[i].len() > MAX_HISTORY {
                                self.gpu_core_histories[i].pop_front();
                            }
                        }
                    }

                    self.gpu_overall_history.push_back(overall_value);
                    if self.gpu_overall_history.len() > MAX_HISTORY {
                        self.gpu_overall_history.pop_front();
                    }
                    updated = true;
                }
                DataUpdate::Memory { usage_value, info } => {
                    self.memory_usage_history.push_back(usage_value);
                    if self.memory_usage_history.len() > MAX_HISTORY {
                        self.memory_usage_history.pop_front();
                    }
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
                    updated = true;
                }
            }
        }

        updated
    }

    pub fn handle_event(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        // Poll with a short timeout for responsive UI
        #[allow(clippy::collapsible_if)] // Suggested fix uses unstable let-else syntax
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                self.handle_key_event(key);
                return Ok(true);
            }
        }
        Ok(false)
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
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.timeline_offset = self.timeline_offset.saturating_sub(30);
            }
            KeyCode::Char('-') => {
                self.timeline_offset = (self.timeline_offset + 30).min(MAX_TIMELINE_OFFSET);
            }
            KeyCode::Char('v') => {
                self.gpu_visible = !self.gpu_visible;
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
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected_process > 0 {
                    self.selected_process -= 1;
                    self.table_state.select(Some(self.selected_process));
                }
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

    pub fn get_cpu_count(&self) -> usize {
        self.cpu_core_histories.len()
    }

    /// Returns current CPU usage for each core (last recorded value)
    pub fn get_cpu_usages(&self) -> Vec<(String, f32)> {
        self.cpu_core_histories
            .iter()
            .enumerate()
            .map(|(i, history)| {
                let usage = history.back().copied().unwrap_or(0.0);
                (format!("CPU {}", i), usage)
            })
            .collect()
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

    pub fn get_timeline_position_text(&self) -> String {
        if self.timeline_offset == 0 {
            "Live".to_string()
        } else {
            let minutes = self.timeline_offset / 60;
            let seconds = self.timeline_offset % 60;
            if minutes > 0 {
                format!("-{}m{}s", minutes, seconds)
            } else {
                format!("-{}s", seconds)
            }
        }
    }

    pub fn get_timeline_offset(&self) -> usize {
        self.timeline_offset
    }

    pub fn is_gpu_visible(&self) -> bool {
        self.gpu_visible && self.gpu_monitor.is_available()
    }

    pub fn get_gpu_monitor(&self) -> &GpuMonitor {
        &self.gpu_monitor
    }

    pub fn get_cpu_average_history(&self) -> &VecDeque<f32> {
        &self.cpu_average_history
    }

    pub fn get_sort_mode(&self) -> SortMode {
        self.sort_mode
    }

    fn kill_process(&self, pid: u32) {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
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
