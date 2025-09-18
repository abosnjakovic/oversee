use std::collections::VecDeque;
use sysinfo::System;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryPressure {
    Green,  // Normal (50-100% free)
    Yellow, // Warning (30-50% free)
    Red,    // Critical (0-30% free)
}

impl MemoryPressure {
    pub fn color_name(&self) -> &'static str {
        match self {
            MemoryPressure::Green => "Normal",
            MemoryPressure::Yellow => "Warning",
            MemoryPressure::Red => "Critical",
        }
    }
}

#[derive(Debug)]
pub struct MemoryInfo {
    pub total_memory: u64,
    pub used_memory: u64,
    pub total_swap: u64,
    pub used_swap: u64,
    pub pressure: MemoryPressure,
    #[allow(dead_code)] // May be used for future features
    pub pressure_percentage: f64,
}

impl MemoryInfo {
    pub fn free_memory(&self) -> u64 {
        self.total_memory.saturating_sub(self.used_memory)
    }

    pub fn memory_usage_percentage(&self) -> f64 {
        if self.total_memory == 0 {
            0.0
        } else {
            (self.used_memory as f64 / self.total_memory as f64) * 100.0
        }
    }

    pub fn swap_usage_percentage(&self) -> f64 {
        if self.total_swap == 0 {
            0.0
        } else {
            (self.used_swap as f64 / self.total_swap as f64) * 100.0
        }
    }
}

#[derive(Debug)]
pub struct MemoryMonitor {
    system: System,
    pressure_history: VecDeque<MemoryPressure>,
    max_history: usize,
}

impl MemoryMonitor {
    pub fn new() -> Self {
        let system = System::new();
        MemoryMonitor {
            system,
            pressure_history: VecDeque::new(),
            max_history: 300, // 5 minutes at 1 second intervals
        }
    }

    pub fn refresh(&mut self) {
        self.system.refresh_memory();

        // Calculate and store pressure
        let info = self.get_memory_info();
        self.pressure_history.push_back(info.pressure);

        // Keep history within bounds
        if self.pressure_history.len() > self.max_history {
            self.pressure_history.pop_front();
        }
    }

    pub fn get_memory_info(&self) -> MemoryInfo {
        let total_memory = self.system.total_memory();
        let used_memory = self.system.used_memory();
        let total_swap = self.system.total_swap();
        let used_swap = self.system.used_swap();

        // Calculate memory pressure using Apple's approximated algorithm
        let free_memory = total_memory.saturating_sub(used_memory);
        let free_percentage = if total_memory == 0 {
            100.0
        } else {
            (free_memory as f64 / total_memory as f64) * 100.0
        };

        // Enhanced pressure calculation considering swap usage
        let swap_factor = if total_swap > 0 {
            (used_swap as f64 / total_swap as f64) * 100.0
        } else {
            0.0
        };

        // Adjust free percentage based on swap usage
        // Heavy swap usage indicates memory pressure even if some RAM is free
        let adjusted_free_percentage = if swap_factor > 10.0 {
            // If swap usage > 10%, reduce perceived free memory
            free_percentage * (1.0 - (swap_factor - 10.0) / 100.0)
        } else {
            free_percentage
        };

        let pressure = match adjusted_free_percentage {
            f if f >= 50.0 => MemoryPressure::Green,
            f if f >= 30.0 => MemoryPressure::Yellow,
            _ => MemoryPressure::Red,
        };

        MemoryInfo {
            total_memory,
            used_memory,
            total_swap,
            used_swap,
            pressure,
            pressure_percentage: 100.0 - adjusted_free_percentage,
        }
    }

    #[allow(dead_code)] // May be used for future timeline features
    pub fn get_pressure_history(&self) -> &VecDeque<MemoryPressure> {
        &self.pressure_history
    }

    #[allow(dead_code)] // May be used for future conditional features
    pub fn is_available(&self) -> bool {
        // Memory monitoring is always available
        true
    }
}

impl Default for MemoryMonitor {
    fn default() -> Self {
        Self::new()
    }
}
