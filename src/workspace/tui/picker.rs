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
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io::stdout;
use std::path::PathBuf;

use crate::workspace::groups::{contract_path, expand_path, Group, GroupsFile};
use crate::workspace::scan::{discover_repos, ScanOptions};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Stage {
    Root,
    Select,
    Alias,
}

pub struct PickerApp {
    stage: Stage,
    root_input: String,
    current_root: Option<PathBuf>,
    all_repos: Vec<PathBuf>,
    search: String,
    filtered: Vec<usize>,
    list_state: ListState,
    selected: std::collections::HashSet<usize>,
    alias_input: String,
    error: Option<String>,
    done: Option<Group>,
    should_quit: bool,
}

impl PickerApp {
    pub fn new(initial_root: Option<String>, initial_alias: Option<String>) -> Self {
        let root_input = initial_root.unwrap_or_else(|| "~/Developer".to_string());
        Self {
            stage: Stage::Root,
            root_input,
            current_root: None,
            all_repos: Vec::new(),
            search: String::new(),
            filtered: Vec::new(),
            list_state: ListState::default(),
            selected: std::collections::HashSet::new(),
            alias_input: initial_alias.unwrap_or_default(),
            error: None,
            done: None,
            should_quit: false,
        }
    }

    fn scan(&mut self) {
        let root = expand_path(&self.root_input);
        if !root.exists() {
            self.error = Some(format!("path not found: {}", root.display()));
            return;
        }
        let opts = ScanOptions::default();
        self.all_repos = discover_repos(&root, &opts);
        self.current_root = Some(root);
        self.refilter();
        if !self.filtered.is_empty() {
            self.list_state.select(Some(0));
        }
        self.stage = Stage::Select;
        self.error = None;
    }

    fn refilter(&mut self) {
        let q = self.search.to_lowercase();
        self.filtered = self
            .all_repos
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                if q.is_empty() {
                    return true;
                }
                let name = p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let path = p.to_string_lossy().to_lowercase();
                name.contains(&q) || path.contains(&q)
            })
            .map(|(i, _)| i)
            .collect();
        if self
            .list_state
            .selected()
            .map(|s| s >= self.filtered.len())
            .unwrap_or(false)
        {
            self.list_state
                .select(if self.filtered.is_empty() { None } else { Some(0) });
        }
    }

    fn toggle_current(&mut self) {
        if let Some(sel) = self.list_state.selected() {
            if let Some(&global_i) = self.filtered.get(sel) {
                if !self.selected.insert(global_i) {
                    self.selected.remove(&global_i);
                }
            }
        }
    }

    fn move_selection(&mut self, delta: i32) {
        if self.filtered.is_empty() {
            return;
        }
        let cur = self.list_state.selected().unwrap_or(0) as i32;
        let new = (cur + delta).clamp(0, self.filtered.len() as i32 - 1);
        self.list_state.select(Some(new as usize));
    }

    fn finalize(&mut self) {
        if self.alias_input.trim().is_empty() {
            self.error = Some("alias required".into());
            return;
        }
        let Some(root) = self.current_root.clone() else {
            self.error = Some("no scan root".into());
            return;
        };

        // Compute pinned (selected repos outside scan) and unpinned (scan repos excluded)
        let selected_paths: Vec<PathBuf> = self
            .selected
            .iter()
            .filter_map(|&i| self.all_repos.get(i).cloned())
            .collect();

        // In this simple flow: save scan_root + unpinned list for anything UNSELECTED
        // Selected set is the desired group; treat scan_root as source of truth
        let unselected: Vec<PathBuf> = self
            .all_repos
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.selected.contains(i))
            .map(|(_, p)| p.clone())
            .collect();

        // If user selected nothing, that's probably a mistake
        if selected_paths.is_empty() {
            self.error = Some("select at least one repo (Space)".into());
            return;
        }

        let group = Group {
            alias: self.alias_input.trim().to_string(),
            scan_root: Some(contract_path(&root)),
            max_depth: 3,
            exclude: Vec::new(),
            pinned: Vec::new(),
            unpinned: unselected.iter().map(|p| contract_path(p)).collect(),
        };
        self.done = Some(group);
        self.should_quit = true;
    }
}

pub fn run_picker(
    initial_root: Option<String>,
    initial_alias: Option<String>,
) -> Result<Option<Group>> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    let mut app = PickerApp::new(initial_root, initial_alias);

    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result?;
    Ok(app.done)
}

fn run_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut PickerApp,
) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|f| render(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                app.should_quit = true;
                app.done = None;
                continue;
            }
            handle_key(app, key);
        }
    }
    Ok(())
}

