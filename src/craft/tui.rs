use std::collections::HashMap;
use std::io::stdout;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use git2::Repository;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::models::{format_relative_time, CommitInfo};
use super::actions::{RebaseAction, SplitGroup, TodoEntry};
use super::split::{get_commit_hunks, Hunk};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    CommitList,
    ActionMenu,
    RewordEdit,
    SplitView,
    SquashTarget,
    ReorderMode,
    Preview,
}

pub enum CraftResult {
    Execute(Vec<TodoEntry>, HashMap<usize, Vec<Hunk>>),
    Cancel,
}

struct App {
    commits: Vec<CommitInfo>,
    entries: Vec<TodoEntry>,
    cursor: usize,
    mode: Mode,
    should_quit: bool,
    result: Option<CraftResult>,

    // selection state
    selected: Vec<bool>,

    // reword state
    reword_buffer: String,
    reword_cursor: usize,

    // split state
    hunks: Vec<Hunk>,
    hunk_cursor: usize,
    hunk_groups: Vec<usize>,  // group assignment per hunk (0 = unassigned)
    split_messages: Vec<String>,
    split_msg_cursor: usize,
    split_editing_msg: bool,
    next_group: usize,

    // squash state
    squash_source: usize,

    // diff preview
    diff_text: String,
    diff_scroll: u16,

    // hunk cache for execution
    hunks_cache: HashMap<usize, Vec<Hunk>>,

    // status message
    status: String,
}

impl App {
    fn new(commits: Vec<CommitInfo>) -> Self {
        let len = commits.len();
        let entries: Vec<TodoEntry> = (0..len).map(TodoEntry::pick).collect();
        Self {
            commits,
            entries,
            cursor: 0,
            mode: Mode::CommitList,
            should_quit: false,
            result: None,
            selected: vec![false; len],
            reword_buffer: String::new(),
            reword_cursor: 0,
            hunks: Vec::new(),
            hunk_cursor: 0,
            hunk_groups: Vec::new(),
            split_messages: Vec::new(),
            split_msg_cursor: 0,
            split_editing_msg: false,
            next_group: 1,
            squash_source: 0,
            diff_text: String::new(),
            diff_scroll: 0,
            hunks_cache: HashMap::new(),
            status: String::new(),
        }
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers, repo: &Repository) {
        match self.mode {
            Mode::CommitList => self.handle_commit_list(code, repo),
            Mode::ActionMenu => self.handle_action_menu(code, repo),
            Mode::RewordEdit => self.handle_reword_edit(code),
            Mode::SplitView => self.handle_split_view(code),
            Mode::SquashTarget => self.handle_squash_target(code),
            Mode::ReorderMode => self.handle_reorder(code, modifiers),
            Mode::Preview => self.handle_preview(code),
        }
    }

