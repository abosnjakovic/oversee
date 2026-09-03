use crate::app::{App, Mode};
use crate::convert::{bytes_to_f64, count_to_f32, to_count, to_f32};
use crate::process::{ConnectionState, PortInfo, ProcessDetails, ProcessInfo, SortMode};
use crate::theme::THEME;
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
        format!("{d}d{h:02}h")
    } else if h > 0 {
        format!("{h}h{m:02}m")
    } else {
        format!("{}m", m.max(1))
    }
}

fn wrap_to_width(s: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![s.to_string()];
    }
    let mut lines = Vec::new();
    let mut buf = String::new();
    let mut buf_len: usize = 0;
    for ch in s.chars() {
        buf.push(ch);
        buf_len = buf_len.saturating_add(1);
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
        format!("{h}h{m:02}m")
    } else if m > 0 {
        format!("{m}m{s:02}s")
    } else {
        format!("{s}s")
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
        ("", None) => format!("  {proto}  {local}"),
        ("", Some(r)) => format!("  {proto}  {local} -> {r}"),
        (st, None) => format!("  {proto}  {local}  {st}"),
        (st, Some(r)) => format!("  {proto}  {local}  {st} -> {r}"),
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
            .map_or_else(|| "…".to_string(), |n| n.to_string())
    } else {
        "…".to_string()
    };
    let fds = details
        .and_then(|d| d.fd_count)
        .map_or_else(|| "…".to_string(), |n| n.to_string());
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
                format!("  ... (+{} more)", total.saturating_sub(MAX_BREAKOUT_PORTS)),
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
        x: size.x.saturating_add(1),
        y: size.y.saturating_add(1),
        width: size.width.saturating_sub(2),
        height: size.height.saturating_sub(2),
    };

    // Main layout: KPI header, separator, bar meters, separator, process list
    let items_len = bar_items(app).len();
    let cols = if margin_area.width >= 80 { 2 } else { 1 };
    let bar_height = u16::try_from(items_len.div_ceil(cols)).unwrap_or(u16::MAX);
    let [header_area, header_rule, bars_area, list_rule, list_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),          // KPI header strip
            Constraint::Length(1),          // Separator under header
            Constraint::Length(bar_height), // Bar meters
            Constraint::Length(1),          // Separator above process list
            Constraint::Min(8),             // Process list
        ])
        .areas(margin_area);

    render_kpi_header(f, app, header_area);
    render_separator(f, header_rule);
    render_bars(f, app, bars_area);
    render_separator(f, list_rule);
    render_process_list(f, app, list_area);

    // Render kill confirmation dialog if active
    if app.mode == Mode::KillConfirmation {
        render_kill_confirmation(f, app, size);
    }

    // Render help popup if active (render last so it appears on top)
    if app.mode == Mode::Help {
        render_help_popup(f, app);
    }
}

/// Column headings, with the active sort column underlined and brightened.
fn process_table_header(sort_mode: SortMode) -> Row<'static> {
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
    Row::new(vec![
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
    .height(1)
}

/// One table row for a process: the collapsed single line, or the expanded
/// form with its breakout lines underneath.
fn process_row(app: &App, i: usize, proc: &ProcessInfo, cmd_col_width: usize) -> Row<'static> {
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

    let mem_mb = bytes_to_f64(proc.memory) / (1024.0 * 1024.0);

    let cmd_display = proc.cmd.clone();

    let pid_display = if is_pinned {
        format!("◆ {}", proc.pid)
    } else {
        proc.pid.to_string()
    };

    // Per-metric coloured numeric cells, dimmed when value is negligible
    // or unavailable.
    let metric_cell = |value: Option<f32>, color: Color, width: usize| -> Cell {
        let style = match value {
            Some(v) if v >= 1.0 => Style::default().fg(color),
            _ => Style::default().fg(THEME.fg_faint),
        };
        Cell::from(Span::styled(format_metric(value, width), style))
    };
    let mem_cell = {
        let style = if mem_mb < 1.0 {
            Style::default().fg(THEME.fg_faint)
        } else {
            Style::default().fg(THEME.mem)
        };
        Cell::from(Span::styled(format!("{mem_mb:>7.0}"), style))
    };
    let pid_cell = Cell::from(Span::styled(
        format!("{pid_display:>8}"),
        Style::default().fg(THEME.fg_dim),
    ));

    if is_expanded {
        let mut cmd_lines: Vec<Line> = vec![Line::from(cmd_display)];
        cmd_lines.extend(build_breakout_lines(
            proc,
            app.selected_details.as_ref(),
            cmd_col_width,
        ));
        let row_height = u16::try_from(cmd_lines.len()).unwrap_or(u16::MAX);
        Row::new(vec![
            pid_cell,
            Cell::from(truncate_string(&proc.user, 8)),
            metric_cell(Some(proc.cpu_usage), THEME.cpu, 6),
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
            metric_cell(Some(proc.cpu_usage), THEME.cpu, 6),
            metric_cell(proc.gpu_usage, THEME.gpu, 6),
            Cell::from(format_ports(&proc.ports)),
            mem_cell,
            Cell::from(cmd_display),
        ])
        .style(row_style)
    }
}

