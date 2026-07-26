//! High-end TUI theme with colors that work on Windows consoles.
//! Uses bright ANSI + Indexed colors (truecolor RGB often collapses to gray
//! in legacy conhost without VT / Windows Terminal).

use crate::history::History;
use crate::model::Snapshot;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Gauge, Padding, Paragraph, Sparkline, SparklineBar,
};
use ratatui::Frame;
use std::time::{SystemTime, UNIX_EPOCH};

// ANSI-bright palette (reliable on Windows Terminal + VT-enabled conhost)
const BG: Color = Color::Black;
const PANEL: Color = Color::Reset; // let terminal bg show through panels via block style
const BORDER: Color = Color::DarkGray;
const TEXT: Color = Color::White;
const MUTED: Color = Color::Gray;
const TEAL: Color = Color::LightCyan;
const TEAL_DIM: Color = Color::Cyan;
const AMBER: Color = Color::Yellow;
const CORAL: Color = Color::LightRed;
const MINT: Color = Color::LightGreen;
const SKY: Color = Color::LightBlue;
const STORAGE: Color = Color::LightMagenta;

pub fn draw(frame: &mut Frame, snap: &Snapshot, hist: &History) {
    frame.render_widget(Block::default().style(Style::default().bg(BG).fg(TEXT)), frame.area());

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Ratio(5, 12),
            Constraint::Ratio(3, 12),
            Constraint::Min(6),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_header(frame, root[0], hist);
    draw_compute_row(frame, root[1], snap, hist);
    draw_memory_net_row(frame, root[2], snap, hist);
    draw_storage(frame, root[3], snap);
    draw_status(frame, root[4], snap);
    draw_footer(frame, root[5], snap);
}

fn panel(title: &str, accent: Color) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(BG).fg(TEXT))
        .padding(Padding::new(1, 1, 0, 0))
}

fn draw_header(frame: &mut Frame, area: Rect, hist: &History) {
    let pulse = if hist.tick % 2 == 0 { "●" } else { "◆" };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() % 86_400)
        .unwrap_or(0);
    let hh = now / 3600;
    let mm = (now % 3600) / 60;
    let ss = now % 60;

    let left = Line::from(vec![
        Span::styled("◈ ", Style::default().fg(TEAL).add_modifier(Modifier::BOLD)),
        Span::styled("PULSE", Style::default().fg(TEAL).add_modifier(Modifier::BOLD)),
        Span::styled(
            "  SYSTEM MONITOR",
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ·  native · nvml", Style::default().fg(MUTED)),
    ]);
    let right = Line::from(vec![
        Span::styled(
            format!("{pulse} LIVE  "),
            Style::default().fg(MINT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{hh:02}:{mm:02}:{ss:02}  "),
            Style::default().fg(MUTED),
        ),
        Span::styled("q", Style::default().fg(AMBER).add_modifier(Modifier::BOLD)),
        Span::styled(" quit", Style::default().fg(MUTED)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(TEAL))
        .style(Style::default().bg(BG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(inner);

    frame.render_widget(Paragraph::new(left), cols[0]);
    frame.render_widget(Paragraph::new(right).alignment(Alignment::Right), cols[1]);
}

fn draw_compute_row(frame: &mut Frame, area: Rect, snap: &Snapshot, hist: &History) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    draw_cpu_panel(frame, cols[0], snap, hist);
    draw_gpu_panel(frame, cols[1], snap, hist);
}

fn draw_cpu_panel(frame: &mut Frame, area: Rect, snap: &Snapshot, hist: &History) {
    let block = panel("CPU", TEAL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(2),
        ])
        .split(inner);

    let c = &snap.cpu;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            truncate(&c.name, rows[0].width as usize),
            Style::default().fg(MUTED),
        ))),
        rows[0],
    );

    let clock = c
        .clock_mhz
        .map(|v| format!("{:>5.0}", v))
        .unwrap_or_else(|| "  —  ".into());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(clock, Style::default().fg(TEAL).add_modifier(Modifier::BOLD)),
            Span::styled(" MHz", Style::default().fg(TEAL_DIM)),
            Span::raw("   "),
            Span::styled(fmt_opt_temp(c.temp_c), temp_style(c.temp_c)),
            Span::raw("   "),
            Span::styled(fmt_opt_w(c.power_w), Style::default().fg(AMBER)),
        ])),
        rows[1],
    );

    let usage = c.usage_pct.unwrap_or(0.0).clamp(0.0, 100.0);
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(TEAL).bg(Color::DarkGray))
            .ratio(usage / 100.0)
            .label(format!("load {usage:5.1}%"))
            .use_unicode(true),
        rows[2],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("FAN  ", Style::default().fg(MUTED)),
            Span::styled(fmt_opt_rpm(c.fan_rpm), Style::default().fg(TEXT)),
            Span::raw("    "),
            Span::styled("usage history", Style::default().fg(MUTED)),
        ])),
        rows[3],
    );

    // Prefer usage sparkline (moves); clock is often flat.
    let spark = hist.cpu_usage.normalized();
    frame.render_widget(
        Sparkline::default()
            .data(spark.into_iter().map(SparklineBar::from).collect::<Vec<_>>())
            .max(100)
            .style(Style::default().fg(TEAL)),
        rows[5],
    );
}