    // --- CommitList mode ---
    fn handle_commit_list(&mut self, code: KeyCode, repo: &Repository) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.cursor = (self.cursor + 1).min(self.commits.len().saturating_sub(1));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
            }
            KeyCode::Char(' ') => {
                self.selected[self.cursor] = !self.selected[self.cursor];
            }
            KeyCode::Enter => {
                if self.selected.iter().any(|&s| s) || true {
                    self.mode = Mode::ActionMenu;
                    self.status = "r=reword s=split q=squash f=fixup d=drop m=reorder".into();
                }
            }
            KeyCode::Char('p') => {
                // preview current plan
                if self.has_actions() {
                    self.mode = Mode::Preview;
                    self.diff_scroll = 0;
                } else {
                    self.status = "no actions assigned yet".into();
                }
            }
            KeyCode::Char('D') => {
                // show diff for selected commit
                self.load_diff_for_cursor(repo);
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.result = Some(CraftResult::Cancel);
                self.should_quit = true;
            }
            _ => {}
        }
    }

    fn load_diff_for_cursor(&mut self, repo: &Repository) {
        let oid = self.commits[self.cursor].id;
        match crate::git::get_commit_diff(repo, oid) {
            Ok(diff) => {
                self.diff_text = diff;
                self.diff_scroll = 0;
            }
            Err(e) => {
                self.status = format!("diff error: {}", e);
            }
        }
    }

    // --- ActionMenu mode ---
    fn handle_action_menu(&mut self, code: KeyCode, repo: &Repository) {
        match code {
            KeyCode::Char('r') => {
                // reword
                let msg = self.commits[self.cursor].message.clone();
                self.reword_buffer = msg;
                self.reword_cursor = self.reword_buffer.len();
                self.mode = Mode::RewordEdit;
                self.status = "editing message - Esc=done".into();
            }
            KeyCode::Char('s') => {
                // split - load hunks
                let oid = self.commits[self.cursor].id;
                match get_commit_hunks(repo, oid) {
                    Ok(hunks) => {
                        if hunks.is_empty() {
                            self.status = "no hunks to split".into();
                            self.mode = Mode::CommitList;
                            return;
                        }
                        let len = hunks.len();
                        self.hunks_cache.insert(self.cursor, hunks.clone());
                        self.hunks = hunks;
                        self.hunk_cursor = 0;
                        self.hunk_groups = vec![0; len];
                        self.split_messages = vec![String::new(); 10]; // up to 9 groups
                        self.split_msg_cursor = 0;
                        self.next_group = 1;
                        self.split_editing_msg = false;
                        self.mode = Mode::SplitView;
                        self.status = "space=toggle group 1-9=assign g=new group Enter=done".into();
                    }
                    Err(e) => {
                        self.status = format!("hunk parse error: {}", e);
                        self.mode = Mode::CommitList;
                    }
                }
            }
            KeyCode::Char('q') => {
                // squash
                self.squash_source = self.cursor;
                self.mode = Mode::SquashTarget;
                self.status = "select target commit to squash into (j/k, Enter)".into();
            }
            KeyCode::Char('f') => {
                // fixup (squash without message edit)
                self.squash_source = self.cursor;
                // For fixup, pick the commit above (cursor - 1)
                if self.cursor > 0 {
                    self.entries[self.cursor] = TodoEntry {
                        original_idx: self.cursor,
                        action: RebaseAction::Fixup { into_idx: self.cursor - 1 },
                    };
                    self.status = format!(
                        "fixup {} into {}",
                        self.commits[self.cursor].short_id,
                        self.commits[self.cursor - 1].short_id
                    );
                } else {
                    self.status = "cannot fixup first commit".into();
                }
                self.mode = Mode::CommitList;
            }
            KeyCode::Char('d') => {
                // drop
                self.entries[self.cursor] = TodoEntry {
                    original_idx: self.cursor,
                    action: RebaseAction::Drop,
                };
                self.status = format!("drop {}", self.commits[self.cursor].short_id);
                self.mode = Mode::CommitList;
            }
            KeyCode::Char('m') => {
                // reorder mode
                self.mode = Mode::ReorderMode;
                self.status = "J/K=move commit Esc=done".into();
            }
            KeyCode::Char('e') => {
                // edit (stop for manual editing)
                self.entries[self.cursor] = TodoEntry {
                    original_idx: self.cursor,
                    action: RebaseAction::Edit,
                };
                self.status = format!("edit stop at {}", self.commits[self.cursor].short_id);
                self.mode = Mode::CommitList;
            }
            KeyCode::Char('x') => {
                // reset to pick
                self.entries[self.cursor] = TodoEntry::pick(self.cursor);
                self.status = format!("reset {} to pick", self.commits[self.cursor].short_id);
                self.mode = Mode::CommitList;
            }
            KeyCode::Esc => {
                self.mode = Mode::CommitList;
                self.status.clear();
            }
            _ => {}
        }
    }

    // --- RewordEdit mode ---
    fn handle_reword_edit(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                // Save reword action
                if !self.reword_buffer.is_empty() && self.reword_buffer != self.commits[self.cursor].message {
                    self.entries[self.cursor] = TodoEntry {
                        original_idx: self.cursor,
                        action: RebaseAction::Reword(self.reword_buffer.clone()),
                    };
                    self.status = format!("reword {}", self.commits[self.cursor].short_id);
                }
                self.mode = Mode::CommitList;
            }
            KeyCode::Enter => {
                self.reword_buffer.insert(self.reword_cursor, '\n');
                self.reword_cursor += 1;
            }
            KeyCode::Backspace => {
                if self.reword_cursor > 0 {
                    self.reword_cursor -= 1;
                    self.reword_buffer.remove(self.reword_cursor);
                }
            }
            KeyCode::Delete => {
                if self.reword_cursor < self.reword_buffer.len() {
                    self.reword_buffer.remove(self.reword_cursor);
                }
            }
            KeyCode::Left => {
                self.reword_cursor = self.reword_cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                self.reword_cursor = (self.reword_cursor + 1).min(self.reword_buffer.len());
            }
            KeyCode::Home => self.reword_cursor = 0,
            KeyCode::End => self.reword_cursor = self.reword_buffer.len(),
            KeyCode::Char(c) => {
                self.reword_buffer.insert(self.reword_cursor, c);
                self.reword_cursor += 1;
            }
            _ => {}
        }
    }

    // --- SplitView mode ---
    fn handle_split_view(&mut self, code: KeyCode) {
        if self.split_editing_msg {
            // editing a group message
            match code {
                KeyCode::Esc | KeyCode::Enter => {
                    self.split_editing_msg = false;
                    self.status = "space=toggle 1-9=assign g=new group Enter=done".into();
                }
                KeyCode::Backspace => {
                    let idx = self.split_msg_cursor;
                    if !self.split_messages[idx].is_empty() {
                        self.split_messages[idx].pop();
                    }
                }
                KeyCode::Char(c) => {
                    let idx = self.split_msg_cursor;
                    self.split_messages[idx].push(c);
                }
                _ => {}
            }
            return;
        }

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.hunk_cursor = (self.hunk_cursor + 1).min(self.hunks.len().saturating_sub(1));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.hunk_cursor = self.hunk_cursor.saturating_sub(1);
            }
            KeyCode::Char(' ') => {
                // toggle current hunk into next_group
                if self.hunk_groups[self.hunk_cursor] == 0 {
                    self.hunk_groups[self.hunk_cursor] = self.next_group;
                } else {
                    self.hunk_groups[self.hunk_cursor] = 0;
                }
            }
            KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                let group = c.to_digit(10).unwrap() as usize;
                self.hunk_groups[self.hunk_cursor] = group;
                if group >= self.next_group {
                    self.next_group = group + 1;
                }
            }
            KeyCode::Char('g') => {
                // new group
                self.hunk_groups[self.hunk_cursor] = self.next_group;
                self.next_group += 1;
            }
            KeyCode::Char('n') => {
                // name/edit group message
                let group = self.hunk_groups[self.hunk_cursor];
                if group > 0 {
                    self.split_msg_cursor = group;
                    self.split_editing_msg = true;
                    self.status = format!("editing message for group {} (Esc=done)", group);
                }
            }
            KeyCode::Enter => {
                // finalize split
                self.finalize_split();
                self.mode = Mode::CommitList;
            }
            KeyCode::Esc => {
                self.mode = Mode::CommitList;
                self.status.clear();
            }
            _ => {}
        }
    }

    fn finalize_split(&mut self) {
        let max_group = self.hunk_groups.iter().copied().max().unwrap_or(0);
        if max_group == 0 {
            self.status = "no hunks assigned to groups".into();
            return;
        }

        let mut groups = Vec::new();
        for g in 1..=max_group {
            let indices: Vec<usize> = self.hunk_groups
                .iter()
                .enumerate()
                .filter(|(_, &grp)| grp == g)
                .map(|(i, _)| i)
                .collect();
            if indices.is_empty() {
                continue;
            }
            let msg = if g < self.split_messages.len() && !self.split_messages[g].is_empty() {
                self.split_messages[g].clone()
            } else {
                format!("split part {}", g)
            };
            groups.push(SplitGroup {
                hunk_indices: indices,
                message: msg,
            });
        }

        self.entries[self.cursor] = TodoEntry {
            original_idx: self.cursor,
            action: RebaseAction::Split { groups },
        };
        self.status = format!("split {} into {} parts", self.commits[self.cursor].short_id, max_group);
    }

    // --- SquashTarget mode ---
    fn handle_squash_target(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.cursor = (self.cursor + 1).min(self.commits.len().saturating_sub(1));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
            }
            KeyCode::Enter => {
                if self.cursor != self.squash_source {
                    self.entries[self.squash_source] = TodoEntry {
                        original_idx: self.squash_source,
                        action: RebaseAction::Squash {
                            into_idx: self.cursor,
                            message: None,
                        },
                    };
                    self.status = format!(
                        "squash {} into {}",
                        self.commits[self.squash_source].short_id,
                        self.commits[self.cursor].short_id
                    );
                }
                self.mode = Mode::CommitList;
            }
            KeyCode::Esc => {
                self.cursor = self.squash_source;
                self.mode = Mode::CommitList;
                self.status.clear();
            }
            _ => {}
        }
    }

    // --- ReorderMode ---
    fn handle_reorder(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        match code {
            KeyCode::Char('J') | KeyCode::Char('j') if modifiers.contains(KeyModifiers::SHIFT) || code == KeyCode::Char('J') => {
                // move down
                if self.cursor < self.entries.len() - 1 {
                    self.entries.swap(self.cursor, self.cursor + 1);
                    self.commits.swap(self.cursor, self.cursor + 1);
                    self.selected.swap(self.cursor, self.cursor + 1);
                    self.cursor += 1;
                }
            }
            KeyCode::Char('K') | KeyCode::Char('k') if modifiers.contains(KeyModifiers::SHIFT) || code == KeyCode::Char('K') => {
                // move up
                if self.cursor > 0 {
                    self.entries.swap(self.cursor, self.cursor - 1);
                    self.commits.swap(self.cursor, self.cursor - 1);
                    self.selected.swap(self.cursor, self.cursor - 1);
                    self.cursor -= 1;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.cursor = (self.cursor + 1).min(self.commits.len().saturating_sub(1));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
            }
            KeyCode::Esc | KeyCode::Enter => {
                self.mode = Mode::CommitList;
                self.status = "reorder applied".into();
            }
            _ => {}
        }
    }

    // --- Preview mode ---
    fn handle_preview(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('y') | KeyCode::Enter => {
                self.result = Some(CraftResult::Execute(self.entries.clone(), self.hunks_cache.clone()));
                self.should_quit = true;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.diff_scroll = self.diff_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.diff_scroll = self.diff_scroll.saturating_sub(1);
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = Mode::CommitList;
            }
            _ => {}
        }
    }

    fn has_actions(&self) -> bool {
        self.entries.iter().any(|e| !matches!(e.action, RebaseAction::Pick))
    }

    fn action_label(&self, idx: usize) -> &str {
        match &self.entries[idx].action {
            RebaseAction::Pick => "",
            RebaseAction::Reword(_) => "reword",
            RebaseAction::Squash { .. } => "squash",
            RebaseAction::Fixup { .. } => "fixup",
            RebaseAction::Drop => "DROP",
            RebaseAction::Split { .. } => "split",
            RebaseAction::Edit => "edit",
        }
    }
}

