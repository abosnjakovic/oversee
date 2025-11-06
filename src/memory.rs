use std::collections::VecDeque;
use sysinfo::System;
use std::mem;

// FFI declaration for sysctlbyname
unsafe extern "C" {
    fn sysctlbyname(
        name: *const libc::c_char,
        oldp: *mut libc::c_void,
        oldlenp: *mut libc::size_t,
        newp: *mut libc::c_void,
        newlen: libc::size_t,
    ) -> libc::c_int;
}

/// Query macOS memory pressure level via sysctl
/// Returns: Some(1) = Normal, Some(2) = Warning, Some(4) = Critical, None = Error
fn get_macos_memory_pressure_level() -> Option<u32> {
    let name = b"kern.memorystatus_vm_pressure_level\0";
    let mut pressure_level: u32 = 0;
    let mut length = mem::size_of::<u32>();

    unsafe {
        let result = sysctlbyname(
            name.as_ptr() as *const i8,
            &mut pressure_level as *mut _ as *mut libc::c_void,
            &mut length,
            std::ptr::null_mut(),
            0,
        );

        if result == 0 {
            Some(pressure_level)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryPressure {
    Green,  // Normal - macOS reports level 1
    Yellow, // Warning - macOS reports level 2
    Red,    // Critical - macOS reports level 4
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

        // Use native macOS memory pressure level from kern.memorystatus_vm_pressure_level
        // This matches Activity Monitor's calculation exactly
        let pressure = if let Some(level) = get_macos_memory_pressure_level() {
            // macOS returns: 1 = Normal, 2 = Warning, 4 = Critical
            match level {
                1 => MemoryPressure::Green,
                2 => MemoryPressure::Yellow,
                4 => MemoryPressure::Red,
                _ => {
                    // Unknown level, fall back to simple heuristic
                    // This should rarely happen
                    let free_memory = total_memory.saturating_sub(used_memory);
                    let free_percentage = if total_memory == 0 {
                        100.0
                    } else {
                        (free_memory as f64 / total_memory as f64) * 100.0
                    };

                    if free_percentage >= 50.0 {
                        MemoryPressure::Green
                    } else if free_percentage >= 30.0 {
                        MemoryPressure::Yellow
                    } else {
                        MemoryPressure::Red
                    }
                }
            }
        } else {
            // Fallback if sysctl fails (non-macOS or permission issue)
            let free_memory = total_memory.saturating_sub(used_memory);
            let free_percentage = if total_memory == 0 {
                100.0
            } else {
                (free_memory as f64 / total_memory as f64) * 100.0
            };

            if free_percentage >= 50.0 {
                MemoryPressure::Green
            } else if free_percentage >= 30.0 {
                MemoryPressure::Yellow
            } else {
                MemoryPressure::Red
            }
        };

        // Calculate pressure percentage for display
        // This is a visual indicator, not used for pressure level determination
        let free_memory = total_memory.saturating_sub(used_memory);
        let free_percentage = if total_memory == 0 {
            100.0
        } else {
            (free_memory as f64 / total_memory as f64) * 100.0
        };

        MemoryInfo {
            total_memory,
            used_memory,
            total_swap,
            used_swap,
            pressure,
            pressure_percentage: 100.0 - free_percentage,
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