fn draw_gpu_panel(frame: &mut Frame, area: Rect, snap: &Snapshot, hist: &History) {
    let block = panel("GPU", SKY);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(2),
        ])
        .split(inner);

    let g = &snap.gpu;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            truncate(&g.name, rows[0].width as usize),
            Style::default().fg(MUTED),
        ))),
        rows[0],
    );

    let temp = g
        .temp_c
        .map(|v| format!("{:>4.0}", v))
        .unwrap_or_else(|| "  — ".into());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(temp, temp_style(g.temp_c).add_modifier(Modifier::BOLD)),
            Span::styled(" °C", Style::default().fg(MUTED)),
            Span::raw("   "),
            Span::styled(
                g.clock_mhz
                    .map(|v| format!("{v:.0} MHz"))
                    .unwrap_or_else(|| "— MHz".into()),
                Style::default().fg(SKY),
            ),
            Span::raw("   "),
            Span::styled(fmt_opt_w(g.power_w), Style::default().fg(AMBER)),
        ])),
        rows[1],
    );

    let usage = g.usage_pct.unwrap_or(0.0).clamp(0.0, 100.0);
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(SKY).bg(Color::DarkGray))
            .ratio(usage / 100.0)
            .label(format!("util {usage:5.1}%"))
            .use_unicode(true),
        rows[2],
    );

    let vram_ratio = match (g.vram_used_mb, g.vram_total_mb) {
        (Some(u), Some(t)) if t > 0 => u as f64 / t as f64,
        _ => 0.0,
    };
    let vram_label = match (g.vram_used_mb, g.vram_total_mb) {
        (Some(u), Some(t)) => format!("VRAM {u}/{t} MB"),
        _ => "VRAM —".into(),
    };
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(AMBER).bg(Color::DarkGray))
            .ratio(vram_ratio.clamp(0.0, 1.0))
            .label(vram_label)
            .use_unicode(true),
        rows[3],
    );

    let fan = g
        .fan_pct
        .map(|p| format!("{p}%"))
        .unwrap_or_else(|| "N/A".into());
    if let Some(err) = &g.error {
        frame.render_widget(
            Paragraph::new(Span::styled(err.clone(), Style::default().fg(CORAL))),
            rows[4],
        );
    } else {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("FAN  ", Style::default().fg(MUTED)),
                Span::styled(fan, Style::default().fg(TEXT)),
                Span::raw("    "),
                Span::styled("temp history", Style::default().fg(MUTED)),
            ])),
            rows[4],
        );
    }

    let spark = hist.gpu_temp.normalized();
    frame.render_widget(
        Sparkline::default()
            .data(spark.into_iter().map(SparklineBar::from).collect::<Vec<_>>())
            .max(100)
            .style(Style::default().fg(temp_color(g.temp_c))),
        rows[5],
    );
}