pub fn run_craft_tui(commits: Vec<CommitInfo>, repo: &Repository) -> Result<CraftResult> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new(commits);

    loop {
        terminal.draw(|f| ui(f, &app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code, key.modifiers, repo);
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(app.result.unwrap_or(CraftResult::Cancel))
}

// ─── Rendering ──────────────────────────────────────────────

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Min(10),   // main
            Constraint::Length(3), // footer
        ])
        .split(f.size());

    render_header(f, app, chunks[0]);

    match app.mode {
        Mode::CommitList | Mode::ActionMenu | Mode::SquashTarget | Mode::ReorderMode => {
            render_main_split(f, app, chunks[1]);
        }
        Mode::RewordEdit => {
            render_reword_editor(f, app, chunks[1]);
        }
        Mode::SplitView => {
            render_split_view(f, app, chunks[1]);
        }
        Mode::Preview => {
            render_preview(f, app, chunks[1]);
        }
    }

    render_footer(f, app, chunks[2]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let action_count = app.entries.iter().filter(|e| !matches!(e.action, RebaseAction::Pick)).count();
    let title = format!(
        " CRAFT  {} commits  {} action(s) ",
        app.commits.len(),
        action_count,
    );

    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).bold())
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(header, area);
}

fn render_main_split(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(55),
            Constraint::Percentage(45),
        ])
        .split(area);

    render_commit_list(f, app, chunks[0]);
    render_context_panel(f, app, chunks[1]);
}

