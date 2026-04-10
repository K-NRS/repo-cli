pub mod dashboard;
pub mod feed;
pub mod picker;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame, Terminal,
};
use std::io::stdout;

use crate::workspace::filter::{parse_filters, WorkspaceFilter};
use crate::workspace::WorkspaceSummary;

use dashboard::DashboardState;
use feed::FeedState;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tab {
    Feed,
    Dashboard,
}

impl Tab {
    fn index(&self) -> usize {
        match self {
            Tab::Feed => 0,
            Tab::Dashboard => 1,
        }
    }
    fn toggle(&self) -> Self {
        match self {
            Tab::Feed => Tab::Dashboard,
            Tab::Dashboard => Tab::Feed,
        }
    }
}

pub struct WorkspaceApp {
    summary: WorkspaceSummary,
    tab: Tab,
    filter_input: String,
    filter_editing: bool,
    filters: Vec<WorkspaceFilter>,
    feed: FeedState,
    dashboard: DashboardState,
    status: String,
    should_quit: bool,
}

impl WorkspaceApp {
    pub fn new(summary: WorkspaceSummary, initial_filter: String) -> Self {
        let filters = parse_filters(&initial_filter);
        let feed = FeedState::new(&summary, &filters);
        let dashboard = DashboardState::new(&summary, &filters);
        Self {
            summary,
            tab: Tab::Dashboard,
            filter_input: initial_filter,
            filter_editing: false,
            filters,
            feed,
            dashboard,
            status: String::new(),
            should_quit: false,
        }
    }

    fn reapply_filters(&mut self) {
        self.filters = parse_filters(&self.filter_input);
        self.feed = FeedState::new(&self.summary, &self.filters);
        self.dashboard = DashboardState::new(&self.summary, &self.filters);
    }
}

pub fn run_workspace_tui(summary: WorkspaceSummary, initial_filter: String) -> Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    let mut app = WorkspaceApp::new(summary, initial_filter);

    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut WorkspaceApp,
) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|f| render(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            handle_key(app, key);
        }
    }
    Ok(())
}

fn handle_key(app: &mut WorkspaceApp, key: crossterm::event::KeyEvent) {
    if app.filter_editing {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => {
                app.filter_editing = false;
                app.reapply_filters();
                app.status.clear();
            }
            KeyCode::Backspace => {
                app.filter_input.pop();
            }
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && (c == 'u') {
                    app.filter_input.clear();
                } else {
                    app.filter_input.push(c);
                }
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true
        }
        KeyCode::Tab | KeyCode::Char('1') => {
            app.tab = app.tab.toggle();
        }
        KeyCode::Char('/') => {
            app.filter_editing = true;
            app.status = "filter: msg: author: date: repo: status: text:".into();
        }
        KeyCode::Char('x') => {
            if !app.filter_input.is_empty() {
                app.filter_input.clear();
                app.reapply_filters();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => match app.tab {
            Tab::Feed => app.feed.next(),
            Tab::Dashboard => app.dashboard.next(),
        },
        KeyCode::Up | KeyCode::Char('k') => match app.tab {
            Tab::Feed => app.feed.prev(),
            Tab::Dashboard => app.dashboard.prev(),
        },
        KeyCode::PageDown => match app.tab {
            Tab::Feed => app.feed.page_down(),
            Tab::Dashboard => app.dashboard.page_down(),
        },
        KeyCode::PageUp => match app.tab {
            Tab::Feed => app.feed.page_up(),
            Tab::Dashboard => app.dashboard.page_up(),
        },
        KeyCode::Home | KeyCode::Char('g') => match app.tab {
            Tab::Feed => app.feed.home(),
            Tab::Dashboard => app.dashboard.home(),
        },
        KeyCode::End | KeyCode::Char('G') => match app.tab {
            Tab::Feed => app.feed.end(),
            Tab::Dashboard => app.dashboard.end(),
        },
        _ => {}
    }
}

fn render(f: &mut Frame, app: &WorkspaceApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Length(1), // tabs
            Constraint::Length(3), // filter bar
            Constraint::Min(0),    // body
            Constraint::Length(1), // footer
        ])
        .split(f.size());

    render_header(f, chunks[0], app);
    render_tabs(f, chunks[1], app);
    render_filter(f, chunks[2], app);
    match app.tab {
        Tab::Feed => feed::render(f, chunks[3], &app.feed, &app.summary),
        Tab::Dashboard => dashboard::render(f, chunks[3], &app.dashboard, &app.summary),
    }
    render_footer(f, chunks[4], app);
}

fn render_header(f: &mut Frame, area: Rect, app: &WorkspaceApp) {
    let src = app.summary.source.display();
    let count = app.summary.repos.len();
    let line = Line::from(vec![
        Span::styled("▣ ", Style::default().fg(Color::Cyan)),
        Span::styled(src, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format!(
            "  {} repo{}",
            count,
            if count == 1 { "" } else { "s" }
        )),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_tabs(f: &mut Frame, area: Rect, app: &WorkspaceApp) {
    let titles = vec![Line::from("Feed"), Line::from("Dashboard")];
    let tabs = Tabs::new(titles)
        .select(app.tab.index())
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" · ");
    f.render_widget(tabs, area);
}

fn render_filter(f: &mut Frame, area: Rect, app: &WorkspaceApp) {
    let border_color = if app.filter_editing {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(" / filter ");
    let content = if app.filter_input.is_empty() && !app.filter_editing {
        Line::from(Span::styled(
            "  (none) — press / to filter",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        let cursor = if app.filter_editing { "▌" } else { "" };
        Line::from(vec![
            Span::raw(" "),
            Span::raw(app.filter_input.clone()),
            Span::styled(cursor, Style::default().fg(Color::Cyan)),
        ])
    };
    f.render_widget(Paragraph::new(content).block(block), area);
}

fn render_footer(f: &mut Frame, area: Rect, app: &WorkspaceApp) {
    let help = if app.filter_editing {
        "Enter: apply  Esc: cancel  ^U: clear"
    } else {
        "Tab: switch  /: filter  x: clear filter  j/k: nav  g/G: top/bottom  q: quit"
    };
    let content = if app.status.is_empty() {
        help.to_string()
    } else {
        format!("{} · {}", app.status, help)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            content,
            Style::default().fg(Color::DarkGray),
        ))),
        area,
    );
}
