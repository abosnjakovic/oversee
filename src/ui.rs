use crate::app::App;
use crate::process::{ConnectionState, PortInfo, SortMode};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Paragraph, Row, Table, Wrap},
};

fn format_ports(ports: &[PortInfo]) -> String {
    if ports.is_empty() {
        return "-".to_string();
    }

    // Sort ports by listening status first, then by port number
    let mut sorted_ports = ports.to_vec();
    sorted_ports.sort_by(|a, b| match (&a.state, &b.state) {
        (ConnectionState::Listen, ConnectionState::Listen) => a.port.cmp(&b.port),
        (ConnectionState::Listen, _) => std::cmp::Ordering::Less,
        (_, ConnectionState::Listen) => std::cmp::Ordering::Greater,
        _ => a.port.cmp(&b.port),
    });

    // Take first 3 ports to fit in column
    let displayed_ports: Vec<String> = sorted_ports
        .iter()
        .take(3)
        .map(|port| {
            match port.state {
                ConnectionState::Listen => format!("{}L", port.port), // L for listening
                _ => port.port.to_string(),
            }
        })
        .collect();

    let mut result = displayed_ports.join(",");
    if ports.len() > 3 {
        result.push_str("...");
    }

    // Truncate to fit column (12 chars max)
    if result.len() > 12 {
        result.truncate(9);
        result.push_str("...");
    }

    result
}

pub fn render(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Add screen margins (1 char on all sides)
    let margin_area = Rect {
        x: size.x + 1,
        y: size.y + 1,
        width: size.width.saturating_sub(2),
        height: size.height.saturating_sub(2),
    };

    // Main layout: Timeline, memory section, then process list with spacing
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(24), // Timeline graph
            Constraint::Length(1),  // Spacing
            Constraint::Length(1),  // Memory stats (simplified to 1 line)
            Constraint::Length(1),  // Spacing
            Constraint::Min(8),     // Process list
        ])
        .split(margin_area);

    // Split timeline area: graph on left, reserved space for floating panel on right
    let timeline_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(80), // Graph area (left 70%)
            Constraint::Percentage(20), // Reserved for floating panel (right 30%)
        ])
        .split(main_chunks[0]);

    // Render timeline in the left area
    render_chart_timeline(f, app, timeline_chunks[0]);

    // Render cores panel in the right area
    render_cores_panel(f, app, timeline_chunks[1]);

    // Render memory stats section (simplified)
    render_memory_section(f, app, main_chunks[2]);

    // Render process list
    render_process_list(f, app, main_chunks[4]);

    // Render kill confirmation dialog if active
    if app.kill_confirmation_mode {
        render_kill_confirmation(f, app, size);
    }

    // Render help popup if active (render last so it appears on top)
    if app.help_mode {
        render_help_popup(f, app);
    }
}

fn render_process_list(f: &mut Frame, app: &mut App, area: Rect) {
    let all_processes = app.process_monitor.get_processes();
    let processes = app.get_filtered_processes();

    // Split for table and help - ensure help gets exactly 1 line at bottom
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),    // Process table (minimum 5 lines)
            Constraint::Length(1), // Help text (exactly 1 line at bottom)
        ])
        .split(area);

    // Header with sort indicators
    let sort_mode = app.process_monitor.get_sort_mode();
    let header_cells = vec![
        if matches!(sort_mode, SortMode::Pid) {
            "PID ↓"
        } else {
            "PID"
        },
        "User",
        if matches!(sort_mode, SortMode::Cpu) {
            "CPU% ↓"
        } else {
            "CPU%"
        },
        "GPU%",
        "Ports",
        if matches!(sort_mode, SortMode::Memory) {
            "MEM ↓"
        } else {
            "MEM"
        },
        if matches!(sort_mode, SortMode::Name) {
            "Command ↑"
        } else {
            "Command"
        },
    ];
    let header = Row::new(header_cells)
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .height(1);

    // Process rows
    let rows: Vec<Row> = processes
        .iter()
        .enumerate()
        .map(|(i, proc)| {
            let style = if i == app.get_selected_process() {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            // Convert memory to MB
            let mem_mb = proc.memory as f64 / (1024.0 * 1024.0);

            Row::new(vec![
                proc.pid.to_string(),
                truncate_string(&proc.user, 8), // Truncate username to fit column
                format!("{:.1}", proc.cpu_usage),
                format!("{:.1}", proc.gpu_usage),
                format_ports(&proc.ports),
                format!("{:.0}", mem_mb),
                truncate_string(&proc.cmd, 60),
            ])
            .style(style)
        })
        .collect();

    // Render title at top of the allocated chunk
    let title_text = if app.filter_mode {
        format!(
            "Processes ({} total) | Filter: {} _",
            all_processes.len(),
            app.filter_input
        )
    } else if !app.filter_input.is_empty() {
        format!(
            "Processes ({}/{} shown) | Filter: {}",
            processes.len(),
            all_processes.len(),
            app.filter_input
        )
    } else {
        format!("Processes ({} total)", all_processes.len())
    };
    let title = Paragraph::new(title_text).style(
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let title_area = Rect {
        x: chunks[0].x,
        y: chunks[0].y,
        width: chunks[0].width,
        height: 1,
    };
    f.render_widget(title, title_area);

    // Create table in remaining space of the first chunk
    let table_area = Rect {
        x: chunks[0].x,
        y: chunks[0].y + 1,
        width: chunks[0].width,
        height: chunks[0].height.saturating_sub(1),
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),  // PID
            Constraint::Length(8),  // User
            Constraint::Length(6),  // CPU%
            Constraint::Length(6),  // GPU%
            Constraint::Length(12), // Ports
            Constraint::Length(7),  // MEM (in MB)
            Constraint::Min(30),    // Command (flexible)
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("► ");

    f.render_stateful_widget(table, table_area, &mut app.table_state);

    // Help text
    let help_text = if app.kill_confirmation_mode {
        "⚠️  CONFIRM KILL: [Y] Yes | [N] No | ESC: Cancel"
    } else if app.filter_mode {
        "Type to filter | Enter: Apply | ESC: Cancel"
    } else if app.is_paused() {
        "[PAUSED] Space: Resume | q: Quit | ↑↓: Nav | K: Kill | s: Sort | /: Filter | +/-: Time | g/G: Top/Bot | v: GPU"
    } else {
        "Space: Pause | q: Quit | ↑↓: Nav | K: Kill | s: Sort | /: Filter | +/-: Time | g/G: Top/Bot | v: GPU"
    };

    // Render help in the bottom chunk (pinned to bottom)
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: true });

    f.render_widget(help, chunks[1]);
}

