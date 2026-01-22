use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Log timing data to /tmp/oversee-profile.log for performance analysis
fn log_timing(label: &str, duration_ms: u128) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/oversee-profile.log")
    {
        let _ = writeln!(file, "{}: {}ms", label, duration_ms);
    }
}

#[derive(Debug, Clone)]
pub struct GpuCoreInfo {
    pub utilization: f32, // 0-100%
}

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub cores: Vec<GpuCoreInfo>,
    pub overall_utilization: f32, // 0-100%
    #[allow(dead_code)] // Used for display/info purposes
    pub core_count: usize,
    #[allow(dead_code)] // Used for display/info purposes
    pub chip_name: String,
}

impl Default for GpuCoreInfo {
    fn default() -> Self {
        GpuCoreInfo { utilization: 0.0 }
    }
}

impl Default for GpuInfo {
    fn default() -> Self {
        GpuInfo {
            cores: Vec::new(),
            overall_utilization: 0.0,
            core_count: 0,
            chip_name: "Unknown".to_string(),
        }
    }
}

/// Shared state for the background powermetrics thread
struct PowermetricsState {
    /// GPU utilization stored as u32 bits (reinterpreted as f32)
    utilization_bits: AtomicU32,
    /// Signal to stop the background thread
    should_stop: AtomicBool,
}

impl PowermetricsState {
    fn new() -> Self {
        Self {
            utilization_bits: AtomicU32::new(0.0_f32.to_bits()),
            should_stop: AtomicBool::new(false),
        }
    }

    fn get_utilization(&self) -> f32 {
        f32::from_bits(self.utilization_bits.load(Ordering::Relaxed))
    }

    fn set_utilization(&self, value: f32) {
        self.utilization_bits
            .store(value.to_bits(), Ordering::Relaxed);
    }
}

pub struct GpuMonitor {
    current_info: GpuInfo,
    core_histories: Vec<VecDeque<f32>>,
    available: bool,
    core_count: usize,
    chip_name: String,
    /// Shared state with background thread
    state: Arc<PowermetricsState>,
    /// Handle to the background thread (for cleanup)
    _background_thread: Option<JoinHandle<()>>,
}

