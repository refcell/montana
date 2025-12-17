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
use montana_batch_runner::{
    BatchSubmissionCallback, BatchSubmissionConfig, BatchSubmissionRunner, RpcBlockSource,
};
use montana_brotli::BrotliCompressor;
use montana_derivation_runner::{DerivationConfig, DerivationRunner};
use montana_pipeline::{
    CompressedBatch, Compressor, L1BatchSource, NoopExecutor, SourceError as PipelineSourceError,
    SubmissionReceipt,
};
use montana_tui_common::{format_bytes, truncate_url};
use montana_zlib::ZlibCompressor;
use montana_zstd::ZstdCompressor;
use ratatui::{
    DefaultTerminal,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::sync::Mutex;

mod app;

use app::{App, LogEntry, LogLevel};
use montana_batch_context::{Address, BatchContext, BatchSink, BatchSubmissionMode};
use montana_batch_runner::BlockSource;

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

    /// Batch submission mode (in-memory, anvil, remote).
    ///
    /// - anvil (default): Spawns a local Anvil chain and submits batches as transactions
    /// - in-memory: Passes batches directly between tasks (fast, no chain simulation)
    /// - remote: Submit to a remote L1 chain (currently unsupported)
    #[arg(long, default_value = "anvil", value_enum)]
    pub(crate) submission_mode: BatchSubmissionMode,

    /// Batch inbox address for batch submission (hex string, e.g., 0x4242...4242).
    ///
    /// This is the address where batches are sent to on the L1/Anvil chain.
    /// Both batch submission and derivation use this address.
    #[arg(long, default_value = "0x4242424242424242424242424242424242424242")]
    pub(crate) batch_inbox: String,
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
    // Create tokio runtime
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    // Parse batch inbox address
    let batch_inbox = parse_address(&args.batch_inbox).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Invalid batch inbox address '{}': {}", args.batch_inbox, e),
        )
    })?;

    // Create batch context (spawns Anvil if needed)
    let batch_context =
        rt.block_on(async { BatchContext::new(args.submission_mode, batch_inbox).await });

    let batch_context = match batch_context {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Error: Failed to create batch context: {}", e);
            return Err(io::Error::other(e.to_string()));
        }
    };

    let anvil_endpoint = batch_context.anvil_endpoint();
    let submission_mode = batch_context.mode();

    // Create shared app state
    let app = Arc::new(Mutex::new(App::new(
        args.rpc.clone(),
        args.compression.clone(),
        submission_mode,
        anvil_endpoint,
    )));

    // Wrap the batch context in an Arc for sharing
    let batch_context = Arc::new(batch_context);

    // Get compressor name
    let compressor_name = args.compression.clone();

    // Build configurations
    let batch_config = BatchSubmissionConfig::builder()
        .poll_interval_ms(args.poll_interval)
        .max_blocks_per_batch(args.max_blocks_per_batch)
        .target_batch_size(args.target_batch_size)
        .build();

    let derivation_config = DerivationConfig::builder().poll_interval_ms(50).build();

    // Get starting block
    let start_block = match args.start {
        Some(start) => start,
        None => rt.block_on(async {
            let source = RpcBlockSource::new(args.rpc.clone());
            match source.get_head().await {
                Ok(head) => {
                    let mut app_guard = app.lock().await;
                    app_guard
                        .log_batch(LogEntry::info(format!("Connected! Chain head at #{}", head)));
                    app_guard.set_chain_head(head);
                    head
                }
                Err(e) => {
                    let mut app_guard = app.lock().await;
                    app_guard.log_batch(LogEntry::error(format!("Failed to connect: {}", e)));
                    0
                }
            }
        }),
    };

    // Dispatch to the appropriate compressor-specific runner function
    // This avoids generic type issues by handling each compressor type separately
    match compressor_name.to_lowercase().as_str() {
        "zlib" => run_with_compressor(
            &mut terminal,
            &rt,
            app,
            batch_context,
            args.rpc,
            batch_config,
            derivation_config,
            start_block,
            ZlibCompressor::balanced(),
        ),
        "zstd" => run_with_compressor(
            &mut terminal,
            &rt,
            app,
            batch_context,
            args.rpc,
            batch_config,
            derivation_config,
            start_block,
            ZstdCompressor::balanced(),
        ),
        _ => run_with_compressor(
            &mut terminal,
            &rt,
            app,
            batch_context,
            args.rpc,
            batch_config,
            derivation_config,
            start_block,
            BrotliCompressor::balanced(),
        ),
    }
}