fn render_process_list(f: &mut Frame, app: &mut App, area: Rect) {
    let all_processes = app.get_all_processes();
    let processes = app.get_filtered_processes();

    // Split for table and help - ensure help gets exactly 1 line at bottom
    let [table_and_title, help_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),    // Process table (minimum 5 lines)
            Constraint::Length(1), // Help text (exactly 1 line at bottom)
        ])
        .areas(area);

    let header = process_table_header(app.get_sort_mode());

    // Width available for the Command column's wrapped breakout content.
    // Fixed cols total 8+8+6+6+12+7 = 47, plus 6 column spacings, plus 2 for highlight symbol.
    let cmd_col_width = usize::from(table_and_title.width).saturating_sub(47 + 6 + 2);

    // Process rows
    let rows: Vec<Row> = processes
        .iter()
        .enumerate()
        .map(|(i, proc)| process_row(app, i, proc, cmd_col_width))
        .collect();

    // Render title at top of the allocated chunk
    let title_text = if app.mode == Mode::Filter {
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
        height: 1,
        ..table_and_title
    };
    f.render_widget(title, title_area);

    // Create table in remaining space of the first chunk
    let table_area = Rect {
        y: table_and_title.y.saturating_add(1),
        height: table_and_title.height.saturating_sub(1),
        ..table_and_title
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
    let help_text = if app.mode == Mode::KillConfirmation {
        "confirm kill · [Y] yes · [N] no · esc cancel"
    } else if app.mode == Mode::Filter {
        "type to filter · enter apply · esc cancel"
    } else if app.is_paused() {
        "[paused] space resume · q quit · ↑↓ nav · enter pin · K kill · s sort · / filter · g/G top/bot · ? help"
    } else {
        "space pause · q quit · ↑↓ nav · enter pin · K kill · s sort · / filter · g/G top/bot · ? help"
    };

    let help_style = if app.mode == Mode::KillConfirmation {
        Style::default().fg(THEME.accent_crit)
    } else {
        Style::default().fg(THEME.fg_faint)
    };

    let help = Paragraph::new(help_text)
        .style(help_style)
        .wrap(Wrap { trim: true });

    f.render_widget(help, help_area);
}

fn render_separator(f: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let line = "─".repeat(usize::from(area.width));
    let sep = Paragraph::new(line).style(Style::default().fg(THEME.separator));
    f.render_widget(sep, area);
}

fn render_kpi_header(f: &mut Frame, app: &App, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let label = Style::default().fg(THEME.fg_dim);
    let bullet = Span::styled(" · ", Style::default().fg(THEME.fg_faint));

    let load = sysinfo::System::load_average();
    let proc_count = app.get_all_processes().len();

    let spans: Vec<Span> = vec![
        Span::styled(
            "oversee",
            Style::default().fg(THEME.fg).add_modifier(Modifier::BOLD),
        ),
        bullet.clone(),
        Span::styled("load ", label),
        Span::styled(
            format!("{:.2} {:.2} {:.2}", load.one, load.five, load.fifteen),
            Style::default().fg(THEME.fg),
        ),
        bullet.clone(),
        Span::styled(format!("{proc_count} procs"), label),
        bullet.clone(),
        Span::styled("up ", label),
        Span::styled(format_uptime_short(), label),
        bullet,
        Span::styled(app.cpu_brand.clone(), label),
    ];

    let header = Paragraph::new(Line::from(spans));
    f.render_widget(header, area);
}

/// One metric rendered as a bar in either placement.
struct BarItem {
    label: String,
    frac: f32,
    value: String,
    color: Color,
}

