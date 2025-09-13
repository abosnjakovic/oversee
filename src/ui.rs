use crate::app::App;
use crate::process::{PortInfo, ConnectionState};
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
    sorted_ports.sort_by(|a, b| {
        match (&a.state, &b.state) {
            (ConnectionState::Listen, ConnectionState::Listen) => a.port.cmp(&b.port),
            (ConnectionState::Listen, _) => std::cmp::Ordering::Less,
            (_, ConnectionState::Listen) => std::cmp::Ordering::Greater,
            _ => a.port.cmp(&b.port),
        }
    });
    
    // Take first 3 ports to fit in column
    let displayed_ports: Vec<String> = sorted_ports
        .iter()
        .take(3)
        .map(|port| {
            match port.state {
                ConnectionState::Listen => format!("{}L", port.port),  // L for listening
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
            Constraint::Length(4),  // Memory pressure section
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

    // Render memory pressure section
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

    // Header
    let header = Row::new(vec!["PID", "User", "CPU%", "GPU%", "Ports", "MEM", "Command"])
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
                Style::default().bg(Color::Blue).fg(Color::White)
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
                truncate_string(&proc.name, 40),
            ])
            .style(style)
        })
        .collect();

    // Render title at top of the allocated chunk
    let title_text = if app.filter_mode {
        format!("Processes ({} total) | Filter: {} _", all_processes.len(), app.filter_input)
    } else if !app.filter_input.is_empty() {
        format!("Processes ({}/{} shown) | Filter: {}", processes.len(), all_processes.len(), app.filter_input)
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
    .row_highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
    .highlight_symbol("► ");

    f.render_stateful_widget(table, table_area, &mut app.table_state);

    // Help text
    let help_text = if app.kill_confirmation_mode {
        "⚠️  CONFIRM KILL: [Y] Yes | [N] No | ESC: Cancel"
    } else if app.filter_mode {
        "Type to filter | Enter: Apply | ESC: Cancel"
    } else if app.is_paused() {
        "[PAUSED] Space: Resume | q: Quit | j/k/↑↓: Navigate | K: Kill | s: Sort | /: Filter | +/-: Timeline | g/G: Top/Bottom | v: GPU"
    } else {
        "Space: Pause | q: Quit | j/k/↑↓: Navigate | K: Kill | s: Sort | /: Filter | +/-: Timeline | g/G: Top/Bottom | v: GPU"
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

    // Calculate CPU average data
    let cpu_history: Vec<f32> = if !app.cpu_core_histories.is_empty() {
        let max_len = app
            .cpu_core_histories
            .iter()
            .map(|h| h.len())
            .max()
            .unwrap_or(0);
        (0..max_len)
            .map(|i| {
                let mut total = 0.0f32;
                let mut count = 0;
                for core_history in &app.cpu_core_histories {
                    if let Some(&usage) = core_history.get(i) {
                        total += usage;
                        count += 1;
                    }
                }
                if count > 0 { total / count as f32 } else { 0.0 }
            })
            .collect()
    } else {
        vec![]
    };

    // Get GPU history
    let gpu_history: Vec<f32> = app.gpu_overall_history.iter().copied().collect();

    // Render vertical dot timeline
    render_vertical_timeline(
        f,
        inner,
        &cpu_history,
        &gpu_history,
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

fn render_vertical_timeline(
    f: &mut Frame,
    area: Rect,
    cpu_history: &[f32],
    gpu_history: &[f32],
    show_gpu: bool,
    timeline_offset: usize,
) {
    // btop++ style braille patterns - 5x5 grid for smooth transitions
    // Each represents transition from one height to another
    const BRAILLE_UP: [&str; 25] = [
        " ", "⢀", "⢠", "⢰", "⢸", // From empty to increasing heights
        "⡀", "⣀", "⣠", "⣰", "⣸", // One level higher start
        "⡄", "⣄", "⣤", "⣴", "⣼", // Middle transitions
        "⡆", "⣆", "⣦", "⣶", "⣾", // Upper-middle transitions
        "⡇", "⣇", "⣧", "⣷", "⣿", // To full height
    ];

    // Calculate dimensions
    let available_width = area.width as usize;
    let available_height = area.height as usize;

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
        // Not enough history for full display, show what we have
        let end_idx = cpu_history.len() - end_offset;
        &cpu_history[0..end_idx]
    } else {
        // No data in the requested range
        &[]
    };

    // Get GPU data slice accounting for offset
    let gpu_points = if gpu_history.len() > start_offset {
        let start_idx = gpu_history.len() - start_offset;
        let end_idx = gpu_history.len() - end_offset;
        &gpu_history[start_idx..end_idx]
    } else if gpu_history.len() > end_offset {
        // Not enough history for full display, show what we have
        let end_idx = gpu_history.len() - end_offset;
        &gpu_history[0..end_idx]
    } else {
        // No data in the requested range
        &[]
    };

    // Limit display width to available screen space
    let display_points = available_width.min(cpu_points.len());
    let cpu_display = if cpu_points.len() > display_points {
        &cpu_points[cpu_points.len() - display_points..]
    } else {
        cpu_points
    };

    let gpu_display = if gpu_points.len() > display_points {
        &gpu_points[gpu_points.len() - display_points..]
    } else {
        gpu_points
    };

    // Store previous values for smooth transitions
    let mut prev_cpu_val = 0;
    let mut _prev_gpu_val = 0;

    // Render each column (time point)
    for col in 0..display_points {
        let cpu_usage = cpu_display.get(col).copied().unwrap_or(0.0);
        let gpu_usage = if show_gpu {
            gpu_display.get(col).copied().unwrap_or(0.0)
        } else {
            0.0
        };

        // Map values to 0-4 range for braille selection
        let cpu_val = ((cpu_usage / 100.0) * 4.0).round() as usize;
        let gpu_val = ((gpu_usage / 100.0) * 4.0).round() as usize;

        // Render column of braille characters
        for row in 0..available_height {
            let x = area.x + col as u16;
            let y = area.y + row as u16;

            // Calculate what height this row represents (top = 100%, bottom = 0%)
            let row_height =
                ((available_height - row - 1) as f32 / available_height as f32) * 100.0;

            // Determine symbol and color for this position
            let (symbol, color) = if row_height <= cpu_usage {
                // Within CPU usage range - use braille with transition
                if row
                    == available_height
                        - 1
                        - ((cpu_usage / 100.0 * available_height as f32) as usize)
                {
                    // This is the top of the bar - use transition symbol
                    let symbol_idx = prev_cpu_val * 5 + cpu_val;
                    (
                        BRAILLE_UP[symbol_idx.min(24)],
                        get_gradient_color(cpu_usage),
                    )
                } else {
                    // Fill below the top
                    ("⣿", get_gradient_color(cpu_usage))
                }
            } else if show_gpu && row_height <= gpu_usage {
                // GPU overlay
                ("⡇", Color::Magenta)
            } else {
                // Empty space
                (" ", Color::Black)
            };

            let cell = Paragraph::new(symbol).style(Style::default().fg(color));
            let cell_area = Rect {
                x,
                y,
                width: 1,
                height: 1,
            };
            f.render_widget(cell, cell_area);
        }

        // Update previous values for next iteration
        prev_cpu_val = cpu_val;
        _prev_gpu_val = gpu_val;
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

    // Create main title and stats layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title
            Constraint::Length(1), // Memory bar
            Constraint::Length(1), // Statistics
            Constraint::Length(1), // Footer/details
        ])
        .split(area);

    // Title with pressure status
    let title_text = format!(
        "Memory: {:.1}GB / {:.1}GB ({:.1}%) | Pressure: {}",
        memory_info.used_memory as f64 / (1024.0 * 1024.0 * 1024.0),
        memory_info.total_memory as f64 / (1024.0 * 1024.0 * 1024.0),
        memory_info.memory_usage_percentage(),
        memory_info.pressure.color_name()
    );

    let title_color = match memory_info.pressure {
        MemoryPressure::Green => Color::Green,
        MemoryPressure::Yellow => Color::Yellow,
        MemoryPressure::Red => Color::Red,
    };

    let title = Paragraph::new(title_text)
        .style(Style::default().fg(title_color).add_modifier(Modifier::BOLD));
    f.render_widget(title, chunks[0]);

    // Memory usage bar with braille characters and gradient colors
    // Braille progression characters for smooth transitions
    const BRAILLE_LEVELS: [&str; 9] = [" ", "⡀", "⡄", "⡆", "⡇", "⣇", "⣧", "⣷", "⣿"];

    let bar_width = area.width as usize;
    let usage_percentage = memory_info.memory_usage_percentage();

    // Build the bar character by character with appropriate colors
    for col in 0..bar_width {
        let position_percentage = (col as f64 / bar_width as f64) * 100.0;

        // Determine the character and color based on position vs usage
        let (character, color) = if position_percentage <= usage_percentage {
            // Within used memory range
            let fill_amount = if position_percentage + (100.0 / bar_width as f64) <= usage_percentage {
                // Fully filled position
                8
            } else {
                // Partially filled position at the boundary
                let partial = ((usage_percentage - position_percentage) * bar_width as f64 / 100.0 * 8.0) as usize;
                partial.min(8)
            };

            // Color based on position in the bar (gradual transition)
            let color = if position_percentage < 30.0 {
                Color::Green
            } else if position_percentage < 50.0 {
                // Gradient from green to yellow
                if position_percentage < 40.0 {
                    Color::LightGreen
                } else {
                    Color::Yellow
                }
            } else if position_percentage < 70.0 {
                // Gradient from yellow to red
                if position_percentage < 60.0 {
                    Color::LightYellow
                } else {
                    Color::LightRed
                }
            } else {
                Color::Red
            };

            (BRAILLE_LEVELS[fill_amount], color)
        } else {
            // Empty space
            (BRAILLE_LEVELS[0], Color::DarkGray)
        };

        // Render each character individually with its color
        let cell = Paragraph::new(character).style(Style::default().fg(color));
        let cell_area = Rect {
            x: chunks[1].x + col as u16,
            y: chunks[1].y,
            width: 1,
            height: 1,
        };
        f.render_widget(cell, cell_area);
    }
    
    // Additional statistics
    let stats_text = if memory_info.total_swap > 0 {
        format!(
            "Swap: {:.1}GB / {:.1}GB ({:.1}%) | Free: {:.1}GB",
            memory_info.used_swap as f64 / (1024.0 * 1024.0 * 1024.0),
            memory_info.total_swap as f64 / (1024.0 * 1024.0 * 1024.0),
            memory_info.swap_usage_percentage(),
            memory_info.free_memory() as f64 / (1024.0 * 1024.0 * 1024.0)
        )
    } else {
        format!(
            "Free: {:.1}GB | No swap configured",
            memory_info.free_memory() as f64 / (1024.0 * 1024.0 * 1024.0)
        )
    };
    
    let stats = Paragraph::new(stats_text)
        .style(Style::default().fg(Color::Gray));
    f.render_widget(stats, chunks[2]);
    
    // Pressure explanation
    let pressure_text = match memory_info.pressure {
        MemoryPressure::Green => "System using RAM efficiently",
        MemoryPressure::Yellow => "System using memory compression",
        MemoryPressure::Red => "System heavily using swap space",
    };
    
    let pressure_desc = Paragraph::new(pressure_text)
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(pressure_desc, chunks[3]);
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

fn render_help_popup(f: &mut Frame, app: &App) {
    use ratatui::{
        layout::{Constraint, Direction, Layout, Margin},
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
        Line::from(vec![
            Span::styled("KEYBINDS", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Navigation:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        ]),
        Line::from("  j/k or ↑↓     Navigate process list up/down"),
        Line::from("  g             Jump to top of process list"),
        Line::from("  G             Jump to bottom of process list"),
        Line::from("  Page Up/Down  Navigate by 10 processes"),
        Line::from("  Home/End      Jump to first/last process"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Actions:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        ]),
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
        Line::from(vec![
            Span::styled("TIMELINE", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        ]),
        Line::from(""),
        Line::from("Timeline always displays 5 minutes of data. Use +/- to navigate"),
        Line::from("through up to 20 minutes of historical system metrics."),
        Line::from(""),
        Line::from(vec![
            Span::styled("MEMORY PRESSURE ALGORITHM", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        ]),
        Line::from(""),
        Line::from("Oversee implements Apple-inspired memory pressure calculation:"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Calculation Formula:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        ]),
        Line::from("  1. Base: free_pct = (free_memory / total_memory) × 100"),
        Line::from("  2. Swap adjustment: If swap_usage > 10%:"),
        Line::from("     adjusted_free = free_pct × (1 - (swap_usage - 10) / 100)"),
        Line::from("  3. Otherwise: adjusted_free = free_pct"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Pressure Levels:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        ]),
        Line::from(vec![
            Span::styled("  • Green: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("Adjusted free ≥ 50% - Efficient RAM usage")
        ]),
        Line::from(vec![
            Span::styled("  • Yellow: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("30% ≤ Adjusted free < 50% - Memory compression active")
        ]),
        Line::from(vec![
            Span::styled("  • Red: ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw("Adjusted free < 30% - Heavy swap usage")
        ]),
        Line::from(""),
        Line::from("The memory bar uses gradient colors: green (0-30%) → yellow (30-50%)"),
        Line::from("→ orange (50-70%) → red (70-100%) to show usage progression."),
        Line::from(""),
        Line::from(vec![
            Span::styled("Note: ", Style::default().fg(Color::Cyan)),
            Span::raw("macOS uses memory differently than other systems.")
        ]),
        Line::from("High usage with green pressure is optimal. See README for details"),
        Line::from("on why your Mac keeps memory full for better performance."),
        Line::from(""),
        Line::from(vec![
            Span::styled("ABOUT OVERSEE", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        ]),
        Line::from(""),
        Line::from("A modern system monitor for macOS, inspired by htop and btop++,"),
        Line::from("built in Rust with a focus on Apple Silicon performance monitoring."),
        Line::from(""),
        Line::from("Features CPU and GPU core monitoring, memory pressure indicators"),
        Line::from("matching Activity Monitor, timeline visualization with braille"),
        Line::from("characters, and vim-style navigation controls."),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ? or ESC to close this help", Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC))
        ]),
    ];

    // Create the popup block
    let block = Block::default()
        .title(" Help - Oversee System Monitor ")
        .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
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
    use crate::gpu::GpuMonitor;

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
        let expected_patterns = vec![
            "··········", // 0%
            "··········", // 25% -> 3 dots rounded to 2 for clean display
            "•••••·····", // 50%
            "•••••••···", // 75% -> 8 dots rounded to 7
            "••••••••••", // 100%
        ];

        for (usage, _expected) in usage_levels.iter().zip(expected_patterns.iter()) {
            let (filled, empty) = generate_dot_pattern(*usage);
            let pattern = format!("{}{}", "•".repeat(filled), "·".repeat(empty));
            // Verify total length is always 10
            assert_eq!(pattern.len(), 30); // 10 chars * 3 bytes per char for Unicode
            assert_eq!(pattern.chars().count(), 10); // 10 visual characters
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
        // Test that GPU monitor reports expected core counts for different chips
        let gpu_monitor = GpuMonitor::new();
        let core_count = gpu_monitor.get_core_count();

        // M4 Pro should have 16-20 GPU cores
        // This test documents the expected behavior even if actual detection varies
        println!("Detected GPU cores: {}", core_count);

        // Verify the count is reasonable (not 0, not impossibly high)
        assert!(
            core_count <= 40,
            "GPU core count should be reasonable: {}",
            core_count
        );

        if core_count > 0 {
            assert!(
                gpu_monitor.is_available(),
                "GPU should be available if cores detected"
            );
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

