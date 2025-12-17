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
use montana_tui_common::{LogEntry, format_bytes, render_logs};

use crate::{App, TuiEvent, TuiHandle};

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

/// Process a TUI event and update the app state.
fn process_event(app: &mut App, event: TuiEvent) {
    match event {
        TuiEvent::Transaction { hash, from, to, gas_limit } => {
            let to_str =
                to.map_or_else(|| "Contract Creation".to_string(), |addr| format!("{:?}", addr));
            app.log_tx(LogEntry::info(format!(
                "Tx 0x{:02x}{:02x}...: {} -> {} (gas: {})",
                hash[0], hash[1], from, to_str, gas_limit
            )));
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
        TuiEvent::BlockDerived { number, derivation_time_ms, execution_time_ms } => {
            app.set_finalized_head(number);
            let total_time = derivation_time_ms + execution_time_ms;
            app.log_derivation(LogEntry::info(format!(
                "Block #{}: derived in {}ms + executed in {}ms = {}ms total",
                number, derivation_time_ms, execution_time_ms, total_time
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
        TuiEvent::TogglePause => {
            app.toggle_pause();
        }
        TuiEvent::Reset => {
            app.reset();
        }
    }
}

/// Draw the main UI layout.
fn draw_ui(frame: &mut ratatui::Frame<'_>, app: &App) {
    let area = frame.area();

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

/// Draw the header with chain state and metrics.
fn draw_header(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Split header into chain state (30%) and metrics (70%)
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    draw_chain_state(frame, app, header_chunks[0]);
    draw_metrics(frame, app, header_chunks[1]);
}

/// Draw chain state panel (unsafe/safe/finalized heads).
fn draw_chain_state(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let text = vec![
        Line::from(vec![Span::styled("Chain State", Style::default().fg(Color::Cyan).bold())]),
        Line::from(""),
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

/// Draw performance metrics panel.
fn draw_metrics(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let avg_latency = app.avg_latency_ms();
    let stddev = app.latency_stddev_ms();
    let blocks_per_sec = if avg_latency > 0.0 { 1000.0 / avg_latency } else { 0.0 };

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
            Span::styled("Performance Metrics", Style::default().fg(Color::Yellow).bold()),
            Span::raw("  |  "),
            sync_status,
        ]),
        Line::from(""),
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

/// Draw the body with 4 vertical columns.
fn draw_body(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    // Split into 4 equal columns
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    draw_tx_pool(frame, app, body_chunks[0]);
    draw_block_builder(frame, app, body_chunks[1]);
    draw_batch_submissions(frame, app, body_chunks[2]);
    draw_derived_blocks(frame, app, body_chunks[3]);
}

/// Draw transaction pool logs.
fn draw_tx_pool(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let logs = render_logs(&app.tx_pool_logs);

    let widget = Paragraph::new(logs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Tx Pool ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

/// Draw block builder logs.
fn draw_block_builder(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let logs = render_logs(&app.block_builder_logs);

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

/// Draw batch submission logs.
fn draw_batch_submissions(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let logs = render_logs(&app.batch_logs);

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

/// Draw derived blocks logs with timing.
fn draw_derived_blocks(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let logs = render_logs(&app.derivation_logs);

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
    let footer_line = Line::from(vec![
        Span::styled("[q]", Style::default().fg(Color::Yellow)),
        Span::raw(" Quit  "),
        Span::styled("[p]", Style::default().fg(Color::Yellow)),
        Span::raw(" Pause  "),
        Span::styled("[r]", Style::default().fg(Color::Yellow)),
        Span::raw(" Reset  "),
        Span::raw("  |  "),
        Span::styled("Mode: ", Style::default().fg(Color::DarkGray)),
        Span::styled("Dual", Style::default().fg(Color::Cyan)),
        Span::raw("  |  "),
        Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            if app.is_paused { "PAUSED" } else { "RUNNING" },
            Style::default().fg(if app.is_paused { Color::Yellow } else { Color::Green }),
        ),
        Span::raw("  |  "),
        Span::styled(
            if app.unsafe_head == app.finalized_head && app.unsafe_head > 0 {
                "IN SYNC ✓"
            } else {
                "SYNCING"
            },
            Style::default().fg(if app.unsafe_head == app.finalized_head && app.unsafe_head > 0 {
                Color::Green
            } else {
                Color::Yellow
            }),
        ),
    ]);

    let widget = Paragraph::new(footer_line).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Montana TUI ")
            .title_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(widget, area);
}