fn render_commit_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app.commits
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let num = format!("{:>3}", i + 1);
            let action = app.action_label(i);
            let action_str = if action.is_empty() {
                String::new()
            } else {
                format!(" [{}]", action)
            };
            let time = format_relative_time(&c.time);

            let text = format!(
                " {} {} {} {}{}",
                num, c.short_id, truncate(&c.message, 35), time, action_str,
            );

            let style = if i == app.cursor {
                match app.mode {
                    Mode::SquashTarget => Style::default().bg(Color::Magenta).fg(Color::White),
                    Mode::ReorderMode => Style::default().bg(Color::Blue).fg(Color::White),
                    _ => Style::default().bg(Color::DarkGray).fg(Color::White),
                }
            } else if !action.is_empty() {
                match &app.entries[i].action {
                    RebaseAction::Drop => Style::default().fg(Color::Red).add_modifier(Modifier::CROSSED_OUT),
                    RebaseAction::Reword(_) => Style::default().fg(Color::Yellow),
                    RebaseAction::Squash { .. } => Style::default().fg(Color::Magenta),
                    RebaseAction::Fixup { .. } => Style::default().fg(Color::Magenta),
                    RebaseAction::Split { .. } => Style::default().fg(Color::Cyan),
                    RebaseAction::Edit => Style::default().fg(Color::Green),
                    _ => Style::default(),
                }
            } else {
                Style::default()
            };

            ListItem::new(text).style(style)
        })
        .collect();

    let border_style = match app.mode {
        Mode::ReorderMode => Style::default().fg(Color::Blue),
        Mode::SquashTarget => Style::default().fg(Color::Magenta),
        _ => Style::default().fg(Color::Cyan),
    };

    let title = match app.mode {
        Mode::ReorderMode => " Commits (REORDER) ",
        Mode::SquashTarget => " Commits (SQUASH TARGET) ",
        _ => " Commits ",
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title),
    );

    f.render_widget(list, area);
}

