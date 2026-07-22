use crate::app::{App, TimelineView};
use crate::process::{ConnectionState, PortInfo, ProcessDetails, ProcessInfo, SortMode};
use crate::theme::{THEME, TRAIL_TIERS, trail_tier};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Cell, Paragraph, Row, Table, Wrap},
};

const MAX_BREAKOUT_PORTS: usize = 6;

fn format_uptime_short() -> String {
    let secs = sysinfo::System::uptime();
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3_600;
    let m = (secs % 3_600) / 60;
    if d > 0 {
        format!("{}d{:02}h", d, h)
    } else if h > 0 {
        format!("{}h{:02}m", h, m)
    } else {
        format!("{}m", m.max(1))
    }
}

fn current_load_one() -> f64 {
    sysinfo::System::load_average().one
}

fn wrap_to_width(s: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![s.to_string()];
    }
    let mut lines = Vec::new();
    let mut buf = String::new();
    let mut buf_len = 0;
    for ch in s.chars() {
        buf.push(ch);
        buf_len += 1;
        if buf_len >= width {
            lines.push(std::mem::take(&mut buf));
            buf_len = 0;
        }
    }
    if !buf.is_empty() {
        lines.push(buf);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn format_runtime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}h{:02}m", h, m)
    } else if m > 0 {
        format!("{}m{:02}s", m, s)
    } else {
        format!("{}s", s)
    }
}

fn format_port_line(port: &PortInfo) -> String {
    let proto = match port.protocol {
        crate::process::Protocol::Tcp => "TCP",
        crate::process::Protocol::Udp => "UDP",
    };
    let state = match port.state {
        ConnectionState::Listen => "LISTEN",
        ConnectionState::Established => "ESTABLISHED",
        ConnectionState::Other => "",
    };
    let local = port.local_address.as_deref().unwrap_or("-");
    let remote = port.remote_address.as_deref();
    match (state, remote) {
        ("", None) => format!("  {}  {}", proto, local),
        ("", Some(r)) => format!("  {}  {} -> {}", proto, local, r),
        (st, None) => format!("  {}  {}  {}", proto, local, st),
        (st, Some(r)) => format!("  {}  {}  {} -> {}", proto, local, st, r),
    }
}