/// Cores, then GPU (when visible), MEM, SWP (when swap exists) — the order
/// bars appear in both placements.
fn bar_items(app: &App) -> Vec<BarItem> {
    let mut items: Vec<BarItem> = app
        .cpu_core_values
        .iter()
        .enumerate()
        .map(|(i, &v)| BarItem {
            label: format!("C{i}"),
            frac: v / 100.0,
            value: format!("{v:.0}%"),
            color: get_gradient_color(v),
        })
        .collect();

    if app.is_gpu_visible() {
        items.push(BarItem {
            label: "GPU".to_string(),
            frac: app.gpu_value / 100.0,
            value: format!("{:.0}%", app.gpu_value),
            color: get_gradient_color(app.gpu_value),
        });
    }

    let gib = 1024.0 * 1024.0 * 1024.0;
    let pressure_color = match app.memory_info.as_ref().map(|m| m.pressure) {
        Some(crate::memory::MemoryPressure::Yellow) => THEME.accent_warn,
        Some(crate::memory::MemoryPressure::Red) => THEME.accent_crit,
        _ => THEME.mem,
    };
    match app.memory_info {
        Some(m) => {
            items.push(BarItem {
                label: "MEM".to_string(),
                frac: to_f32(m.memory_usage_percentage() / 100.0),
                value: format!(
                    "{:.1}/{:.1}G",
                    bytes_to_f64(m.used_memory) / gib,
                    bytes_to_f64(m.total_memory) / gib
                ),
                color: pressure_color,
            });
            if m.total_swap > 0 {
                items.push(BarItem {
                    label: "SWP".to_string(),
                    frac: to_f32(m.swap_usage_percentage() / 100.0),
                    value: format!(
                        "{:.1}/{:.1}G",
                        bytes_to_f64(m.used_swap) / gib,
                        bytes_to_f64(m.total_swap) / gib
                    ),
                    color: pressure_color,
                });
            }
        }
        None => items.push(BarItem {
            label: "MEM".to_string(),
            frac: 0.0,
            value: "—".to_string(),
            color: THEME.fg_faint,
        }),
    }
    items
}

/// htop-style bar section: 2-column grid (1 column on narrow terminals),
/// filling column-first like htop. Row layout per bar: `LBL ━━━╸···  value`.
fn render_bars(f: &mut Frame, app: &App, area: Rect) {
    const VALUE_W: usize = 11; // fits "30.0/48.0G"
    const LABEL_W: usize = 4;

    if area.width == 0 || area.height == 0 {
        return;
    }

    let items = &bar_items(app);
    let cols: usize = if area.width >= 80 { 2 } else { 1 };
    let rows = items.len().div_ceil(cols).max(1);
    let col_w = usize::from(area.width).checked_div(cols).unwrap_or(0);
    let bar_w = col_w.saturating_sub(LABEL_W + VALUE_W + 3).max(4);

    for (i, item) in items.iter().enumerate() {
        let (col, row) = (
            i.checked_div(rows).unwrap_or(0),
            i.checked_rem(rows).unwrap_or(0),
        );
        if u16::try_from(row).unwrap_or(u16::MAX) >= area.height {
            continue;
        }
        let bar = hori_bar(item.frac, bar_w);
        let split = bar.find('·').unwrap_or(bar.len());
        let (fill, rest) = bar.split_at(split);
        let line = Line::from(vec![
            Span::styled(
                format!("{:<1$}", item.label, LABEL_W),
                Style::default().fg(THEME.fg_dim),
            ),
            Span::styled(fill.to_string(), Style::default().fg(item.color)),
            Span::styled(rest.to_string(), Style::default().fg(THEME.grid)),
            Span::styled(
                format!("{:>1$}", item.value, VALUE_W + 1),
                Style::default().fg(item.color),
            ),
        ]);
        f.render_widget(
            Paragraph::new(line),
            Rect {
                x: area
                    .x
                    .saturating_add(u16::try_from(col.saturating_mul(col_w)).unwrap_or(u16::MAX)),
                y: area
                    .y
                    .saturating_add(u16::try_from(row).unwrap_or(u16::MAX)),
                width: u16::try_from(col_w).unwrap_or(u16::MAX),
                height: 1,
            },
        );
    }
}