impl GpuMonitor {
    pub fn new() -> Self {
        let available = Self::is_apple_silicon();
        let (chip_name, core_count) = Self::detect_gpu_cores();

        let core_histories = (0..core_count)
            .map(|_| VecDeque::with_capacity(300))
            .collect();

        let current_info = GpuInfo {
            core_count,
            chip_name: chip_name.clone(),
            cores: (0..core_count).map(|_| GpuCoreInfo::default()).collect(),
            ..Default::default()
        };

        let state = Arc::new(PowermetricsState::new());

        // Spawn background thread for powermetrics polling if GPU is available
        let background_thread = if available {
            let state_clone = Arc::clone(&state);
            Some(thread::spawn(move || {
                Self::powermetrics_background_loop(state_clone);
            }))
        } else {
            None
        };

        GpuMonitor {
            current_info,
            core_histories,
            available,
            core_count,
            chip_name,
            state,
            _background_thread: background_thread,
        }
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    pub fn refresh(&mut self) {
        if !self.available {
            return;
        }

        // Generate GPU info with per-core data (reads from shared state)
        self.current_info = self.get_gpu_info();

        // Add each core's data to history
        for (i, core) in self.current_info.cores.iter().enumerate() {
            if i < self.core_histories.len() {
                self.core_histories[i].push_back(core.utilization);

                // Keep only the last 300 points (5 minutes)
                if self.core_histories[i].len() > 300 {
                    self.core_histories[i].pop_front();
                }
            }
        }
    }

    pub fn get_info(&self) -> &GpuInfo {
        &self.current_info
    }

    pub fn get_core_count(&self) -> usize {
        self.core_count
    }

    /// Background loop that polls powermetrics every 5 seconds
    fn powermetrics_background_loop(state: Arc<PowermetricsState>) {
        // Initial delay to let the app start up
        thread::sleep(Duration::from_millis(500));

        while !state.should_stop.load(Ordering::Relaxed) {
            if let Some(util) = Self::get_gpu_utilization_from_powermetrics() {
                state.set_utilization(util);
            }

            // Poll every 5 seconds (increased from 2s for lower overhead)
            for _ in 0..50 {
                if state.should_stop.load(Ordering::Relaxed) {
                    return;
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    // Check if running on Apple Silicon
    fn is_apple_silicon() -> bool {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;

            // Check if we're running on Apple Silicon
            if let Ok(output) = Command::new("uname").arg("-m").output() {
                let arch = String::from_utf8_lossy(&output.stdout);
                arch.trim() == "arm64"
            } else {
                false
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    // Detect GPU core count based on Apple Silicon chip
    fn detect_gpu_cores() -> (String, usize) {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;

            // Try to get chip name from system_profiler
            if let Ok(output) = Command::new("system_profiler")
                .arg("SPHardwareDataType")
                .output()
            {
                let info = String::from_utf8_lossy(&output.stdout);

                // Parse chip name and determine core count
                if info.contains("Apple M1") {
                    if info.contains("M1 Max") {
                        return ("M1 Max".to_string(), 32);
                    } else if info.contains("M1 Pro") {
                        return ("M1 Pro".to_string(), 16);
                    } else {
                        return ("M1".to_string(), 8);
                    }
                } else if info.contains("Apple M2") {
                    if info.contains("M2 Max") {
                        return ("M2 Max".to_string(), 38);
                    } else if info.contains("M2 Pro") {
                        return ("M2 Pro".to_string(), 19);
                    } else if info.contains("M2 Ultra") {
                        return ("M2 Ultra".to_string(), 76);
                    } else {
                        return ("M2".to_string(), 10);
                    }
                } else if info.contains("Apple M3") {
                    if info.contains("M3 Max") {
                        return ("M3 Max".to_string(), 40);
                    } else if info.contains("M3 Pro") {
                        return ("M3 Pro".to_string(), 18);
                    } else {
                        return ("M3".to_string(), 10);
                    }
                } else if info.contains("Apple M4") {
                    if info.contains("M4 Max") {
                        return ("M4 Max".to_string(), 40);
                    } else if info.contains("M4 Pro") {
                        return ("M4 Pro".to_string(), 20);
                    } else {
                        return ("M4".to_string(), 10);
                    }
                }
            }
        }

        // Fallback
        ("Unknown".to_string(), 8)
    }

    // Get real GPU info from shared state (updated by background thread)
    fn get_gpu_info(&self) -> GpuInfo {
        // Read utilization from shared state (lock-free)
        let overall_utilization = self.state.get_utilization();

        // Generate per-core utilization based on overall
        // Apple Silicon doesn't expose per-core GPU stats, so we estimate
        let mut cores = Vec::with_capacity(self.core_count);
        for i in 0..self.core_count {
            // Distribute load across cores with some variation
            // First half of cores (performance) get slightly more
            let core_factor = if i < self.core_count / 2 {
                1.0 + (i as f32 * 0.02) // Slight increase for first cores
            } else {
                0.9 - ((i - self.core_count / 2) as f32 * 0.02) // Slight decrease
            };

            let core_util = (overall_utilization * core_factor).clamp(0.0, 100.0);

            cores.push(GpuCoreInfo {
                utilization: core_util,
            });
        }

        GpuInfo {
            cores,
            overall_utilization,
            core_count: self.core_count,
            chip_name: self.chip_name.clone(),
        }
    }

    // Query powermetrics for GPU utilization (requires root)
    fn get_gpu_utilization_from_powermetrics() -> Option<f32> {
        use std::process::Command;

        // Run powermetrics to get GPU stats
        // -i 200 = 200ms sample (reduced from 500ms for faster response)
        // -n 1 = one sample only
        let start = Instant::now();
        let output = Command::new("powermetrics")
            .args(["--sampler", "gpu_power", "-i", "200", "-n", "1"])
            .output()
            .ok()?;
        log_timing("powermetrics_command", start.elapsed().as_millis());

        if !output.status.success() {
            return None; // Probably not running as root
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse "GPU HW active residency: XX.XX%" or similar patterns
        for line in stdout.lines() {
            let line_lower = line.to_lowercase();

            // Look for GPU active residency
            if line_lower.contains("gpu")
                && line_lower.contains("active")
                && line_lower.contains("residency")
            {
                // Extract percentage value
                if let Some(pct) = Self::extract_percentage(line) {
                    return Some(pct);
                }
            }

            // Alternative: "GPU Power" percentage
            // Note: Nested if required for MSRV compatibility (let chains are unstable)
            #[allow(clippy::collapsible_if)]
            if line_lower.contains("gpu") && line.contains("%") {
                if let Some(pct) = Self::extract_percentage(line) {
                    return Some(pct);
                }
            }
        }

        None
    }

    // Extract percentage value from a line like "GPU HW active residency:   5.23%"
    fn extract_percentage(line: &str) -> Option<f32> {
        // Find the percentage value (number followed by %)
        let mut num_start = None;
        let mut num_end = None;

        for (i, c) in line.chars().enumerate() {
            if c.is_ascii_digit() || c == '.' {
                if num_start.is_none() {
                    num_start = Some(i);
                }
                num_end = Some(i + 1);
            } else if c == '%' && num_end.is_some() {
                // Found the percentage
                break;
            } else if num_start.is_some() && !c.is_ascii_digit() && c != '.' {
                // Reset if we hit non-numeric before %
                num_start = None;
                num_end = None;
            }
        }

        if let (Some(start), Some(end)) = (num_start, num_end) {
            let num_str: String = line.chars().skip(start).take(end - start).collect();
            num_str.parse::<f32>().ok()
        } else {
            None
        }
    }
}

impl Default for GpuMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for GpuMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuMonitor")
            .field("available", &self.available)
            .field("core_count", &self.core_count)
            .field("chip_name", &self.chip_name)
            .finish_non_exhaustive()
    }
}

impl Drop for GpuMonitor {
    fn drop(&mut self) {
        // Signal the background thread to stop
        self.state.should_stop.store(true, Ordering::Relaxed);
    }
}
