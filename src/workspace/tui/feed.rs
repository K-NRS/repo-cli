use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::models::{format_relative_time, CommitInfo};
use crate::workspace::filter::{commit_matches, WorkspaceFilter};
use crate::workspace::{RepoSnapshot, WorkspaceSummary};

#[derive(Debug, Clone)]
pub struct FeedEntry {
    pub repo_index: usize,
    pub commit_index: usize,
}

pub struct FeedState {
    pub entries: Vec<FeedEntry>,
    pub list_state: ListState,
}

impl FeedState {
    pub fn new(summary: &WorkspaceSummary, filters: &[WorkspaceFilter]) -> Self {
        let mut entries: Vec<(chrono::DateTime<chrono::Local>, FeedEntry)> = Vec::new();
        for (ri, repo) in summary.repos.iter().enumerate() {
            for (ci, commit) in repo.recent_commits.iter().enumerate() {
                if commit_matches(commit, &repo.name, filters) {
                    entries.push((
                        commit.time,
                        FeedEntry {
                            repo_index: ri,
                            commit_index: ci,
                        },
                    ));
                }
            }
        }
        entries.sort_by(|a, b| b.0.cmp(&a.0));
        let entries: Vec<FeedEntry> = entries.into_iter().map(|(_, e)| e).take(500).collect();

        let mut list_state = ListState::default();
        if !entries.is_empty() {
            list_state.select(Some(0));
        }
        Self { entries, list_state }
    }

    pub fn next(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some((i + 1).min(self.entries.len() - 1)));
    }

    pub fn prev(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(i.saturating_sub(1)));
    }

    pub fn page_down(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some((i + 10).min(self.entries.len() - 1)));
    }

    pub fn page_up(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(i.saturating_sub(10)));
    }

    pub fn home(&mut self) {
        if !self.entries.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn end(&mut self) {
        if !self.entries.is_empty() {
            self.list_state.select(Some(self.entries.len() - 1));
        }
    }
}

pub fn render(f: &mut Frame, area: Rect, state: &FeedState, summary: &WorkspaceSummary) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(format!(" commits ({}) ", state.entries.len()));

    if state.entries.is_empty() {
        let inner = block.inner(area);
        f.render_widget(block, area);
        f.render_widget(
            ratatui::widgets::Paragraph::new("no commits match").style(Style::default().fg(Color::DarkGray)),
            inner,
        );
        return;
    }

    let name_width = summary
        .repos
        .iter()
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(10)
        .min(24);

    let items: Vec<ListItem> = state
        .entries
        .iter()
        .map(|entry| {
            let repo = &summary.repos[entry.repo_index];
            let commit = &repo.recent_commits[entry.commit_index];
            list_item(repo, commit, name_width)
        })
        .collect();

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    let mut state_copy = state.list_state.clone();
    f.render_stateful_widget(list, area, &mut state_copy);
}

fn list_item<'a>(repo: &'a RepoSnapshot, commit: &'a CommitInfo, name_width: usize) -> ListItem<'a> {
    let when = format_relative_time(&commit.time);
    let author = commit
        .author
        .split_whitespace()
        .next()
        .unwrap_or(&commit.author);
    let short = commit.short_id.clone();
    let msg = commit.message.lines().next().unwrap_or("").to_string();

    let repo_cell = pad(&repo.name, name_width);

    ListItem::new(Line::from(vec![
        Span::styled(format!(" {:>4} ", when), Style::default().fg(Color::DarkGray)),
        Span::styled(
            repo_cell,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(short, Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::raw(truncate(&msg, 60)),
        Span::raw("  "),
        Span::styled(author.to_string(), Style::default().fg(Color::DarkGray)),
    ]))
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