/// Run the TUI with a specific compressor type.
fn run_with_compressor<C: Compressor + Clone + Send + Sync + 'static>(
    terminal: &mut DefaultTerminal,
    rt: &tokio::runtime::Runtime,
    app: Arc<Mutex<App>>,
    batch_context: Arc<BatchContext>,
    rpc_url: String,
    batch_config: BatchSubmissionConfig,
    derivation_config: DerivationConfig,
    start_block: u64,
    compressor: C,
) -> io::Result<()> {
    // Create batch submission callback
    let callback_app = Arc::clone(&app);
    let callback = TuiCallback::new(callback_app);

    // Create batch submission runner
    let block_source = RpcBlockSource::new(rpc_url);
    let sink_wrapper = BatchSinkWrapper::new(batch_context.sink_arc());
    let batch_runner =
        BatchSubmissionRunner::new(block_source, compressor.clone(), sink_wrapper, batch_config)
            .with_callback(callback);
    let batch_runner = Arc::new(Mutex::new(batch_runner));

    // Create derivation runner
    let source_adapter = BatchSourceAdapter::new(Arc::clone(&batch_context));
    let executor = NoopExecutor::new();
    let derivation_runner =
        DerivationRunner::new(source_adapter, compressor, executor, derivation_config);
    let derivation_runner = Arc::new(Mutex::new(derivation_runner));

    // Spawn batch submission task
    let batch_task_runner = Arc::clone(&batch_runner);
    let batch_task_app = Arc::clone(&app);
    rt.spawn(async move {
        let mut runner = batch_task_runner.lock().await;
        if let Err(e) = runner.run(start_block).await {
            let mut app_guard = batch_task_app.lock().await;
            app_guard.log_batch(LogEntry::error(format!("Batch runner error: {}", e)));
        }
    });

    // Spawn derivation task
    let derivation_task_runner = Arc::clone(&derivation_runner);
    let derivation_task_app = Arc::clone(&app);
    rt.spawn(async move {
        loop {
            let result = {
                let mut runner = derivation_task_runner.lock().await;
                runner.tick().await
            };

            match result {
                Ok(Some(metrics)) => {
                    let batch_number = metrics.batches_derived.saturating_sub(1);
                    let mut app_guard = derivation_task_app.lock().await;
                    app_guard.stats.batches_derived = metrics.batches_derived;
                    app_guard.stats.blocks_derived = metrics.blocks_derived;
                    app_guard.stats.bytes_decompressed = metrics.bytes_decompressed as usize;
                    app_guard.stats.derivation_healthy = true;

                    // Calculate latency if available
                    if let Some(submit_time) =
                        app_guard.batch_submission_times.remove(&batch_number)
                    {
                        let latency = std::time::Instant::now().duration_since(submit_time);
                        app_guard.stats.record_latency(latency.as_millis() as u64);
                    }

                    app_guard.log_derivation(LogEntry::info(format!(
                        "Derived batch #{}: decompressed to {} bytes",
                        batch_number, metrics.bytes_decompressed
                    )));
                }
                Ok(None) => {
                    // No batch available, continue
                }
                Err(e) => {
                    let mut app_guard = derivation_task_app.lock().await;
                    app_guard.stats.derivation_healthy = false;
                    app_guard.log_derivation(LogEntry::error(format!("Derivation error: {}", e)));
                }
            }
        }
    });

    // Main UI loop
    loop {
        // Draw UI
        let app_guard = rt.block_on(app.lock());
        terminal.draw(|frame| draw_ui(frame, &app_guard))?;
        drop(app_guard);

        // Handle input (non-blocking)
        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
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

                    // Update runners
                    let paused = app_guard.is_paused;
                    drop(app_guard);

                    let mut batch = rt.block_on(batch_runner.lock());
                    let mut derivation = rt.block_on(derivation_runner.lock());

                    if paused {
                        batch.pause();
                        derivation.pause();
                    } else {
                        batch.resume();
                        derivation.resume();
                    }
                }
                _ => {}
            }
        }
    }
}

/// Callback implementation for TUI updates.
#[derive(derive_more::Deref)]
struct TuiCallback {
    #[deref]
    app: Arc<Mutex<App>>,
}

impl TuiCallback {
    fn new(app: Arc<Mutex<App>>) -> Self {
        Self { app }
    }
}