fn handle_key(app: &mut PickerApp, key: crossterm::event::KeyEvent) {
    match app.stage {
        Stage::Root => match key.code {
            KeyCode::Esc => {
                app.should_quit = true;
            }
            KeyCode::Enter => app.scan(),
            KeyCode::Backspace => {
                app.root_input.pop();
            }
            KeyCode::Char(c) => app.root_input.push(c),
            _ => {}
        },
        Stage::Select => match key.code {
            KeyCode::Esc => app.stage = Stage::Root,
            KeyCode::Enter => {
                if !app.selected.is_empty() {
                    app.stage = Stage::Alias;
                    app.error = None;
                } else {
                    app.error = Some("select at least one repo (Space)".into());
                }
            }
            KeyCode::Char(' ') => app.toggle_current(),
            KeyCode::Tab => {
                // select all filtered
                for &i in &app.filtered {
                    app.selected.insert(i);
                }
            }
            KeyCode::BackTab => {
                for &i in &app.filtered {
                    app.selected.remove(&i);
                }
            }
            KeyCode::Down => app.move_selection(1),
            KeyCode::Up => app.move_selection(-1),
            KeyCode::PageDown => app.move_selection(10),
            KeyCode::PageUp => app.move_selection(-10),
            KeyCode::Backspace => {
                app.search.pop();
                app.refilter();
            }
            KeyCode::Char(c) => {
                app.search.push(c);
                app.refilter();
            }
            _ => {}
        },
        Stage::Alias => match key.code {
            KeyCode::Esc => app.stage = Stage::Select,
            KeyCode::Enter => app.finalize(),
            KeyCode::Backspace => {
                app.alias_input.pop();
            }
            KeyCode::Char(c) => app.alias_input.push(c),
            _ => {}
        },
    }
}

fn render(f: &mut Frame, app: &PickerApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.size());

    let title = match app.stage {
        Stage::Root => "Create Group · step 1/3: scan root",
        Stage::Select => "Create Group · step 2/3: pick repos",
        Stage::Alias => "Create Group · step 3/3: alias",
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            title,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))),
        chunks[0],
    );

    // Root input
    let root_border = if app.stage == Stage::Root {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let root_text = format!(" {}{}", app.root_input, if app.stage == Stage::Root { "▌" } else { "" });
    f.render_widget(
        Paragraph::new(root_text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(root_border))
                .title(" scan root "),
        ),
        chunks[1],
    );

    // Search input
    let search_border = if app.stage == Stage::Select {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let search_text = format!(
        " {}{}  ·  {} selected",
        app.search,
        if app.stage == Stage::Select { "▌" } else { "" },
        app.selected.len()
    );
    f.render_widget(
        Paragraph::new(search_text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(search_border))
                .title(" search "),
        ),
        chunks[2],
    );

    // Body
    match app.stage {
        Stage::Root => render_root_hint(f, chunks[3], app),
        Stage::Select => render_list(f, chunks[3], app),
        Stage::Alias => render_alias(f, chunks[3], app),
    }

    // Footer
    let help = match app.stage {
        Stage::Root => "Enter: scan  Esc: cancel",
        Stage::Select => "Space: toggle  Tab: all  S-Tab: none  Enter: next  Esc: back",
        Stage::Alias => "Enter: save  Esc: back",
    };
    let footer_line = if let Some(err) = &app.error {
        Line::from(vec![
            Span::styled("! ", Style::default().fg(Color::Red)),
            Span::styled(err.clone(), Style::default().fg(Color::Red)),
            Span::raw("  "),
            Span::styled(help, Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(Span::styled(help, Style::default().fg(Color::DarkGray)))
    };
    f.render_widget(Paragraph::new(footer_line), chunks[4]);
}

fn render_root_hint(f: &mut Frame, area: Rect, _app: &PickerApp) {
    f.render_widget(
        Paragraph::new("Enter a directory to scan. Common: ~/Developer/PROJECTS")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray))),
        area,
    );
}

fn render_list(f: &mut Frame, area: Rect, app: &PickerApp) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(format!(" {} repos ", app.filtered.len()));

    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&i| {
            let path = &app.all_repos[i];
            let checked = if app.selected.contains(&i) { "[x]" } else { "[ ]" };
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("?");
            let rel = contract_path(path);
            ListItem::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    checked.to_string(),
                    Style::default().fg(if app.selected.contains(&i) {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::raw("  "),
                Span::styled(name.to_string(), Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(rel, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    let mut state = app.list_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

fn render_alias(f: &mut Frame, area: Rect, app: &PickerApp) {
    let line = Line::from(vec![
        Span::raw(" alias: "),
        Span::styled(
            app.alias_input.clone(),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled("▌", Style::default().fg(Color::Cyan)),
    ]);
    f.render_widget(
        Paragraph::new(vec![
            line,
            Line::from(""),
            Line::from(Span::styled(
                format!("{} repos selected · scan root will be saved", app.selected.len()),
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" alias "),
        ),
        area,
    );
}

/// High-level entry: run picker, write to groups.toml on success
pub fn create_group_interactive(initial_root: Option<String>) -> Result<Option<String>> {
    let Some(group) = run_picker(initial_root, None)? else {
        return Ok(None);
    };
    let mut file = GroupsFile::load().unwrap_or_default();
    let alias = group.alias.clone();
    file.upsert(group);
    file.save()?;
    Ok(Some(alias))
}

pub fn edit_group_interactive(alias: &str) -> Result<bool> {
    let file = GroupsFile::load().unwrap_or_default();
    let Some(existing) = file.find(alias).cloned() else {
        return Ok(false);
    };
    let Some(group) = run_picker(existing.scan_root.clone(), Some(alias.to_string()))? else {
        return Ok(false);
    };
    let mut file = file;
    file.upsert(group);
    file.save()?;
    Ok(true)
}