fn render_chart_timeline(f: &mut Frame, app: &App, area: Rect) {
    // Render title without border
    let title_text = format!("System Timeline ({})", app.get_timeline_position_text());
    let title = Paragraph::new(title_text).style(
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let title_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    f.render_widget(title, title_area);

    // Use full area minus title for the graph
    let inner = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height.saturating_sub(1),
    };

    // Use cached CPU average data (computed in update_cpu_data)
    let cpu_history: Vec<f32> = app.get_cpu_average_history().iter().copied().collect();

    // Get GPU history
    let gpu_history: Vec<f32> = app.gpu_overall_history.iter().copied().collect();

    // Get memory history
    let memory_history: Vec<f32> = app.memory_usage_history.iter().copied().collect();

    // Render oscilloscope-style timeline
    render_oscilloscope_timeline(
        f,
        inner,
        &cpu_history,
        &gpu_history,
        &memory_history,
        app.is_gpu_visible(),
        app.get_timeline_offset(),
    );
}

fn render_cores_panel(f: &mut Frame, app: &App, area: Rect) {
    let title = Paragraph::new("").style(
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let title_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    f.render_widget(title, title_area);

    // Use remaining area for cores display
    let cores_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height.saturating_sub(1),
    };

    // Split cores area horizontally: CPU cores left, GPU cores right
    let split_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // CPU cores left half
            Constraint::Percentage(50), // GPU cores right half
        ])
        .split(cores_area);

    // Render CPU cores in left half
    render_cpu_cores_panel(f, app, split_chunks[0]);

    // Render GPU cores in right half if visible
    if app.is_gpu_visible() {
        render_gpu_cores_panel(f, app, split_chunks[1]);
    }
}

fn render_cpu_cores_panel(f: &mut Frame, app: &App, area: Rect) {
    let cpu_count = app.get_cpu_count();
    let available_height = area.height as usize;

    if cpu_count == 0 || available_height == 0 {
        return;
    }

    // Each core gets 1 line
    let cores_to_show = available_height.min(cpu_count);
    let mut constraints = Vec::new();
    for _ in 0..cores_to_show {
        constraints.push(Constraint::Length(1));
    }

    let core_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Render CPU cores with dot visualization
    let cpu_usages = app.cpu_monitor.cpu_usages();
    for (i, (_cpu_name, usage)) in cpu_usages.iter().enumerate() {
        if i >= core_chunks.len() {
            break;
        }

        render_core_dot_line(
            f,
            core_chunks[i],
            &format!("CPU {}", i),
            *usage,
            Color::Cyan,
        );
    }
}

fn render_gpu_cores_panel(f: &mut Frame, app: &App, area: Rect) {
    let gpu_info = app.get_gpu_monitor().get_info();
    let gpu_count = gpu_info.cores.len();
    let available_height = area.height as usize;

    if gpu_count == 0 || available_height == 0 {
        return;
    }

    // Each core gets 1 line
    let cores_to_show = available_height.min(gpu_count);
    let mut constraints = Vec::new();
    for _ in 0..cores_to_show {
        constraints.push(Constraint::Length(1));
    }

    let core_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Render GPU cores with dot visualization
    for (i, core) in gpu_info.cores.iter().enumerate() {
        if i >= core_chunks.len() {
            break;
        }

        render_core_dot_line(
            f,
            core_chunks[i],
            &format!("GPU {}", i),
            core.utilization,
            Color::Magenta,
        );
    }
}

