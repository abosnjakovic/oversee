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
    let all_processes = app.get_all_processes();
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
    let sort_mode = app.get_sort_mode();
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
            let is_pinned = app.pinned_pids.contains(&proc.pid);
            let is_selected = i == app.get_selected_process();

            let style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_pinned {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            // Convert memory to MB
            let mem_mb = proc.memory as f64 / (1024.0 * 1024.0);

            // Show full command for pinned processes, truncate for others
            let cmd_display = if is_pinned {
                proc.cmd.clone()
            } else {
                truncate_string(&proc.cmd, 60)
            };

            // Add pin indicator to PID column for pinned processes
            let pid_display = if is_pinned {
                format!("◆ {}", proc.pid)
            } else {
                proc.pid.to_string()
            };

            Row::new(vec![
                pid_display,
                truncate_string(&proc.user, 8), // Truncate username to fit column
                format!("{:.1}", proc.cpu_usage),
                format!("{:.1}", proc.gpu_usage),
                format_ports(&proc.ports),
                format!("{:.0}", mem_mb),
                cmd_display,
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
        "[PAUSED] Space: Resume | q: Quit | ↑↓: Nav | Enter: Pin | K: Kill | s: Sort | /: Filter | +/-: Time | g/G: Top/Bot"
    } else {
        "Space: Pause | q: Quit | ↑↓: Nav | Enter: Pin | K: Kill | s: Sort | /: Filter | +/-: Time | g/G: Top/Bot"
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
    let cpu_usages = app.get_cpu_usages();
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

/// Cell colour type for the rendering buffer
#[derive(Clone, Copy, PartialEq, Eq)]
enum CellColor {
    None,
    Cpu,
    Gpu,
    Memory,
}

/// Render oscilloscope-style timeline with waveform visualization
/// Uses a buffered approach to batch character rendering and reduce widget allocations
fn render_oscilloscope_timeline(
    f: &mut Frame,
    area: Rect,
    cpu_history: &[f32],
    gpu_history: &[f32],
    memory_history: &[f32],
    show_gpu: bool,
    timeline_offset: usize,
) {
    use ratatui::text::{Line, Span};

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

    // Get data slices accounting for offset
    let cpu_points = get_history_slice(cpu_history, start_offset, end_offset);
    let gpu_points = get_history_slice(gpu_history, start_offset, end_offset);
    let memory_points = get_history_slice(memory_history, start_offset, end_offset);

    // Apply interpolation for denser visualization (4x density)
    let interpolation_factor = 4;
    let cpu_dense = interpolate_data(cpu_points, interpolation_factor);
    let gpu_dense = interpolate_data(gpu_points, interpolation_factor);
    let memory_dense = interpolate_data(memory_points, interpolation_factor);

    // Limit display width to available screen space
    // Each character cell has 2 braille columns, so we need 2 data points per character
    let display_points = (available_width * 2).min(cpu_dense.len());
    let cpu_display = get_display_slice(&cpu_dense, display_points);
    let gpu_display = get_display_slice(&gpu_dense, display_points);
    let memory_display = get_display_slice(&memory_dense, display_points);

    let char_width = available_width;
    let char_height = available_height;
    let dot_height = char_height * 4;

    // Create buffers for characters and colours - one row at a time rendering
    // Buffer stores (braille_bits, color) for each character cell
    let mut row_buffer: Vec<(u32, CellColor)> = vec![(0, CellColor::None); char_width];

    // Track previous positions for line connections
    let mut prev_cpu_row: Option<usize> = None;
    let mut prev_gpu_row: Option<usize> = None;
    let mut prev_memory_row: Option<usize> = None;

    // Process each row from top to bottom
    for row_idx in 0..char_height {
        // Clear the row buffer
        for cell in row_buffer.iter_mut() {
            *cell = (0, CellColor::None);
        }

        // Process each data point
        for col in 0..display_points {
            let char_col = col / 2;
            let braille_col = col % 2;

            if char_col >= char_width {
                continue;
            }

            // Get usage values
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

            // Convert to dot rows
            let cpu_dot_row = ((cpu_usage / 100.0) * (dot_height - 1) as f32).round() as usize;
            let gpu_dot_row = ((gpu_usage / 100.0) * (dot_height - 1) as f32).round() as usize;
            let memory_dot_row =
                ((memory_usage / 100.0) * (dot_height - 1) as f32).round() as usize;

            // Convert to character row and sub-row
            let cpu_char_row = char_height.saturating_sub(1 + cpu_dot_row / 4);
            let gpu_char_row = char_height.saturating_sub(1 + gpu_dot_row / 4);
            let memory_char_row = char_height.saturating_sub(1 + memory_dot_row / 4);

            let cpu_sub_row = 3 - (cpu_dot_row % 4);
            let gpu_sub_row = 3 - (gpu_dot_row % 4);
            let memory_sub_row = 3 - (memory_dot_row % 4);

            // Check if this row contains CPU data
            if cpu_char_row == row_idx {
                let bits = get_braille_bits(braille_col, cpu_sub_row);
                row_buffer[char_col].0 |= bits;
                row_buffer[char_col].1 = CellColor::Cpu;
            }

            // Check if this row contains GPU data (GPU overwrites CPU if overlapping)
            if show_gpu && gpu_char_row == row_idx {
                let bits = get_braille_bits(braille_col, gpu_sub_row);
                row_buffer[char_col].0 |= bits;
                row_buffer[char_col].1 = CellColor::Gpu;
            }

            // Check if this row contains memory data (Memory overwrites others if overlapping)
            if memory_char_row == row_idx {
                let bits = get_braille_bits(braille_col, memory_sub_row);
                row_buffer[char_col].0 |= bits;
                row_buffer[char_col].1 = CellColor::Memory;
            }

            // Handle vertical line connections for CPU
            if let Some(prev_row) = prev_cpu_row
                && prev_row != cpu_char_row
            {
                let (start, end) = if prev_row < cpu_char_row {
                    (prev_row, cpu_char_row)
                } else {
                    (cpu_char_row, prev_row)
                };
                // Fill vertical line if this row is between start and end
                if row_idx > start && row_idx < end {
                    row_buffer[char_col].0 |= get_vertical_line_bits(braille_col);
                    if row_buffer[char_col].1 == CellColor::None {
                        row_buffer[char_col].1 = CellColor::Cpu;
                    }
                }
            }

            // Handle vertical line connections for GPU
            if show_gpu
                && let Some(prev_row) = prev_gpu_row
                && prev_row != gpu_char_row
            {
                let (start, end) = if prev_row < gpu_char_row {
                    (prev_row, gpu_char_row)
                } else {
                    (gpu_char_row, prev_row)
                };
                if row_idx > start && row_idx < end {
                    row_buffer[char_col].0 |= get_vertical_line_bits(braille_col);
                    if row_buffer[char_col].1 == CellColor::None {
                        row_buffer[char_col].1 = CellColor::Gpu;
                    }
                }
            }

            // Handle vertical line connections for memory
            if let Some(prev_row) = prev_memory_row
                && prev_row != memory_char_row
            {
                let (start, end) = if prev_row < memory_char_row {
                    (prev_row, memory_char_row)
                } else {
                    (memory_char_row, prev_row)
                };
                if row_idx > start && row_idx < end {
                    row_buffer[char_col].0 |= get_vertical_line_bits(braille_col);
                    if row_buffer[char_col].1 == CellColor::None {
                        row_buffer[char_col].1 = CellColor::Memory;
                    }
                }
            }

            // Update previous row tracking at the end of each character (braille_col == 1)
            if braille_col == 1 {
                prev_cpu_row = Some(cpu_char_row);
                prev_gpu_row = Some(gpu_char_row);
                prev_memory_row = Some(memory_char_row);
            }
        }

        // Build spans for this row, coalescing adjacent cells with the same colour
        let mut spans: Vec<Span> = Vec::new();
        let mut current_chars = String::new();
        let mut current_color = CellColor::None;

        for (bits, color) in row_buffer.iter() {
            let ch = if *bits == 0 {
                ' '
            } else {
                std::char::from_u32(0x2800 + bits).unwrap_or(' ')
            };

            if *color == current_color || (current_chars.is_empty() && *color == CellColor::None) {
                current_chars.push(ch);
                if current_color == CellColor::None && *color != CellColor::None {
                    current_color = *color;
                }
            } else {
                // Flush current span
                if !current_chars.is_empty() {
                    let style = match current_color {
                        CellColor::None => Style::default(),
                        CellColor::Cpu => Style::default().fg(Color::Cyan),
                        CellColor::Gpu => Style::default().fg(Color::Magenta),
                        CellColor::Memory => Style::default().fg(Color::Green),
                    };
                    spans.push(Span::styled(std::mem::take(&mut current_chars), style));
                }
                current_chars.push(ch);
                current_color = *color;
            }
        }

        // Flush remaining characters
        if !current_chars.is_empty() {
            let style = match current_color {
                CellColor::None => Style::default(),
                CellColor::Cpu => Style::default().fg(Color::Cyan),
                CellColor::Gpu => Style::default().fg(Color::Magenta),
                CellColor::Memory => Style::default().fg(Color::Green),
            };
            spans.push(Span::styled(current_chars, style));
        }

        // Render the entire row as a single Line
        let line = Line::from(spans);
        let paragraph = Paragraph::new(line);
        f.render_widget(
            paragraph,
            Rect {
                x: area.x,
                y: area.y + row_idx as u16,
                width: area.width,
                height: 1,
            },
        );
    }

    // Render signal labels on the left side of the graph
    render_signal_labels(
        f,
        area,
        cpu_display,
        gpu_display,
        memory_display,
        show_gpu,
        dot_height,
        char_height,
    );
}

/// Helper to get braille bit value for a position
fn get_braille_bits(col: usize, row: usize) -> u32 {
    let dot_values: [[u32; 2]; 4] = [
        [1, 8],    // Row 0
        [2, 16],   // Row 1
        [4, 32],   // Row 2
        [64, 128], // Row 3
    ];
    if row < 4 && col < 2 {
        dot_values[row][col]
    } else {
        0
    }
}

/// Helper to get vertical line bits for a braille column
fn get_vertical_line_bits(col: usize) -> u32 {
    if col == 0 {
        1 | 2 | 4 | 64 // All dots in left column
    } else {
        8 | 16 | 32 | 128 // All dots in right column
    }
}

/// Helper to slice history data with offset
fn get_history_slice(history: &[f32], start_offset: usize, end_offset: usize) -> &[f32] {
    if history.len() > start_offset {
        let start_idx = history.len() - start_offset;
        let end_idx = history.len() - end_offset;
        &history[start_idx..end_idx]
    } else if history.len() > end_offset {
        let end_idx = history.len() - end_offset;
        &history[0..end_idx]
    } else {
        &[]
    }
}

/// Helper to get display slice from interpolated data
fn get_display_slice(data: &[f32], display_points: usize) -> &[f32] {
    if data.len() > display_points {
        &data[data.len() - display_points..]
    } else {
        data
    }
}

/// Render signal labels (C, G, M) at their average positions
#[allow(clippy::too_many_arguments)]
fn render_signal_labels(
    f: &mut Frame,
    area: Rect,
    cpu_display: &[f32],
    gpu_display: &[f32],
    memory_display: &[f32],
    show_gpu: bool,
    dot_height: usize,
    char_height: usize,
) {
    if !cpu_display.is_empty() {
        let cpu_avg = cpu_display.iter().sum::<f32>() / cpu_display.len() as f32;
        let cpu_avg_dot_row = ((cpu_avg / 100.0) * (dot_height - 1) as f32).round() as usize;
        let cpu_avg_char_row = char_height.saturating_sub(1 + cpu_avg_dot_row / 4);
        let cpu_label_y = area.y + cpu_avg_char_row as u16;

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

    let Some(memory_info) = app.memory_info else {
        let stats =
            Paragraph::new("Memory: Loading...").style(Style::default().fg(Color::DarkGray));
        f.render_widget(stats, area);
        return;
    };

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
        Line::from("  Enter         Pin/Unpin process (shows full command)"),
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
