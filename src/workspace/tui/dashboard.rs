use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::models::format_relative_time;
use crate::workspace::filter::{filter_repos, WorkspaceFilter};
use crate::workspace::{RepoSnapshot, WorkspaceSummary};

pub struct DashboardState {
    pub visible: Vec<usize>,
    pub list_state: ListState,
}

impl DashboardState {
    pub fn new(summary: &WorkspaceSummary, filters: &[WorkspaceFilter]) -> Self {
        let visible: Vec<usize> = if filters.is_empty() {
            (0..summary.repos.len()).collect()
        } else {
            let kept = filter_repos(&summary.repos, filters);
            kept.iter()
                .filter_map(|snap| {
                    summary
                        .repos
                        .iter()
                        .position(|r| std::ptr::eq(r, *snap))
                })
                .collect()
        };
        let mut list_state = ListState::default();
        if !visible.is_empty() {
            list_state.select(Some(0));
        }
        Self { visible, list_state }
    }

    pub fn next(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some((i + 1).min(self.visible.len() - 1)));
    }

    pub fn prev(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(i.saturating_sub(1)));
    }

    pub fn page_down(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some((i + 10).min(self.visible.len() - 1)));
    }

    pub fn page_up(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(i.saturating_sub(10)));
    }

    pub fn home(&mut self) {
        if !self.visible.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn end(&mut self) {
        if !self.visible.is_empty() {
            self.list_state.select(Some(self.visible.len() - 1));
        }
    }
}

pub fn render(f: &mut Frame, area: Rect, state: &DashboardState, summary: &WorkspaceSummary) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(format!(" repos ({}) ", state.visible.len()));

    if state.visible.is_empty() {
        let inner = block.inner(area);
        f.render_widget(block, area);
        f.render_widget(
            ratatui::widgets::Paragraph::new("no repos match")
                .style(Style::default().fg(Color::DarkGray)),
            inner,
        );
        return;
    }

    let name_width = state
        .visible
        .iter()
        .map(|&i| summary.repos[i].name.chars().count())
        .max()
        .unwrap_or(10)
        .min(28);

    let branch_width = state
        .visible
        .iter()
        .map(|&i| summary.repos[i].branch.chars().count())
        .max()
        .unwrap_or(10)
        .min(24);

    let items: Vec<ListItem> = state
        .visible
        .iter()
        .map(|&i| card_item(&summary.repos[i], name_width, branch_width))
        .collect();

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    let mut state_copy = state.list_state.clone();
    f.render_stateful_widget(list, area, &mut state_copy);
}

fn card_item<'a>(repo: &'a RepoSnapshot, name_w: usize, branch_w: usize) -> ListItem<'a> {
    let (dot_char, dot_color) = if repo.is_dirty() {
        ("●", Color::Yellow)
    } else if repo.stale {
        ("●", Color::DarkGray)
    } else {
        ("●", Color::Green)
    };

    let status_text = status_chunks(repo);
    let upstream_text = upstream_chunks(repo);
    let activity = repo
        .last_activity
        .as_ref()
        .map(format_relative_time)
        .unwrap_or_else(|| "—".into());
    let last_msg = repo
        .recent_commits
        .first()
        .map(|c| truncate(c.message.lines().next().unwrap_or(""), 48))
        .unwrap_or_default();

    let mut spans = vec![
        Span::raw(" "),
        Span::styled(dot_char, Style::default().fg(dot_color)),
        Span::raw("  "),
        Span::styled(
            pad(&repo.name, name_w),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(pad(&repo.branch, branch_w), Style::default().fg(Color::Cyan)),
        Span::raw("  "),
    ];
    spans.extend(status_text);
    spans.push(Span::raw(" "));
    spans.extend(upstream_text);
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        format!("{:>4}", activity),
        Style::default().fg(Color::DarkGray),
    ));
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        last_msg,
        Style::default().fg(Color::DarkGray),
    ));

    ListItem::new(Line::from(spans))
}

fn status_chunks(repo: &RepoSnapshot) -> Vec<Span<'static>> {
    let s = &repo.status;
    if s.is_clean() {
        return vec![Span::styled(
            pad("clean", 12),
            Style::default().fg(Color::DarkGray),
        )];
    }
    let mut out = Vec::new();
    let mut visible_len = 0;
    if s.staged > 0 {
        let t = format!("{}s", s.staged);
        visible_len += t.chars().count() + 1;
        out.push(Span::styled(t, Style::default().fg(Color::Green)));
        out.push(Span::raw(" "));
    }
    if s.modified > 0 {
        let t = format!("{}m", s.modified);
        visible_len += t.chars().count() + 1;
        out.push(Span::styled(t, Style::default().fg(Color::Yellow)));
        out.push(Span::raw(" "));
    }
    if s.untracked > 0 {
        let t = format!("{}?", s.untracked);
        visible_len += t.chars().count() + 1;
        out.push(Span::styled(t, Style::default().fg(Color::DarkGray)));
        out.push(Span::raw(" "));
    }
    if s.conflicted > 0 {
        let t = format!("{}!", s.conflicted);
        visible_len += t.chars().count() + 1;
        out.push(Span::styled(t, Style::default().fg(Color::Red)));
        out.push(Span::raw(" "));
    }
    if visible_len < 12 {
        out.push(Span::raw(" ".repeat(12 - visible_len)));
    }
    out
}

fn upstream_chunks(repo: &RepoSnapshot) -> Vec<Span<'static>> {
    let Some(up) = &repo.upstream else {
        return vec![Span::raw("     ")];
    };
    let mut out = Vec::new();
    if up.ahead > 0 {
        out.push(Span::styled(
            format!("{}↑", up.ahead),
            Style::default().fg(Color::Green),
        ));
    }
    if up.behind > 0 {
        out.push(Span::styled(
            format!("{}↓", up.behind),
            Style::default().fg(Color::Red),
        ));
    }
    if out.is_empty() {
        out.push(Span::raw("     "));
    }
    out
}

fn pad(s: &str, width: usize) -> String {
    let visible = s.chars().count();
    if visible >= width {
        s.chars().take(width).collect()
    } else {
        format!("{}{}", s, " ".repeat(width - visible))
    }
}

fn truncate(s: &str, max: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    if c.len() <= max {
        s.to_string()
    } else {
        let mut out: String = c.into_iter().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