/// htop-style horizontal meter: `━` full cells, one `╸` half-step, `·` rest.
/// Exactly `width` chars; `frac` is clamped to 0.0–1.0.
fn hori_bar(frac: f32, width: usize) -> String {
    let units = to_count((frac.clamp(0.0, 1.0) * count_to_f32(width.saturating_mul(2))).round());
    let full = units / 2;
    let half = units % 2;
    format!(
        "{}{}{}",
        "━".repeat(full),
        if half == 1 { "╸" } else { "" },
        "·".repeat(width.saturating_sub(full).saturating_sub(half))
    )
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
    let [title_line, _, info_line, warning_line, _, options_line] = Layout::default()
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
        .areas(dialog_area);

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
    f.render_widget(title, title_line);

    // Process information
    let process_info = app.kill_target_pid.map_or_else(
        || "unknown process".to_string(),
        |pid| format!("PID {} · {}", pid, app.kill_target_name),
    );
    let process_text = Paragraph::new(process_info)
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(THEME.fg));
    f.render_widget(process_text, info_line);

    // Warning message
    let warning_text = "this action cannot be undone";
    let warning = Paragraph::new(warning_text)
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(THEME.accent_warn));
    f.render_widget(warning, warning_line);

    // Options
    let options_text = "[Y] kill    [N] cancel";
    let options = Paragraph::new(options_text)
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(THEME.fg_dim));
    f.render_widget(options, options_line);
}

/// The keybind reference shown in the help popup.
/// A top-level heading in the help popup.
fn help_heading(text: &'static str) -> Line<'static> {
    Line::from(vec![Span::styled(
        text,
        Style::default()
            .fg(THEME.accent_warn)
            .add_modifier(Modifier::BOLD),
    )])
}

/// A section heading within one part of the help popup.
fn help_subheading(text: &'static str) -> Line<'static> {
    Line::from(vec![Span::styled(
        text,
        Style::default().fg(THEME.cpu).add_modifier(Modifier::BOLD),
    )])
}

fn help_popup_lines() -> Vec<Line<'static>> {
    let mut lines = help_keybind_lines();
    lines.extend(help_colour_key_lines());
    lines.extend(help_pressure_lines());
    lines.extend(help_about_lines());
    lines
}

fn help_keybind_lines() -> Vec<Line<'static>> {
    vec![
        help_heading("KEYBINDS"),
        Line::from(""),
        help_subheading("Navigation:"),
        Line::from("  j/k or ↑↓     Navigate process list up/down"),
        Line::from("  g             Jump to top of process list"),
        Line::from("  G             Jump to bottom of process list"),
        Line::from("  Page Up/Down  Navigate by 10 processes"),
        Line::from("  Home/End      Jump to first/last process"),
        Line::from(""),
        help_subheading("Actions:"),
        Line::from("  Space         Pause/Resume monitoring"),
        Line::from("  Enter         Pin/Unpin process (shows full command)"),
        Line::from("  s             Cycle through sort modes"),
        Line::from("  v             Toggle GPU bar"),
        Line::from("  K             Kill selected process (with confirmation)"),
        Line::from("  /             Enter filter mode"),
        Line::from("  ?             Toggle this help popup"),
        Line::from("  q or ESC      Quit application"),
        Line::from(""),
    ]
}

fn help_colour_key_lines() -> Vec<Line<'static>> {
    vec![
        help_heading("COLOUR KEY"),
        Line::from(""),
        help_subheading("Load gradient (CPU / GPU bars):"),
        Line::from(vec![
            Span::styled("  • dim", Style::default().fg(THEME.fg_dim)),
            Span::raw(" < 25%   "),
            Span::styled(
                "• yellow-green",
                Style::default().fg(Color::Rgb(200, 200, 120)),
            ),
            Span::raw(" ≥ 25%   "),
            Span::styled("• yellow", Style::default().fg(THEME.accent_warn)),
            Span::raw(" ≥ 50%"),
        ]),
        Line::from(vec![
            Span::styled("  • orange", Style::default().fg(Color::Rgb(255, 140, 80))),
            Span::raw(" ≥ 75%   "),
            Span::styled("• red", Style::default().fg(THEME.accent_crit)),
            Span::raw(" ≥ 90%"),
        ]),
        Line::from(""),
        help_subheading("Memory pressure (MEM / SWP bar colour):"),
        Line::from(vec![
            Span::styled("  • green", Style::default().fg(THEME.mem)),
            Span::raw(" normal   "),
            Span::styled("• yellow", Style::default().fg(THEME.accent_warn)),
            Span::raw(" warning   "),
            Span::styled("• red", Style::default().fg(THEME.accent_crit)),
            Span::raw(" critical"),
        ]),
        Line::from(""),
    ]
}

fn help_pressure_lines() -> Vec<Line<'static>> {
    vec![
        help_heading("MEMORY PRESSURE ALGORITHM"),
        Line::from(""),
        Line::from("Oversee uses macOS's native memory pressure reporting:"),
        Line::from(""),
        help_subheading("How it works:"),
        Line::from("  Queries kern.memorystatus_vm_pressure_level sysctl"),
        Line::from("  Same metric used by Activity Monitor for accuracy"),
        Line::from("  Considers file cache, compression, and memory demand"),
        Line::from(""),
        help_subheading("Pressure Levels:"),
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
    ]
}

