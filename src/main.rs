mod app;
mod cpu;
mod gpu;
mod memory;
mod process;
mod tui;
mod ui;

use app::App;

/// Messages sent from the background data collector to the main thread
pub enum DataUpdate {
    Cpu {
        core_histories: Vec<std::collections::VecDeque<f32>>,
        average_history: std::collections::VecDeque<f32>,
    },
    Gpu {
        core_histories: Vec<std::collections::VecDeque<f32>>,
        overall_history: std::collections::VecDeque<f32>,
    },
    Memory {
        usage_history: std::collections::VecDeque<f32>,
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
    use std::collections::VecDeque;

    const MAX_HISTORY: usize = 1200;

    let mut cpu_monitor = CpuMonitor::new();
    let mut gpu_monitor = GpuMonitor::new();
    let mut memory_monitor = MemoryMonitor::new();
    let mut process_monitor = ProcessMonitor::new();

    let cpu_count = cpu_monitor.cpu_count();
    let gpu_core_count = gpu_monitor.get_core_count();

    // History buffers
    let mut cpu_core_histories: Vec<VecDeque<f32>> = (0..cpu_count)
        .map(|_| VecDeque::with_capacity(MAX_HISTORY))
        .collect();
    let mut cpu_average_history: VecDeque<f32> = VecDeque::with_capacity(MAX_HISTORY);
    let mut gpu_core_histories: Vec<VecDeque<f32>> = (0..gpu_core_count)
        .map(|_| VecDeque::with_capacity(MAX_HISTORY))
        .collect();
    let mut gpu_overall_history: VecDeque<f32> = VecDeque::with_capacity(MAX_HISTORY);
    let mut memory_usage_history: VecDeque<f32> = VecDeque::with_capacity(MAX_HISTORY);

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
                for (i, (_, usage)) in usages.iter().enumerate() {
                    if i < cpu_core_histories.len() {
                        cpu_core_histories[i].push_back(*usage);
                        if cpu_core_histories[i].len() > MAX_HISTORY {
                            cpu_core_histories[i].pop_front();
                        }
                    }
                }
                let cpu_count = usages.len();
                if cpu_count > 0 {
                    let total: f32 = usages.iter().map(|(_, u)| u).sum();
                    cpu_average_history.push_back(total / cpu_count as f32);
                    if cpu_average_history.len() > MAX_HISTORY {
                        cpu_average_history.pop_front();
                    }
                }

                // GPU
                profile!("gpu_refresh", gpu_monitor.refresh());
                let gpu_info = gpu_monitor.get_info();
                gpu_overall_history.push_back(gpu_info.overall_utilization);
                if gpu_overall_history.len() > MAX_HISTORY {
                    gpu_overall_history.pop_front();
                }
                for (i, core) in gpu_info.cores.iter().enumerate() {
                    if i < gpu_core_histories.len() {
                        gpu_core_histories[i].push_back(core.utilization);
                        if gpu_core_histories[i].len() > MAX_HISTORY {
                            gpu_core_histories[i].pop_front();
                        }
                    }
                }

                // Memory
                profile!("memory_refresh", memory_monitor.refresh());
                let mem_info = memory_monitor.get_memory_info();
                memory_usage_history.push_back(mem_info.memory_usage_percentage() as f32);
                if memory_usage_history.len() > MAX_HISTORY {
                    memory_usage_history.pop_front();
                }

                // Processes (ports every 15 seconds - lsof is expensive)
                let include_ports = now.duration_since(last_port_update) >= Duration::from_secs(15);
                if include_ports {
                    profile!("process_refresh_with_ports", process_monitor.refresh(true));
                    last_port_update = now;
                } else {
                    profile!("process_refresh", process_monitor.refresh(false));
                }

                // Send updates (measure total channel send time)
                let send_start = Instant::now();
                let _ = tx.send(DataUpdate::Cpu {
                    core_histories: cpu_core_histories.clone(),
                    average_history: cpu_average_history.clone(),
                });
                let _ = tx.send(DataUpdate::Gpu {
                    core_histories: gpu_core_histories.clone(),
                    overall_history: gpu_overall_history.clone(),
                });
                let _ = tx.send(DataUpdate::Memory {
                    usage_history: memory_usage_history.clone(),
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
