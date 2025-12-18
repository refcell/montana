use std::{io, time::Duration};

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
use tokio::sync::mpsc;
use montana_tui_common::{LogEntry, format_bytes, render_logs_reversed};

use crate::{App, SyncState, TuiEvent, TuiHandle};

/// The main Montana TUI.
///
/// This struct manages the TUI event loop, rendering the 4-pane layout and
/// processing events from the Montana binary. It uses the Ratatui library
/// for terminal rendering and Crossterm for terminal control.
///
/// The TUI displays:
/// - **Top-left**: Transaction pool activity
/// - **Top-right**: Block builder and batch submission
/// - **Bottom-left**: Batch status and compression metrics
/// - **Bottom-right**: Block derivation and execution timing
///
/// # Example
///
/// ```rust,ignore
/// use montana_tui::create_tui;
///
/// // Create the TUI and handle
/// let (tui, handle) = create_tui();
///
/// // Run the TUI (blocks until user quits with 'q')
/// tui.run()?;
/// ```
#[derive(Debug)]
pub struct MontanaTui {
    /// Receiver for TUI events
    #[allow(dead_code)] // Will be used in full TUI implementation
    event_rx: mpsc::UnboundedReceiver<TuiEvent>,
}

impl MontanaTui {
    /// Create a new Montana TUI with an event receiver.
    ///
    /// This is typically called internally by [`create_tui`]. Users should
    /// use [`create_tui`] instead to get both the TUI and its handle.
    ///
    /// # Arguments
    ///
    /// * `event_rx` - Unbounded receiver for TUI events
    pub const fn new(event_rx: mpsc::UnboundedReceiver<TuiEvent>) -> Self {
        Self { event_rx }
    }

    /// Run the TUI - this blocks until the user quits.
    ///
    /// This method enters the main TUI event loop, which:
    /// 1. Sets up the terminal in raw mode
    /// 2. Renders the 4-pane layout
    /// 3. Processes keyboard input and TUI events
    /// 4. Updates the display in real-time
    /// 5. Cleans up and restores the terminal on exit
    ///
    /// The TUI can be exited by pressing 'q'.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal setup or rendering fails.
    pub fn run(mut self) -> io::Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let mut terminal = ratatui::init();

        // Create app state
        let mut app = App::new();

        // Run the main event loop
        let result = self.event_loop(&mut terminal, &mut app);

        // Cleanup terminal
        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;
        ratatui::restore();

        result
    }

    /// Main event loop - polls for keyboard events and TUI events.
    fn event_loop(&mut self, terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
        loop {
            // Draw the UI
            terminal.draw(|frame| draw_ui(frame, app))?;

            // Poll for keyboard events (100ms timeout)
            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        return Ok(());
                    }
                    KeyCode::Char('p') => {
                        app.toggle_pause();
                    }
                    KeyCode::Char('r') => {
                        app.reset();
                    }
                    _ => {}
                }
            }

            // Process TUI events from the channel (non-blocking)
            while let Ok(event) = self.event_rx.try_recv() {
                if !app.is_paused {
                    process_event(app, event);
                }
            }
        }
    }
}

/// Create a TUI and its handle for sending events.
///
/// This function creates an unbounded channel for TUI events and returns
/// both the TUI (which receives events) and a handle (which sends events).
///
/// The handle can be cloned and passed to different parts of the Montana
/// binary to send events to the TUI from multiple locations.
///
/// # Returns
///
/// A tuple of `(MontanaTui, TuiHandle)`:
/// - `MontanaTui` - The TUI instance that should be run with [`MontanaTui::run`]
/// - `TuiHandle` - A handle for sending events to the TUI
///
/// # Example
///
/// ```rust,ignore
/// use montana_tui::{create_tui, TuiEvent};
///
/// // Create the TUI and handle
/// let (tui, handle) = create_tui();
///
/// // Clone the handle to use in multiple places
/// let handle_clone = handle.clone();
///
/// // Spawn a thread to run the TUI
/// std::thread::spawn(move || {
///     tui.run().expect("TUI failed");
/// });
///
/// // Send events from the main thread
/// handle.send(TuiEvent::UnsafeHeadUpdated(100));
/// handle_clone.send(TuiEvent::SafeHeadUpdated(90));
/// ```
pub fn create_tui() -> (MontanaTui, TuiHandle) {
    let (tx, rx) = mpsc::unbounded_channel();
    (MontanaTui::new(rx), TuiHandle::new(tx))
}

