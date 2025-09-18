use crate::{cpu::CpuMonitor, gpu::GpuMonitor, memory::MemoryMonitor, process::ProcessMonitor};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::widgets::TableState;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const MAX_CPU_HISTORY: usize = 1200; // 20 minutes at 1 second intervals
const MAX_TIMELINE_OFFSET: usize = 900; // Allow scrolling back 15 minutes

#[derive(Debug)]
pub struct App {
    pub cpu_monitor: CpuMonitor,
    pub process_monitor: ProcessMonitor,
    pub cpu_core_histories: Vec<VecDeque<f32>>,
    pub gpu_monitor: GpuMonitor,
    pub gpu_core_histories: Vec<VecDeque<f32>>,
    pub gpu_overall_history: VecDeque<f32>,
    pub memory_monitor: MemoryMonitor,
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
    last_cpu_update: Instant,
    last_process_update: Instant,
    last_memory_update: Instant,
}

impl App {
    pub fn new() -> Self {
        let cpu_monitor = CpuMonitor::new();
        let cpu_count = cpu_monitor.cpu_count();
        let gpu_monitor = GpuMonitor::new();
        let gpu_core_count = gpu_monitor.get_core_count();

        let mut table_state = TableState::default();
        table_state.select(Some(0));

        let mut app = App {
            cpu_monitor,
            process_monitor: ProcessMonitor::new(),
            cpu_core_histories: (0..cpu_count)
                .map(|_| VecDeque::with_capacity(MAX_CPU_HISTORY))
                .collect(),
            gpu_monitor,
            gpu_core_histories: (0..gpu_core_count)
                .map(|_| VecDeque::with_capacity(MAX_CPU_HISTORY))
                .collect(),
            gpu_overall_history: VecDeque::with_capacity(MAX_CPU_HISTORY),
            memory_monitor: MemoryMonitor::new(),
            gpu_visible: true, // Show GPU by default if available
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
            last_cpu_update: Instant::now(),
            last_process_update: Instant::now(),
            last_memory_update: Instant::now(),
        };

        // Initialize with some data
        app.update_cpu_data();
        app.update_gpu_data();
        app.update_memory_data();
        app.update_process_data();

        app
    }

    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let mut needs_render = false;

        // Update CPU data every 1 second for timeline
        if now.duration_since(self.last_cpu_update) >= Duration::from_secs(1) {
            self.update_cpu_data();
            self.update_gpu_data();
            self.last_cpu_update = now;
            needs_render = true;
        }

        // Update memory data every 1 second
        if now.duration_since(self.last_memory_update) >= Duration::from_secs(1) {
            self.update_memory_data();
            self.last_memory_update = now;
            needs_render = true;
        }

        // Update process data every 1 second (less frequent for performance)
        if now.duration_since(self.last_process_update) >= Duration::from_secs(1) {
            self.update_process_data();
            self.last_process_update = now;
            needs_render = true;
        }

