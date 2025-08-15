use sysinfo::{CpuRefreshKind, RefreshKind, System};
use std::thread;
use std::time::Duration;

#[derive(Debug)]
pub struct CpuMonitor {
    system: System,
}

impl CpuMonitor {
    pub fn new() -> Self {
        let mut system = System::new_with_specifics(
            RefreshKind::new().with_cpu(CpuRefreshKind::everything()),
        );
        
        // Initial refresh to establish baseline
        system.refresh_cpu_usage();
        
        // Wait minimum interval before next refresh
        thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        
        // Second refresh to get actual usage
        system.refresh_cpu_usage();
        
        CpuMonitor { system }
    }
    
    pub fn refresh(&mut self) {
        self.system.refresh_cpu_usage();
    }
    
    pub fn global_cpu_usage(&self) -> f32 {
        self.system.global_cpu_usage()
    }
    
    pub fn cpu_count(&self) -> usize {
        self.system.cpus().len()
    }
    
    pub fn cpu_usages(&self) -> Vec<(String, f32)> {
        self.system.cpus()
            .iter()
            .enumerate()
            .map(|(i, cpu)| {
                (format!("CPU {}", i), cpu.cpu_usage())
            })
            .collect()
    }
    
    pub fn stream_cpu_data<F>(&mut self, interval_ms: u64, mut callback: F)
    where
        F: FnMut(&CpuMonitor),
    {
        let interval = Duration::from_millis(interval_ms.max(200)); // Ensure minimum interval
        
        loop {
            self.refresh();
            callback(self);
            thread::sleep(interval);
        }
    }
}

impl Default for CpuMonitor {
    fn default() -> Self {
        Self::new()
    }
}