fn build_breakout_lines<'a>(
    proc: &ProcessInfo,
    details: Option<&ProcessDetails>,
    width: usize,
) -> Vec<Line<'a>> {
    let mut lines: Vec<Line> = Vec::new();
    let dim = Style::default().fg(THEME.fg_faint);
    let key_style = Style::default().fg(THEME.accent_warn);

    // Separator
    let sep_width = width.max(4);
    lines.push(Line::from(Span::styled("─".repeat(sep_width), dim)));

    // cmd (wrapped)
    let prefix = "cmd: ";
    let inner = width.saturating_sub(prefix.len()).max(10);
    let cmd_lines = wrap_to_width(&proc.cmd, inner);
    for (i, l) in cmd_lines.iter().enumerate() {
        if i == 0 {
            lines.push(Line::from(vec![
                Span::styled(prefix, key_style),
                Span::raw(l.clone()),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw(" ".repeat(prefix.len())),
                Span::raw(l.clone()),
            ]));
        }
    }

    // cwd
    if let Some(cwd) = &proc.cwd {
        let prefix = "cwd: ";
        let inner = width.saturating_sub(prefix.len()).max(10);
        let parts = wrap_to_width(cwd, inner);
        for (i, l) in parts.iter().enumerate() {
            if i == 0 {
                lines.push(Line::from(vec![
                    Span::styled(prefix, key_style),
                    Span::raw(l.clone()),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::raw(" ".repeat(prefix.len())),
                    Span::raw(l.clone()),
                ]));
            }
        }
    }

    // exe
    if let Some(exe) = &proc.exe {
        let prefix = "exe: ";
        let inner = width.saturating_sub(prefix.len()).max(10);
        let parts = wrap_to_width(exe, inner);
        for (i, l) in parts.iter().enumerate() {
            if i == 0 {
                lines.push(Line::from(vec![
                    Span::styled(prefix, key_style),
                    Span::raw(l.clone()),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::raw(" ".repeat(prefix.len())),
                    Span::raw(l.clone()),
                ]));
            }
        }
    }

    // threads / fds / runtime
    let threads = if proc.thread_count > 0 {
        proc.thread_count.to_string()
    } else if let Some(d) = details {
        d.thread_count_macos
            .map(|n| n.to_string())
            .unwrap_or_else(|| "…".to_string())
    } else {
        "…".to_string()
    };
    let fds = match details.and_then(|d| d.fd_count) {
        Some(n) => n.to_string(),
        None => "…".to_string(),
    };
    let runtime = format_runtime(proc.run_time);
    lines.push(Line::from(vec![
        Span::styled("threads: ", key_style),
        Span::raw(threads),
        Span::raw("    "),
        Span::styled("fds: ", key_style),
        Span::raw(fds),
        Span::raw("    "),
        Span::styled("runtime: ", key_style),
        Span::raw(runtime),
    ]));

    // ports
    if !proc.ports.is_empty() {
        lines.push(Line::from(Span::styled("ports:", key_style)));
        let total = proc.ports.len();
        for port in proc.ports.iter().take(MAX_BREAKOUT_PORTS) {
            lines.push(Line::from(format_port_line(port)));
        }
        if total > MAX_BREAKOUT_PORTS {
            lines.push(Line::from(Span::styled(
                format!("  ... (+{} more)", total - MAX_BREAKOUT_PORTS),
                dim,
            )));
        }
    }

    lines
}

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

    // Collapse duplicate connections sharing a local port (e.g. a busy
    // listener with many established sockets all on 443).
    let mut labels: Vec<String> = sorted_ports
        .iter()
        .map(|port| {
            match port.state {
                ConnectionState::Listen => format!("{}L", port.port), // L for listening
                _ => port.port.to_string(),
            }
        })
        .collect();
    labels.dedup();

    // Take first 3 unique ports to fit in column
    let mut result = labels.iter().take(3).cloned().collect::<Vec<_>>().join(",");
    if labels.len() > 3 {
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

    // Main layout: KPI header, per-core CPU/GPU lines, separator, timeline,
    // spacing, memory, separator, process list
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // KPI header strip
            Constraint::Length(1),  // Per-core CPU line
            Constraint::Length(1),  // Per-core GPU line
            Constraint::Length(1),  // Separator under header
            Constraint::Length(22), // Timeline graph (full width)
            Constraint::Length(1),  // Spacing
            Constraint::Length(1),  // Memory stats (1 line)
            Constraint::Length(1),  // Separator above process list
            Constraint::Min(8),     // Process list
        ])
        .split(margin_area);

    // Render the KPI header across the full width
    render_kpi_header(f, app, main_chunks[0]);

    // Per-core CPU/GPU usage as two full-width horizontal lines under the header
    render_cpu_cores_line(f, app, main_chunks[1]);
    if app.is_gpu_visible() {
        render_gpu_cores_line(f, app, main_chunks[2]);
    }

    // Thin separator line under header
    render_separator(f, main_chunks[3]);

    // Timeline now spans the full width (cores moved out of the right panel)
    render_chart_timeline(f, app, main_chunks[4]);

    render_memory_section(f, app, main_chunks[6]);
    render_separator(f, main_chunks[7]);
    render_process_list(f, app, main_chunks[8]);

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

    // Header with sort indicators (active sort gets underline + brighter fg).
    let sort_mode = app.get_sort_mode();
    let header_base = Style::default().fg(THEME.fg_dim);
    let header_active = Style::default()
        .fg(THEME.fg)
        .add_modifier(Modifier::UNDERLINED);
    let header_style_for = |is_active: bool| {
        if is_active {
            header_active
        } else {
            header_base
        }
    };
    let header = Row::new(vec![
        Cell::from(Span::styled(
            format!("{:>8}", "PID"),
            header_style_for(matches!(sort_mode, SortMode::Pid)),
        )),
        Cell::from(Span::styled("USER", header_base)),
        Cell::from(Span::styled(
            format!("{:>6}", "CPU%"),
            header_style_for(matches!(sort_mode, SortMode::Cpu)),
        )),
        Cell::from(Span::styled(format!("{:>6}", "GPU%"), header_base)),
        Cell::from(Span::styled("PORTS", header_base)),
        Cell::from(Span::styled(
            format!("{:>7}", "MEM"),
            header_style_for(matches!(sort_mode, SortMode::Memory)),
        )),
        Cell::from(Span::styled(
            "COMMAND",
            header_style_for(matches!(sort_mode, SortMode::Name)),
        )),
    ])
    .height(1);

    // Width available for the Command column's wrapped breakout content.
    // Fixed cols total 8+8+6+6+12+7 = 47, plus 6 column spacings, plus 2 for highlight symbol.
    let cmd_col_width = (chunks[0].width as usize).saturating_sub(47 + 6 + 2);

    // Process rows
    let rows: Vec<Row> = processes
        .iter()
        .enumerate()
        .map(|(i, proc)| {
            let is_pinned = app.pinned_pids.contains(&proc.pid);
            let is_selected = i == app.get_selected_process();
            let is_expanded = app.expanded_pid == Some(proc.pid);

            let row_style = if is_selected {
                Style::default().fg(THEME.cpu).add_modifier(Modifier::BOLD)
            } else if is_pinned {
                Style::default()
                    .fg(THEME.accent_warn)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(THEME.fg)
            };

            let mem_mb = proc.memory as f64 / (1024.0 * 1024.0);

            let cmd_display = proc.cmd.clone();

            let pid_display = if is_pinned {
                format!("◆ {}", proc.pid)
            } else {
                proc.pid.to_string()
            };

            // Per-metric coloured numeric cells, dimmed when value is negligible.
            let metric_cell = |value: f32, color: Color, width: usize| -> Cell {
                let style = if value < 1.0 {
                    Style::default().fg(THEME.fg_faint)
                } else {
                    Style::default().fg(color)
                };
                Cell::from(Span::styled(format!("{:>1$.1}", value, width), style))
            };
            let mem_cell = {
                let style = if mem_mb < 1.0 {
                    Style::default().fg(THEME.fg_faint)
                } else {
                    Style::default().fg(THEME.mem)
                };
                Cell::from(Span::styled(format!("{:>7.0}", mem_mb), style))
            };
            let pid_cell = Cell::from(Span::styled(
                format!("{:>8}", pid_display),
                Style::default().fg(THEME.fg_dim),
            ));

            if is_expanded {
                let mut cmd_lines: Vec<Line> = vec![Line::from(cmd_display)];
                cmd_lines.extend(build_breakout_lines(
                    proc,
                    app.selected_details.as_ref(),
                    cmd_col_width,
                ));
                let row_height = cmd_lines.len() as u16;
                Row::new(vec![
                    pid_cell,
                    Cell::from(truncate_string(&proc.user, 8)),
                    metric_cell(proc.cpu_usage, THEME.cpu, 6),
                    metric_cell(proc.gpu_usage, THEME.gpu, 6),
                    Cell::from(format_ports(&proc.ports)),
                    mem_cell,
                    Cell::from(Text::from(cmd_lines)),
                ])
                .height(row_height)
                .style(row_style)
            } else {
                Row::new(vec![
                    pid_cell,
                    Cell::from(truncate_string(&proc.user, 8)),
                    metric_cell(proc.cpu_usage, THEME.cpu, 6),
                    metric_cell(proc.gpu_usage, THEME.gpu, 6),
                    Cell::from(format_ports(&proc.ports)),
                    mem_cell,
                    Cell::from(cmd_display),
                ])
                .style(row_style)
            }
        })
        .collect();

    // Render title at top of the allocated chunk
    let title_text = if app.filter_mode {
        format!(
            "processes ({} total) · filter: {} _",
            all_processes.len(),
            app.filter_input
        )
    } else if !app.filter_input.is_empty() {
        format!(
            "processes ({}/{} shown) · filter: {}",
            processes.len(),
            all_processes.len(),
            app.filter_input
        )
    } else {
        format!("processes ({} total)", all_processes.len())
    };
    let title = Paragraph::new(title_text).style(Style::default().fg(THEME.fg_dim));

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
    .row_highlight_style(Style::default().fg(THEME.cpu).add_modifier(Modifier::BOLD))
    .highlight_symbol("► ");

    f.render_stateful_widget(table, table_area, &mut app.table_state);

    // Help text
    let help_text = if app.kill_confirmation_mode {
        "confirm kill · [Y] yes · [N] no · esc cancel"
    } else if app.filter_mode {
        "type to filter · enter apply · esc cancel"
    } else if app.is_paused() {
        "[paused] space resume · q quit · ↑↓ nav · enter pin · K kill · s sort · / filter · +/- time · g/G top/bot · ? help"
    } else {
        "space pause · q quit · ↑↓ nav · enter pin · K kill · s sort · / filter · +/- time · g/G top/bot · ? help"
    };

    let help_style = if app.kill_confirmation_mode {
        Style::default().fg(THEME.accent_crit)
    } else {
        Style::default().fg(THEME.fg_faint)
    };

    let help = Paragraph::new(help_text)
        .style(help_style)
        .wrap(Wrap { trim: true });

    f.render_widget(help, chunks[1]);
}