fn render_context_panel(f: &mut Frame, app: &App, area: Rect) {
    if !app.diff_text.is_empty() {
        // show diff
        let lines: Vec<Line> = app.diff_text
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
                    .title(" Diff Preview "),
            );
        f.render_widget(widget, area);
    } else if app.mode == Mode::ActionMenu {
        // show action menu
        let menu_text = vec![
            Line::from(""),
            Line::styled("  r  reword message", Style::default().fg(Color::Yellow)),
            Line::styled("  s  split into hunks", Style::default().fg(Color::Cyan)),
            Line::styled("  q  squash into another", Style::default().fg(Color::Magenta)),
            Line::styled("  f  fixup (squash, keep older msg)", Style::default().fg(Color::Magenta)),
            Line::styled("  d  drop commit", Style::default().fg(Color::Red)),
            Line::styled("  m  reorder commits", Style::default().fg(Color::Blue)),
            Line::styled("  e  edit (stop for manual)", Style::default().fg(Color::Green)),
            Line::styled("  x  reset to pick", Style::default()),
            Line::from(""),
            Line::styled("  Esc  back", Style::default().fg(Color::DarkGray)),
        ];

        let widget = Paragraph::new(menu_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title(" Actions "),
            );
        f.render_widget(widget, area);
    } else {
        // show commit details
        let c = &app.commits[app.cursor];
        let details = vec![
            Line::from(""),
            Line::styled(format!("  SHA: {}", c.id), Style::default().fg(Color::Yellow)),
            Line::from(format!("  Author: {}", c.author)),
            Line::from(format!("  Time: {}", format_relative_time(&c.time))),
            Line::from(""),
            Line::from(format!("  {}", c.message)),
            Line::from(""),
            Line::styled("  D=show diff  Enter=actions  p=preview plan", Style::default().fg(Color::DarkGray)),
        ];

        let widget = Paragraph::new(details)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Details "),
            );
        f.render_widget(widget, area);
    }
}

fn render_reword_editor(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
        ])
        .split(area);

    // Original message
    let original = Paragraph::new(format!("  Original: {}", app.commits[app.cursor].message))
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).title(" Original "));
    f.render_widget(original, chunks[0]);

    // Editor
    let (before, after) = app.reword_buffer.split_at(app.reword_cursor.min(app.reword_buffer.len()));
    let text = format!("{}|{}", before, after);

    let editor = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .title(" New Message (Esc=save) "),
        );
    f.render_widget(editor, chunks[1]);
}

