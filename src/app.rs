use crate::{cpu::CpuMonitor, gpu::GpuMonitor, process::ProcessMonitor};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const MAX_CPU_HISTORY: usize = 300; // 5 minutes at 1 second intervals

#[derive(Debug, Clone, Copy)]
pub enum TimelineScope {
    Seconds30,
    Seconds60,
    Seconds120,
    Seconds300,
}

impl TimelineScope {
    pub fn name(self) -> &'static str {
        match self {
            TimelineScope::Seconds30 => "30s",
            TimelineScope::Seconds60 => "60s",
            TimelineScope::Seconds120 => "120s",
            TimelineScope::Seconds300 => "300s",
        }
    }
    
    pub fn next(self) -> Self {
        match self {
            TimelineScope::Seconds30 => TimelineScope::Seconds60,
            TimelineScope::Seconds60 => TimelineScope::Seconds120,
            TimelineScope::Seconds120 => TimelineScope::Seconds300,
            TimelineScope::Seconds300 => TimelineScope::Seconds300,
        }
    }
    
    pub fn prev(self) -> Self {
        match self {
            TimelineScope::Seconds30 => TimelineScope::Seconds30,
            TimelineScope::Seconds60 => TimelineScope::Seconds30,
            TimelineScope::Seconds120 => TimelineScope::Seconds60,
            TimelineScope::Seconds300 => TimelineScope::Seconds120,
        }
    }
}

#[derive(Debug)]
pub struct App {
    pub cpu_monitor: CpuMonitor,
    pub process_monitor: ProcessMonitor,
    pub cpu_core_histories: Vec<VecDeque<f32>>,
    pub gpu_monitor: GpuMonitor,
    pub gpu_core_histories: Vec<VecDeque<f32>>,
    pub gpu_overall_history: VecDeque<f32>,
    pub gpu_visible: bool,
    pub selected_process: usize,
    pub running: bool,
    pub paused: bool,
    pub timeline_scope: TimelineScope,
    last_cpu_update: Instant,
    last_process_update: Instant,
}

impl App {
    pub fn new() -> Self {
        let cpu_monitor = CpuMonitor::new();
        let cpu_count = cpu_monitor.cpu_count();
        let gpu_monitor = GpuMonitor::new();
        let gpu_core_count = gpu_monitor.get_core_count();
        
        let mut app = App {
            cpu_monitor,
            process_monitor: ProcessMonitor::new(),
            cpu_core_histories: (0..cpu_count).map(|_| VecDeque::with_capacity(MAX_CPU_HISTORY)).collect(),
            gpu_monitor,
            gpu_core_histories: (0..gpu_core_count).map(|_| VecDeque::with_capacity(MAX_CPU_HISTORY)).collect(),
            gpu_overall_history: VecDeque::with_capacity(MAX_CPU_HISTORY),
            gpu_visible: true, // Show GPU by default if available
            selected_process: 0,
            running: true,
            paused: false,
            timeline_scope: TimelineScope::Seconds30,
            last_cpu_update: Instant::now(),
            last_process_update: Instant::now(),
        };
        
        // Initialize with some data
        app.update_cpu_data();
        app.update_gpu_data();
        app.update_process_data();
        
        app
    }
    
    pub fn tick(&mut self) {
        let now = Instant::now();
        
        // Update CPU data every 1 second for timeline
        if now.duration_since(self.last_cpu_update) >= Duration::from_secs(1) {
            self.update_cpu_data();
            self.update_gpu_data();
            self.last_cpu_update = now;
        }
        
        // Update process data every 1 second (less frequent for performance)
        if now.duration_since(self.last_process_update) >= Duration::from_secs(1) {
            self.update_process_data();
            self.last_process_update = now;
        }
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
            self.gpu_overall_history.push_back(gpu_info.overall_utilization);
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
    
    pub fn handle_event(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                self.handle_key_event(key);
            }
        }
        Ok(())
    }
    
    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.running = false;
            }
            KeyCode::Char(' ') => {
                self.paused = !self.paused;
            }
            KeyCode::Char('s') => {
                self.process_monitor.next_sort_mode();
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.timeline_scope = self.timeline_scope.next();
            }
            KeyCode::Char('-') => {
                self.timeline_scope = self.timeline_scope.prev();
            }
            KeyCode::Char('v') => {
                self.gpu_visible = !self.gpu_visible;
            }
            // Vim-style navigation
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected_process > 0 {
                    self.selected_process -= 1;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let process_count = self.process_monitor.get_processes().len();
                if process_count > 0 && self.selected_process < process_count - 1 {
                    self.selected_process += 1;
                }
            }
            KeyCode::Char('g') => {
                self.selected_process = 0;
            }
            KeyCode::Char('G') => {
                let process_count = self.process_monitor.get_processes().len();
                if process_count > 0 {
                    self.selected_process = process_count - 1;
                }
            }
            // Vim-style page navigation
            KeyCode::PageUp => {
                self.selected_process = self.selected_process.saturating_sub(10);
            }
            KeyCode::PageDown => {
                let process_count = self.process_monitor.get_processes().len();
                if process_count > 0 {
                    self.selected_process = (self.selected_process + 10).min(process_count - 1);
                }
            }
            KeyCode::Home => {
                self.selected_process = 0;
            }
            KeyCode::End => {
                let process_count = self.process_monitor.get_processes().len();
                if process_count > 0 {
                    self.selected_process = process_count - 1;
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
    
    pub fn get_timeline_scope(&self) -> TimelineScope {
        self.timeline_scope
    }
    
    pub fn is_gpu_visible(&self) -> bool {
        self.gpu_visible && self.gpu_monitor.is_available()
    }
    
    pub fn get_gpu_monitor(&self) -> &GpuMonitor {
        &self.gpu_monitor
    }
    
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