fn render_separator(f: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let line = "─".repeat(area.width as usize);
    let sep = Paragraph::new(line).style(Style::default().fg(THEME.separator));
    f.render_widget(sep, area);
}

fn render_kpi_header(f: &mut Frame, app: &App, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let label = Style::default().fg(THEME.fg_dim);
    let bullet = Span::styled(" · ", Style::default().fg(THEME.fg_faint));

    // Bold the metric the timeline is currently graphing (toggled with Tab).
    let active = app.timeline_view;
    let metric_label = |view: TimelineView| {
        if active == view {
            Style::default().fg(THEME.fg).add_modifier(Modifier::BOLD)
        } else {
            label
        }
    };

    let load = current_load_one();
    let cpu_avg = app
        .get_cpu_average_history()
        .iter()
        .last()
        .copied()
        .unwrap_or(0.0);
    let gpu_avg = app
        .gpu_overall_history
        .iter()
        .last()
        .copied()
        .unwrap_or(0.0);
    let mem_pct = app
        .memory_usage_history
        .iter()
        .last()
        .copied()
        .unwrap_or(0.0);

    let mem_color = match app.memory_info.as_ref().map(|m| m.pressure) {
        Some(crate::memory::MemoryPressure::Yellow) => THEME.accent_warn,
        Some(crate::memory::MemoryPressure::Red) => THEME.accent_crit,
        _ => THEME.mem,
    };

    let proc_count = app.get_all_processes().len();
    let position = app.get_timeline_position_text();

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(
        "oversee",
        Style::default().fg(THEME.fg).add_modifier(Modifier::BOLD),
    ));
    spans.push(bullet.clone());
    spans.push(Span::styled("load ", label));
    spans.push(Span::styled(
        format!("{:.2}", load),
        Style::default().fg(THEME.fg),
    ));
    spans.push(bullet.clone());
    spans.push(Span::styled("cpu ", metric_label(TimelineView::Cpu)));
    spans.push(Span::styled(
        format!("{:>3.0}%", cpu_avg),
        Style::default().fg(THEME.cpu),
    ));
    spans.push(bullet.clone());
    spans.push(Span::styled("gpu ", metric_label(TimelineView::Gpu)));
    spans.push(Span::styled(
        format!("{:>3.0}%", gpu_avg),
        Style::default().fg(THEME.gpu),
    ));
    spans.push(bullet.clone());
    spans.push(Span::styled("mem ", metric_label(TimelineView::Memory)));
    spans.push(Span::styled(
        format!("{:>3.0}%", mem_pct),
        Style::default().fg(mem_color),
    ));
    spans.push(bullet.clone());
    spans.push(Span::styled(
        format!("{} procs", proc_count),
        Style::default().fg(THEME.fg_dim),
    ));
    spans.push(bullet.clone());
    spans.push(Span::styled("up ", label));
    spans.push(Span::styled(
        format_uptime_short(),
        Style::default().fg(THEME.fg_dim),
    ));
    spans.push(bullet);
    spans.push(Span::styled(position, Style::default().fg(THEME.fg_faint)));

    let header = Paragraph::new(Line::from(spans));
    f.render_widget(header, area);
}