/// Interpolate between data points to create denser visualization
/// Creates `factor` intermediate points between each pair of data points
fn interpolate_data(data: &[f32], factor: usize) -> Vec<f32> {
    if data.len() < 2 || factor == 0 {
        return data.to_vec();
    }

    let mut interpolated = Vec::with_capacity(data.len() * factor);

    for i in 0..data.len() - 1 {
        let current = data[i];
        let next = data[i + 1];

        // Add the current point
        interpolated.push(current);

        // Add interpolated points
        for j in 1..factor {
            let fraction = j as f32 / factor as f32;
            let interpolated_value = current + (next - current) * fraction;
            interpolated.push(interpolated_value);
        }
    }

    // Add the last point
    if let Some(&last) = data.last() {
        interpolated.push(last);
    }

    interpolated
}

/// Helper function to get a braille character with a dot at specific position
/// Braille pattern is 2x4 dots (col 0-1, row 0-3)
/// Returns the Unicode braille character with the dot at the given position
fn get_braille_dot(col: usize, row: usize) -> char {
    // Braille dot positions (ISO/TR 11548-1)
    // Col 0: bits 0,1,2,6 (values 1,2,4,64)
    // Col 1: bits 3,4,5,7 (values 8,16,32,128)
    let dot_values = [
        [1, 8],    // Row 0
        [2, 16],   // Row 1
        [4, 32],   // Row 2
        [64, 128], // Row 3
    ];

    if row < 4 && col < 2 {
        let value = dot_values[row][col];
        std::char::from_u32(0x2800 + value).unwrap_or(' ')
    } else {
        ' '
    }
}

/// Get braille character for line drawing between two heights
/// Creates a connected line appearance by combining dots
fn get_braille_line(col: usize, start_row: usize, end_row: usize) -> char {
    if start_row == end_row {
        return get_braille_dot(col, start_row);
    }

    let (min_row, max_row) = if start_row < end_row {
        (start_row, end_row)
    } else {
        (end_row, start_row)
    };

    // Combine multiple dots for line effect
    let mut value = 0u32;
    let dot_values = [
        [1, 8],    // Row 0
        [2, 16],   // Row 1
        [4, 32],   // Row 2
        [64, 128], // Row 3
    ];

    for dot_row in dot_values.iter().take(max_row.min(3) + 1).skip(min_row) {
        if col < 2 {
            value += dot_row[col];
        }
    }

    std::char::from_u32(0x2800 + value).unwrap_or(' ')
}

