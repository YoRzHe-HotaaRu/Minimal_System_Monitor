mod collectors;
mod gpu;
mod history;
mod model;
mod network;
mod storage;
mod ui;

use std::io::{self, stdout};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use collectors::Collector;
use gpu::GpuCollector;
use history::History;
use model::Snapshot;
use network::NetworkCollector;

fn main() -> io::Result<()> {
    if std::env::args().any(|a| a == "--dump") {
        return dump_once();
    }

    enable_windows_colors();

    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

/// Enable Virtual Terminal Processing so ANSI / bright colors work in conhost.
fn enable_windows_colors() {
    #[cfg(windows)]
    {
        use windows::Win32::System::Console::{
            GetConsoleMode, GetStdHandle, SetConsoleMode, SetConsoleOutputCP,
            ENABLE_PROCESSED_OUTPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_OUTPUT_HANDLE,
            STD_ERROR_HANDLE,
        };
        unsafe {
            let _ = SetConsoleOutputCP(65001); // UTF-8
            for handle_id in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
                if let Ok(handle) = GetStdHandle(handle_id) {
                    let mut mode = Default::default();
                    if GetConsoleMode(handle, &mut mode).is_ok() {
                        let new_mode = mode
                            | ENABLE_VIRTUAL_TERMINAL_PROCESSING
                            | ENABLE_PROCESSED_OUTPUT;
                        let _ = SetConsoleMode(handle, new_mode);
                    }
                }
            }
        }
    }
}

fn dump_once() -> io::Result<()> {
    let mut collector = Collector::new();
    let gpu = GpuCollector::new();
    let mut network = NetworkCollector::new();
    let mut snap = Snapshot {
        drives: storage::list_ssds(),
        ..Default::default()
    };
    std::thread::sleep(Duration::from_millis(200));
    collector.collect(&mut snap);
    gpu.collect(&mut snap.gpu);
    network.collect(&mut snap.network);

    println!(
        "CPU  {} | {} | {:?} | {} | {} | {}",
        snap.cpu.name,
        opt_f(snap.cpu.clock_mhz, "MHz"),
        snap.cpu.usage_pct,
        opt_f(snap.cpu.temp_c, "C"),
        opt_f(snap.cpu.fan_rpm, "RPM"),
        opt_f(snap.cpu.power_w, "W"),
    );
    println!(
        "GPU  {} | {} | {:?} | {} | {} | VRAM {:?}/{:?} MB | fan {:?}",
        snap.gpu.name,
        opt_f(snap.gpu.clock_mhz, "MHz"),
        snap.gpu.usage_pct,
        opt_f(snap.gpu.temp_c, "C"),
        opt_f(snap.gpu.power_w, "W"),
        snap.gpu.vram_used_mb,
        snap.gpu.vram_total_mb,
        snap.gpu.fan_pct,
    );
    Ok(())
}

fn opt_f(v: Option<f64>, unit: &str) -> String {
    v.map(|x| format!("{x:.1} {unit}"))
        .unwrap_or_else(|| "N/A".into())
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut collector = Collector::new();
    let gpu = GpuCollector::new();
    let mut network = NetworkCollector::new();
    let mut hist = History::default();
    let mut snap = Snapshot {
        drives: storage::list_ssds(),
        ..Default::default()
    };

    // Prime CPU usage (sysinfo needs two samples).
    collector.collect(&mut snap);
    std::thread::sleep(Duration::from_millis(150));

    let refresh = Duration::from_millis(500);
    let mut last = Instant::now()
        .checked_sub(refresh)
        .unwrap_or_else(Instant::now);

    loop {
        if last.elapsed() >= refresh {
            collector.collect(&mut snap);
            gpu.collect(&mut snap.gpu);
            network.collect(&mut snap.network);
            storage::refresh_temps(&mut snap.drives);
            if snap.drives.is_empty() {
                snap.drives = storage::list_ssds();
            }
            hist.push_from_snapshot(&snap);
            last = Instant::now();
        }

        terminal.draw(|f| ui::draw(f, &snap, &hist))?;

        if event::poll(Duration::from_millis(33))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press
                    && (key.code == KeyCode::Char('q') || key.code == KeyCode::Esc)
                {
                    break;
                }
            }
        }
    }

    Ok(())
}