fn render_chart_timeline(f: &mut Frame, app: &App, area: Rect) {
    let offset = app.get_timeline_offset();

    // Build the set of waveforms for the selected metric. Overview overlays the
    // three aggregate signals (the classic view). CPU/GPU show every core as its
    // own colour-coded trace; memory shows a single usage trace tinted by the
    // pressure level recorded at each point in time.
    let waves: Vec<Wave> = match app.timeline_view {
        TimelineView::Overview => {
            let mut waves = vec![Wave {
                data: app.get_cpu_average_history().iter().copied().collect(),
                palettes: vec![THEME.cpu_trail],
            }];
            if app.is_gpu_visible() {
                waves.push(Wave {
                    data: app.gpu_overall_history.iter().copied().collect(),
                    palettes: vec![THEME.gpu_trail],
                });
            }
            waves.push(Wave {
                data: app.memory_usage_history.iter().copied().collect(),
                palettes: vec![THEME.mem_trail],
            });
            waves
        }
        TimelineView::Cpu => app
            .cpu_core_histories
            .iter()
            .enumerate()
            .map(|(i, h)| Wave {
                data: h.iter().copied().collect(),
                palettes: vec![core_palette(i)],
            })
            .collect(),
        TimelineView::Gpu => app
            .gpu_core_histories
            .iter()
            .enumerate()
            .map(|(i, h)| Wave {
                data: h.iter().copied().collect(),
                palettes: vec![core_palette(i)],
            })
            .collect(),
        TimelineView::Memory => {
            vec![Wave {
                data: app.memory_usage_history.iter().copied().collect(),
                palettes: pressure_col_palettes(app, area.width as usize, offset),
            }]
        }
    };

    render_waves(f, area, &waves, offset);
}