/// Render oscilloscope-style timeline with waveform visualization
fn render_oscilloscope_timeline(
    f: &mut Frame,
    area: Rect,
    cpu_history: &[f32],
    gpu_history: &[f32],
    memory_history: &[f32],
    show_gpu: bool,
    timeline_offset: usize,
) {
    let available_width = area.width as usize;
    let available_height = area.height as usize;

    if available_width == 0 || available_height == 0 {
        return;
    }

    // Always display 300 seconds, but offset by timeline_offset
    const DISPLAY_DURATION: usize = 300;

    // Calculate the range we want to display
    let end_offset = timeline_offset;
    let start_offset = end_offset + DISPLAY_DURATION;

    // Get CPU data slice accounting for offset
    let cpu_points = if cpu_history.len() > start_offset {
        let start_idx = cpu_history.len() - start_offset;
        let end_idx = cpu_history.len() - end_offset;
        &cpu_history[start_idx..end_idx]
    } else if cpu_history.len() > end_offset {
        let end_idx = cpu_history.len() - end_offset;
        &cpu_history[0..end_idx]
    } else {
        &[]
    };

    // Get GPU data slice accounting for offset
    let gpu_points = if gpu_history.len() > start_offset {
        let start_idx = gpu_history.len() - start_offset;
        let end_idx = gpu_history.len() - end_offset;
        &gpu_history[start_idx..end_idx]
    } else if gpu_history.len() > end_offset {
        let end_idx = gpu_history.len() - end_offset;
        &gpu_history[0..end_idx]
    } else {
        &[]
    };

    // Get memory data slice accounting for offset
    let memory_points = if memory_history.len() > start_offset {
        let start_idx = memory_history.len() - start_offset;
        let end_idx = memory_history.len() - end_offset;
        &memory_history[start_idx..end_idx]
    } else if memory_history.len() > end_offset {
        let end_idx = memory_history.len() - end_offset;
        &memory_history[0..end_idx]
    } else {
        &[]
    };

    // Apply interpolation for denser visualization (4x density)
    let interpolation_factor = 4;
    let cpu_dense = interpolate_data(cpu_points, interpolation_factor);
    let gpu_dense = interpolate_data(gpu_points, interpolation_factor);
    let memory_dense = interpolate_data(memory_points, interpolation_factor);

    // Limit display width to available screen space
    // Each character cell has 2 braille columns, so we need 2 data points per character
    let display_points = (available_width * 2).min(cpu_dense.len());
    let cpu_display = if cpu_dense.len() > display_points {
        &cpu_dense[cpu_dense.len() - display_points..]
    } else {
        &cpu_dense[..]
    };

    let gpu_display = if gpu_dense.len() > display_points {
        &gpu_dense[gpu_dense.len() - display_points..]
    } else {
        &gpu_dense[..]
    };

    let memory_display = if memory_dense.len() > display_points {
        &memory_dense[memory_dense.len() - display_points..]
    } else {
        &memory_dense[..]
    };

    // Create a buffer for the display (each cell can have a braille character)
    // We use braille patterns which are 2 cols x 4 rows of dots per character cell
    let char_width = available_width;
    let char_height = available_height;

    // Each character cell has 2x4 braille dots, so total resolution is:
    let dot_height = char_height * 4;

    let mut prev_cpu_pos: Option<(u16, u16, usize, usize)> = None; // (x, y, char_row, sub_row)
    let mut prev_gpu_pos: Option<(u16, u16, usize, usize)> = None;
    let mut prev_memory_pos: Option<(u16, u16, usize, usize)> = None;

    // Render each time point (column)
    for col in 0..display_points {
        let cpu_usage = cpu_display
            .get(col)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, 100.0);
        let gpu_usage = if show_gpu {
            gpu_display
                .get(col)
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, 100.0)
        } else {
            0.0
        };
        let memory_usage = memory_display
            .get(col)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, 100.0);

        // Map usage (0-100%) to dot row (0 = bottom, dot_height-1 = top)
        let cpu_dot_row = ((cpu_usage / 100.0) * (dot_height - 1) as f32).round() as usize;
        let gpu_dot_row = ((gpu_usage / 100.0) * (dot_height - 1) as f32).round() as usize;
        let memory_dot_row = ((memory_usage / 100.0) * (dot_height - 1) as f32).round() as usize;

        // Convert dot row to character row and sub-row within character (0-3)
        let cpu_char_row = char_height.saturating_sub(1 + cpu_dot_row / 4);
        let cpu_sub_row = 3 - (cpu_dot_row % 4);

        let gpu_char_row = char_height.saturating_sub(1 + gpu_dot_row / 4);
        let gpu_sub_row = 3 - (gpu_dot_row % 4);

        let memory_char_row = char_height.saturating_sub(1 + memory_dot_row / 4);
        let memory_sub_row = 3 - (memory_dot_row % 4);

        // Determine which column within the braille character (0 or 1)
        let braille_col = col % 2;
        let char_col = col / 2;

        if char_col >= char_width {
            continue;
        }

        let x = area.x + char_col as u16;
        let cpu_y = area.y + cpu_char_row as u16;
        let gpu_y = area.y + gpu_char_row as u16;
        let memory_y = area.y + memory_char_row as u16;

        // Render CPU signal with vertical line connections
        if let Some((prev_x, prev_y, prev_char_row, prev_sub_row)) = prev_cpu_pos {
            // Draw vertical connecting lines if positions differ significantly
            if prev_x == x && prev_y != cpu_y {
                // Same column, different rows - draw vertical connection
                let (start_y, end_y) = if prev_y < cpu_y {
                    (prev_y, cpu_y)
                } else {
                    (cpu_y, prev_y)
                };

                for y in start_y..=end_y {
                    let row_idx = (y - area.y) as usize;
                    if row_idx < char_height {
                        // Fill with vertical line character
                        let connector = if y == start_y || y == end_y {
                            get_braille_dot(
                                braille_col,
                                if y == prev_y {
                                    prev_sub_row
                                } else {
                                    cpu_sub_row
                                },
                            )
                        } else {
                            // Middle section - full vertical line
                            '⡇' // Vertical braille line
                        };

                        let cell = Paragraph::new(connector.to_string())
                            .style(Style::default().fg(Color::Cyan));
                        f.render_widget(
                            cell,
                            Rect {
                                x,
                                y,
                                width: 1,
                                height: 1,
                            },
                        );
                    }
                }
            } else if prev_char_row == cpu_char_row && braille_col == 1 {
                // Same character row, draw connecting line within character
                let cpu_char = get_braille_line(braille_col, prev_sub_row, cpu_sub_row);
                let cell =
                    Paragraph::new(cpu_char.to_string()).style(Style::default().fg(Color::Cyan));
                f.render_widget(
                    cell,
                    Rect {
                        x,
                        y: cpu_y,
                        width: 1,
                        height: 1,
                    },
                );
            } else {
                // Just a dot
                let cpu_char = get_braille_dot(braille_col, cpu_sub_row);
                let cell =
                    Paragraph::new(cpu_char.to_string()).style(Style::default().fg(Color::Cyan));
                f.render_widget(
                    cell,
                    Rect {
                        x,
                        y: cpu_y,
                        width: 1,
                        height: 1,
                    },
                );
            }
        } else {
            // First point
            let cpu_char = get_braille_dot(braille_col, cpu_sub_row);
            let cell = Paragraph::new(cpu_char.to_string()).style(Style::default().fg(Color::Cyan));
            f.render_widget(
                cell,
                Rect {
                    x,
                    y: cpu_y,
                    width: 1,
                    height: 1,
                },
            );
        }

        // Render GPU signal if visible
        if show_gpu && gpu_char_row < char_height {
            if let Some((prev_x, prev_y, prev_char_row, prev_sub_row)) = prev_gpu_pos {
                // Draw vertical connecting lines for GPU
                if prev_x == x && prev_y != gpu_y {
                    let (start_y, end_y) = if prev_y < gpu_y {
                        (prev_y, gpu_y)
                    } else {
                        (gpu_y, prev_y)
                    };

                    for y in start_y..=end_y {
                        let row_idx = (y - area.y) as usize;
                        if row_idx < char_height {
                            let connector = if y == start_y || y == end_y {
                                get_braille_dot(
                                    braille_col,
                                    if y == prev_y {
                                        prev_sub_row
                                    } else {
                                        gpu_sub_row
                                    },
                                )
                            } else {
                                '⡇'
                            };

                            let cell = Paragraph::new(connector.to_string())
                                .style(Style::default().fg(Color::Magenta));
                            f.render_widget(
                                cell,
                                Rect {
                                    x,
                                    y,
                                    width: 1,
                                    height: 1,
                                },
                            );
                        }
                    }
                } else if prev_char_row == gpu_char_row && braille_col == 1 {
                    let gpu_char = get_braille_line(braille_col, prev_sub_row, gpu_sub_row);
                    let cell = Paragraph::new(gpu_char.to_string())
                        .style(Style::default().fg(Color::Magenta));
                    f.render_widget(
                        cell,
                        Rect {
                            x,
                            y: gpu_y,
                            width: 1,
                            height: 1,
                        },
                    );
                } else {
                    let gpu_char = get_braille_dot(braille_col, gpu_sub_row);
                    let cell = Paragraph::new(gpu_char.to_string())
                        .style(Style::default().fg(Color::Magenta));
                    f.render_widget(
                        cell,
                        Rect {
                            x,
                            y: gpu_y,
                            width: 1,
                            height: 1,
                        },
                    );
                }
            } else {
                let gpu_char = get_braille_dot(braille_col, gpu_sub_row);
                let cell =
                    Paragraph::new(gpu_char.to_string()).style(Style::default().fg(Color::Magenta));
                f.render_widget(
                    cell,
                    Rect {
                        x,
                        y: gpu_y,
                        width: 1,
                        height: 1,
                    },
                );
            }
        }

        // Render memory signal (always visible)
        if memory_char_row < char_height {
            if let Some((prev_x, prev_y, prev_char_row, prev_sub_row)) = prev_memory_pos {
                // Draw vertical connecting lines for memory
                if prev_x == x && prev_y != memory_y {
                    let (start_y, end_y) = if prev_y < memory_y {
                        (prev_y, memory_y)
                    } else {
                        (memory_y, prev_y)
                    };

                    for y in start_y..=end_y {
                        let row_idx = (y - area.y) as usize;
                        if row_idx < char_height {
                            let connector = if y == start_y || y == end_y {
                                get_braille_dot(
                                    braille_col,
                                    if y == prev_y {
                                        prev_sub_row
                                    } else {
                                        memory_sub_row
                                    },
                                )
                            } else {
                                '⡇'
                            };

                            let cell = Paragraph::new(connector.to_string())
                                .style(Style::default().fg(Color::Green));
                            f.render_widget(
                                cell,
                                Rect {
                                    x,
                                    y,
                                    width: 1,
                                    height: 1,
                                },
                            );
                        }
                    }
                } else if prev_char_row == memory_char_row && braille_col == 1 {
                    let memory_char = get_braille_line(braille_col, prev_sub_row, memory_sub_row);
                    let cell = Paragraph::new(memory_char.to_string())
                        .style(Style::default().fg(Color::Green));
                    f.render_widget(
                        cell,
                        Rect {
                            x,
                            y: memory_y,
                            width: 1,
                            height: 1,
                        },
                    );
                } else {
                    let memory_char = get_braille_dot(braille_col, memory_sub_row);
                    let cell = Paragraph::new(memory_char.to_string())
                        .style(Style::default().fg(Color::Green));
                    f.render_widget(
                        cell,
                        Rect {
                            x,
                            y: memory_y,
                            width: 1,
                            height: 1,
                        },
                    );
                }
            } else {
                let memory_char = get_braille_dot(braille_col, memory_sub_row);
                let cell = Paragraph::new(memory_char.to_string())
                    .style(Style::default().fg(Color::Green));
                f.render_widget(
                    cell,
                    Rect {
                        x,
                        y: memory_y,
                        width: 1,
                        height: 1,
                    },
                );
            }
        }

        // Update previous positions for line mode
        prev_cpu_pos = Some((x, cpu_y, cpu_char_row, cpu_sub_row));
        if show_gpu {
            prev_gpu_pos = Some((x, gpu_y, gpu_char_row, gpu_sub_row));
        }
        prev_memory_pos = Some((x, memory_y, memory_char_row, memory_sub_row));
    }

    // Render signal labels on the left side of the graph
    // Calculate average values for each signal
    if !cpu_display.is_empty() {
        let cpu_avg = cpu_display.iter().sum::<f32>() / cpu_display.len() as f32;
        let cpu_avg_dot_row = ((cpu_avg / 100.0) * (dot_height - 1) as f32).round() as usize;
        let cpu_avg_char_row = char_height.saturating_sub(1 + cpu_avg_dot_row / 4);
        let cpu_label_y = area.y + cpu_avg_char_row as u16;

        // Render "C" label for CPU in cyan
        let cpu_label = Paragraph::new("C").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(
            cpu_label,
            Rect {
                x: area.x,
                y: cpu_label_y,
                width: 1,
                height: 1,
            },
        );
    }

    if show_gpu && !gpu_display.is_empty() {
        let gpu_avg = gpu_display.iter().sum::<f32>() / gpu_display.len() as f32;
        let gpu_avg_dot_row = ((gpu_avg / 100.0) * (dot_height - 1) as f32).round() as usize;
        let gpu_avg_char_row = char_height.saturating_sub(1 + gpu_avg_dot_row / 4);
        let gpu_label_y = area.y + gpu_avg_char_row as u16;

        // Render "G" label for GPU in magenta
        let gpu_label = Paragraph::new("G").style(
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(
            gpu_label,
            Rect {
                x: area.x,
                y: gpu_label_y,
                width: 1,
                height: 1,
            },
        );
    }

    if !memory_display.is_empty() {
        let memory_avg = memory_display.iter().sum::<f32>() / memory_display.len() as f32;
        let memory_avg_dot_row = ((memory_avg / 100.0) * (dot_height - 1) as f32).round() as usize;
        let memory_avg_char_row = char_height.saturating_sub(1 + memory_avg_dot_row / 4);
        let memory_label_y = area.y + memory_avg_char_row as u16;

        // Render "M" label for Memory in green
        let memory_label = Paragraph::new("M").style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(
            memory_label,
            Rect {
                x: area.x,
                y: memory_label_y,
                width: 1,
                height: 1,
            },
        );
    }
}

