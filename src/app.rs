use crate::{cpu::CpuMonitor, process::ProcessMonitor};
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
    pub fn duration_secs(self) -> usize {
        match self {
            TimelineScope::Seconds30 => 30,
            TimelineScope::Seconds60 => 60,
            TimelineScope::Seconds120 => 120,
            TimelineScope::Seconds300 => 300,
        }
    }
    
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
        
        let mut app = App {
            cpu_monitor,
            process_monitor: ProcessMonitor::new(),
            cpu_core_histories: (0..cpu_count).map(|_| VecDeque::with_capacity(MAX_CPU_HISTORY)).collect(),
            selected_process: 0,
            running: true,
            paused: false,
            timeline_scope: TimelineScope::Seconds30,
            last_cpu_update: Instant::now(),
            last_process_update: Instant::now(),
        };
        
        // Initialize with some data
        app.update_cpu_data();
        app.update_process_data();
        
        app
    }
    
    pub fn tick(&mut self) {
        let now = Instant::now();
        
        // Update CPU data every 1 second for timeline
        if now.duration_since(self.last_cpu_update) >= Duration::from_secs(1) {
            self.update_cpu_data();
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
    
    pub fn get_cpu_timeline_data(&self, bar_width: usize) -> Vec<Vec<u8>> {
        let scope_duration = self.timeline_scope.duration_secs();
        let slice_duration = scope_duration as f32 / bar_width as f32;
        
        self.cpu_core_histories
            .iter()
            .map(|core_history| {
                let mut timeline = Vec::with_capacity(bar_width);
                
                for i in 0..bar_width {
                    let start_idx = (i as f32 * slice_duration) as usize;
                    let end_idx = ((i + 1) as f32 * slice_duration) as usize;
                    
                    // Calculate average usage for this time slice
                    let slice_usage = if start_idx < core_history.len() {
                        let actual_end = end_idx.min(core_history.len());
                        let slice: Vec<f32> = core_history
                            .range(start_idx..actual_end)
                            .copied()
                            .collect();
                        
                        if slice.is_empty() {
                            0.0
                        } else {
                            slice.iter().sum::<f32>() / slice.len() as f32
                        }
                    } else {
                        0.0
                    };
                    
                    // Convert to block character intensity (0-7)
                    let intensity = if slice_usage >= 80.0 {
                        7 // █
                    } else if slice_usage >= 60.0 {
                        6 // ▉
                    } else if slice_usage >= 40.0 {
                        5 // ▊
                    } else if slice_usage >= 20.0 {
                        4 // ▋
                    } else if slice_usage >= 10.0 {
                        3 // ▌
                    } else if slice_usage >= 5.0 {
                        2 // ▍
                    } else if slice_usage >= 1.0 {
                        1 // ▎
                    } else {
                        0 // ░
                    };
                    
                    timeline.push(intensity);
                }
                
                timeline
            })
            .collect()
    }
    
    pub fn get_current_cpu_usage(&self) -> f32 {
        self.cpu_monitor.global_cpu_usage()
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
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
