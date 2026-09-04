mod app;
mod category;
mod convert;
mod cpu;
mod gpu;
mod memory;
mod process;
mod theme;
mod tui;
mod ui;

use app::App;

/// Messages sent from the background data collector to the main thread
/// Uses incremental updates to avoid cloning large history buffers every second
pub enum DataUpdate {
    /// Incremental CPU update - just the new values for this tick
    Cpu {
        core_values: Vec<f32>, // Current value for each core
        average_value: f32,    // Current average across all cores
    },
    /// Incremental GPU update - just the new value for this tick.
    /// System-wide only; macOS exposes no per-core GPU breakdown.
    Gpu { overall_value: f32 },
    /// Incremental memory update - just the new value for this tick
    Memory {
        usage_value: f32, // Current memory usage percentage
        info: memory::MemoryInfo,
    },
    Processes {
        processes: Vec<process::ProcessInfo>,
    },
}

/// Commands sent from the main thread to control the data collector
pub enum DataCommand {
    Pause,
    Resume,
    Stop,
    ChangeSortMode,
    SetGpuActive(bool),
}
use std::error::Error;
#[cfg(feature = "profile")]
use std::fs::OpenOptions;
#[cfg(feature = "profile")]
use std::io::Write;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

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

/// Time an expression and log the result. No-op when the `profile` feature is off.
#[cfg(feature = "profile")]
macro_rules! profile {
    ($label:expr, $expr:expr) => {{
        let start = Instant::now();
        let result = $expr;
        log_timing($label, start.elapsed().as_millis());
        result
    }};
}

#[cfg(not(feature = "profile"))]
macro_rules! profile {
    ($label:expr, $expr:expr) => {{ $expr }};
}

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("oversee {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Create channels for communication with background thread
    let (update_tx, update_rx) = mpsc::channel::<DataUpdate>();
    let (command_tx, command_rx) = mpsc::channel::<DataCommand>();

    // Spawn background data collection thread
    let collector_handle = thread::spawn(move || {
        run_data_collector(&update_tx, &command_rx);
    });

    // Initialize terminal
    let mut terminal = tui::TuiGuard::new()?;

    // Create app with command channel
    let mut app = App::new(command_tx.clone());

    // Wait briefly for initial data to arrive
    thread::sleep(Duration::from_millis(100));
    app.process_updates(&update_rx);

    // Initial render
    terminal.draw(|f| ui::render(f, &mut app))?;

    // Main event loop
    while app.is_running() {
        // Handle keyboard events (blocks for up to 16ms)
        let event_occurred = profile!("event_poll", app.handle_event()?);

        // Process any data updates from background thread
        let data_updated = profile!("process_updates", app.process_updates(&update_rx));

        // Only render if something changed
        if data_updated || event_occurred {
            if std::mem::take(&mut app.needs_clear) {
                terminal.clear()?;
            }
            profile!("render", terminal.draw(|f| ui::render(f, &mut app))?);
        }
    }

    // Signal background thread to stop
    let _ = command_tx.send(DataCommand::Stop);
    let _ = collector_handle.join();

    Ok(())
}

/// Base interval between lsof port scans.
const PORT_SCAN_INTERVAL: Duration = Duration::from_secs(30);

/// Floor between port scans when a process the last scan never saw is running.
/// Without it, the short-lived processes macOS spawns constantly would drag
/// lsof back to every tick and undo the point of caching.
const PORT_RESCAN_DEBOUNCE: Duration = Duration::from_secs(5);

/// lsof is expensive, so ports are scanned on a long interval and cached in
/// between. A process that appeared after the last scan has unknown ports, so
/// it brings the next scan forward instead of waiting out the full interval.
fn should_scan_ports(port_age: Duration, unscanned_pids: bool) -> bool {
    port_age >= PORT_SCAN_INTERVAL || (unscanned_pids && port_age >= PORT_RESCAN_DEBOUNCE)
}