#[async_trait::async_trait]
impl BatchSubmissionCallback for TuiCallback {
    async fn on_batch_submitted(
        &self,
        batch_number: u64,
        blocks_count: usize,
        original_size: usize,
        compressed_size: usize,
        tx_hash: [u8; 32],
    ) {
        let mut app_guard = self.app.lock().await;
        app_guard.stats.batches_submitted += 1;
        app_guard.stats.blocks_processed += blocks_count as u64;
        app_guard.stats.bytes_original += original_size;
        app_guard.stats.bytes_compressed += compressed_size;
        app_guard.stats.compression_ratio = if app_guard.stats.bytes_original > 0 {
            app_guard.stats.bytes_compressed as f64 / app_guard.stats.bytes_original as f64
        } else {
            1.0
        };

        // Record submission timestamp for latency calculation
        app_guard.batch_submission_times.insert(batch_number, std::time::Instant::now());

        // Log success with tx hash if available
        let has_tx = tx_hash != [0u8; 32];
        let ratio =
            if original_size > 0 { compressed_size as f64 / original_size as f64 } else { 1.0 };
        let msg = if has_tx {
            format!(
                "Batch #{}: {} blocks, {} -> {} bytes ({:.1}%) tx:0x{:02x}{:02x}{:02x}{:02x}...",
                batch_number,
                blocks_count,
                original_size,
                compressed_size,
                ratio * 100.0,
                tx_hash[0],
                tx_hash[1],
                tx_hash[2],
                tx_hash[3]
            )
        } else {
            format!(
                "Batch #{}: {} blocks, {} -> {} bytes ({:.1}%)",
                batch_number,
                blocks_count,
                original_size,
                compressed_size,
                ratio * 100.0
            )
        };
        app_guard.log_batch(LogEntry::info(msg));
    }

    async fn on_batch_failed(
        &self,
        batch_number: u64,
        error: &montana_batch_runner::BatchSubmissionError,
    ) {
        let mut app_guard = self.app.lock().await;
        app_guard.log_batch(LogEntry::error(format!(
            "Batch #{} submit failed: {}",
            batch_number, error
        )));
    }

    async fn on_block_processed(&self, block_number: u64, tx_count: usize, size: usize) {
        let mut app_guard = self.app.lock().await;
        app_guard.set_current_block(block_number);
        app_guard.log_batch(LogEntry::info(format!(
            "Block #{}: {} txs, {} bytes",
            block_number, tx_count, size
        )));
    }

    async fn on_chain_head_updated(&self, head: u64) {
        let mut app_guard = self.app.lock().await;
        app_guard.set_chain_head(head);
    }
}

/// Adapter that wraps BatchContext source to implement L1BatchSource.
#[derive(derive_more::Deref)]
struct BatchSourceAdapter {
    #[deref]
    context: Arc<BatchContext>,
}

impl BatchSourceAdapter {
    fn new(context: Arc<BatchContext>) -> Self {
        Self { context }
    }
}

#[async_trait::async_trait]
impl L1BatchSource for BatchSourceAdapter {
    async fn next_batch(&mut self) -> Result<Option<CompressedBatch>, PipelineSourceError> {
        self.context
            .source()
            .next_batch()
            .await
            .map_err(|e| PipelineSourceError::Connection(e.to_string()))
    }

    async fn l1_head(&self) -> Result<u64, PipelineSourceError> {
        Ok(0)
    }
}

/// Wrapper that adapts BatchSink to montana_pipeline::BatchSink.
#[derive(derive_more::Deref)]
struct BatchSinkWrapper {
    #[deref]
    inner: Arc<Box<dyn BatchSink>>,
}

