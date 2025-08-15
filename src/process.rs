use sysinfo::{System, ProcessRefreshKind, ProcessesToUpdate};

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum SortMode {
    Cpu,
    Memory,
    Name,
    Pid,
}

impl SortMode {
    pub fn next(self) -> Self {
        match self {
            SortMode::Cpu => SortMode::Memory,
            SortMode::Memory => SortMode::Name,
            SortMode::Name => SortMode::Pid,
            SortMode::Pid => SortMode::Cpu,
        }
    }
    
    pub fn name(self) -> &'static str {
        match self {
            SortMode::Cpu => "CPU%",
            SortMode::Memory => "Memory",
            SortMode::Name => "Name",
            SortMode::Pid => "PID",
        }
    }
}

#[derive(Debug)]
pub struct ProcessMonitor {
    system: System,
    processes: Vec<ProcessInfo>,
    sort_mode: SortMode,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        let mut system = System::new();
        
        // Initial refresh
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::new()
                .with_cpu()
                .with_memory()
        );
        
        ProcessMonitor {
            system,
            processes: Vec::new(),
            sort_mode: SortMode::Cpu,
        }
    }
    
    pub fn refresh(&mut self) {
        // Refresh process information
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::new()
                .with_cpu()
                .with_memory()
        );
        
        // Convert to our ProcessInfo format
        self.processes = self.system.processes()
            .iter()
            .map(|(pid, process)| {
                ProcessInfo {
                    pid: pid.as_u32(),
                    name: process.name().to_string_lossy().to_string(),
                    cpu_usage: process.cpu_usage(),
                    memory: process.memory(),
                }
            })
            .collect();
        
        // Sort by current sort mode
        self.sort_processes();
        
        // Limit to top 100 processes for performance
        self.processes.truncate(100);
    }
    
    fn sort_processes(&mut self) {
        match self.sort_mode {
            SortMode::Cpu => {
                self.processes.sort_by(|a, b| b.cpu_usage.partial_cmp(&a.cpu_usage).unwrap());
            }
            SortMode::Memory => {
                self.processes.sort_by(|a, b| b.memory.cmp(&a.memory));
            }
            SortMode::Name => {
                self.processes.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SortMode::Pid => {
                self.processes.sort_by(|a, b| a.pid.cmp(&b.pid));
            }
        }
    }
    
    pub fn get_processes(&self) -> &[ProcessInfo] {
        &self.processes
    }
    
    pub fn set_sort_mode(&mut self, mode: SortMode) {
        self.sort_mode = mode;
        self.sort_processes();
    }
    
    pub fn next_sort_mode(&mut self) {
        self.sort_mode = self.sort_mode.next();
        self.sort_processes();
    }
    
    pub fn current_sort_mode(&self) -> SortMode {
        self.sort_mode
    }
    
    pub fn format_memory(bytes: u64) -> String {
        const GB: u64 = 1024 * 1024 * 1024;
        const MB: u64 = 1024 * 1024;
        const KB: u64 = 1024;
        
        if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }
}

impl Default for ProcessMonitor {
    fn default() -> Self {
        Self::new()
    }
}