fn run_data_collector(tx: &mpsc::Sender<DataUpdate>, rx: &mpsc::Receiver<DataCommand>) {
    use crate::cpu::CpuMonitor;
    use crate::gpu::GpuMonitor;
    use crate::memory::MemoryMonitor;
    use crate::process::ProcessMonitor;

    let mut cpu_monitor = CpuMonitor::new();
    let gpu_monitor = GpuMonitor::new();
    let mut memory_monitor = MemoryMonitor::new();
    let mut process_monitor = ProcessMonitor::new();

    let mut paused = false;
    // `None` means the timer has never fired, so the first tick runs everything.
    let mut last_update: Option<Instant> = None;
    let mut last_port_update: Option<Instant> = None;
    let mut unscanned_pids = false;
    let mut last_full_process_refresh: Option<Instant> = None;

    loop {
        // Check for commands (non-blocking)
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                DataCommand::Pause => paused = true,
                DataCommand::Resume => paused = false,
                DataCommand::Stop => return,
                DataCommand::ChangeSortMode => {
                    process_monitor.next_sort_mode();
                    let _ = tx.send(DataUpdate::Processes {
                        processes: process_monitor.get_processes().to_vec(),
                    });
                }
                DataCommand::SetGpuActive(active) => gpu_monitor.set_active(active),
            }
        }

        if !paused {
            let now = Instant::now();

            // Update everything every 2 seconds. Each tick drives a sysinfo
            // process refresh which on macOS dispatches work across libdispatch
            // workers; halving the rate halves that idle cost.
            if last_update.is_none_or(|t| now.duration_since(t) >= Duration::from_secs(2)) {
                // CPU
                profile!("cpu_refresh", cpu_monitor.refresh());
                let usages = cpu_monitor.cpu_usages();

                // Memory
                profile!("memory_refresh", memory_monitor.refresh());
                let mem_info = memory_monitor.get_memory_info();

                // Processes: CPU-only refresh every 2 seconds, full refresh every 10 seconds.
                // Ports come from the monitor's cache in between scans.
                let port_age = last_port_update.map_or(Duration::MAX, |t| now.duration_since(t));
                let include_ports = should_scan_ports(port_age, unscanned_pids);
                let full_refresh = last_full_process_refresh
                    .is_none_or(|t| now.duration_since(t) >= Duration::from_secs(10));

                unscanned_pids = if include_ports {
                    last_port_update = Some(now);
                    last_full_process_refresh = Some(now);
                    profile!(
                        "process_refresh_with_ports",
                        process_monitor.refresh(true, true)
                    )
                } else if full_refresh {
                    last_full_process_refresh = Some(now);
                    profile!("process_refresh_full", process_monitor.refresh(false, true))
                } else {
                    profile!(
                        "process_refresh_cpu_only",
                        process_monitor.refresh(false, false)
                    )
                };

                // Send incremental updates (only new values, not full histories)
                #[cfg(feature = "profile")]
                let send_start = Instant::now();

                // CPU: send current values for each core
                let cpu_core_values: Vec<f32> = usages.iter().map(|(_, u)| *u).collect();
                let cpu_avg = if cpu_core_values.is_empty() {
                    0.0
                } else {
                    cpu_core_values.iter().sum::<f32>()
                        / convert::count_to_f32(cpu_core_values.len())
                };
                let _ = tx.send(DataUpdate::Cpu {
                    core_values: cpu_core_values,
                    average_value: cpu_avg,
                });

                // GPU: send current system-wide utilisation
                let _ = tx.send(DataUpdate::Gpu {
                    overall_value: gpu_monitor.utilization(),
                });

                // Memory: send current usage percentage
                let _ = tx.send(DataUpdate::Memory {
                    usage_value: convert::to_f32(mem_info.memory_usage_percentage()),
                    info: mem_info,
                });

                let _ = tx.send(DataUpdate::Processes {
                    processes: process_monitor.get_processes().to_vec(),
                });
                #[cfg(feature = "profile")]
                log_timing("channel_send_all", send_start.elapsed().as_millis());

                last_update = Some(now);
            }
        }

        // Sleep to avoid busy-waiting
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_scan_waits_out_the_interval_when_nothing_new_appeared() {
        assert!(!should_scan_ports(Duration::from_secs(29), false));
        assert!(should_scan_ports(PORT_SCAN_INTERVAL, false));
    }

    #[test]
    fn a_new_process_pulls_the_scan_forward_but_not_to_every_tick() {
        // A dev server that just started should not wait out the full interval
        // for its port to show up...
        assert!(should_scan_ports(PORT_RESCAN_DEBOUNCE, true));
        // ...but the churn of short-lived processes must not run lsof on every
        // 2-second tick.
        assert!(!should_scan_ports(Duration::from_secs(2), true));
    }
}
