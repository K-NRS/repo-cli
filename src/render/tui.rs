use std::io::stdout;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::models::{format_relative_time, RepoSummary};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Panel {
    Branches,
    Commits,
    Status,
}

struct App {
    summary: RepoSummary,
    active_panel: Panel,
    branch_index: usize,
    commit_index: usize,
    should_quit: bool,
}

impl App {
    fn new(summary: RepoSummary) -> Self {
        Self {
            summary,
            active_panel: Panel::Branches,
            branch_index: 0,
            commit_index: 0,
            should_quit: false,
        }
    }

    fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab => {
                self.active_panel = match self.active_panel {
                    Panel::Branches => Panel::Commits,
                    Panel::Commits => Panel::Status,
                    Panel::Status => Panel::Branches,
                };
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('h') | KeyCode::Left => self.prev_panel(),
            KeyCode::Char('l') | KeyCode::Right => self.next_panel(),
            _ => {}
        }
    }

    fn move_down(&mut self) {
        match self.active_panel {
            Panel::Branches => {
                let max = self.summary.local_branches.len().saturating_sub(1);
                self.branch_index = (self.branch_index + 1).min(max);
            }
            Panel::Commits => {
                let max = self.summary.recent_commits.len().saturating_sub(1);
                self.commit_index = (self.commit_index + 1).min(max);
            }
            Panel::Status => {}
        }
    }

    fn move_up(&mut self) {
        match self.active_panel {
            Panel::Branches => {
                self.branch_index = self.branch_index.saturating_sub(1);
            }
            Panel::Commits => {
                self.commit_index = self.commit_index.saturating_sub(1);
            }
            Panel::Status => {}
        }
    }

    fn next_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Branches => Panel::Commits,
            Panel::Commits => Panel::Status,
            Panel::Status => Panel::Branches,
        };
    }

    fn prev_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Branches => Panel::Status,
            Panel::Commits => Panel::Branches,
            Panel::Status => Panel::Commits,
        };
    }
}

pub fn run_tui(summary: RepoSummary) -> Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new(summary);

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
    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Footer
        ])
        .split(f.size());

    // Header
    let header = render_header(app);
    f.render_widget(header, chunks[0]);

    // Main content - split into columns
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(chunks[1]);

    // Branches panel
    let branches = render_branches(app);
    f.render_widget(branches, main_chunks[0]);

    // Commits panel
    let commits = render_commits(app);
    f.render_widget(commits, main_chunks[1]);

    // Status panel
    let status = render_status(app);
    f.render_widget(status, main_chunks[2]);

    // Footer
    let footer = render_footer();
    f.render_widget(footer, chunks[2]);
}

fn render_header(app: &App) -> Paragraph<'static> {
    let branch = &app.summary.current_branch;
    let mut text = format!(" ON: {}", branch.name);

    if let Some(ref upstream) = branch.upstream {
        if upstream.ahead > 0 || upstream.behind > 0 {
            text.push_str(&format!(" ({}↑ {}↓)", upstream.ahead, upstream.behind));
        }
    }

    Paragraph::new(text)
        .style(Style::default().fg(Color::Cyan).bold())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" repo ")
                .title_style(Style::default().bold()),
        )
}

fn render_branches(app: &App) -> List<'static> {
    let items: Vec<ListItem> = app
        .summary
        .local_branches
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let marker = if b.is_head { "* " } else { "  " };
            let text = format!("{}{}", marker, b.name);

            let style = if i == app.branch_index && app.active_panel == Panel::Branches {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else if b.is_head {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            ListItem::new(text).style(style)
        })
        .collect();

    let border_style = if app.active_panel == Panel::Branches {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Branches "),
    )
}

fn render_commits(app: &App) -> List<'static> {
    let items: Vec<ListItem> = app
        .summary
        .recent_commits
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let time = format_relative_time(&c.time);
            let text = format!("{:>4}  {}", time, truncate(&c.message, 35));

            let style = if i == app.commit_index && app.active_panel == Panel::Commits {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            ListItem::new(text).style(style)
        })
        .collect();

    let border_style = if app.active_panel == Panel::Commits {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Recent Commits "),
    )
}

fn render_status(app: &App) -> Paragraph<'static> {
    let status = &app.summary.status;

    let mut lines = Vec::new();

    if status.is_clean() {
        lines.push(Line::from("Working tree clean"));
    } else {
        if status.staged > 0 {
            lines.push(Line::from(format!("Staged: {}", status.staged)));
        }
        if status.modified > 0 {
            lines.push(Line::from(format!("Modified: {}", status.modified)));
        }
        if status.untracked > 0 {
            lines.push(Line::from(format!("Untracked: {}", status.untracked)));
        }
        if status.conflicted > 0 {
            lines.push(Line::from(Span::styled(
                format!("Conflicted: {}", status.conflicted),
                Style::default().fg(Color::Red),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(format!("Stashes: {}", app.summary.stashes.len())));

    for stash in app.summary.stashes.iter().take(3) {
        lines.push(Line::from(format!("  {}: {}", stash.index, truncate(&stash.message, 20))));
    }

    let border_style = if app.active_panel == Panel::Status {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Status "),
    )
}

fn render_footer() -> Paragraph<'static> {
    Paragraph::new(" q: quit | tab: switch panel | j/k: navigate | ↑↓←→: move")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL))
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