/// Format a duration nicely for display.
fn format_duration(d: std::time::Duration) -> String {
    let total_secs = d.as_secs();
    if total_secs < 60 {
        format!("{}s", total_secs)
    } else if total_secs < 3600 {
        format!("{}m {}s", total_secs / 60, total_secs % 60)
    } else {
        format!("{}h {}m", total_secs / 3600, (total_secs % 3600) / 60)
    }
}

/// Process a TUI event and update the app state.
fn process_event(app: &mut App, event: TuiEvent) {
    match event {
        TuiEvent::SyncStarted { start_block, target_block } => {
            app.start_sync(start_block, target_block);
            app.log_sync(LogEntry::info(format!(
                "Sync started: #{} -> #{} ({} blocks)",
                start_block,
                target_block,
                target_block.saturating_sub(start_block)
            )));
        }
        TuiEvent::SyncProgress { current_block, target_block, blocks_per_second, eta } => {
            app.update_sync_progress(current_block, target_block, blocks_per_second, eta);
            let eta_str = eta.map_or_else(|| "calculating...".to_string(), format_duration);
            let remaining = target_block.saturating_sub(current_block);
            app.log_sync(LogEntry::info(format!(
                "Block #{}: {:.1} blks/s, {} remaining, ETA {}",
                current_block, blocks_per_second, remaining, eta_str
            )));
            // In harness mode, log to harness panel only for blocks after init
            // (blocks during init were already logged via HarnessInitProgress)
            if app.harness_mode
                && app.is_after_harness_init(current_block)
                && current_block > app.harness_block
            {
                app.record_harness_block(current_block, 0);
            }
        }
        TuiEvent::SyncCompleted { blocks_synced, duration_secs } => {
            app.complete_sync(blocks_synced, duration_secs);
            let speed =
                if duration_secs > 0.0 { blocks_synced as f64 / duration_secs } else { 0.0 };
            app.log_sync(LogEntry::info(format!(
                "SYNC COMPLETE: {} blocks in {:.1}s ({:.1} blks/s)",
                blocks_synced, duration_secs, speed
            )));
        }
        TuiEvent::Transaction { .. } => {
            // Transaction events are no longer displayed in the TUI
            // The sync panel replaced the transaction pool panel
        }
        TuiEvent::BlockBuilt { number, tx_count, size_bytes, gas_used } => {
            app.set_unsafe_head(number);
            app.log_block(LogEntry::info(format!(
                "Block #{}: {} txs, {} bytes, {} gas",
                number,
                tx_count,
                format_bytes(size_bytes),
                gas_used
            )));
            // In harness mode, log to harness panel only for blocks after init
            // (blocks during init were already logged via HarnessInitProgress)
            if app.harness_mode && app.is_after_harness_init(number) {
                app.record_harness_block(number, tx_count);
            }
        }
        TuiEvent::BatchSubmitted {
            batch_number,
            block_count,
            first_block,
            last_block,
            uncompressed_size,
            compressed_size,
        } => {
            app.record_batch_submit(batch_number);
            app.set_safe_head(last_block);
            let ratio = if uncompressed_size > 0 {
                (compressed_size as f64 / uncompressed_size as f64) * 100.0
            } else {
                100.0
            };
            // Track compression ratio for stats
            app.record_compression_ratio(ratio);
            app.log_batch(LogEntry::info(format!(
                "Batch #{}: blocks {}-{} ({} blocks), {} -> {} ({:.1}%)",
                batch_number,
                first_block,
                last_block,
                block_count,
                format_bytes(uncompressed_size),
                format_bytes(compressed_size),
                ratio
            )));
        }
        TuiEvent::BlockDerived { number, tx_count, derivation_time_ms, execution_time_ms } => {
            app.set_finalized_head(number);
            // Log execution to the derivation execution logs section
            app.log_derivation_execution(LogEntry::info(format!(
                "Block #{}: {} txs, {}ms",
                number, tx_count, execution_time_ms
            )));
            // Log derivation time only to the derivation logs section
            app.log_derivation(LogEntry::info(format!(
                "Block #{}: derived in {}ms",
                number, derivation_time_ms
            )));
        }
        TuiEvent::BatchDerived { batch_number, block_count, first_block, last_block } => {
            // Record the batch derivation (increments counter and calculates latency)
            app.record_batch_derived(batch_number);
            app.set_finalized_head(last_block);
            app.log_derivation(LogEntry::info(format!(
                "Batch #{}: derived {} blocks ({}-{})",
                batch_number, block_count, first_block, last_block
            )));
        }
        TuiEvent::UnsafeHeadUpdated(head) => {
            app.set_unsafe_head(head);
        }
        TuiEvent::SafeHeadUpdated(head) => {
            app.set_safe_head(head);
        }
        TuiEvent::FinalizedUpdated(head) => {
            app.set_finalized_head(head);
        }
        TuiEvent::ModeInfo { node_role, start_block, skip_sync } => {
            app.set_mode_info(node_role, start_block, skip_sync);
        }
        TuiEvent::BlockExecuted { block_number, execution_time_ms } => {
            app.record_block_executed(block_number, execution_time_ms);
            let backlog = app.backlog_size();
            let rate = app.execution_rate();
            app.log_execution(LogEntry::info(format!(
                "Block #{}: {}ms, {:.1} blks/s, {} in backlog",
                block_number, execution_time_ms, rate, backlog
            )));
        }
        TuiEvent::BacklogUpdated { blocks_fetched, last_fetched_block } => {
            app.update_backlog(blocks_fetched, last_fetched_block);
        }
        TuiEvent::TogglePause => {
            app.toggle_pause();
        }
        TuiEvent::Reset => {
            app.reset();
        }
        TuiEvent::HarnessModeEnabled => {
            app.harness_mode = true;
        }
        TuiEvent::HarnessBlockProduced { block_number, tx_count } => {
            app.record_harness_block(block_number, tx_count);
        }
        TuiEvent::HarnessInitProgress { current_block, total_blocks, message } => {
            // Log progress to the harness panel
            let pct = if total_blocks > 0 {
                (current_block as f64 / total_blocks as f64) * 100.0
            } else {
                0.0
            };
            app.log_harness(LogEntry::info(format!(
                "[{}/{}] {:.0}% - {}",
                current_block, total_blocks, pct, message
            )));
            // Also update harness block counter so the panel title updates
            app.harness_block = current_block;
        }
        TuiEvent::HarnessInitComplete { final_block } => {
            // Mark initialization complete - blocks <= final_block won't be re-logged
            app.complete_harness_init(final_block);
        }
        TuiEvent::SequencerInit {
            checkpoint_batch,
            min_batch_size,
            batch_interval_secs,
            max_blocks_per_batch,
        } => {
            app.log_batch(LogEntry::info(format!(
                "Sequencer initialized: checkpoint batch #{}",
                checkpoint_batch
            )));
            app.log_batch(LogEntry::info(format!(
                "Config: min_size={} bytes, interval={}s, max_blocks={}",
                min_batch_size, batch_interval_secs, max_blocks_per_batch
            )));
            if app.harness_mode {
                app.log_batch(LogEntry::info("Waiting for chain initialization...".to_string()));
            }
        }
        TuiEvent::ValidatorInit { checkpoint_batch } => {
            app.log_derivation(LogEntry::info(format!(
                "Validator initialized: checkpoint batch #{}",
                checkpoint_batch
            )));
            if app.harness_mode {
                app.log_derivation(LogEntry::info(
                    "Waiting for chain initialization...".to_string(),
                ));
            }
        }
        TuiEvent::ExecutionInit { start_block, checkpoint_block, harness_mode } => {
            app.log_execution(LogEntry::info(format!(
                "Execution initialized: start block #{}, checkpoint block #{}",
                start_block, checkpoint_block
            )));
            if harness_mode {
                app.log_execution(LogEntry::info(
                    "Waiting for chain initialization...".to_string(),
                ));
            }
        }
        TuiEvent::BlockBuilderInit { rpc_url, poll_interval_ms } => {
            // Truncate RPC URL for display
            let display_url =
                if rpc_url.len() > 40 { format!("{}...", &rpc_url[..37]) } else { rpc_url };
            app.log_block(LogEntry::info(format!(
                "Block builder: {} (poll {}ms)",
                display_url, poll_interval_ms
            )));
            if app.harness_mode {
                app.log_block(LogEntry::info("Waiting for chain initialization...".to_string()));
            }
        }
        TuiEvent::WaitingForChain { service } => match service.as_str() {
            "sequencer" | "batch" => {
                app.log_batch(LogEntry::info("Waiting for blocks...".to_string()));
            }
            "validator" | "derivation" => {
                app.log_derivation(LogEntry::info("Waiting for batches...".to_string()));
            }
            "execution" => {
                app.log_execution(LogEntry::info("Waiting for blocks...".to_string()));
            }
            "block_builder" => {
                app.log_block(LogEntry::info("Waiting for blocks...".to_string()));
            }
            _ => {}
        },
    }
}

