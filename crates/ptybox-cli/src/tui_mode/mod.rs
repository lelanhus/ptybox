//! Interactive TUI mode for running scenarios.
//!
//! Provides a live terminal UI showing:
//! - The emulated terminal screen
//! - Step list with status
//! - Run progress and controls

// TUI-specific lint allowances - ratatui layouts have fixed indices
#![allow(clippy::indexing_slicing)]

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use miette::{IntoDiagnostic, Result};
use ptybox::artifacts::ArtifactsWriterConfig;
use ptybox::model::{RunResult, Scenario, ScreenSnapshot, StepStatus};
use ptybox::runner::{run_scenario, ProgressCallback, ProgressEvent, RunnerOptions};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Run a scenario in interactive TUI mode.
pub fn run_tui(scenario: Scenario, artifacts: Option<ArtifactsWriterConfig>) -> Result<()> {
    // Set up terminal
    enable_raw_mode().into_diagnostic()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).into_diagnostic()?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).into_diagnostic()?;

    // Create channel for progress updates
    let (tx, rx) = mpsc::channel::<TuiEvent>();

    // Create app state
    let mut app = App::new(scenario.clone());

    // Run scenario in background thread
    let progress_tx = tx.clone();
    let finish_tx = tx.clone();
    let scenario_clone = scenario.clone();
    thread::spawn(move || {
        let callback = Arc::new(TuiProgressCallback { tx: progress_tx });
        let options = RunnerOptions {
            artifacts,
            progress: Some(callback as Arc<dyn ProgressCallback>),
        };
        let result = run_scenario(scenario_clone, options);
        // Ignore send error if receiver dropped
        let _ = finish_tx.send(TuiEvent::RunFinished(Box::new(result)));
    });

    // Main UI loop
    let result = run_app(&mut terminal, &mut app, rx);

    // Restore terminal
    disable_raw_mode().into_diagnostic()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .into_diagnostic()?;
    terminal.show_cursor().into_diagnostic()?;

    // Handle result
    if let Some(run_result) = app.run_result {
        eprintln!("run completed: {:?}", run_result.status);
        if matches!(run_result.status, ptybox::model::RunStatus::Failed) {
            std::process::exit(1);
        }
    }

    result
}

/// Events sent from the runner to the TUI.
enum TuiEvent {
    Progress(ProgressEvent),
    #[allow(dead_code)] // Reserved for future snapshot streaming
    Snapshot(ScreenSnapshot),
    RunFinished(Box<std::result::Result<RunResult, ptybox::runner::RunnerError>>),
}

/// Progress callback that sends events to the TUI.
struct TuiProgressCallback {
    tx: Sender<TuiEvent>,
}

impl ProgressCallback for TuiProgressCallback {
    fn on_progress(&self, event: &ProgressEvent) {
        let _ = self.tx.send(TuiEvent::Progress(event.clone()));
    }
}

/// Application state for the TUI.
struct App {
    #[allow(dead_code)] // Reserved for future use (e.g., showing scenario metadata)
    scenario: Scenario,
    steps: Vec<StepState>,
    current_step: usize,
    snapshot: Option<ScreenSnapshot>,
    running: bool,
    run_result: Option<RunResult>,
    error_message: Option<String>,
    scroll_offset: u16,
}

struct StepState {
    name: String,
    status: StepStatus,
    duration_ms: Option<u64>,
}

impl App {
    fn new(scenario: Scenario) -> Self {
        let steps = scenario
            .steps
            .iter()
            .map(|s| StepState {
                name: if s.name.is_empty() {
                    format!("{:?}", s.action.action_type)
                } else {
                    s.name.clone()
                },
                status: StepStatus::Skipped, // Not started yet
                duration_ms: None,
            })
            .collect();

        Self {
            scenario,
            steps,
            current_step: 0,
            snapshot: None,
            running: true,
            run_result: None,
            error_message: None,
            scroll_offset: 0,
        }
    }

