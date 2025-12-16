#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

//! Shadow binary for chain shadowing with TUI.
//!
//! This binary shadows a chain by streaming blocks from an RPC endpoint
//! and simulates batch submission to L1, displaying real-time progress
//! in a terminal user interface with batch submission on the left and
//! derivation on the right.

use std::{io, sync::Arc};

use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    DefaultTerminal,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::sync::Mutex;

mod app;
mod rpc;
mod runner;

use app::{App, LogEntry, LogLevel};
use runner::{run_batch_submission, run_derivation};

/// Default Base mainnet RPC URL.
const DEFAULT_RPC_URL: &str = "https://mainnet.base.org";

/// Shadow TUI - Real-time chain shadowing with batch submission and derivation.
#[derive(Parser, Debug, Clone)]
#[command(name = "shadow")]
#[command(about = "Shadow a chain with real-time batch submission and derivation simulation")]
#[command(version)]
pub(crate) struct Args {
    /// Verbosity level (-v, -vv, -vvv).
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub(crate) verbose: u8,

    /// RPC URL for the L2 chain (defaults to Base mainnet).
    #[arg(short, long, default_value = DEFAULT_RPC_URL)]
    pub(crate) rpc: String,

    /// Starting block number. If not specified, starts from the latest block.
    #[arg(short, long)]
    pub(crate) start: Option<u64>,

    /// Compression algorithm to use (brotli, zlib, zstd).
    #[arg(short, long, default_value = "brotli")]
    pub(crate) compression: String,

    /// Block polling interval in milliseconds.
    #[arg(long, default_value = "2000")]
    pub(crate) poll_interval: u64,

    /// Maximum blocks per batch for simulation.
    #[arg(long, default_value = "10")]
    pub(crate) max_blocks_per_batch: usize,

    /// Target batch size in bytes before submitting.
    #[arg(long, default_value = "131072")]
    pub(crate) target_batch_size: usize,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let terminal = ratatui::init();

    // Run the app
    let result = run_app(terminal, args);

    // Cleanup
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    ratatui::restore();

    result
}