/// Draw the main UI layout.
fn draw_ui(frame: &mut ratatui::Frame<'_>, app: &App) {
    let area = frame.area();

    // If harness mode is enabled, add a banner at the top
    if app.harness_mode {
        // Layout with harness banner (1 line), header (1/4), body (flexible), footer (3 lines)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Ratio(1, 4),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        // Draw harness banner
        draw_harness_banner(frame, main_chunks[0]);

        // Draw header (split into chain state + metrics)
        draw_header(frame, app, main_chunks[1]);

        // Draw body (4 equal columns)
        draw_body(frame, app, main_chunks[2]);

        // Draw footer
        draw_footer(frame, app, main_chunks[3]);
    } else {
        // Main layout: header (1/4 height), body (flexible), footer (3 lines)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Ratio(1, 4), Constraint::Min(5), Constraint::Length(3)])
            .split(area);

        // Draw header (split into chain state + metrics)
        draw_header(frame, app, main_chunks[0]);

        // Draw body (4 equal columns)
        draw_body(frame, app, main_chunks[1]);

        // Draw footer
        draw_footer(frame, app, main_chunks[2]);
    }
}

/// Draw the harness mode banner.
fn draw_harness_banner(frame: &mut ratatui::Frame<'_>, area: Rect) {
    let banner = Paragraph::new(" ⚠ HARNESS MODE (TESTING) - Local anvil chain, not production ⚠ ")
        .style(Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(banner, area);
}

/// Draw the header with chain state, sync stats, metrics, and execution stats.
fn draw_header(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Split header into chain state (25%), sync stats (25%), metrics (25%), execution (25%)
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    draw_chain_state(frame, app, header_chunks[0]);
    draw_sync_stats(frame, app, header_chunks[1]);
    draw_metrics(frame, app, header_chunks[2]);
    draw_execution_stats(frame, app, header_chunks[3]);
}

/// Draw chain state panel (unsafe/safe/finalized heads).
fn draw_chain_state(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let text = vec![
        Line::from(vec![
            Span::styled("Unsafe Head:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("#{}", app.unsafe_head), Style::default().fg(Color::White).bold()),
        ]),
        Line::from(vec![
            Span::styled("Safe Head:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("#{}", app.safe_head), Style::default().fg(Color::Green).bold()),
        ]),
        Line::from(vec![
            Span::styled("Finalized:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("#{}", app.finalized_head),
                Style::default().fg(Color::Blue).bold(),
            ),
        ]),
    ];

    let widget = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Chain State ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

/// Draw sync stats panel with progress, speed, and completion status.
fn draw_sync_stats(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Determine sync status and build content
    let (status_line, details) = match app.sync_state {
        SyncState::Idle => {
            if app.skip_sync {
                // Sync was skipped via --skip-sync flag
                let status = Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                    Span::styled("SKIPPED", Style::default().fg(Color::Magenta).bold()),
                ]);
                let details = vec![
                    Line::from(""),
                    Line::from(vec![Span::styled(
                        "Sync skipped via --skip-sync",
                        Style::default().fg(Color::Magenta),
                    )]),
                ];
                (status, details)
            } else {
                let status = Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                    Span::styled("IDLE", Style::default().fg(Color::DarkGray)),
                ]);
                let details = vec![
                    Line::from(""),
                    Line::from(vec![Span::styled(
                        "Waiting for sync...",
                        Style::default().fg(Color::DarkGray),
                    )]),
                ];
                (status, details)
            }
        }
        SyncState::Syncing { current_block, target_block, blocks_per_second, eta, .. } => {
            let progress = app.sync_progress_percent();
            let remaining = target_block.saturating_sub(current_block);
            let eta_str = eta.map_or_else(|| "calculating...".to_string(), format_duration);

            let status = Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("SYNCING {:.1}%", progress),
                    Style::default().fg(Color::Yellow).bold(),
                ),
            ]);

            let details = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("Block:   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("#{}", current_block), Style::default().fg(Color::White)),
                    Span::styled(
                        format!(" / #{}", target_block),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Speed:   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{:.1} blks/s", blocks_per_second),
                        Style::default().fg(Color::Cyan),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("ETA:     ", Style::default().fg(Color::DarkGray)),
                    Span::styled(eta_str, Style::default().fg(Color::White)),
                    Span::styled(
                        format!(" ({} left)", remaining),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
            ];
            (status, details)
        }
        SyncState::Completed { blocks_synced, duration_secs } => {
            let speed =
                if duration_secs > 0.0 { blocks_synced as f64 / duration_secs } else { 0.0 };

            let status = Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                Span::styled("SYNC COMPLETE ✓", Style::default().fg(Color::Green).bold()),
            ]);

            let details = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("Synced:  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{} blocks", blocks_synced),
                        Style::default().fg(Color::Green),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Time:    ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{:.1}s", duration_secs),
                        Style::default().fg(Color::White),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Avg:     ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:.1} blks/s", speed), Style::default().fg(Color::Cyan)),
                ]),
            ];
            (status, details)
        }
    };

    // Build the full text content
    let mut text = vec![status_line];
    text.extend(details);

    // Add syncs completed counter
    if app.syncs_completed > 0 {
        text.push(Line::from(vec![
            Span::styled("Syncs:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} completed", app.syncs_completed),
                Style::default().fg(Color::Magenta),
            ),
        ]));
    }

    let title_color = match app.sync_state {
        SyncState::Completed { .. } => Color::Green,
        SyncState::Syncing { .. } => Color::Yellow,
        SyncState::Idle => Color::Magenta,
    };

    let widget = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Sync Stats ")
                .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

