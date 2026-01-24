mod app;
mod cpu;
mod gpu;
mod memory;
mod process;
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
    /// Incremental GPU update - just the new values for this tick
    Gpu {
        core_values: Vec<f32>, // Current value for each core
        overall_value: f32,    // Current overall utilisation
    },
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
}
use std::error::Error;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::mpsc;
use std::thread;
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

/// Macro to time an expression and log the result
macro_rules! profile {
    ($label:expr, $expr:expr) => {{
        let start = Instant::now();
        let result = $expr;
        log_timing($label, start.elapsed().as_millis());
        result
    }};
}

fn main() -> Result<(), Box<dyn Error>> {
    // Create channels for communication with background thread
    let (update_tx, update_rx) = mpsc::channel::<DataUpdate>();
    let (command_tx, command_rx) = mpsc::channel::<DataCommand>();

    // Spawn background data collection thread
    let collector_handle = thread::spawn(move || {
        run_data_collector(update_tx, command_rx);
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
            profile!("render", terminal.draw(|f| ui::render(f, &mut app))?);
        }
    }

    // Signal background thread to stop
    let _ = command_tx.send(DataCommand::Stop);
    let _ = collector_handle.join();

    Ok(())
}

fn run_data_collector(tx: mpsc::Sender<DataUpdate>, rx: mpsc::Receiver<DataCommand>) {
    use crate::cpu::CpuMonitor;
    use crate::gpu::GpuMonitor;
    use crate::memory::MemoryMonitor;
    use crate::process::ProcessMonitor;

    let mut cpu_monitor = CpuMonitor::new();
    let mut gpu_monitor = GpuMonitor::new();
    let mut memory_monitor = MemoryMonitor::new();
    let mut process_monitor = ProcessMonitor::new();

    let mut paused = false;
    let mut last_update = Instant::now() - Duration::from_secs(10); // Force immediate update
    let mut last_port_update = Instant::now() - Duration::from_secs(10);

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
            }
        }

        if !paused {
            let now = Instant::now();

            // Update everything every 1 second
            if now.duration_since(last_update) >= Duration::from_secs(1) {
                // CPU
                profile!("cpu_refresh", cpu_monitor.refresh());
                let usages = cpu_monitor.cpu_usages();

                // GPU
                profile!("gpu_refresh", gpu_monitor.refresh());
                let gpu_info = gpu_monitor.get_info();

                // Memory
                profile!("memory_refresh", memory_monitor.refresh());
                let mem_info = memory_monitor.get_memory_info();

                // Processes (ports every 15 seconds - lsof is expensive)
                let include_ports = now.duration_since(last_port_update) >= Duration::from_secs(15);
                if include_ports {
                    profile!("process_refresh_with_ports", process_monitor.refresh(true));
                    last_port_update = now;
                } else {
                    profile!("process_refresh", process_monitor.refresh(false));
                }

                // Send incremental updates (only new values, not full histories)
                let send_start = Instant::now();

                // CPU: send current values for each core
                let cpu_core_values: Vec<f32> = usages.iter().map(|(_, u)| *u).collect();
                let cpu_avg = if !cpu_core_values.is_empty() {
                    cpu_core_values.iter().sum::<f32>() / cpu_core_values.len() as f32
                } else {
                    0.0
                };
                let _ = tx.send(DataUpdate::Cpu {
                    core_values: cpu_core_values,
                    average_value: cpu_avg,
                });

                // GPU: send current values
                let _ = tx.send(DataUpdate::Gpu {
                    core_values: gpu_info.cores.iter().map(|c| c.utilization).collect(),
                    overall_value: gpu_info.overall_utilization,
                });

                // Memory: send current usage percentage
                let _ = tx.send(DataUpdate::Memory {
                    usage_value: mem_info.memory_usage_percentage() as f32,
                    info: mem_info,
                });

                let _ = tx.send(DataUpdate::Processes {
                    processes: process_monitor.get_processes().to_vec(),
                });
                log_timing("channel_send_all", send_start.elapsed().as_millis());

                last_update = now;
            }
        }

        // Sleep to avoid busy-waiting
        thread::sleep(Duration::from_millis(100));
    }
}