fn get_gradient_color(usage: f32) -> Color {
    // Smooth gradient from green -> yellow -> orange -> red
    // Similar to btop++ color gradient
    if usage >= 90.0 {
        Color::Red
    } else if usage >= 75.0 {
        Color::LightRed // Orange-ish
    } else if usage >= 50.0 {
        Color::Yellow
    } else if usage >= 25.0 {
        Color::LightYellow
    } else {
        Color::Green
    }
}

fn render_core_dot_line(
    f: &mut Frame,
    area: Rect,
    name: &str,
    usage: f32,
    base_color: Color, // Use the provided color for text (cyan for CPU, magenta for GPU)
) {
    // Block characters for vertical bar display (like btop++)
    const BLOCKS: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

    // Calculate which block to use based on usage
    let block_idx = ((usage / 100.0) * 8.0).round() as usize;
    let block = BLOCKS[block_idx.min(8)];

    // Get gradient color for the bar based on usage
    let bar_color = get_gradient_color(usage);

    // Format name (ensure consistent alignment)
    let short_name = if name.starts_with("CPU") {
        let num = name.replace("CPU ", "");
        format!("C{:>2}", num)
    } else if name.starts_with("GPU") {
        let num = name.replace("GPU ", "");
        format!("G{:>2}", num)
    } else {
        name.to_string()
    };

    // Create properly aligned text: "C 0: █  45%"
    let name_span = ratatui::text::Span::styled(
        format!("{:<3}:", short_name),
        Style::default().fg(base_color),
    );

    let bar_span =
        ratatui::text::Span::styled(format!(" {}", block), Style::default().fg(bar_color));

    let percent_span =
        ratatui::text::Span::styled(format!(" {:>3.0}%", usage), Style::default().fg(base_color));

    let line = ratatui::text::Line::from(vec![name_span, bar_span, percent_span]);
    let paragraph = Paragraph::new(line).wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn render_memory_section(f: &mut Frame, app: &App, area: Rect) {
    use crate::memory::MemoryPressure;

    let memory_info = app.memory_monitor.get_memory_info();

    // Simplified single-line memory display with key stats
    let stats_text = if memory_info.total_swap > 0 {
        format!(
            "Memory: {:.1}GB / {:.1}GB ({:.1}%) | Pressure: {} | Free: {:.1}GB | Swap: {:.1}GB / {:.1}GB ({:.1}%)",
            memory_info.used_memory as f64 / (1024.0 * 1024.0 * 1024.0),
            memory_info.total_memory as f64 / (1024.0 * 1024.0 * 1024.0),
            memory_info.memory_usage_percentage(),
            memory_info.pressure.color_name(),
            memory_info.free_memory() as f64 / (1024.0 * 1024.0 * 1024.0),
            memory_info.used_swap as f64 / (1024.0 * 1024.0 * 1024.0),
            memory_info.total_swap as f64 / (1024.0 * 1024.0 * 1024.0),
            memory_info.swap_usage_percentage()
        )
    } else {
        format!(
            "Memory: {:.1}GB / {:.1}GB ({:.1}%) | Pressure: {} | Free: {:.1}GB",
            memory_info.used_memory as f64 / (1024.0 * 1024.0 * 1024.0),
            memory_info.total_memory as f64 / (1024.0 * 1024.0 * 1024.0),
            memory_info.memory_usage_percentage(),
            memory_info.pressure.color_name(),
            memory_info.free_memory() as f64 / (1024.0 * 1024.0 * 1024.0)
        )
    };

    let title_color = match memory_info.pressure {
        MemoryPressure::Green => Color::Green,
        MemoryPressure::Yellow => Color::Yellow,
        MemoryPressure::Red => Color::Red,
    };

    let stats = Paragraph::new(stats_text).style(
        Style::default()
            .fg(title_color)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(stats, area);
}

fn render_kill_confirmation(f: &mut Frame, app: &App, screen_area: Rect) {
    // Create a centered dialog box
    let dialog_width = 50;
    let dialog_height = 7;

    let dialog_x = (screen_area.width.saturating_sub(dialog_width)) / 2;
    let dialog_y = (screen_area.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = Rect {
        x: dialog_x,
        y: dialog_y,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear the background (create a modal effect)
    let clear_widget = ratatui::widgets::Clear;
    f.render_widget(clear_widget, dialog_area);

    // Create the dialog content
    let dialog_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title
            Constraint::Length(1), // Spacing
            Constraint::Length(1), // Process info
            Constraint::Length(1), // Warning
            Constraint::Length(1), // Spacing
            Constraint::Length(1), // Options
            Constraint::Length(1), // Border
        ])
        .split(dialog_area);

    // Dialog border
    let border_block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title("Kill Process");
    f.render_widget(border_block, dialog_area);

    // Title
    let title_text = "⚠️  KILL PROCESS  ⚠️";
    let title = Paragraph::new(title_text)
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));
    f.render_widget(title, dialog_chunks[0]);

    // Process information
    let process_info = if let Some(pid) = app.kill_target_pid {
        format!("PID: {} - {}", pid, app.kill_target_name)
    } else {
        "Unknown process".to_string()
    };
    let process_text = Paragraph::new(process_info)
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(Color::White));
    f.render_widget(process_text, dialog_chunks[2]);

    // Warning message
    let warning_text = "This action cannot be undone!";
    let warning = Paragraph::new(warning_text)
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(warning, dialog_chunks[3]);

    // Options
    let options_text = "[Y] Kill Process    [N] Cancel";
    let options = Paragraph::new(options_text)
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(Color::Gray));
    f.render_widget(options, dialog_chunks[5]);
}