impl BatchSinkWrapper {
    fn new(inner: Arc<Box<dyn BatchSink>>) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl montana_pipeline::BatchSink for BatchSinkWrapper {
    async fn submit(
        &mut self,
        batch: CompressedBatch,
    ) -> Result<SubmissionReceipt, montana_pipeline::SinkError> {
        self.inner
            .submit(batch)
            .await
            .map_err(|e| montana_pipeline::SinkError::TxFailed(e.to_string()))
    }

    async fn capacity(&self) -> Result<usize, montana_pipeline::SinkError> {
        Ok(128 * 1024) // 128 KB default capacity
    }

    async fn health_check(&self) -> Result<(), montana_pipeline::SinkError> {
        Ok(())
    }
}

/// Draw the TUI layout.
fn draw_ui(frame: &mut ratatui::Frame<'_>, app: &App) {
    let area = frame.area();

    // Main layout: header (1/4 height), body (flexible), and footer (1 line)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 4), Constraint::Min(5), Constraint::Length(3)])
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

    // Draw footer with DA provider info
    draw_footer(frame, app, main_chunks[2]);
}

/// Draw the header with stats and metrics.
fn draw_header(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let stats = &app.stats;

    // Split header into left (stats) and right (timing metrics) sections
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    // Left side: existing stats
    let left_text = vec![
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
            Span::raw("  |  "),
            Span::raw(format!("Mode: {}", app.submission_mode)),
        ]),
    ];

    let left_header = Paragraph::new(left_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Stats ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(left_header, header_chunks[0]);

    // Right side: timing metrics
    let avg_latency = stats.avg_latency_ms();
    let stddev = stats.latency_stddev_ms();
    let sample_count = stats.derivation_latencies_ms.len();

    let right_text = vec![
        Line::from(vec![Span::styled(
            "Derivation Latency",
            Style::default().fg(Color::Yellow).bold(),
        )]),
        Line::from(vec![]),
        Line::from(vec![
            Span::styled("Avg: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                avg_latency.map_or_else(|| "--".to_string(), |v| format!("{:.1}ms", v)),
                Style::default().fg(Color::White).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("Std Dev: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                stddev.map_or_else(|| "--".to_string(), |v| format!("{:.1}ms", v)),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Samples: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", sample_count), Style::default().fg(Color::White)),
        ]),
    ];

    let right_header = Paragraph::new(right_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Timing Metrics ")
                .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(right_header, header_chunks[1]);
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

/// Draw the footer with DA provider and sink/source info.
fn draw_footer(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let stats = &app.stats;

    // Build DA provider info
    let (da_icon, da_name, da_color) = match app.submission_mode {
        BatchSubmissionMode::InMemory => ("MEM", "In-Memory", Color::Cyan),
        BatchSubmissionMode::Anvil => ("ANV", "Anvil", Color::Green),
        BatchSubmissionMode::Remote => ("RPC", "Remote", Color::Yellow),
    };

    // Build endpoint info for Anvil mode
    let endpoint_info = app
        .anvil_endpoint
        .as_ref()
        .map_or_else(String::new, |endpoint| format!(" @ {}", truncate_url(endpoint, 25)));

    let footer_text = Line::from(vec![
        Span::styled(" DA Provider: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("[{}] ", da_icon), Style::default().fg(da_color).bold()),
        Span::styled(da_name, Style::default().fg(da_color)),
        Span::styled(endpoint_info, Style::default().fg(Color::DarkGray)),
        Span::raw("  |  "),
        Span::styled("Sink: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} batches", stats.batches_submitted),
            Style::default().fg(Color::Blue),
        ),
        Span::raw("  |  "),
        Span::styled("Source: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} batches", stats.batches_derived),
            Style::default().fg(Color::Magenta),
        ),
        Span::raw("  |  "),
        Span::styled(
            if stats.batches_submitted == stats.batches_derived { "IN SYNC" } else { "SYNCING" },
            Style::default().fg(if stats.batches_submitted == stats.batches_derived {
                Color::Green
            } else {
                Color::Yellow
            }),
        ),
    ]);

    let footer = Paragraph::new(footer_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Data Availability ")
            .title_style(Style::default().fg(Color::Gray)),
    );

    frame.render_widget(footer, area);
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

/// Parse a hex address string into an Address.
fn parse_address(s: &str) -> Result<Address, String> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() != 40 {
        return Err(format!("Expected 40 hex characters, got {}", s.len()));
    }
    let bytes: [u8; 20] = hex_decode(s)?;
    Ok(Address::from(bytes))
}

/// Decode a hex string into bytes.
fn hex_decode(s: &str) -> Result<[u8; 20], String> {
    let mut bytes = [0u8; 20];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        if i >= 20 {
            return Err("Too many bytes".to_string());
        }
        let high = hex_char_to_nibble(chunk[0])?;
        let low = hex_char_to_nibble(chunk[1])?;
        bytes[i] = (high << 4) | low;
    }
    Ok(bytes)
}

/// Convert a hex character to its nibble value.
fn hex_char_to_nibble(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("Invalid hex character: {}", c as char)),
    }
}