        needs_render
    }

    fn update_cpu_data(&mut self) {
        if !self.paused {
            self.cpu_monitor.refresh();
            let cpu_usages = self.cpu_monitor.cpu_usages();

            // Update each core's history
            for (i, (_, usage)) in cpu_usages.iter().enumerate() {
                if i < self.cpu_core_histories.len() {
                    self.cpu_core_histories[i].push_back(*usage);

                    // Keep only the last MAX_CPU_HISTORY points
                    if self.cpu_core_histories[i].len() > MAX_CPU_HISTORY {
                        self.cpu_core_histories[i].pop_front();
                    }
                }
            }
        }
    }

    fn update_gpu_data(&mut self) {
        if !self.paused {
            self.gpu_monitor.refresh();
            let gpu_info = self.gpu_monitor.get_info();

            // Update overall GPU utilization history
            self.gpu_overall_history
                .push_back(gpu_info.overall_utilization);
            if self.gpu_overall_history.len() > MAX_CPU_HISTORY {
                self.gpu_overall_history.pop_front();
            }

            // Update individual GPU core histories
            for (i, core) in gpu_info.cores.iter().enumerate() {
                if i < self.gpu_core_histories.len() {
                    self.gpu_core_histories[i].push_back(core.utilization);
                    if self.gpu_core_histories[i].len() > MAX_CPU_HISTORY {
                        self.gpu_core_histories[i].pop_front();
                    }
                }
            }
        }
    }

    fn update_memory_data(&mut self) {
        if !self.paused {
            self.memory_monitor.refresh();
        }
    }

    fn update_process_data(&mut self) {
        if !self.paused {
            self.process_monitor.refresh();

            // Reset selection if out of bounds
            let process_count = self.process_monitor.get_processes().len();
            if self.selected_process >= process_count && process_count > 0 {
                self.selected_process = process_count - 1;
            }
        }
    }

    pub fn handle_event(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        // Use zero timeout - don't block here since we handle timing in main loop
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                self.handle_key_event(key);
                return Ok(true); // Key events always need render
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
                    // Confirm kill
                    if let Some(pid) = self.kill_target_pid {
                        self.kill_process(pid);
                    }
                    self.kill_confirmation_mode = false;
                    self.kill_target_pid = None;
                    self.kill_target_name.clear();
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    // Cancel kill
                    self.kill_confirmation_mode = false;
                    self.kill_target_pid = None;
                    self.kill_target_name.clear();
                }
                _ => {}
            }
            return;
        }

        // Handle filter mode input separately
        if self.filter_mode {
            match key.code {
                KeyCode::Esc => {
                    // Cancel filter mode and clear filter
                    self.filter_mode = false;
                    self.filter_input.clear();
                    self.update_filtered_indices();
                }
                KeyCode::Enter => {
                    // Apply filter and exit filter mode
                    self.filter_mode = false;
                    self.update_filtered_indices();
                }
                KeyCode::Backspace => {
                    // Remove last character from filter
                    self.filter_input.pop();
                    self.update_filtered_indices();
                }
                KeyCode::Char(c) => {
                    // Add character to filter
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
            KeyCode::Char('?') => {
                // Toggle help mode
                self.help_mode = true;
            }
            KeyCode::Char('/') => {
                // Enter filter mode
                self.filter_mode = true;
            }
            KeyCode::Char(' ') => {
                self.paused = !self.paused;
            }
            KeyCode::Char('s') => {
                self.process_monitor.next_sort_mode();
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                // Move forward in time (decrease offset, min 0)
                self.timeline_offset = self.timeline_offset.saturating_sub(30);
            }
            KeyCode::Char('-') => {
                // Move backward in time (increase offset, max MAX_TIMELINE_OFFSET)
                self.timeline_offset = (self.timeline_offset + 30).min(MAX_TIMELINE_OFFSET);
            }
            KeyCode::Char('v') => {
                self.gpu_visible = !self.gpu_visible;
            }
            KeyCode::Char('K') => {
                // Enter kill confirmation mode for selected process
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
            // Vim-style page navigation
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
        self.cpu_monitor.cpu_count()
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

    fn kill_process(&self, pid: u32) {
        unsafe {
            // Use SIGTERM (15) first for graceful shutdown
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }

    pub fn update_filtered_indices(&mut self) {
        if self.filter_input.is_empty() {
            // No filter, show all processes
            self.filtered_indices.clear();
        } else {
            // Filter processes by name, user, PID, or port (case-insensitive)
            let filter_lower = self.filter_input.to_lowercase();
            self.filtered_indices = self
                .process_monitor
                .get_processes()
                .iter()
                .enumerate()
                .filter(|(_, proc)| {
                    // Search by process name
                    proc.name.to_lowercase().contains(&filter_lower) ||
                    // Search by username
                    proc.user.to_lowercase().contains(&filter_lower) ||
                    // Search by PID (convert to string)
                    proc.pid.to_string().contains(&filter_lower) ||
                    // Search by any port
                    proc.ports.iter().any(|port| {
                        port.port.to_string().contains(&filter_lower)
                    })
                })
                .map(|(i, _)| i)
                .collect();
        }

        // Reset selection if it's out of bounds
        if !self.filtered_indices.is_empty() {
            if self.selected_process >= self.filtered_indices.len() {
                self.selected_process = 0;
                self.table_state.select(Some(0));
            }
        }
    }

    pub fn get_filtered_processes(&self) -> Vec<&crate::process::ProcessInfo> {
        if self.filtered_indices.is_empty() && !self.filter_input.is_empty() {
            // Filter active but no matches
            Vec::new()
        } else if self.filtered_indices.is_empty() {
            // No filter active, return all
            self.process_monitor.get_processes().iter().collect()
        } else {
            // Return filtered processes
            self.filtered_indices
                .iter()
                .filter_map(|&i| self.process_monitor.get_processes().get(i))
                .collect()
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