fn draw_memory_net_row(frame: &mut Frame, area: Rect, snap: &Snapshot, hist: &History) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);
    draw_ram_panel(frame, cols[0], snap, hist);
    draw_net_panel(frame, cols[1], snap, hist);
}

fn draw_ram_panel(frame: &mut Frame, area: Rect, snap: &Snapshot, hist: &History) {
    let block = panel("MEMORY", AMBER);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);

    let m = &snap.memory;
    let pct = if m.total_gb > 0.0 {
        (m.used_gb / m.total_gb) * 100.0
    } else {
        0.0
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{:.1}", m.used_gb),
                Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" / {:.1} GB", m.total_gb), Style::default().fg(MUTED)),
            Span::raw("   "),
            Span::styled(
                format!("{pct:.0}%"),
                Style::default().fg(heat_pct(pct)).add_modifier(Modifier::BOLD),
            ),
        ])),
        rows[0],
    );

    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(heat_pct(pct)).bg(Color::DarkGray))
            .ratio((pct / 100.0).clamp(0.0, 1.0))
            .use_unicode(true),
        rows[1],
    );

    let slot = m
        .slots
        .first()
        .map(|s| {
            format!(
                "{}  {}",
                truncate(&s.label, 28),
                s.temp_c
                    .map(|t| format!("{t:.0}°C"))
                    .unwrap_or_else(|| "temp N/A".into())
            )
        })
        .unwrap_or_else(|| "no DIMM info".into());
    frame.render_widget(
        Paragraph::new(Span::styled(slot, Style::default().fg(MUTED))),
        rows[2],
    );

    let spark = hist.ram_pct.normalized();
    frame.render_widget(
        Sparkline::default()
            .data(spark.into_iter().map(SparklineBar::from).collect::<Vec<_>>())
            .max(100)
            .style(Style::default().fg(AMBER)),
        rows[3],
    );
}

fn draw_net_panel(frame: &mut Frame, area: Rect, snap: &Snapshot, hist: &History) {
    let block = panel("NETWORK", MINT);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(2),
        ])
        .split(inner);

    let n = &snap.network;
    frame.render_widget(
        Paragraph::new(Span::styled(
            truncate(&n.adapter, rows[0].width as usize),
            Style::default().fg(MUTED),
        )),
        rows[0],
    );

    let ping = n
        .ping_ms
        .map(|p| format!("{p:.0} ms"))
        .unwrap_or_else(|| "—".into());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("↓ ", Style::default().fg(MINT).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{:>7.2}", n.download_mbps),
                Style::default().fg(MINT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Mbps", Style::default().fg(MUTED)),
            Span::raw("    "),
            Span::styled("↑ ", Style::default().fg(SKY).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{:>7.2}", n.upload_mbps),
                Style::default().fg(SKY).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Mbps", Style::default().fg(MUTED)),
            Span::raw("    "),
            Span::styled("ping ", Style::default().fg(MUTED)),
            Span::styled(
                ping,
                Style::default()
                    .fg(ping_color(n.ping_ms))
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
        rows[1],
    );

    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("target  {}", n.ping_target),
            Style::default().fg(MUTED),
        )),
        rows[2],
    );

    let spark_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[3]);

    let down = hist.net_down.normalized();
    let up = hist.net_up.normalized();
    frame.render_widget(
        Sparkline::default()
            .data(down.into_iter().map(SparklineBar::from).collect::<Vec<_>>())
            .max(100)
            .style(Style::default().fg(MINT)),
        spark_row[0],
    );
    frame.render_widget(
        Sparkline::default()
            .data(up.into_iter().map(SparklineBar::from).collect::<Vec<_>>())
            .max(100)
            .style(Style::default().fg(SKY)),
        spark_row[1],
    );
}