fn run_app(mut terminal: DefaultTerminal, args: Args) -> io::Result<()> {
    // Create shared app state
    let app = Arc::new(Mutex::new(App::new(args.rpc.clone(), args.compression.clone())));

    // Create tokio runtime
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    // Spawn background tasks
    let batch_app = Arc::clone(&app);
    let derivation_app = Arc::clone(&app);
    let batch_args = args.clone();

    rt.spawn(async move {
        run_batch_submission(batch_app, batch_args).await;
    });

    rt.spawn(async move {
        run_derivation(derivation_app).await;
    });

    // Main UI loop
    loop {
        // Draw UI
        let app_guard = rt.block_on(app.lock());
        terminal.draw(|frame| draw_ui(frame, &app_guard))?;
        drop(app_guard);

        // Handle input (non-blocking)
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            return Ok(());
                        }
                        KeyCode::Char('r') => {
                            // Reset/restart
                            let mut app_guard = rt.block_on(app.lock());
                            app_guard.reset();
                        }
                        KeyCode::Char('p') => {
                            // Toggle pause
                            let mut app_guard = rt.block_on(app.lock());
                            app_guard.toggle_pause();
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Draw the TUI layout.
fn draw_ui(frame: &mut ratatui::Frame<'_>, app: &App) {
    let area = frame.area();

    // Main layout: header (1/4 height) and body (3/4 height)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 4), Constraint::Ratio(3, 4)])
        .split(area);

    // Draw header
    draw_header(frame, app, main_chunks[0]);

    // Split body into left and right panes
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[1]);

    // Draw batch submission pane (left)
    draw_batch_pane(frame, app, body_chunks[0]);

    // Draw derivation pane (right)
    draw_derivation_pane(frame, app, body_chunks[1]);
}

/// Draw the header with stats and metrics.
fn draw_header(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let stats = &app.stats;

    // Create header content
    let header_text = vec![
        Line::from(vec![
            Span::styled("  MONTANA SHADOW  ", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" | "),
            Span::styled(
                if app.is_paused { "PAUSED" } else { "RUNNING" },
                Style::default().fg(if app.is_paused { Color::Yellow } else { Color::Green }),
            ),
            Span::raw(" | "),
            Span::raw(format!("RPC: {}", truncate_url(&app.rpc_url, 40))),
        ]),
        Line::from(vec![
            Span::raw("Chain Head: "),
            Span::styled(
                format!("#{}", stats.chain_head),
                Style::default().fg(Color::White).bold(),
            ),
            Span::raw("  |  Current: "),
            Span::styled(
                format!("#{}", stats.current_block),
                Style::default().fg(Color::White).bold(),
            ),
            Span::raw(format!(
                "  |  Lag: {} blocks",
                stats.chain_head.saturating_sub(stats.current_block)
            )),
        ]),
        Line::from(vec![
            Span::styled("Batch Submission", Style::default().fg(Color::Blue).bold()),
            Span::raw("  |  "),
            Span::raw(format!("Batches: {}", stats.batches_submitted)),
            Span::raw("  |  "),
            Span::raw(format!("Blocks: {}", stats.blocks_processed)),
            Span::raw("  |  "),
            Span::raw(format!("Bytes: {}", format_bytes(stats.bytes_compressed))),
            Span::raw("  |  "),
            Span::raw(format!("Ratio: {:.1}%", stats.compression_ratio * 100.0)),
        ]),
        Line::from(vec![
            Span::styled("Derivation      ", Style::default().fg(Color::Magenta).bold()),
            Span::raw("  |  "),
            Span::raw(format!("Batches: {}", stats.batches_derived)),
            Span::raw("  |  "),
            Span::raw(format!("Blocks: {}", stats.blocks_derived)),
            Span::raw("  |  "),
            Span::raw(format!("Bytes: {}", format_bytes(stats.bytes_decompressed))),
            Span::raw("  |  "),
            Span::styled(
                if stats.derivation_healthy { "HEALTHY" } else { "UNHEALTHY" },
                Style::default().fg(if stats.derivation_healthy {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
        ]),
        Line::from(vec![
            Span::styled("[q]", Style::default().fg(Color::Yellow)),
            Span::raw(" Quit  "),
            Span::styled("[p]", Style::default().fg(Color::Yellow)),
            Span::raw(" Pause  "),
            Span::styled("[r]", Style::default().fg(Color::Yellow)),
            Span::raw(" Reset  "),
            Span::raw("  |  "),
            Span::raw(format!("Compression: {}", app.compression)),
        ]),
    ];

    let header = Paragraph::new(header_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Stats & Metrics ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(header, area);
}

/// Draw the batch submission log pane.
fn draw_batch_pane(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let logs = render_logs(&app.batch_logs, area.height.saturating_sub(2) as usize);

    let batch_pane = Paragraph::new(logs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Batch Submission ")
                .title_style(Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(batch_pane, area);
}

/// Draw the derivation log pane.
fn draw_derivation_pane(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let logs = render_logs(&app.derivation_logs, area.height.saturating_sub(2) as usize);

    let derivation_pane = Paragraph::new(logs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Derivation ")
                .title_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(derivation_pane, area);
}

/// Render log entries as styled lines.
fn render_logs(logs: &[LogEntry], max_lines: usize) -> Vec<Line<'static>> {
    let start = logs.len().saturating_sub(max_lines);
    logs[start..]
        .iter()
        .map(|entry| {
            let (prefix, color) = match entry.level {
                LogLevel::Info => ("INFO ", Color::Green),
                LogLevel::Warn => ("WARN ", Color::Yellow),
                LogLevel::Error => ("ERROR", Color::Red),
            };
            Line::from(vec![
                Span::styled(format!("[{}] ", prefix), Style::default().fg(color)),
                Span::raw(entry.message.clone()),
            ])
        })
        .collect()
}

/// Format bytes as human-readable string.
fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Truncate a URL for display.
fn truncate_url(url: &str, max_len: usize) -> String {
    if url.len() <= max_len {
        url.to_string()
    } else {
        format!("{}...", &url[..max_len.saturating_sub(3)])
    }
}
