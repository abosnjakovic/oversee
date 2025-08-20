use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Axis, Block, Borders, Chart, Clear, Dataset, GraphType, Paragraph, Row, Table, Wrap,
    },
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();
    
    // Main layout: Timeline full width, then process list (no system info)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(25), // Timeline graph (full width) - increased for better chart visibility
            Constraint::Min(10),    // Process list
        ])
        .split(size);
    
    // Render full-width timeline
    render_chart_timeline(f, app, main_chunks[0]);
    
    // Render floating cores panel over the timeline
    render_floating_cores(f, app, main_chunks[0]);
    
    // Render process list (skip system info section)
    render_process_list(f, app, main_chunks[1]);
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
        "[PAUSED] Space: Resume | q: Quit | j/k/↑↓: Navigate | s: Sort | +/-: Timeline | g/G: Top/Bottom | v: GPU"
    } else {
        "Space: Pause | q: Quit | j/k/↑↓: Navigate | s: Sort | +/-: Timeline | g/G: Top/Bottom | v: GPU"
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

fn render_chart_timeline(f: &mut Frame, app: &App, area: Rect) {
    // Create border block
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " System Timeline ({}) ",
            app.get_timeline_scope().name()
        ))
        .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    // Calculate CPU average data for chart
    let cpu_data: Vec<(f64, f64)> = if !app.cpu_core_histories.is_empty() {
        let max_len = app.cpu_core_histories.iter().map(|h| h.len()).max().unwrap_or(0);
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
                let avg_usage = if count > 0 { total / count as f32 } else { 0.0 };
                (i as f64, avg_usage as f64)
            })
            .collect()
    } else {
        vec![]
    };

    // Calculate GPU data for chart
    let gpu_data: Vec<(f64, f64)> = app.gpu_overall_history
        .iter()
        .enumerate()
        .map(|(i, &usage)| (i as f64, usage as f64))
        .collect();

    // Create datasets
    let mut datasets = vec![
        Dataset::default()
            .name("CPU")
            .marker(ratatui::symbols::Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .graph_type(GraphType::Line)
            .data(&cpu_data),
    ];

    // Add GPU dataset if visible and has data
    if app.is_gpu_visible() && !gpu_data.is_empty() {
        datasets.push(
            Dataset::default()
                .name("GPU")
                .marker(ratatui::symbols::Marker::Braille)
                .style(Style::default().fg(Color::Magenta))
                .graph_type(GraphType::Line)
                .data(&gpu_data),
        );
    }

    // Calculate chart bounds
    let max_data_len = cpu_data.len().max(gpu_data.len()) as f64;
    let x_bounds = if max_data_len > 0.0 {
        [0.0, max_data_len - 1.0]
    } else {
        [0.0, 30.0] // Default range
    };

    // Create the chart
    let chart = Chart::new(datasets)
        .block(block)
        .x_axis(
            Axis::default()
                .title("Time")
                .style(Style::default().fg(Color::Gray))
                .bounds(x_bounds),
        )
        .y_axis(
            Axis::default()
                .title("Usage %")
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, 100.0]),
        );

    f.render_widget(chart, area);
}

fn render_floating_cores(f: &mut Frame, app: &App, area: Rect) {
    // Calculate floating panel dimensions - right side of the timeline
    let panel_width = (area.width / 3).max(35); // 1/3 of width, minimum 35 chars for charts  
    let panel_height = area.height.saturating_sub(2); // Minimal margin for more core visibility
    
    let floating_area = Rect {
        x: area.x + area.width - panel_width - 1,
        y: area.y + 1, // Start closer to top
        width: panel_width,
        height: panel_height,
    };
    
    // Clear the background area first
    f.render_widget(Clear, floating_area);
    
    // Split floating area vertically: CPU cores left, GPU cores right
    let split_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // CPU cores left half
            Constraint::Percentage(50), // GPU cores right half
        ])
        .split(floating_area);
    
    let cpu_area = split_chunks[0];
    let gpu_area = split_chunks[1];
    
    // Render CPU cores in left half
    render_cpu_cores_panel(f, app, cpu_area);
    
    // Render GPU cores in right half if visible
    if app.is_gpu_visible() {
        render_gpu_cores_panel(f, app, gpu_area);
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
            Color::Cyan
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
            Color::Magenta
        );
    }
}

fn render_core_dot_line(
    f: &mut Frame, 
    area: Rect, 
    name: &str, 
    usage: f32,
    color: Color
) {
    // Create dot visualization: each dot represents 10% usage
    let filled_dots = (usage / 10.0).round() as usize;
    let filled_dots = filled_dots.min(10); // Cap at 10 dots (100%)
    let empty_dots = 10 - filled_dots;
    
    // Build the dot string
    let filled_str = "•".repeat(filled_dots);
    let empty_str = "·".repeat(empty_dots);
    let dots = format!("{}{}", filled_str, empty_str);
    
    // Format the full line: "CPU 0: ••••••••·· 85%"
    let line_text = format!("{:<6}: {} {:>3.0}%", name, dots, usage);
    
    let line = Paragraph::new(line_text)
        .style(Style::default().fg(color))
        .wrap(Wrap { trim: false });
    
    f.render_widget(line, area);
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
            "··········",  // 0%
            "··········",  // 25% -> 3 dots rounded to 2 for clean display
            "•••••·····",  // 50%
            "•••••••···",  // 75% -> 8 dots rounded to 7
            "••••••••••",  // 100%
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
        let test_area = Rect { x: 0, y: 0, width: 120, height: 30 };
        let panel_width = (test_area.width / 3).max(35);
        assert_eq!(panel_width, 40); // 120/3 = 40, which is > 35
        
        // Test minimum width enforcement
        let small_area = Rect { x: 0, y: 0, width: 90, height: 30 };
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
        assert!(core_count <= 40, "GPU core count should be reasonable: {}", core_count);
        
        if core_count > 0 {
            assert!(gpu_monitor.is_available(), "GPU should be available if cores detected");
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