fn draw_storage(frame: &mut Frame, area: Rect, snap: &Snapshot) {
    let block = panel("STORAGE", STORAGE);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if snap.drives.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("No drives detected", Style::default().fg(MUTED))),
            inner,
        );
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    for d in &snap.drives {
        let bar = temp_bar(d.temp_c, 14);
        let temp_txt = d
            .temp_c
            .map(|t| format!("{t:>4.0}°C"))
            .unwrap_or_else(|| "  N/A".into());
        lines.push(Line::from(vec![
            Span::styled("▣ ", Style::default().fg(temp_color(d.temp_c))),
            Span::styled(
                format!("{:<34}", truncate(&d.name, 34)),
                Style::default().fg(TEXT),
            ),
            Span::styled(format!("{:>6.0} GB  ", d.size_gb), Style::default().fg(MUTED)),
            Span::styled(temp_txt, temp_style(d.temp_c).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(bar, Style::default().fg(temp_color(d.temp_c))),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_status(frame: &mut Frame, area: Rect, snap: &Snapshot) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(BG))
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chips = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(inner);

    render_chip(frame, chips[0], "CPU SENSORS", snap.cpu.temp_c.is_some());
    render_chip(frame, chips[1], "GPU NVML", snap.gpu.temp_c.is_some());
    render_chip(
        frame,
        chips[2],
        "SSD SMART",
        snap.drives.iter().any(|d| d.temp_c.is_some()),
    );
    render_chip(frame, chips[3], "NETWORK", snap.network.ping_ms.is_some());
}

fn render_chip(frame: &mut Frame, area: Rect, label: &str, ok: bool) {
    let (dot, color, tag) = if ok {
        ("●", MINT, "OK")
    } else {
        ("○", MUTED, "--")
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{dot} "), Style::default().fg(color)),
            Span::styled(format!("{label} "), Style::default().fg(MUTED)),
            Span::styled(tag, Style::default().fg(color).add_modifier(Modifier::BOLD)),
        ])),
        area,
    );
}

fn draw_footer(frame: &mut Frame, area: Rect, snap: &Snapshot) {
    frame.render_widget(
        Paragraph::new(Span::styled(
            truncate(&snap.note, area.width as usize),
            Style::default().fg(MUTED),
        ))
        .style(Style::default().bg(BG)),
        area,
    );
}

fn temp_bar(temp: Option<f64>, width: usize) -> String {
    let Some(t) = temp else {
        return "·".repeat(width);
    };
    let ratio = ((t - 20.0) / 60.0).clamp(0.0, 1.0);
    let filled = (ratio * width as f64).round() as usize;
    let mut s = String::new();
    for i in 0..width {
        s.push(if i < filled { '█' } else { '░' });
    }
    s
}

fn temp_color(temp: Option<f64>) -> Color {
    match temp {
        Some(t) if t >= 85.0 => CORAL,
        Some(t) if t >= 70.0 => AMBER,
        Some(t) if t >= 50.0 => TEAL,
        Some(_) => MINT,
        None => MUTED,
    }
}

fn temp_style(temp: Option<f64>) -> Style {
    Style::default().fg(temp_color(temp))
}

fn heat_pct(pct: f64) -> Color {
    if pct >= 90.0 {
        CORAL
    } else if pct >= 75.0 {
        AMBER
    } else if pct >= 50.0 {
        TEAL
    } else {
        MINT
    }
}

fn ping_color(ms: Option<f64>) -> Color {
    match ms {
        Some(p) if p <= 30.0 => MINT,
        Some(p) if p <= 80.0 => AMBER,
        Some(_) => CORAL,
        None => MUTED,
    }
}

fn fmt_opt_temp(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.0}°C"))
        .unwrap_or_else(|| "temp N/A".into())
}
fn fmt_opt_w(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.0} W"))
        .unwrap_or_else(|| "— W".into())
}
fn fmt_opt_rpm(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.0} RPM"))
        .unwrap_or_else(|| "N/A".into())
}

fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

#[allow(dead_code)]
fn _panel_const() {
    let _ = PANEL;
}
