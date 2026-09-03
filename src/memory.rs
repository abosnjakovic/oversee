use crate::convert::bytes_to_f64;
use std::collections::VecDeque;
use std::mem;
use sysinfo::System;

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
            name.as_ptr().cast::<i8>(),
            (&raw mut pressure_level).cast::<libc::c_void>(),
            &raw mut length,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    Green,  // Normal - macOS reports level 1
    Yellow, // Warning - macOS reports level 2
    Red,    // Critical - macOS reports level 4
}

/// Free memory as a percentage of the total. A total of zero means the system
/// reported nothing, which reads as fully free rather than as a division by zero.
fn free_percentage(total_memory: u64, used_memory: u64) -> f64 {
    if total_memory == 0 {
        return 100.0;
    }
    (bytes_to_f64(total_memory.saturating_sub(used_memory)) / bytes_to_f64(total_memory)) * 100.0
}

impl MemoryPressure {
    /// macOS reports 1 = Normal, 2 = Warning, 4 = Critical. Anything else is a
    /// level this code does not know, so the caller falls back to the heuristic.
    const fn from_macos_level(level: u32) -> Option<Self> {
        match level {
            1 => Some(Self::Green),
            2 => Some(Self::Yellow),
            4 => Some(Self::Red),
            _ => None,
        }
    }

    /// Fallback when the sysctl is unavailable or reports an unknown level.
    fn from_free_percentage(free: f64) -> Self {
        if free >= 50.0 {
            Self::Green
        } else if free >= 30.0 {
            Self::Yellow
        } else {
            Self::Red
        }
    }

    pub const fn color_name(self) -> &'static str {
        match self {
            Self::Green => "Normal",
            Self::Yellow => "Warning",
            Self::Red => "Critical",
        }
    }
}

#[derive(Debug, Clone, Copy)]
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
    pub const fn free_memory(&self) -> u64 {
        self.total_memory.saturating_sub(self.used_memory)
    }

    pub fn memory_usage_percentage(&self) -> f64 {
        if self.total_memory == 0 {
            0.0
        } else {
            (bytes_to_f64(self.used_memory) / bytes_to_f64(self.total_memory)) * 100.0
        }
    }

    pub fn swap_usage_percentage(&self) -> f64 {
        if self.total_swap == 0 {
            0.0
        } else {
            (bytes_to_f64(self.used_swap) / bytes_to_f64(self.total_swap)) * 100.0
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
        Self {
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

        // Native macOS pressure from kern.memorystatus_vm_pressure_level, which
        // is what Activity Monitor shows. The free-memory heuristic is only a
        // fallback for a failed sysctl or a level macOS has not documented.
        let free_percentage = free_percentage(total_memory, used_memory);
        let pressure = get_macos_memory_pressure_level()
            .and_then(MemoryPressure::from_macos_level)
            .unwrap_or_else(|| MemoryPressure::from_free_percentage(free_percentage));

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
    pub const fn get_pressure_history(&self) -> &VecDeque<MemoryPressure> {
        &self.pressure_history
    }
}

impl Default for MemoryMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// macOS documents 1, 2 and 4. Treating an undocumented level as Normal
    /// would hide real pressure, so it must fall through to the heuristic
    /// rather than map to anything.
    #[test]
    fn an_undocumented_macos_level_has_no_mapping() {
        assert_eq!(
            MemoryPressure::from_macos_level(1),
            Some(MemoryPressure::Green)
        );
        assert_eq!(
            MemoryPressure::from_macos_level(2),
            Some(MemoryPressure::Yellow)
        );
        assert_eq!(
            MemoryPressure::from_macos_level(4),
            Some(MemoryPressure::Red)
        );
        assert_eq!(MemoryPressure::from_macos_level(3), None);
        assert_eq!(MemoryPressure::from_macos_level(0), None);
    }

    /// A zero total comes from a failed reading, not from a machine with no
    /// memory, so it must not divide by zero or report full pressure.
    #[test]
    fn free_percentage_treats_an_unreported_total_as_free() {
        assert!((free_percentage(0, 0) - 100.0).abs() < f64::EPSILON);
        assert!((free_percentage(100, 25) - 75.0).abs() < f64::EPSILON);
        assert!((free_percentage(100, 100)).abs() < f64::EPSILON);
    }
}