fn render_split_view(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    // Hunk list
    let items: Vec<ListItem> = app.hunks
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let group = app.hunk_groups[i];
            let group_str = if group > 0 {
                format!("[G{}]", group)
            } else {
                "    ".to_string()
            };

            let text = format!(" {} {} {}", group_str, h.summary(), h.header);
            let style = if i == app.hunk_cursor {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else if group > 0 {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let hunk_list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Hunks "),
    );
    f.render_widget(hunk_list, chunks[0]);

    // Hunk detail / group messages
    if app.split_editing_msg {
        let idx = app.split_msg_cursor;
        let msg = &app.split_messages[idx];
        let text = format!("Group {} message:\n\n{}_", idx, msg);
        let widget = Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green))
                    .title(" Group Message "),
            );
        f.render_widget(widget, chunks[1]);
    } else if !app.hunks.is_empty() {
        // Show current hunk lines
        let hunk = &app.hunks[app.hunk_cursor];
        let lines: Vec<Line> = hunk.lines
            .iter()
            .map(|l| match l {
                super::split::DiffLine::Added(s) => {
                    Line::styled(format!("+{}", s.trim_end()), Style::default().fg(Color::Green))
                }
                super::split::DiffLine::Removed(s) => {
                    Line::styled(format!("-{}", s.trim_end()), Style::default().fg(Color::Red))
                }
                super::split::DiffLine::Context(s) => {
                    Line::from(format!(" {}", s.trim_end()))
                }
            })
            .collect();

        let widget = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", hunk.file_path)),
            );
        f.render_widget(widget, chunks[1]);
    }
}

fn render_preview(f: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::styled("  Rebase Plan:", Style::default().bold()));
    lines.push(Line::from(""));

    for entry in &app.entries {
        let c = &app.commits[entry.original_idx];
        let action_style = match &entry.action {
            RebaseAction::Pick => Style::default().fg(Color::DarkGray),
            RebaseAction::Reword(_) => Style::default().fg(Color::Yellow),
            RebaseAction::Squash { .. } => Style::default().fg(Color::Magenta),
            RebaseAction::Fixup { .. } => Style::default().fg(Color::Magenta),
            RebaseAction::Drop => Style::default().fg(Color::Red),
            RebaseAction::Split { .. } => Style::default().fg(Color::Cyan),
            RebaseAction::Edit => Style::default().fg(Color::Green),
        };

        let detail = match &entry.action {
            RebaseAction::Reword(msg) => format!(" -> \"{}\"", truncate(msg, 40)),
            RebaseAction::Squash { into_idx, .. } => {
                format!(" -> into {}", app.commits[*into_idx].short_id)
            }
            RebaseAction::Fixup { into_idx } => {
                format!(" -> into {}", app.commits[*into_idx].short_id)
            }
            RebaseAction::Split { groups } => format!(" -> {} parts", groups.len()),
            _ => String::new(),
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:>7}", entry.action),
                action_style,
            ),
            Span::from(format!(" {} {}", c.short_id, truncate(&c.message, 30))),
            Span::styled(detail, action_style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::styled(
        "  y/Enter=execute  Esc=back",
        Style::default().fg(Color::DarkGray),
    ));

    let widget = Paragraph::new(lines)
        .scroll((app.diff_scroll, 0))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .title(" Preview Plan "),
        );

    f.render_widget(widget, area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let help = match app.mode {
        Mode::CommitList => "j/k:nav  Enter:actions  D:diff  p:preview  q:quit",
        Mode::ActionMenu => "r:reword s:split q:squash f:fixup d:drop m:reorder e:edit x:reset  Esc:back",
        Mode::RewordEdit => "type to edit  Esc:save and return",
        Mode::SplitView if app.split_editing_msg => "type message  Esc/Enter:done",
        Mode::SplitView => "j/k:nav  space:toggle  1-9:assign  g:new group  n:name group  Enter:done",
        Mode::SquashTarget => "j/k:select target  Enter:confirm  Esc:cancel",
        Mode::ReorderMode => "J/K:move commit  j/k:nav  Esc/Enter:done",
        Mode::Preview => "y/Enter:execute  j/k:scroll  Esc:back",
    };

    let status_text = if app.status.is_empty() {
        help.to_string()
    } else {
        format!("{} | {}", app.status, help)
    };

    let footer = Paragraph::new(format!(" {}", status_text))
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(footer, area);
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
