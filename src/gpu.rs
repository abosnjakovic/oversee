#[cfg(feature = "profile")]
use std::fs::OpenOptions;
#[cfg(feature = "profile")]
use std::io::Write as IoWrite;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;
#[cfg(feature = "profile")]
use std::time::Instant;

/// Log timing data to /tmp/oversee-profile.log. Active only with `--features profile`.
#[cfg(feature = "profile")]
fn log_timing(label: &str, duration_ms: u128) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/oversee-profile.log")
    {
        let _ = writeln!(file, "{}: {}ms", label, duration_ms);
    }
}

/// Shared state for the background powermetrics thread
struct PowermetricsState {
    /// GPU utilization stored as u32 bits (reinterpreted as f32)
    utilization_bits: AtomicU32,
    /// Signal to stop the background thread
    should_stop: AtomicBool,
    /// When false, the background loop skips spawning `powermetrics`.
    /// Updated from the data collector when the user toggles the GPU panel.
    active: AtomicBool,
}

impl PowermetricsState {
    const fn new() -> Self {
        Self {
            utilization_bits: AtomicU32::new(0.0_f32.to_bits()),
            should_stop: AtomicBool::new(false),
            active: AtomicBool::new(true),
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
    available: bool,
    /// Shared state with background thread
    state: Arc<PowermetricsState>,
    /// Handle to the background thread (for cleanup)
    _background_thread: Option<JoinHandle<()>>,
}

impl GpuMonitor {
    pub fn new() -> Self {
        let available = Self::is_apple_silicon();
        let state = Arc::new(PowermetricsState::new());

        // Spawn background thread for powermetrics polling if GPU is available
        let background_thread = if available {
            let state_clone = Arc::clone(&state);
            Some(thread::spawn(move || {
                Self::powermetrics_background_loop(&state_clone);
            }))
        } else {
            None
        };

        Self {
            available,
            state,
            _background_thread: background_thread,
        }
    }

    pub const fn is_available(&self) -> bool {
        self.available
    }

    /// Toggle whether the background thread spawns `powermetrics`. When the GPU
    /// panel is hidden we skip the subprocess to drop idle CPU.
    pub fn set_active(&self, active: bool) {
        self.state.active.store(active, Ordering::Relaxed);
    }

    /// System-wide GPU utilisation, 0-100%. Updated by the background thread.
    ///
    /// This is the only GPU figure macOS makes available to us: the `gpu_power`
    /// sampler reports aggregate hardware active residency, never per-core or
    /// per-process breakdowns. Reads 0.0 until the first successful
    /// `powermetrics` run, which requires root.
    pub fn utilization(&self) -> f32 {
        self.state.get_utilization()
    }

    /// Background loop that polls powermetrics every 5 seconds
    fn powermetrics_background_loop(state: &PowermetricsState) {
        // Initial delay to let the app start up
        thread::sleep(Duration::from_millis(500));

        while !state.should_stop.load(Ordering::Relaxed) {
            // Skip the powermetrics subprocess entirely when the GPU panel is hidden.
            // The 5s sleep below still runs so we resume promptly when re-activated.
            if state.active.load(Ordering::Relaxed)
                && let Some(util) = Self::get_gpu_utilization_from_powermetrics()
            {
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

    // Query powermetrics for GPU utilization (requires root)
    fn get_gpu_utilization_from_powermetrics() -> Option<f32> {
        use std::process::Command;

        // Run powermetrics to get GPU stats
        // -i 200 = 200ms sample (reduced from 500ms for faster response)
        // -n 1 = one sample only
        #[cfg(feature = "profile")]
        let start = Instant::now();
        let output = Command::new("powermetrics")
            .args(["--sampler", "gpu_power", "-i", "200", "-n", "1"])
            .output()
            .ok()?;
        #[cfg(feature = "profile")]
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
            if line_lower.contains("gpu") && line.contains('%') {
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
                num_end = Some(i.saturating_add(1));
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
            let num_str: String = line
                .chars()
                .skip(start)
                .take(end.saturating_sub(start))
                .collect();
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
            .finish_non_exhaustive()
    }
}

impl Drop for GpuMonitor {
    fn drop(&mut self) {
        // Signal the background thread to stop
        self.state.should_stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_percentage_reads_the_value_before_the_percent_sign() {
        // Real powermetrics gpu_power output, including the trailing frequency
        // residency breakdown that also contains percentages.
        assert_eq!(
            GpuMonitor::extract_percentage(
                "GPU HW active residency:  12.23% (444 MHz: 12% 612 MHz: 0%)"
            ),
            Some(12.23)
        );

        // Digits before the real figure must not be picked up.
        assert_eq!(
            GpuMonitor::extract_percentage("GPU 0 HW active residency: 5.23%"),
            Some(5.23)
        );

        // No numeric value at all.
        assert_eq!(
            GpuMonitor::extract_percentage("GPU HW active residency:"),
            None
        );
    }
}