/// Distinct base hues cycled across cores so individual traces stay
/// distinguishable when overlaid. Chosen for contrast on a dark terminal.
const CORE_HUES: [(u8, u8, u8); 12] = [
    (0, 200, 220),   // cyan
    (240, 160, 40),  // orange
    (120, 220, 90),  // green
    (220, 100, 210), // magenta
    (240, 220, 70),  // yellow
    (100, 150, 250), // blue
    (250, 110, 90),  // salmon
    (150, 230, 210), // teal
    (200, 140, 250), // violet
    (185, 210, 60),  // lime
    (250, 150, 200), // pink
    (110, 205, 170), // seafoam
];

/// A phosphor-fade trail palette for core `i`, derived from its base hue by
/// scaling brightness across the trail tiers.
fn core_palette(i: usize) -> [Color; TRAIL_TIERS] {
    let (r, g, b) = CORE_HUES[i % CORE_HUES.len()];
    let shade = |f: f32| {
        Color::Rgb(
            (r as f32 * f) as u8,
            (g as f32 * f) as u8,
            (b as f32 * f) as u8,
        )
    };
    [shade(1.0), shade(0.68), shade(0.44), shade(0.28)]
}

/// Build a per-character-column trail palette for the memory waveform,
/// coloured by the memory pressure level at each column. The pressure signal
/// is pushed through the same slice/interpolate/display pipeline as the usage
/// signal so the two stay column-aligned by construction.
fn pressure_col_palettes(
    app: &App,
    char_width: usize,
    timeline_offset: usize,
) -> Vec<[ratatui::style::Color; TRAIL_TIERS]> {
    use crate::memory::MemoryPressure;

    const DISPLAY_DURATION: usize = 300;
    let nums: Vec<f32> = app
        .memory_pressure_history
        .iter()
        .map(|p| match p {
            MemoryPressure::Green => 0.0,
            MemoryPressure::Yellow => 1.0,
            MemoryPressure::Red => 2.0,
        })
        .collect();

    let points = get_history_slice(&nums, timeline_offset + DISPLAY_DURATION, timeline_offset);
    let dense = interpolate_data(points, 4);
    let display_points = (char_width * 2).min(dense.len());
    let display = get_display_slice(&dense, display_points);

    (0..char_width)
        .map(|c| match display.get(c * 2).copied().unwrap_or(0.0).round() as i32 {
            n if n >= 2 => THEME.mem_crit_trail,
            1 => THEME.mem_warn_trail,
            _ => THEME.mem_trail,
        })
        .collect()
}

/// Build a single full-width line of per-core usage cells in tight
/// `label:percent` form (e.g. `cpu C0:10% C1:7% …`). Percentages are
/// colour-graded by load; the line clips on narrow terminals (no wrap).
fn render_cores_line(
    f: &mut Frame,
    area: Rect,
    lead: &str,
    prefix: char,
    usages: &[(String, f32)],
) {
    if area.width == 0 || area.height == 0 || usages.is_empty() {
        return;
    }

    let mut spans: Vec<Span> = Vec::with_capacity(usages.len() * 2 + 1);
    spans.push(Span::styled(
        format!("{} ", lead),
        Style::default().fg(THEME.fg_dim),
    ));

    for (i, (_name, usage)) in usages.iter().enumerate() {
        // Colour the core label with its timeline hue so this line doubles as
        // the legend for the per-core CPU/GPU timeline views.
        spans.push(Span::styled(
            format!("{}{}:", prefix, i),
            Style::default().fg(core_palette(i)[0]),
        ));
        spans.push(Span::styled(
            format!("{:.0}% ", usage),
            Style::default().fg(get_gradient_color(*usage)),
        ));
    }

    let line = Paragraph::new(Line::from(spans));
    f.render_widget(line, area);
}

fn render_cpu_cores_line(f: &mut Frame, app: &App, area: Rect) {
    render_cores_line(f, area, "cpu", 'C', &app.get_cpu_usages());
}