/// Draw performance metrics panel.
fn draw_metrics(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let avg_latency = app.avg_latency_ms();
    let stddev = app.latency_stddev_ms();
    let blocks_per_sec = if avg_latency > 0.0 { 1000.0 / avg_latency } else { 0.0 };
    let avg_compression = app.avg_compression_ratio();
    let compression_stddev = app.compression_stddev();

    let sync_status = if app.unsafe_head == app.finalized_head && app.unsafe_head > 0 {
        Span::styled("IN SYNC ✓", Style::default().fg(Color::Green).bold())
    } else if app.unsafe_head > 0 {
        Span::styled(
            format!("SYNCING ({} behind)", app.unsafe_head.saturating_sub(app.finalized_head)),
            Style::default().fg(Color::Yellow),
        )
    } else {
        Span::styled("WAITING...", Style::default().fg(Color::DarkGray))
    };

    let text = vec![
        Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
            sync_status,
        ]),
        Line::from(vec![
            Span::styled("Round-trip:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if avg_latency > 0.0 {
                    format!("{:.1}ms avg", avg_latency)
                } else {
                    "--".to_string()
                },
                Style::default().fg(Color::White),
            ),
            Span::styled(
                if stddev > 0.0 { format!(" (±{:.1}ms)", stddev) } else { String::new() },
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::styled("Blocks/s:       ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if blocks_per_sec > 0.0 {
                    format!("{:.2}", blocks_per_sec)
                } else {
                    "--".to_string()
                },
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Compression:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if avg_compression > 0.0 {
                    format!("{:.1}% avg", avg_compression)
                } else {
                    "--".to_string()
                },
                Style::default().fg(Color::White),
            ),
            Span::styled(
                if compression_stddev > 0.0 {
                    format!(" (±{:.1}%)", compression_stddev)
                } else {
                    String::new()
                },
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::styled("Batches:        ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", app.batches_submitted), Style::default().fg(Color::Blue)),
            Span::raw(" submitted, "),
            Span::styled(format!("{}", app.batches_derived), Style::default().fg(Color::Magenta)),
            Span::raw(" derived"),
        ]),
    ];

    let widget = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Performance Metrics ")
                .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