    fn handle_event(&mut self, event: TuiEvent) {
        match event {
            TuiEvent::Progress(progress) => match progress {
                ProgressEvent::StepStarted {
                    step_index, name, ..
                } => {
                    if step_index > 0 && step_index <= self.steps.len() {
                        self.current_step = step_index - 1;
                        self.steps[self.current_step].name = name;
                    }
                }
                ProgressEvent::StepCompleted {
                    name,
                    status,
                    duration_ms,
                    ..
                } => {
                    if self.current_step < self.steps.len() {
                        self.steps[self.current_step].name = name;
                        self.steps[self.current_step].status = status;
                        self.steps[self.current_step].duration_ms = Some(duration_ms);
                    }
                }
                ProgressEvent::RunCompleted { .. } => {
                    self.running = false;
                }
                ProgressEvent::RunStarted { .. } => {}
            },
            TuiEvent::Snapshot(snapshot) => {
                self.snapshot = Some(snapshot);
            }
            TuiEvent::RunFinished(result) => {
                self.running = false;
                match *result {
                    Ok(run_result) => {
                        self.run_result = Some(run_result);
                    }
                    Err(err) => {
                        self.error_message = Some(err.to_string());
                    }
                }
            }
        }
    }
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    rx: Receiver<TuiEvent>,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app)).into_diagnostic()?;

        // Handle incoming events from runner
        while let Ok(event) = rx.try_recv() {
            app.handle_event(event);
        }

        // Handle keyboard input
        if event::poll(Duration::from_millis(50)).into_diagnostic()? {
            if let Event::Key(key) = event::read().into_diagnostic()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            return Ok(());
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if app.scroll_offset > 0 {
                                app.scroll_offset -= 1;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.scroll_offset += 1;
                        }
                        _ => {}
                    }
                }
            }
        }

        // Exit when run is complete and user can review
        if !app.running && app.run_result.is_some() {
            // Wait for user to press 'q' to exit
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.area());

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(3)])
        .split(chunks[0]);

    // Terminal panel
    render_terminal(f, left_chunks[0], app);

    // Footer
    render_footer(f, left_chunks[1], app);

    // Steps panel
    render_steps(f, chunks[1], app);
}

fn render_terminal(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Terminal ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(ref snapshot) = app.snapshot {
        let lines: Vec<Line> = snapshot
            .lines
            .iter()
            .skip(app.scroll_offset as usize)
            .take(inner.height as usize)
            .map(|line| Line::from(line.as_str()))
            .collect();
        let paragraph = Paragraph::new(lines);
        f.render_widget(paragraph, inner);
    } else {
        let placeholder = Paragraph::new("Waiting for terminal output...")
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(placeholder, inner);
    }
}

fn render_steps(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Steps ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let items: Vec<ListItem> = app
        .steps
        .iter()
        .enumerate()
        .map(|(i, step)| {
            let (icon, style) = match step.status {
                StepStatus::Passed => ("✓", Style::default().fg(Color::Green)),
                StepStatus::Failed => ("✗", Style::default().fg(Color::Red)),
                StepStatus::Errored => ("!", Style::default().fg(Color::Red)),
                StepStatus::Skipped => {
                    if i == app.current_step && app.running {
                        ("▶", Style::default().fg(Color::Yellow))
                    } else {
                        (" ", Style::default().fg(Color::DarkGray))
                    }
                }
            };

            let duration_str = step
                .duration_ms
                .map(|d| format!(" ({d}ms)"))
                .unwrap_or_default();

            let content = Line::from(vec![
                Span::styled(format!("{icon} "), style),
                Span::styled(&step.name, style),
                Span::styled(duration_str, Style::default().fg(Color::DarkGray)),
            ]);

            let mut item = ListItem::new(content);
            if i == app.current_step {
                item = item.style(Style::default().add_modifier(Modifier::BOLD));
            }
            item
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_footer(f: &mut Frame, area: Rect, app: &App) {
    let status = if app.running {
        Span::styled("Running", Style::default().fg(Color::Yellow))
    } else if let Some(ref result) = app.run_result {
        match result.status {
            ptybox::model::RunStatus::Passed => {
                Span::styled("Passed", Style::default().fg(Color::Green))
            }
            ptybox::model::RunStatus::Failed => {
                Span::styled("Failed", Style::default().fg(Color::Red))
            }
            ptybox::model::RunStatus::Errored => {
                Span::styled("Errored", Style::default().fg(Color::Red))
            }
            ptybox::model::RunStatus::Canceled => {
                Span::styled("Canceled", Style::default().fg(Color::DarkGray))
            }
        }
    } else if let Some(ref err) = app.error_message {
        Span::styled(
            format!("Error: {}", err.chars().take(50).collect::<String>()),
            Style::default().fg(Color::Red),
        )
    } else {
        Span::styled("Unknown", Style::default().fg(Color::DarkGray))
    };

    let step_info = format!("Step {}/{}", app.current_step + 1, app.steps.len());

    let content = Line::from(vec![
        Span::raw(" "),
        status,
        Span::raw(" │ "),
        Span::raw(step_info),
        Span::raw(" │ "),
        Span::styled("[q]uit", Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
        Span::styled("[↑↓]scroll", Style::default().fg(Color::DarkGray)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(content).block(block);
    f.render_widget(paragraph, area);
}