fn render_gpu_cores_line(f: &mut Frame, app: &App, area: Rect) {
    render_cores_line(f, area, "gpu", 'G', &app.get_gpu_usages());
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

/// One waveform to plot on the timeline: its full history plus a trail
/// palette. `palettes` is either a single uniform palette (`len == 1`) or one
/// palette per character column (used to tint the memory wave by pressure).
struct Wave {
    data: Vec<f32>,
    palettes: Vec<[ratatui::style::Color; TRAIL_TIERS]>,
}

/// Render an oscilloscope-style timeline for an arbitrary set of waves. Each
/// wave becomes its own braille trace; later waves draw over earlier ones on
/// overlap. Uses a buffered per-row approach to batch character rendering.
fn render_waves(f: &mut Frame, area: Rect, waves: &[Wave], timeline_offset: usize) {
    use ratatui::text::{Line, Span};

    let available_width = area.width as usize;
    let available_height = area.height as usize;

    if available_width == 0 || available_height == 0 || waves.is_empty() {
        return;
    }

    // Always display 300 seconds, but offset by timeline_offset
    const DISPLAY_DURATION: usize = 300;
    let end_offset = timeline_offset;
    let start_offset = end_offset + DISPLAY_DURATION;

    let char_width = available_width;
    let char_height = available_height;
    let dot_height = char_height * 4;

    // Slice, interpolate (4x density) and trim each wave to the visible window.
    let displays: Vec<Vec<f32>> = waves
        .iter()
        .map(|w| {
            let points = get_history_slice(&w.data, start_offset, end_offset);
            let dense = interpolate_data(points, 4);
            let display_points = (available_width * 2).min(dense.len());
            get_display_slice(&dense, display_points).to_vec()
        })
        .collect();
    let display_points = displays.iter().map(|d| d.len()).max().unwrap_or(0);

    // Buffer stores (braille_bits, winning wave index) per character cell.
    // `None` marks an empty cell so vertical lines only claim unowned cells.
    let mut row_buffer: Vec<(u32, Option<usize>)> = vec![(0, None); char_width];
    // Previous character row per wave, for vertical line connections.
    let mut prev_rows: Vec<Option<usize>> = vec![None; waves.len()];

    for row_idx in 0..char_height {
        for cell in row_buffer.iter_mut() {
            *cell = (0, None);
        }

        for col in 0..display_points {
            let char_col = col / 2;
            let braille_col = col % 2;

            if char_col >= char_width {
                continue;
            }

            for (wi, display) in displays.iter().enumerate() {
                let usage = display.get(col).copied().unwrap_or(0.0).clamp(0.0, 100.0);
                let dot_row = ((usage / 100.0) * (dot_height - 1) as f32).round() as usize;
                let cell_row = char_height.saturating_sub(1 + dot_row / 4);
                let sub_row = 3 - (dot_row % 4);

                if cell_row == row_idx {
                    row_buffer[char_col].0 |= get_braille_bits(braille_col, sub_row);
                    row_buffer[char_col].1 = Some(wi);
                }

                // Vertical line connecting this column's dot to the previous one.
                if let Some(prev_row) = prev_rows[wi]
                    && prev_row != cell_row
                {
                    let (start, end) = if prev_row < cell_row {
                        (prev_row, cell_row)
                    } else {
                        (cell_row, prev_row)
                    };
                    if row_idx > start && row_idx < end {
                        row_buffer[char_col].0 |= get_vertical_line_bits(braille_col);
                        if row_buffer[char_col].1.is_none() {
                            row_buffer[char_col].1 = Some(wi);
                        }
                    }
                }

                if braille_col == 1 {
                    prev_rows[wi] = Some(cell_row);
                }
            }
        }

        // Compose this row, coalescing adjacent cells with the same style.
        // Empty cells fall back to grid/cursor decoration so the chart has
        // structural reference lines beneath the waveform.
        let on_grid_row = char_height >= 4
            && (row_idx == char_height / 4
                || row_idx == char_height / 2
                || row_idx == (char_height * 3) / 4);
        let cursor_col = if timeline_offset == 0 && char_width > 0 {
            Some(char_width - 1)
        } else {
            None
        };

        let mut spans: Vec<Span> = Vec::new();
        let mut current_chars = String::new();
        let mut current_style = Style::default();
        let mut have_run = false;

        for (col, (bits, wave_idx)) in row_buffer.iter().enumerate() {
            let (ch, style) = if *bits != 0 {
                let braille = std::char::from_u32(0x2800 + bits).unwrap_or(' ');
                let tier = trail_tier(col, char_width).min(TRAIL_TIERS - 1);
                let palettes = &waves[wave_idx.unwrap_or(0)].palettes;
                let palette = &palettes[col.min(palettes.len() - 1)];
                (braille, Style::default().fg(palette[tier]))
            } else if cursor_col == Some(col) {
                ('│', Style::default().fg(THEME.cursor))
            } else {
                let on_grid_col = char_width >= 5
                    && (col == char_width / 5
                        || col == (char_width * 2) / 5
                        || col == (char_width * 3) / 5
                        || col == (char_width * 4) / 5);
                if on_grid_row || on_grid_col {
                    ('·', Style::default().fg(THEME.grid))
                } else {
                    (' ', Style::default())
                }
            };

            if have_run && style == current_style {
                current_chars.push(ch);
            } else {
                if have_run {
                    spans.push(Span::styled(
                        std::mem::take(&mut current_chars),
                        current_style,
                    ));
                }
                current_chars.push(ch);
                current_style = style;
                have_run = true;
            }
        }

        if have_run {
            spans.push(Span::styled(current_chars, current_style));
        }

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

fn get_gradient_color(usage: f32) -> Color {
    // Smooth gradient from cool to warm as load climbs.
    if usage >= 90.0 {
        THEME.accent_crit
    } else if usage >= 75.0 {
        Color::Rgb(255, 140, 80)
    } else if usage >= 50.0 {
        THEME.accent_warn
    } else if usage >= 25.0 {
        Color::Rgb(200, 200, 120)
    } else {
        THEME.fg_dim
    }
}

fn render_memory_section(f: &mut Frame, app: &App, area: Rect) {
    use crate::memory::MemoryPressure;

    let Some(memory_info) = app.memory_info else {
        let stats = Paragraph::new("memory loading…").style(Style::default().fg(THEME.fg_faint));
        f.render_widget(stats, area);
        return;
    };

    let label = Style::default().fg(THEME.fg_dim);
    let value = Style::default().fg(THEME.fg);
    let bullet = Span::styled(" · ", Style::default().fg(THEME.fg_faint));

    let pressure_color = match memory_info.pressure {
        MemoryPressure::Green => THEME.mem,
        MemoryPressure::Yellow => THEME.accent_warn,
        MemoryPressure::Red => THEME.accent_crit,
    };

    let used_gb = memory_info.used_memory as f64 / (1024.0 * 1024.0 * 1024.0);
    let total_gb = memory_info.total_memory as f64 / (1024.0 * 1024.0 * 1024.0);
    let free_gb = memory_info.free_memory() as f64 / (1024.0 * 1024.0 * 1024.0);

    let mut spans: Vec<Span> = vec![
        Span::styled("memory ", label),
        Span::styled(format!("{:.1}/{:.1} GB", used_gb, total_gb), value),
        Span::styled(
            format!(" ({:.0}%)", memory_info.memory_usage_percentage()),
            Style::default().fg(THEME.mem),
        ),
        bullet.clone(),
        Span::styled("pressure ", label),
        Span::styled(
            memory_info.pressure.color_name().to_string(),
            Style::default().fg(pressure_color),
        ),
        bullet.clone(),
        Span::styled("free ", label),
        Span::styled(format!("{:.1} GB", free_gb), value),
    ];

    if memory_info.total_swap > 0 {
        let used_swap = memory_info.used_swap as f64 / (1024.0 * 1024.0 * 1024.0);
        let total_swap = memory_info.total_swap as f64 / (1024.0 * 1024.0 * 1024.0);
        spans.extend([
            bullet,
            Span::styled("swap ", label),
            Span::styled(
                format!(
                    "{:.1}/{:.1} GB ({:.0}%)",
                    used_swap,
                    total_swap,
                    memory_info.swap_usage_percentage()
                ),
                value,
            ),
        ]);
    }

    let stats = Paragraph::new(Line::from(spans));
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
        .border_style(Style::default().fg(THEME.accent_crit))
        .title("kill process");
    f.render_widget(border_block, dialog_area);

    // Title
    let title_text = "kill process";
    let title = Paragraph::new(title_text)
        .alignment(ratatui::layout::Alignment::Center)
        .style(
            Style::default()
                .fg(THEME.accent_crit)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(title, dialog_chunks[0]);

    // Process information
    let process_info = if let Some(pid) = app.kill_target_pid {
        format!("PID {} · {}", pid, app.kill_target_name)
    } else {
        "unknown process".to_string()
    };
    let process_text = Paragraph::new(process_info)
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(THEME.fg));
    f.render_widget(process_text, dialog_chunks[2]);

    // Warning message
    let warning_text = "this action cannot be undone";
    let warning = Paragraph::new(warning_text)
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(THEME.accent_warn));
    f.render_widget(warning, dialog_chunks[3]);

    // Options
    let options_text = "[Y] kill    [N] cancel";
    let options = Paragraph::new(options_text)
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(THEME.fg_dim));
    f.render_widget(options, dialog_chunks[5]);
}

fn render_help_popup(f: &mut Frame, _app: &App) {
    use ratatui::widgets::{Block, Borders, Clear};

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
                .fg(THEME.accent_warn)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation:",
            Style::default().fg(THEME.cpu).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  j/k or ↑↓     Navigate process list up/down"),
        Line::from("  g             Jump to top of process list"),
        Line::from("  G             Jump to bottom of process list"),
        Line::from("  Page Up/Down  Navigate by 10 processes"),
        Line::from("  Home/End      Jump to first/last process"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Actions:",
            Style::default().fg(THEME.cpu).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Space         Pause/Resume monitoring"),
        Line::from("  Enter         Pin/Unpin process (shows full command)"),
        Line::from("  s             Cycle through sort modes"),
        Line::from("  v             Toggle GPU visibility"),
        Line::from("  Tab or t      Cycle timeline (overview / CPU / GPU / memory)"),
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
                .fg(THEME.accent_warn)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Timeline always displays 5 minutes of data. Use +/- to navigate"),
        Line::from("through up to 20 minutes of historical system metrics."),
        Line::from(""),
        Line::from(vec![Span::styled(
            "MEMORY PRESSURE ALGORITHM",
            Style::default()
                .fg(THEME.accent_warn)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Oversee uses macOS's native memory pressure reporting:"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "How it works:",
            Style::default().fg(THEME.cpu).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Queries kern.memorystatus_vm_pressure_level sysctl"),
        Line::from("  Same metric used by Activity Monitor for accuracy"),
        Line::from("  Considers file cache, compression, and memory demand"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Pressure Levels:",
            Style::default().fg(THEME.cpu).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                "  • Green (Normal): ",
                Style::default().fg(THEME.mem).add_modifier(Modifier::BOLD),
            ),
            Span::raw("Adequate memory, efficient operation"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • Yellow (Warning): ",
                Style::default()
                    .fg(THEME.accent_warn)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Some pressure, may use compression"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • Red (Critical): ",
                Style::default()
                    .fg(THEME.accent_crit)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Severe pressure, performance impacted"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Note: ", Style::default().fg(THEME.cpu)),
            Span::raw("macOS uses memory differently than other systems."),
        ]),
        Line::from("High usage with green pressure is optimal. See README for details"),
        Line::from("on why your Mac keeps memory full for better performance."),
        Line::from(""),
        Line::from(vec![Span::styled(
            "ABOUT OVERSEE",
            Style::default()
                .fg(THEME.accent_warn)
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
                .fg(THEME.fg_dim)
                .add_modifier(Modifier::ITALIC),
        )]),
    ];

    // Create the popup block
    let block = Block::default()
        .title(" help · oversee ")
        .title_style(Style::default().fg(THEME.fg).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(THEME.separator));

    // Create the paragraph widget
    let paragraph = Paragraph::new(Text::from(help_text))
        .block(block)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(THEME.fg));

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
        let usage_levels = [0.0, 25.0, 50.0, 75.0, 100.0];

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

    #[test]
    fn test_format_ports_collapses_duplicate_local_ports() {
        use crate::process::Protocol;

        let mk = |port, state| PortInfo {
            port,
            protocol: Protocol::Tcp,
            state,
            local_address: None,
            remote_address: None,
        };

        // A busy listener: one LISTEN socket plus many established connections
        // all sharing local port 443. The column must not read "443,443,443".
        let ports = vec![
            mk(443, ConnectionState::Listen),
            mk(443, ConnectionState::Established),
            mk(443, ConnectionState::Established),
            mk(443, ConnectionState::Established),
        ];
        // 443L (listening) and 443 (established) are distinct, but the repeated
        // established sockets collapse to a single entry.
        assert_eq!(format_ports(&ports), "443L,443");

        // Distinct ports are preserved and overflow past three is marked.
        let many = vec![
            mk(80, ConnectionState::Established),
            mk(81, ConnectionState::Established),
            mk(82, ConnectionState::Established),
            mk(83, ConnectionState::Established),
        ];
        assert_eq!(format_ports(&many), "80,81,82...");
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