/// Draw execution stats panel in the header.
fn draw_execution_stats(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let backlog = app.backlog_size();
    let rate = app.execution_rate();
    let avg_time = app.avg_execution_time_ms();

    // Determine execution status
    let (status_text, status_color) = if app.blocks_executed == 0 && app.blocks_fetched == 0 {
        ("WAITING", Color::DarkGray)
    } else if app.blocks_executed == 0 && app.blocks_fetched > 0 {
        ("STARTING", Color::Yellow)
    } else if let Some(elapsed) = app.time_since_last_execution() {
        if elapsed.as_secs() > 30 {
            ("STALLED", Color::Red)
        } else if elapsed.as_secs() > 5 {
            ("SLOW", Color::Yellow)
        } else {
            ("RUNNING", Color::Green)
        }
    } else {
        ("IDLE", Color::DarkGray)
    };

    let text = vec![
        Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
            Span::styled(status_text, Style::default().fg(status_color).bold()),
        ]),
        Line::from(vec![
            Span::styled("Block:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("#{}", app.last_executed_block),
                Style::default().fg(Color::White).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("Rate:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1} blks/s", rate), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Backlog:", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(" {}", backlog),
                if backlog > 100 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("Avg:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if avg_time > 0.0 { format!("{:.1}ms", avg_time) } else { "--".to_string() },
                Style::default().fg(Color::White),
            ),
        ]),
    ];

    let widget = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Execution Stats ")
                .title_style(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

/// Draw the body with grouped sections: Sync Updates (optional), Sequencer, and Validator.
fn draw_body(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    if app.harness_mode {
        if app.skip_sync {
            // Harness mode with sync skipped: Anvil (20%), gap, Sequencer (39%), gap, Validator (39%)
            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(20),
                    Constraint::Length(1), // gap
                    Constraint::Percentage(39),
                    Constraint::Length(1), // gap
                    Constraint::Percentage(39),
                ])
                .split(area);

            draw_harness_activity(frame, app, body_chunks[0]);
            draw_sequencer_section(frame, app, body_chunks[2]);
            draw_validator_section(frame, app, body_chunks[4]);
        } else {
            // In harness mode with sync: Anvil (16%), Sync (16%), gap, Sequencer (33%), gap, Validator (33%)
            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(16),
                    Constraint::Percentage(16),
                    Constraint::Length(1), // gap
                    Constraint::Percentage(33),
                    Constraint::Length(1), // gap
                    Constraint::Percentage(33),
                ])
                .split(area);

            draw_harness_activity(frame, app, body_chunks[0]);
            draw_sync_updates(frame, app, body_chunks[1]);
            draw_sequencer_section(frame, app, body_chunks[3]);
            draw_validator_section(frame, app, body_chunks[5]);
        }
    } else if app.skip_sync {
        // Sync skipped: Sequencer (49%), gap, Validator (49%)
        let body_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(49),
                Constraint::Length(2), // gap
                Constraint::Percentage(49),
            ])
            .split(area);

        draw_sequencer_section(frame, app, body_chunks[0]);
        draw_validator_section(frame, app, body_chunks[2]);
    } else {
        // Normal mode: Sync Updates (24%), gap, Sequencer (37%), gap, Validator (37%)
        let body_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(24),
                Constraint::Length(1), // gap
                Constraint::Percentage(37),
                Constraint::Length(1), // gap
                Constraint::Percentage(37),
            ])
            .split(area);

        draw_sync_updates(frame, app, body_chunks[0]);
        draw_sequencer_section(frame, app, body_chunks[2]);
        draw_validator_section(frame, app, body_chunks[4]);
    }
}