fn help_about_lines() -> Vec<Line<'static>> {
    vec![
        help_heading("ABOUT OVERSEE"),
        Line::from(""),
        Line::from("A modern system monitor for macOS, inspired by htop and btop++,"),
        Line::from("built in Rust with a focus on Apple Silicon performance monitoring."),
        Line::from(""),
        Line::from("Features per-core CPU and GPU bar meters, memory pressure"),
        Line::from("indicators matching Activity Monitor, and vim-style navigation."),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press ? or ESC to close this help",
            Style::default()
                .fg(THEME.fg_dim)
                .add_modifier(Modifier::ITALIC),
        )]),
    ]
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
            width: area
                .width
                .saturating_sub(horizontal_margin.saturating_mul(2)),
            height: area
                .height
                .saturating_sub(vertical_margin.saturating_mul(2)),
        }
    };

    // Clear the area
    f.render_widget(Clear, popup_area);

    let help_text = help_popup_lines();

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

/// Right-align a metric in `width` columns. `None` means the metric has no data
/// source and renders as an em dash — never as `0.0`, which would be
/// indistinguishable from a genuine zero reading.
fn format_metric(value: Option<f32>, width: usize) -> String {
    value.map_or_else(
        || format!("{:>1$}", "—", width),
        |v| format!("{v:>width$.1}"),
    )
}

/// Truncate to `max_len` columns, counting characters rather than bytes: a
/// non-ASCII username sliced on a byte boundary panics, and its byte length
/// overstates how wide it renders.
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        return s.to_string();
    }
    let kept: String = s.chars().take(max_len.saturating_sub(3)).collect();
    format!("{kept}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Usernames are not all ASCII, and byte-slicing one mid-codepoint panics.
    #[test]
    fn test_truncate_string_counts_characters_not_bytes() {
        assert_eq!(truncate_string("ärlig", 8), "ärlig");
        assert_eq!(truncate_string("ärligtalad", 8), "ärlig...");
        assert_eq!(truncate_string("日本語のユーザー名", 8), "日本語のユ...");
    }

    #[test]
    fn test_unavailable_metric_is_not_rendered_as_zero() {
        // A missing data source must be visually distinct from a real 0.0
        // reading. Regression guard against reintroducing a fabricated value
        // (e.g. estimating per-process GPU from the process name).
        let unavailable = format_metric(None, 6);
        assert_eq!(unavailable, "     —");
        assert_ne!(unavailable.trim(), "0.0");

        // Real readings still format as before.
        assert_eq!(format_metric(Some(0.0), 6), "   0.0");
        assert_eq!(format_metric(Some(12.34), 6), "  12.3");

        // Both branches occupy the same number of terminal columns, so the
        // table stays aligned. Note: chars(), not len() — the em dash is 3 bytes.
        for value in [None, Some(0.0), Some(100.0)] {
            assert_eq!(format_metric(value, 6).chars().count(), 6, "{value:?}");
        }
    }

    #[test]
    fn test_horizontal_grid_fills_column_first() {
        // 17 items, 2 columns → 9 rows; item i maps to (col = i/9, row = i%9).
        let rows = 17usize.div_ceil(2);
        assert_eq!(rows, 9);
        let pos = |i: usize| (i / rows, i % rows);
        assert_eq!(pos(0), (0, 0)); // C0 top-left
        assert_eq!(pos(8), (0, 8)); // C8 bottom-left
        assert_eq!(pos(9), (1, 0)); // C9 top-right
        assert_eq!(pos(16), (1, 7)); // last item second column
    }

    #[test]
    fn test_hori_bar_half_step_resolution() {
        assert_eq!(hori_bar(0.0, 18), "·".repeat(18));
        assert_eq!(hori_bar(1.0, 18), "━".repeat(18));
        // 0.43 × 36 half-units = 15.48 → 15 units = 7 full + 1 half + 10 empty
        assert_eq!(
            hori_bar(0.43, 18),
            format!("{}╸{}", "━".repeat(7), "·".repeat(10))
        );
        // Clamps out-of-range input.
        assert_eq!(hori_bar(1.7, 10), "━".repeat(10));
        assert_eq!(hori_bar(-0.2, 10), "·".repeat(10));
        // Always exactly `width` chars.
        for frac in [0.0f32, 0.1, 0.5, 0.99, 1.0] {
            assert_eq!(hori_bar(frac, 18).chars().count(), 18, "{frac}");
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
}
