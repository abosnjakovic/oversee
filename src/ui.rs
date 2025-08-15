use crate::{app::App, process::ProcessMonitor};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, Borders, Paragraph, Row, Table, Wrap,
    },
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();
    
    // Calculate number of CPU rows needed (4 cores per row)
    let cpu_count = app.get_cpu_count();
    let cores_per_row = 4;
    let cpu_rows = (cpu_count + cores_per_row - 1) / cores_per_row;
    
    // Vertical layout: CPU gauges at top, system info, then processes
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((cpu_rows * 2 + 2) as u16), // CPU gauges + border
            Constraint::Length(3),                          // System info
            Constraint::Min(10),                           // Process list
        ])
        .split(size);
    
    // Render CPU gauges
    render_cpu_gauges(f, app, main_chunks[0]);
    
    // Render system info
    render_system_info(f, app, main_chunks[1]);
    
    // Render process list
    render_process_list(f, app, main_chunks[2]);
}

fn render_cpu_gauges(f: &mut Frame, app: &App, area: Rect) {
    let cpu_usages = app.cpu_monitor.cpu_usages();
    
    // Create outer block
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " CPU Usage - Overall: {:.1}% ({} cores) Timeline: {} {}",
            app.get_current_cpu_usage(),
            app.get_cpu_count(),
            app.get_timeline_scope().name(),
            if app.is_paused() { "[PAUSED]" } else { "" }
        ))
        .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    
    let inner_area = block.inner(area);
    f.render_widget(block, area);
    
    // Calculate available width for timeline bars
    let available_width = inner_area.width.saturating_sub(20); // Reserve space for "CPU X [" and "] XX.X%"
    let bar_width = available_width.max(20) as usize; // Minimum 20 chars for timeline
    
    // Get timeline data for all cores
    let timeline_data = app.get_cpu_timeline_data(bar_width);
    
    // Define colors for CPU cores
    let colors = [
        Color::Cyan, Color::Yellow, Color::Green, Color::Magenta,
        Color::Red, Color::Blue, Color::LightCyan, Color::LightYellow,
        Color::LightGreen, Color::LightMagenta, Color::LightRed, Color::LightBlue,
        Color::White, Color::Gray,
    ];
    
    // Block characters for different intensities
    let block_chars = ['░', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
    
    // Create layout for each CPU core (one line each)
    let cpu_count = cpu_usages.len();
    let row_constraints: Vec<Constraint> = (0..cpu_count)
        .map(|_| Constraint::Length(1))
        .collect();
    
    let rows_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(inner_area);
    
    // Render each CPU core on its own line
    for (cpu_idx, row_area) in rows_layout.iter().enumerate() {
        if cpu_idx < cpu_usages.len() && cpu_idx < timeline_data.len() {
            let (cpu_name, current_usage) = &cpu_usages[cpu_idx];
            let timeline = &timeline_data[cpu_idx];
            
            // Build the timeline bar string
            let timeline_str: String = timeline
                .iter()
                .map(|&intensity| block_chars[intensity.min(8) as usize])
                .collect();
            
            // Create the full line: "CPU X [████████████████████████████████] XX.X%"
            let line_text = format!(
                "{:<6} [{}] {:>5.1}%",
                cpu_name,
                timeline_str,
                current_usage
            );
            
            let cpu_line = Paragraph::new(line_text)
                .style(Style::default().fg(colors[cpu_idx % colors.len()]))
                .wrap(Wrap { trim: false });
            
            f.render_widget(cpu_line, *row_area);
        }
    }
}

fn render_system_info(f: &mut Frame, app: &App, area: Rect) {
    let processes = app.process_monitor.get_processes();
    let total_processes = processes.len();
    
    // Calculate memory usage summary (simplified)
    let total_memory: u64 = processes.iter().map(|p| p.memory).sum();
    
    let info_text = format!(
        "Tasks: {} total | Sort: {} | Memory: {} | Timeline: [- {} +]{}",
        total_processes,
        app.process_monitor.current_sort_mode().name(),
        ProcessMonitor::format_memory(total_memory),
        app.get_timeline_scope().name(),
        if app.is_paused() { " [PAUSED]" } else { "" }
    );
    
    let info = Paragraph::new(info_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" System ")
        )
        .style(Style::default().fg(Color::White));
    
    f.render_widget(info, area);
}

fn render_process_list(f: &mut Frame, app: &App, area: Rect) {
    let processes = app.process_monitor.get_processes();
    
    // Split for table and help
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);
    
    // Header
    let header = Row::new(vec![
        "PID",
        "User",
        "CPU%",
        "MEM%", 
        "Command",
    ])
    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
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
            
            // Calculate memory percentage (simplified - assume 16GB total for now)
            let mem_percent = (proc.memory as f64 / (16.0 * 1024.0 * 1024.0 * 1024.0)) * 100.0;
            
            Row::new(vec![
                proc.pid.to_string(),
                "user".to_string(), // Simplified - sysinfo doesn't easily provide user
                format!("{:.1}", proc.cpu_usage),
                format!("{:.1}", mem_percent),
                truncate_string(&proc.name, 40),
            ])
            .style(style)
        })
        .collect();
    
    let table = Table::new(
        rows,
        [
            Constraint::Length(8),   // PID
            Constraint::Length(8),   // User
            Constraint::Length(6),   // CPU%
            Constraint::Length(6),   // MEM%
            Constraint::Min(30),     // Command (flexible)
        ]
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " Processes ({} total) ",
                processes.len()
            ))
            .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
    )
    .row_highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
    .highlight_symbol("► ");
    
    f.render_widget(table, chunks[0]);
    
    // Help text
    let help_text = if app.is_paused() {
        "[PAUSED] Space: Resume | q: Quit | j/k/↑↓: Navigate | s: Sort | +/-: Timeline | g/G: Top/Bottom"
    } else {
        "Space: Pause | q: Quit | j/k/↑↓: Navigate | s: Sort | +/-: Timeline | g/G: Top/Bottom"
    };
    
    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Controls ")
        )
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: true });
    
    f.render_widget(help, chunks[1]);
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}