/// Draw the Sequencer section with a grouping box containing Block Builder, Execution Logs, and Batch Submissions.
fn draw_sequencer_section(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Draw the outer Sequencer box
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .title(" Sequencer ")
        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    let inner_area = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    // Split inner area into two columns: Block Builder (50%) and Execution+Batches (50%)
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner_area);

    draw_block_builder(frame, app, columns[0]);
    draw_execution_and_batches(frame, app, columns[1]);
}

/// Draw the Validator section with a grouping box containing Execution Logs and Derivation.
fn draw_validator_section(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Draw the outer Validator box
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .title(" Validator ")
        .title_style(Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD));
    let inner_area = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    // Draw the derivation content (execution logs on top, derivation logs below)
    draw_derived_blocks(frame, app, inner_area);
}

/// Draw harness activity (only shown in harness mode).
fn draw_harness_activity(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Render logs in reverse order (newest first) since wrap() and scroll() conflict
    let logs = render_logs_reversed(&app.harness_logs);

    let title = format!(" L2 Anvil (Block #{}) ", app.harness_block);

    let widget = Paragraph::new(logs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title(title)
                .title_style(Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

/// Draw sync updates stream.
fn draw_sync_updates(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Render logs in reverse order (newest first) since wrap() and scroll() conflict
    let logs = render_logs_reversed(&app.sync_logs);

    // Determine title color based on sync state
    let title_color = match app.sync_state {
        SyncState::Completed { .. } => Color::Green,
        SyncState::Syncing { .. } => Color::Yellow,
        SyncState::Idle => Color::Cyan,
    };

    let widget = Paragraph::new(logs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Sync Updates ")
                .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

/// Draw block builder logs.
fn draw_block_builder(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Render logs in reverse order (newest first) since wrap() and scroll() conflict
    let logs = render_logs_reversed(&app.block_builder_logs);

    let widget = Paragraph::new(logs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Block Builder ")
                .title_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

/// Draw execution and batch submissions in a vertically split column.
/// Execution section is on top, batch submissions below.
fn draw_execution_and_batches(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Split vertically: execution (top half) and batch submissions (bottom half)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_execution(frame, app, chunks[0]);
    draw_batch_submissions(frame, app, chunks[1]);
}

/// Draw execution section with logs only (stats are now in header).
fn draw_execution(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let backlog = app.backlog_size();

    // Determine title color based on execution state
    let title_color = if app.blocks_executed == 0 && app.blocks_fetched == 0 {
        Color::DarkGray
    } else if let Some(elapsed) = app.time_since_last_execution() {
        if elapsed.as_secs() > 30 {
            Color::Red
        } else if elapsed.as_secs() > 5 || backlog > 100 {
            Color::Yellow
        } else {
            Color::LightGreen
        }
    } else {
        Color::LightGreen
    };

    // Render logs in reverse order (newest first) since wrap() and scroll() conflict
    let lines = render_logs_reversed(&app.execution_logs);

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Execution Logs ")
                .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

/// Draw batch submission logs.
fn draw_batch_submissions(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Render logs in reverse order (newest first) since wrap() and scroll() conflict
    let logs = render_logs_reversed(&app.batch_logs);

    let widget = Paragraph::new(logs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Batch Submissions ")
                .title_style(Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

/// Draw derivation section with execution logs and derivation logs in a vertically split column.
/// Execution logs on top, derivation logs below.
fn draw_derived_blocks(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Split vertically: execution logs (top half) and derivation logs (bottom half)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_derivation_execution(frame, app, chunks[0]);
    draw_derivation_logs(frame, app, chunks[1]);
}

/// Draw derivation execution logs (block execution during derivation).
fn draw_derivation_execution(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Render logs in reverse order (newest first) since wrap() and scroll() conflict
    let logs = render_logs_reversed(&app.derivation_execution_logs);

    let widget = Paragraph::new(logs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Execution Logs ")
                .title_style(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

/// Draw derivation logs (batch derivation info).
fn draw_derivation_logs(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Render logs in reverse order (newest first) since wrap() and scroll() conflict
    let logs = render_logs_reversed(&app.derivation_logs);

    let widget = Paragraph::new(logs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Derived Blocks ")
                .title_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

/// Draw the footer with controls and status.
fn draw_footer(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Build start block display string
    let start_block_display =
        app.start_block.map_or_else(|| "checkpoint".to_string(), |block| format!("#{block}"));

    // Build sync status display
    let sync_display = if app.skip_sync {
        "SKIPPED"
    } else if app.unsafe_head == app.finalized_head && app.unsafe_head > 0 {
        "IN SYNC ✓"
    } else {
        "SYNCING"
    };

    let sync_color = if app.skip_sync {
        Color::Magenta
    } else if app.unsafe_head == app.finalized_head && app.unsafe_head > 0 {
        Color::Green
    } else {
        Color::Yellow
    };

    let footer_line = Line::from(vec![
        Span::styled("[q]", Style::default().fg(Color::Yellow)),
        Span::raw(" Quit  "),
        Span::styled("[p]", Style::default().fg(Color::Yellow)),
        Span::raw(" Pause  "),
        Span::styled("[r]", Style::default().fg(Color::Yellow)),
        Span::raw(" Reset  "),
        Span::raw("  |  "),
        Span::styled("Mode: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&app.node_role, Style::default().fg(Color::Cyan)),
        Span::raw("  |  "),
        Span::styled("Start: ", Style::default().fg(Color::DarkGray)),
        Span::styled(start_block_display, Style::default().fg(Color::Blue)),
        Span::raw("  |  "),
        Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            if app.is_paused { "PAUSED" } else { "RUNNING" },
            Style::default().fg(if app.is_paused { Color::Yellow } else { Color::Green }),
        ),
        Span::raw("  |  "),
        Span::styled("Sync: ", Style::default().fg(Color::DarkGray)),
        Span::styled(sync_display, Style::default().fg(sync_color)),
    ]);

    let widget = Paragraph::new(footer_line).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Montana TUI ")
            .title_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(widget, area);
}
