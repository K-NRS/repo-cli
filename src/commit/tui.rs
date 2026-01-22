use std::io::stdout;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::ai::{generate_commit_message, AiProvider};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Review,
    Edit,
    DiffView,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TuiResult {
    Commit,
    Cancel,
}

pub struct CommitApp {
    pub message: String,
    pub diff: String,
    pub provider: AiProvider,
    pub staged_files: Vec<String>,
    mode: Mode,
    cursor_pos: usize,
    diff_scroll: u16,
    should_quit: bool,
    result: Option<TuiResult>,
    status: String,
}

impl CommitApp {
    pub fn new(
        message: String,
        diff: String,
        provider: AiProvider,
        staged_files: Vec<String>,
    ) -> Self {
        Self {
            cursor_pos: message.len(),
            message,
            diff,
            provider,
            staged_files,
            mode: Mode::Review,
            diff_scroll: 0,
            should_quit: false,
            result: None,
            status: String::new(),
        }
    }

    fn handle_key(&mut self, key: KeyCode) {
        match self.mode {
            Mode::Review => self.handle_review_key(key),
            Mode::Edit => self.handle_edit_key(key),
            Mode::DiffView => self.handle_diff_key(key),
        }
    }

    fn handle_review_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('y') | KeyCode::Enter => {
                self.result = Some(TuiResult::Commit);
                self.should_quit = true;
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.result = Some(TuiResult::Cancel);
                self.should_quit = true;
            }
            KeyCode::Char('r') => {
                self.status = "Regenerating...".to_string();
                match generate_commit_message(self.provider, &self.diff, None) {
                    Ok(msg) => {
                        self.message = msg;
                        self.cursor_pos = self.message.len();
                        self.status = "Message regenerated".to_string();
                    }
                    Err(e) => {
                        self.status = format!("Error: {}", e);
                    }
                }
            }
            KeyCode::Char('e') => {
                self.mode = Mode::Edit;
                self.cursor_pos = self.message.len();
            }
            KeyCode::Char('d') => {
                self.mode = Mode::DiffView;
                self.diff_scroll = 0;
            }
            _ => {}
        }
    }

    fn handle_edit_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc => {
                self.mode = Mode::Review;
            }
            KeyCode::Enter => {
                self.message.insert(self.cursor_pos, '\n');
                self.cursor_pos += 1;
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.message.remove(self.cursor_pos);
                }
            }
            KeyCode::Delete => {
                if self.cursor_pos < self.message.len() {
                    self.message.remove(self.cursor_pos);
                }
            }
            KeyCode::Left => {
                self.cursor_pos = self.cursor_pos.saturating_sub(1);
            }
            KeyCode::Right => {
                self.cursor_pos = (self.cursor_pos + 1).min(self.message.len());
            }
            KeyCode::Home => {
                self.cursor_pos = 0;
            }
            KeyCode::End => {
                self.cursor_pos = self.message.len();
            }
            KeyCode::Char(c) => {
                self.message.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
            }
            _ => {}
        }
    }

    fn handle_diff_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('d') | KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = Mode::Review;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.diff_scroll = self.diff_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.diff_scroll = self.diff_scroll.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.diff_scroll = self.diff_scroll.saturating_add(10);
            }
            KeyCode::PageUp => {
                self.diff_scroll = self.diff_scroll.saturating_sub(10);
            }
            _ => {}
        }
    }
}

pub fn run_commit_tui(mut app: CommitApp) -> Result<(String, TuiResult)> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    loop {
        terminal.draw(|f| ui(f, &app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code);
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    let result = app.result.unwrap_or(TuiResult::Cancel);
    Ok((app.message, result))
}

fn ui(f: &mut Frame, app: &CommitApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(5),  // Staged files
            Constraint::Min(8),     // Message or Diff
            Constraint::Length(3),  // Footer
        ])
        .split(f.size());

    // Header
    render_header(f, app, chunks[0]);

    // Staged files
    render_staged_files(f, app, chunks[1]);

    // Main content (message or diff based on mode)
    match app.mode {
        Mode::DiffView => render_diff(f, app, chunks[2]),
        _ => render_message(f, app, chunks[2]),
    }

    // Footer
    render_footer(f, app, chunks[3]);
}

fn render_header(f: &mut Frame, app: &CommitApp, area: Rect) {
    let title = format!(" repo commit ({}) ", app.provider.name());
    let status = if app.status.is_empty() {
        String::new()
    } else {
        format!(" {} ", app.status)
    };

    let header = Paragraph::new(format!("{}{}", title, status))
        .style(Style::default().fg(Color::Cyan).bold())
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(header, area);
}

fn render_staged_files(f: &mut Frame, app: &CommitApp, area: Rect) {
    let files: String = app
        .staged_files
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");

    let more = if app.staged_files.len() > 3 {
        format!(" (+{} more)", app.staged_files.len() - 3)
    } else {
        String::new()
    };

    let text = format!(
        "{} file(s): {}{}",
        app.staged_files.len(),
        files,
        more
    );

    let widget = Paragraph::new(text)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Staged Changes "),
        );

    f.render_widget(widget, area);
}

fn render_message(f: &mut Frame, app: &CommitApp, area: Rect) {
    let title = match app.mode {
        Mode::Edit => " Commit Message (EDITING) ",
        _ => " Commit Message ",
    };

    let border_color = match app.mode {
        Mode::Edit => Color::Green,
        _ => Color::White,
    };

    // Show cursor in edit mode
    let text = if app.mode == Mode::Edit {
        let (before, after) = app.message.split_at(app.cursor_pos.min(app.message.len()));
        format!("{}|{}", before, after)
    } else {
        app.message.clone()
    };

    let widget = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(title),
        );

    f.render_widget(widget, area);
}

fn render_diff(f: &mut Frame, app: &CommitApp, area: Rect) {
    let lines: Vec<Line> = app
        .diff
        .lines()
        .map(|line| {
            let style = if line.starts_with('+') && !line.starts_with("+++") {
                Style::default().fg(Color::Green)
            } else if line.starts_with('-') && !line.starts_with("---") {
                Style::default().fg(Color::Red)
            } else if line.starts_with("@@") {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            Line::styled(line.to_string(), style)
        })
        .collect();

    let widget = Paragraph::new(lines)
        .scroll((app.diff_scroll, 0))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Diff (d to close, j/k to scroll) "),
        );

    f.render_widget(widget, area);
}

fn render_footer(f: &mut Frame, app: &CommitApp, area: Rect) {
    let help = match app.mode {
        Mode::Review => "y/Enter: commit | q/Esc: cancel | r: regenerate | e: edit | d: view diff",
        Mode::Edit => "Esc: done editing | Type to edit message",
        Mode::DiffView => "d/Esc: close | j/k: scroll | PgUp/PgDn: fast scroll",
    };

    let footer = Paragraph::new(format!(" {}", help))
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(footer, area);
}