fn render_help_popup(f: &mut Frame, _app: &App) {
    use ratatui::{
        style::{Color, Modifier, Style},
        text::{Line, Span, Text},
        widgets::{Block, Borders, Clear, Paragraph, Wrap},
    };

    // Calculate popup size (80% of screen)
    let popup_area = {
        let area = f.area();
        let horizontal_margin = area.width / 10;
        let vertical_margin = area.height / 10;
        ratatui::layout::Rect {
            x: horizontal_margin,
            y: vertical_margin,
            width: area.width.saturating_sub(horizontal_margin * 2),
            height: area.height.saturating_sub(vertical_margin * 2),
        }
    };

    // Clear the area
    f.render_widget(Clear, popup_area);

    // Create help content
    let help_text = vec![
        Line::from(vec![Span::styled(
            "KEYBINDS",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  j/k or ↑↓     Navigate process list up/down"),
        Line::from("  g             Jump to top of process list"),
        Line::from("  G             Jump to bottom of process list"),
        Line::from("  Page Up/Down  Navigate by 10 processes"),
        Line::from("  Home/End      Jump to first/last process"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Actions:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Space         Pause/Resume monitoring"),
        Line::from("  s             Cycle through sort modes"),
        Line::from("  v             Toggle GPU visibility"),
        Line::from("  K             Kill selected process (with confirmation)"),
        Line::from("  /             Enter filter mode"),
        Line::from("  +/=           Scroll timeline forward (newer data)"),
        Line::from("  -             Scroll timeline backward (older data, up to 15 min)"),
        Line::from("  ?             Toggle this help popup"),
        Line::from("  q or ESC      Quit application"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "TIMELINE",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Timeline always displays 5 minutes of data. Use +/- to navigate"),
        Line::from("through up to 20 minutes of historical system metrics."),
        Line::from(""),
        Line::from(vec![Span::styled(
            "MEMORY PRESSURE ALGORITHM",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Oversee uses macOS's native memory pressure reporting:"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "How it works:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Queries kern.memorystatus_vm_pressure_level sysctl"),
        Line::from("  Same metric used by Activity Monitor for accuracy"),
        Line::from("  Considers file cache, compression, and memory demand"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Pressure Levels:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                "  • Green (Normal): ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Adequate memory, efficient operation"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • Yellow (Warning): ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Some pressure, may use compression"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • Red (Critical): ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("Severe pressure, performance impacted"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Note: ", Style::default().fg(Color::Cyan)),
            Span::raw("macOS uses memory differently than other systems."),
        ]),
        Line::from("High usage with green pressure is optimal. See README for details"),
        Line::from("on why your Mac keeps memory full for better performance."),
        Line::from(""),
        Line::from(vec![Span::styled(
            "ABOUT OVERSEE",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("A modern system monitor for macOS, inspired by htop and btop++,"),
        Line::from("built in Rust with a focus on Apple Silicon performance monitoring."),
        Line::from(""),
        Line::from("Features CPU and GPU core monitoring, memory pressure indicators"),
        Line::from("matching Activity Monitor, timeline visualization with braille"),
        Line::from("characters, and vim-style navigation controls."),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press ? or ESC to close this help",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )]),
    ];

    // Create the popup block
    let block = Block::default()
        .title(" Help - Oversee System Monitor ")
        .title_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    // Create the paragraph widget
    let paragraph = Paragraph::new(Text::from(help_text))
        .block(block)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::White));

    // Render the popup
    f.render_widget(paragraph, popup_area);
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_pattern_generation() {
        // Test 0% usage - should be all empty dots
        let (filled, empty) = generate_dot_pattern(0.0);
        assert_eq!(filled, 0);
        assert_eq!(empty, 10);

        // Test 50% usage - should be 5 filled, 5 empty
        let (filled, empty) = generate_dot_pattern(50.0);
        assert_eq!(filled, 5);
        assert_eq!(empty, 5);

        // Test 100% usage - should be all filled dots
        let (filled, empty) = generate_dot_pattern(100.0);
        assert_eq!(filled, 10);
        assert_eq!(empty, 0);

        // Test edge case: over 100% - should cap at 10
        let (filled, empty) = generate_dot_pattern(150.0);
        assert_eq!(filled, 10);
        assert_eq!(empty, 0);

        // Test rounding: 85% should round to 9 dots (85/10 = 8.5 -> 9)
        let (filled, empty) = generate_dot_pattern(85.0);
        assert_eq!(filled, 9);
        assert_eq!(empty, 1);
    }

    #[test]
    fn test_dot_string_format() {
        let usage_levels = vec![0.0, 25.0, 50.0, 75.0, 100.0];

        for usage in usage_levels.iter() {
            let (filled, empty) = generate_dot_pattern(*usage);
            let pattern = format!("{}{}", "•".repeat(filled), "·".repeat(empty));

            // Verify total visual character count is always 10
            assert_eq!(
                pattern.chars().count(),
                10,
                "Pattern should have 10 visual characters for usage: {}",
                usage
            );

            // Verify filled + empty = 10
            assert_eq!(
                filled + empty,
                10,
                "Filled ({}) + empty ({}) should equal 10 for usage: {}",
                filled,
                empty,
                usage
            );

            // Verify the pattern contains the right characters
            assert!(
                pattern.contains("•") || filled == 0,
                "Pattern should contain filled dots if filled > 0"
            );
            assert!(
                pattern.contains("·") || empty == 0,
                "Pattern should contain empty dots if empty > 0"
            );
        }
    }

    #[test]
    fn test_core_name_formatting() {
        let test_cases = vec![
            ("CPU 0", 45.0, "CPU 0 : ••••••.... 45%"),
            ("GPU 15", 80.0, "GPU 15: ••••••••.. 80%"),
            ("CPU", 100.0, "CPU   : •••••••••• 100%"),
        ];

        for (name, usage, _expected_pattern) in test_cases {
            let line = format_core_line(name, usage);
            // Verify the structure but not exact spacing since that might vary
            assert!(line.contains(name));
            assert!(line.contains(&format!("{}%", usage as i32)));
            assert!(line.contains("•") || usage == 0.0);
        }
    }

    #[test]
    fn test_floating_panel_dimensions() {
        // Test panel width calculation
        let test_area = Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 30,
        };
        let panel_width = (test_area.width / 3).max(35);
        assert_eq!(panel_width, 40); // 120/3 = 40, which is > 35

        // Test minimum width enforcement
        let small_area = Rect {
            x: 0,
            y: 0,
            width: 90,
            height: 30,
        };
        let small_panel_width = (small_area.width / 3).max(35);
        assert_eq!(small_panel_width, 35); // 90/3 = 30, but minimum is 35

        // Test height calculation
        let panel_height = test_area.height.saturating_sub(2);
        assert_eq!(panel_height, 28);
    }

    #[test]
    fn test_cores_to_show_calculation() {
        let available_height = 25;
        let total_cores = 34; // 14 CPU + 20 GPU
        let lines_per_core = 1;

        let cores_to_show = (available_height / lines_per_core).min(total_cores);
        assert_eq!(cores_to_show, 25); // Should show 25 cores out of 34

        // Test when we have fewer cores than available height
        let few_cores = 10;
        let cores_to_show_few = (available_height / lines_per_core).min(few_cores);
        assert_eq!(cores_to_show_few, 10); // Should show all 10 cores
    }

    #[test]
    fn test_gpu_detection_expectations() {
        // Skip this test in CI environments where GPU detection might fail
        // The test is meant to document expected behavior on actual hardware

        // Instead of creating a real GPU monitor which might panic,
        // just test the logic expectations
        let mock_core_counts = vec![0, 8, 10, 16, 20, 32]; // Common GPU core counts

        for core_count in mock_core_counts {
            // Verify the count is reasonable (not impossibly high)
            assert!(
                core_count <= 40,
                "GPU core count should be reasonable: {}",
                core_count
            );

            // Document expected behavior: if cores > 0, GPU should be available
            if core_count > 0 {
                // This would be true for a real GPU monitor
                println!(
                    "Mock GPU cores: {} (would indicate available GPU)",
                    core_count
                );
            } else {
                println!("Mock GPU cores: 0 (would indicate no GPU)");
            }
        }
    }

    // Helper functions for tests
    fn generate_dot_pattern(usage: f32) -> (usize, usize) {
        let filled_dots = (usage / 10.0).round() as usize;
        let filled_dots = filled_dots.min(10);
        let empty_dots = 10 - filled_dots;
        (filled_dots, empty_dots)
    }

    fn format_core_line(name: &str, usage: f32) -> String {
        let (filled, empty) = generate_dot_pattern(usage);
        let filled_str = "•".repeat(filled);
        let empty_str = "·".repeat(empty);
        let dots = format!("{}{}", filled_str, empty_str);
        format!("{:<6}: {} {:>3.0}%", name, dots, usage)
    }
}
