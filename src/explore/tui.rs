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
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame, Terminal,
};
use std::io::stdout;

use git2::Repository;

use crate::config::Config;
use crate::models::RepoSummary;

use super::ai::AiState;
use super::branches::tree::build_branch_tree;
use super::branches::BranchesState;
use super::highlight::{self, Highlighter};
use super::history::HistoryState;
use super::layout::LayoutMode;
use super::overlay::{render_overlay, Overlay};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Summary,
    History,
    Branches,
}

impl Tab {
    fn index(&self) -> usize {
        match self {
            Tab::Summary => 0,
            Tab::History => 1,
            Tab::Branches => 2,
        }
    }
}

pub struct ExploreApp {
    pub active_tab: Tab,
    pub summary: RepoSummary,
    pub history: HistoryState,
    pub branches: BranchesState,
    pub ai: AiState,
    pub overlay: Option<Overlay>,
    pub layout: LayoutMode,
    pub should_quit: bool,
    pub status_message: String,
    pub repo_path: String,
    pub highlighter: Highlighter,
}

impl ExploreApp {
    pub fn new(
        repo: &Repository,
        summary: RepoSummary,
        tab: Option<String>,
        page_size: usize,
        config: &Config,
    ) -> Result<Self> {
        let repo_path = repo
            .workdir()
            .unwrap_or_else(|| repo.path())
            .to_string_lossy()
            .trim_end_matches('/')
            .to_string();

        let initial_tab = match tab.as_deref() {
            Some("summary") | Some("s") => Tab::Summary,
            Some("history") | Some("h") => Tab::History,
            Some("branches") | Some("b") => Tab::Branches,
            None => Tab::History,
            _ => Tab::History,
        };

        let initial_commits = crate::git::get_recent_commits(repo, page_size)?;

        let tree_nodes =
            build_branch_tree(repo, &summary.local_branches, config.stale_branch_days)?;

        Ok(Self {
            active_tab: initial_tab,
            history: HistoryState::new(initial_commits, page_size),
            branches: BranchesState::new(tree_nodes, summary.remote_branches.clone()),
            ai: AiState::new(config),
            summary,
            overlay: None,
            layout: LayoutMode::Large,
            should_quit: false,
            status_message: String::new(),
            repo_path,
            highlighter: Highlighter::new(),
        })
    }
}

pub fn run_explore_tui(
    repo: Repository,
    summary: RepoSummary,
    tab: Option<String>,
    page_size: usize,
    config: &Config,
) -> Result<()> {
    let mut app = ExploreApp::new(&repo, summary, tab, page_size, config)?;

    // Pre-load diff for first commit
    load_commit_data(&mut app, &repo);

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let result = run_event_loop(&mut terminal, &mut app, &repo);

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;

    result
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut ExploreApp,
    repo: &Repository,
) -> Result<()> {
    loop {
        terminal.draw(|f| render(f, app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key(app, key.code, key.modifiers, repo);
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

// ── Key handling ──

fn handle_key(app: &mut ExploreApp, code: KeyCode, modifiers: KeyModifiers, repo: &Repository) {
    if app.overlay.is_some() {
        handle_overlay_key(app, code, repo);
        return;
    }

    if app.active_tab == Tab::History && app.history.filter_active {
        handle_filter_key(app, code);
        return;
    }
    if app.active_tab == Tab::Branches && app.branches.search_active {
        handle_branch_search_key(app, code);
        return;
    }

    match code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Tab => {
            app.active_tab = match app.active_tab {
                Tab::Summary => Tab::History,
                Tab::History => Tab::Branches,
                Tab::Branches => Tab::Summary,
            };
        }
        KeyCode::BackTab => {
            app.active_tab = match app.active_tab {
                Tab::Summary => Tab::Branches,
                Tab::History => Tab::Summary,
                Tab::Branches => Tab::History,
            };
        }
        KeyCode::Char('1') => app.active_tab = Tab::Summary,
        KeyCode::Char('2') => app.active_tab = Tab::History,
        KeyCode::Char('3') => app.active_tab = Tab::Branches,
        KeyCode::Char('?') => app.overlay = Some(Overlay::Help),
        KeyCode::Char('/') => match app.active_tab {
            Tab::History => app.history.filter_active = true,
            Tab::Branches => app.branches.search_active = true,
            _ => {}
        },
        KeyCode::Char('a') => handle_ai_menu(app),
        KeyCode::Char('A') => {
            if app.ai.has_provider() {
                app.ai.cycle_provider();
                app.status_message = format!("AI: {}", app.ai.provider_display());
            }
        }
        _ => match app.active_tab {
            Tab::Summary => {}
            Tab::History => handle_history_key(app, code, modifiers, repo),
            Tab::Branches => handle_branch_key(app, code, modifiers, repo),
        },
    }
}

fn handle_history_key(
    app: &mut ExploreApp,
    code: KeyCode,
    _modifiers: KeyModifiers,
    repo: &Repository,
) {
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            app.history.move_down();
            if app.history.cursor + 10 >= app.history.visible_count() && !app.history.all_loaded {
                app.history.load_more(repo);
            }
            load_commit_data(app, repo);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.history.move_up();
            load_commit_data(app, repo);
        }
        KeyCode::Char('l') | KeyCode::Right => app.history.cycle_detail_tab(),
        KeyCode::Char('h') | KeyCode::Left => {
            app.history.detail_sub_tab = match app.history.detail_sub_tab {
                super::history::DetailSubTab::Diff => super::history::DetailSubTab::Files,
                super::history::DetailSubTab::Blame => super::history::DetailSubTab::Diff,
                super::history::DetailSubTab::Files => super::history::DetailSubTab::Blame,
            };
        }
        KeyCode::Tab => {
            if app.layout.is_small() {
                app.history.show_detail = !app.history.show_detail;
            }
        }
        KeyCode::Enter => {
            if app.layout.is_small() {
                app.history.show_detail = true;
            }
        }
        KeyCode::Esc => {
            if app.layout.is_small() && app.history.show_detail {
                app.history.show_detail = false;
            }
        }
        KeyCode::Char('x') => {
            if app.history.selected_commit().is_some() {
                app.overlay = Some(Overlay::action_menu(
                    "Actions",
                    vec![
                        ('c', "Cherry-pick".into()),
                        ('b', "Create branch here".into()),
                        ('r', "Revert".into()),
                        ('d', "View full diff".into()),
                        ('y', "Copy hash".into()),
                    ],
                ));
            }
        }
        KeyCode::Char('J') | KeyCode::PageDown => app.history.detail_scroll += 5,
        KeyCode::Char('K') | KeyCode::PageUp => {
            app.history.detail_scroll = app.history.detail_scroll.saturating_sub(5)
        }
        KeyCode::Char('}') => app.history.detail_scroll += 1,
        KeyCode::Char('{') => {
            app.history.detail_scroll = app.history.detail_scroll.saturating_sub(1)
        }
        _ => {}
    }
}

fn handle_branch_key(
    app: &mut ExploreApp,
    code: KeyCode,
    _modifiers: KeyModifiers,
    repo: &Repository,
) {
    match code {
        KeyCode::Char('j') | KeyCode::Down => app.branches.move_down(),
        KeyCode::Char('k') | KeyCode::Up => app.branches.move_up(),
        KeyCode::Char('l') | KeyCode::Right => app.branches.cycle_detail_tab(),
        KeyCode::Char('h') | KeyCode::Left => {
            app.branches.detail_sub_tab = match app.branches.detail_sub_tab {
                super::branches::BranchDetailSubTab::Info => {
                    super::branches::BranchDetailSubTab::Diff
                }
                super::branches::BranchDetailSubTab::Commits => {
                    super::branches::BranchDetailSubTab::Info
                }
                super::branches::BranchDetailSubTab::Diff => {
                    super::branches::BranchDetailSubTab::Commits
                }
            };
        }
        KeyCode::Tab => {
            if app.layout.is_small() {
                app.branches.show_detail = !app.branches.show_detail;
            }
        }
        KeyCode::Esc => {
            if app.branches.compare_mode {
                app.branches.compare_mode = false;
                app.branches.compare_source = None;
                app.status_message = String::new();
            } else if app.layout.is_small() && app.branches.show_detail {
                app.branches.show_detail = false;
            }
        }
        KeyCode::Enter => {
            if app.layout.is_small() {
                app.branches.show_detail = true;
            }
            load_branch_data(app, repo);
        }
        KeyCode::Char('c') => {
            if app.branches.compare_mode {
                // Execute comparison
                let source_idx = app.branches.compare_source.unwrap_or(0);
                let visible = app.branches.visible_nodes();
                if let (Some(a), Some(b)) =
                    (visible.get(source_idx), visible.get(app.branches.cursor))
                {
                    let a_name = a.branch.name.clone();
                    let b_name = b.branch.name.clone();
                    match super::branches::compare::compare_branches(repo, &a_name, &b_name) {
                        Ok(cmp) => {
                            let text = format!(
                                "{} vs {}\n\n{} ahead, {} behind\n\n{}\n\nUnique to {}:\n{}\n\nUnique to {}:\n{}",
                                cmp.branch_a, cmp.branch_b,
                                cmp.a_ahead, cmp.a_behind,
                                cmp.diff_summary,
                                cmp.branch_a,
                                cmp.a_unique_commits.iter().map(|c| format!("  {}", c)).collect::<Vec<_>>().join("\n"),
                                cmp.branch_b,
                                cmp.b_unique_commits.iter().map(|c| format!("  {}", c)).collect::<Vec<_>>().join("\n"),
                            );
                            app.branches.detail_cache =
                                Some((format!("cmp:{}:{}", a_name, b_name), text));
                            app.branches.detail_sub_tab =
                                super::branches::BranchDetailSubTab::Info;
                            app.status_message =
                                format!("Comparing {} vs {}", a_name, b_name);
                        }
                        Err(e) => {
                            app.status_message = format!("Compare error: {}", e);
                        }
                    }
                }
                app.branches.compare_mode = false;
                app.branches.compare_source = None;
            } else {
                app.branches.compare_mode = true;
                app.branches.compare_source = Some(app.branches.cursor);
                app.status_message =
                    "Select branch to compare (c to confirm, Esc to cancel)".to_string();
            }
        }
        KeyCode::Char('f') => app.branches.cycle_filter(),
        KeyCode::Char('s') => app.branches.cycle_sort(),
        KeyCode::Char('x') => {
            if app.branches.selected_branch().is_some() {
                app.overlay = Some(Overlay::action_menu(
                    "Actions",
                    vec![
                        ('c', "Checkout".into()),
                        ('n', "New branch from".into()),
                        ('m', "Merge into current".into()),
                        ('r', "Rebase onto".into()),
                        ('d', "Delete".into()),
                        ('y', "Copy name".into()),
                    ],
                ));
            }
        }
        KeyCode::Char('J') | KeyCode::PageDown => app.branches.detail_scroll += 5,
        KeyCode::Char('K') | KeyCode::PageUp => {
            app.branches.detail_scroll = app.branches.detail_scroll.saturating_sub(5)
        }
        KeyCode::Char('}') => app.branches.detail_scroll += 1,
        KeyCode::Char('{') => {
            app.branches.detail_scroll = app.branches.detail_scroll.saturating_sub(1)
        }
        _ => {}
    }
}

fn handle_filter_key(app: &mut ExploreApp, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.history.filter_active = false;
        }
        KeyCode::Enter => {
            app.history.filter_active = false;
            let filters = super::history::filters::parse_filters(&app.history.filter_input);
            app.history.filters = filters.clone();
            app.history.filtered_indices =
                super::history::filters::apply_client_filters(&app.history.commits, &filters);
            app.history.cursor = 0;
            let count = app.history.filtered_indices.len();
            app.status_message = format!("{} commits match", count);
        }
        KeyCode::Backspace => {
            if app.history.filter_input.is_empty() {
                app.history.filter_active = false;
            } else {
                app.history.filter_input.pop();
            }
        }
        KeyCode::Char(c) => {
            app.history.filter_input.push(c);
        }
        _ => {}
    }
}

fn handle_branch_search_key(app: &mut ExploreApp, code: KeyCode) {
    match code {
        KeyCode::Esc => app.branches.search_active = false,
        KeyCode::Enter => app.branches.search_active = false,
        KeyCode::Backspace => {
            if app.branches.search_input.is_empty() {
                app.branches.search_active = false;
            } else {
                app.branches.search_input.pop();
            }
        }
        KeyCode::Char(c) => app.branches.search_input.push(c),
        _ => {}
    }
}

fn handle_overlay_key(app: &mut ExploreApp, code: KeyCode, repo: &Repository) {
    match &app.overlay {
        Some(Overlay::Help) => {
            app.overlay = None;
        }
        Some(Overlay::ActionMenu { items, .. }) => {
            let items = items.clone();
            match code {
                KeyCode::Esc => app.overlay = None,
                KeyCode::Char(c) => {
                    if let Some((_, label)) = items.iter().find(|(k, _)| *k == c) {
                        let action = label.clone();
                        app.overlay = None;
                        execute_action(app, &action, repo);
                    }
                }
                _ => {}
            }
        }
        Some(Overlay::Confirm { .. }) => match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.overlay = None;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                app.overlay = None;
            }
            _ => {}
        },
        _ => {
            if code == KeyCode::Esc {
                app.overlay = None;
            }
        }
    }
}

fn handle_ai_menu(app: &mut ExploreApp) {
    if !app.ai.has_provider() {
        app.status_message = "No AI provider found (install claude/codex/gemini CLI)".to_string();
        return;
    }

    let items = match app.active_tab {
        Tab::History => vec![
            ('s', "Summarize commit".into()),
            ('e', "Explain changes".into()),
            ('f', "Find (natural language)".into()),
            ('r', "Range summary".into()),
        ],
        Tab::Branches => vec![
            ('s', "Summarize branch".into()),
            ('c', "Compare branches".into()),
            ('d', "Deletion advice".into()),
            ('m', "Merge advice".into()),
        ],
        Tab::Summary => vec![],
    };

    if !items.is_empty() {
        app.overlay = Some(Overlay::action_menu(
            &format!("AI Actions ({})", app.ai.provider_display()),
            items,
        ));
    }
}

fn execute_action(app: &mut ExploreApp, action: &str, repo: &Repository) {
    match action {
        // History actions
        "Cherry-pick" => {
            if let Some(commit) = app.history.selected_commit() {
                let hash = commit.id.to_string();
                match super::branches::actions::cherry_pick(&app.repo_path, &hash, repo) {
                    Ok(super::branches::actions::ActionResult::Success(msg)) => {
                        app.status_message = msg;
                    }
                    Ok(super::branches::actions::ActionResult::Error(msg)) => {
                        app.status_message = msg;
                    }
                    Ok(super::branches::actions::ActionResult::NeedsStash) => {
                        app.status_message = "Working tree dirty — stash first".to_string();
                    }
                    Err(e) => app.status_message = format!("Error: {}", e),
                }
            }
        }
        "Create branch here" => {
            if let Some(commit) = app.history.selected_commit() {
                let ref_name = commit.id.to_string();
                let branch_name = format!("branch-from-{}", &commit.short_id);
                match super::branches::actions::create_branch(
                    &app.repo_path,
                    &branch_name,
                    &ref_name,
                ) {
                    Ok(super::branches::actions::ActionResult::Success(msg)) => {
                        app.status_message = msg;
                    }
                    Ok(super::branches::actions::ActionResult::Error(msg)) => {
                        app.status_message = msg;
                    }
                    _ => {}
                }
            }
        }
        "Revert" => {
            if let Some(commit) = app.history.selected_commit() {
                let hash = commit.id.to_string();
                match super::branches::actions::revert_commit(&app.repo_path, &hash, repo) {
                    Ok(super::branches::actions::ActionResult::Success(msg)) => {
                        app.status_message = msg;
                    }
                    Ok(super::branches::actions::ActionResult::Error(msg)) => {
                        app.status_message = msg;
                    }
                    Ok(super::branches::actions::ActionResult::NeedsStash) => {
                        app.status_message = "Working tree dirty — stash first".to_string();
                    }
                    Err(e) => app.status_message = format!("Error: {}", e),
                }
            }
        }
        "View full diff" => {
            load_commit_data(app, repo);
            app.history.detail_sub_tab = super::history::DetailSubTab::Diff;
        }
        "Copy hash" => {
            if let Some(commit) = app.history.selected_commit() {
                copy_to_clipboard(&commit.id.to_string());
                app.status_message = format!("Copied {}", commit.short_id);
            }
        }
        // Branch actions
        "Checkout" => {
            if let Some(name) = app.branches.selected_branch_name() {
                match super::branches::actions::checkout_branch(&app.repo_path, &name, repo) {
                    Ok(super::branches::actions::ActionResult::Success(msg)) => {
                        app.status_message = msg;
                        refresh_branches(app, repo);
                    }
                    Ok(super::branches::actions::ActionResult::Error(msg)) => {
                        app.status_message = msg;
                    }
                    Ok(super::branches::actions::ActionResult::NeedsStash) => {
                        app.status_message = "Working tree dirty — stash first".to_string();
                    }
                    Err(e) => app.status_message = format!("Error: {}", e),
                }
            }
        }
        "New branch from" => {
            if let Some(node) = app.branches.selected_branch() {
                let from = node.branch.name.clone();
                let new_name = format!("new-from-{}", from);
                match super::branches::actions::create_branch(&app.repo_path, &new_name, &from) {
                    Ok(super::branches::actions::ActionResult::Success(msg)) => {
                        app.status_message = msg;
                        refresh_branches(app, repo);
                    }
                    Ok(super::branches::actions::ActionResult::Error(msg)) => {
                        app.status_message = msg;
                    }
                    _ => {}
                }
            }
        }
        "Merge into current" => {
            if let Some(name) = app.branches.selected_branch_name() {
                match super::branches::actions::merge_branch(&app.repo_path, &name, repo) {
                    Ok(super::branches::actions::ActionResult::Success(msg)) => {
                        app.status_message = msg;
                        refresh_branches(app, repo);
                    }
                    Ok(super::branches::actions::ActionResult::Error(msg)) => {
                        app.status_message = msg;
                    }
                    Ok(super::branches::actions::ActionResult::NeedsStash) => {
                        app.status_message = "Working tree dirty — stash first".to_string();
                    }
                    Err(e) => app.status_message = format!("Error: {}", e),
                }
            }
        }
        "Rebase onto" => {
            if let Some(name) = app.branches.selected_branch_name() {
                match super::branches::actions::rebase_onto(&app.repo_path, &name, repo) {
                    Ok(super::branches::actions::ActionResult::Success(msg)) => {
                        app.status_message = msg;
                        refresh_branches(app, repo);
                    }
                    Ok(super::branches::actions::ActionResult::Error(msg)) => {
                        app.status_message = msg;
                    }
                    Ok(super::branches::actions::ActionResult::NeedsStash) => {
                        app.status_message = "Working tree dirty — stash first".to_string();
                    }
                    Err(e) => app.status_message = format!("Error: {}", e),
                }
            }
        }
        "Delete" => {
            if let Some(node) = app.branches.selected_branch() {
                if node.branch.is_head {
                    app.status_message = "Cannot delete current branch".to_string();
                    return;
                }
                let name = node.branch.name.clone();
                let force = !node.is_merged;
                match super::branches::actions::delete_branch(&app.repo_path, &name, force) {
                    Ok(super::branches::actions::ActionResult::Success(msg)) => {
                        app.status_message = msg;
                        refresh_branches(app, repo);
                    }
                    Ok(super::branches::actions::ActionResult::Error(msg)) => {
                        app.status_message = msg;
                    }
                    _ => {}
                }
            }
        }
        "Copy name" => {
            if let Some(name) = app.branches.selected_branch_name() {
                copy_to_clipboard(&name);
                app.status_message = format!("Copied {}", name);
            }
        }
        // AI actions
        "Summarize commit" => execute_ai_action(app, "summarize_commit", repo),
        "Explain changes" => execute_ai_action(app, "explain_commit", repo),
        "Find (natural language)" => execute_ai_action(app, "nl_search", repo),
        "Range summary" => execute_ai_action(app, "range_summary", repo),
        "Summarize branch" => execute_ai_action(app, "summarize_branch", repo),
        "Compare branches" => execute_ai_action(app, "compare_branches", repo),
        "Deletion advice" => execute_ai_action(app, "deletion_advice", repo),
        "Merge advice" => execute_ai_action(app, "merge_advice", repo),
        _ => {
            app.status_message = format!("Unknown action: {}", action);
        }
    }
}

fn execute_ai_action(app: &mut ExploreApp, action: &str, repo: &Repository) {
    let Some(provider) = app.ai.provider else {
        app.status_message = "No AI provider available".to_string();
        return;
    };

    let context = match action {
        "summarize_commit" | "explain_commit" => {
            if let Some(oid) = app.history.selected_oid() {
                // Check cache
                let cache_key = if action == "summarize_commit" {
                    super::ai::AiCacheKey::CommitSummary(oid.to_string())
                } else {
                    super::ai::AiCacheKey::CommitExplain(oid.to_string())
                };
                if let Some(cached) = app.ai.get_cached(&cache_key) {
                    app.ai.last_result = Some(cached.clone());
                    app.status_message = format!("AI (cached): {}", action);
                    return;
                }
                match crate::git::get_commit_diff(repo, oid) {
                    Ok(diff) => diff,
                    Err(e) => {
                        app.status_message = format!("diff error: {}", e);
                        return;
                    }
                }
            } else {
                app.status_message = "No commit selected".to_string();
                return;
            }
        }
        "summarize_branch" => {
            if let Some(node) = app.branches.selected_branch() {
                let name = node.branch.name.clone();
                let cache_key = super::ai::AiCacheKey::BranchSummary(name.clone());
                if let Some(cached) = app.ai.get_cached(&cache_key) {
                    app.ai.last_result = Some(cached.clone());
                    app.status_message = format!("AI (cached): {}", action);
                    return;
                }
                // Gather unique commits as context
                if let Some(tip) = node.branch.tip_commit {
                    let mut revwalk = match repo.revwalk() {
                        Ok(rw) => rw,
                        Err(_) => return,
                    };
                    let _ = revwalk.push(tip);
                    let _ = revwalk.set_sorting(git2::Sort::TIME);
                    let msgs: Vec<String> = revwalk
                        .take(20)
                        .filter_map(|oid| oid.ok())
                        .filter_map(|oid| repo.find_commit(oid).ok())
                        .map(|c| c.summary().unwrap_or("").to_string())
                        .collect();
                    format!("Branch: {}\n\n{}", name, msgs.join("\n"))
                } else {
                    return;
                }
            } else {
                return;
            }
        }
        "deletion_advice" => {
            let cache_key = super::ai::AiCacheKey::DeletionAdvice;
            if let Some(cached) = app.ai.get_cached(&cache_key) {
                app.ai.last_result = Some(cached.clone());
                return;
            }
            app.branches
                .tree_nodes
                .iter()
                .map(|n| {
                    format!(
                        "{}: merged={}, stale={}, commits={}",
                        n.branch.name, n.is_merged, n.is_stale, n.unique_commits
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        "merge_advice" => {
            let cache_key = super::ai::AiCacheKey::MergeAdvice;
            if let Some(cached) = app.ai.get_cached(&cache_key) {
                app.ai.last_result = Some(cached.clone());
                return;
            }
            app.branches
                .tree_nodes
                .iter()
                .map(|n| {
                    let upstream = n
                        .branch
                        .upstream
                        .as_ref()
                        .map(|u| format!("ahead={} behind={}", u.ahead, u.behind))
                        .unwrap_or_else(|| "no upstream".to_string());
                    format!(
                        "{}: merged={}, stale={}, {}",
                        n.branch.name, n.is_merged, n.is_stale, upstream
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => {
            app.status_message = format!("AI action not yet implemented: {}", action);
            return;
        }
    };

    app.status_message = format!("AI thinking ({})...", provider.name());

    let prompt = super::ai::build_prompt(action, &context);
    let model = app.ai.model.as_deref();
    match super::ai::run_ai_query(provider, &prompt, model) {
        Ok(result) => {
            // Cache the result
            let cache_key = match action {
                "summarize_commit" => {
                    if let Some(oid) = app.history.selected_oid() {
                        Some(super::ai::AiCacheKey::CommitSummary(oid.to_string()))
                    } else {
                        None
                    }
                }
                "explain_commit" => {
                    if let Some(oid) = app.history.selected_oid() {
                        Some(super::ai::AiCacheKey::CommitExplain(oid.to_string()))
                    } else {
                        None
                    }
                }
                "summarize_branch" => app
                    .branches
                    .selected_branch_name()
                    .map(super::ai::AiCacheKey::BranchSummary),
                "deletion_advice" => Some(super::ai::AiCacheKey::DeletionAdvice),
                "merge_advice" => Some(super::ai::AiCacheKey::MergeAdvice),
                _ => None,
            };
            if let Some(key) = cache_key {
                app.ai.set_cached(key, result.clone());
            }
            app.ai.last_result = Some(result);
            app.status_message = format!("AI: {}", action);
        }
        Err(e) => {
            app.ai.last_error = Some(e.to_string());
            app.status_message = format!("AI error: {}", e);
        }
    }
}

fn copy_to_clipboard(text: &str) {
    let _ = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(text.as_bytes())?;
            }
            child.wait()
        });
}

fn refresh_branches(app: &mut ExploreApp, repo: &Repository) {
    if let Ok(branches) = crate::git::get_local_branches(repo) {
        let stale_days = 30;
        if let Ok(nodes) = super::branches::tree::build_branch_tree(repo, &branches, stale_days) {
            app.branches.tree_nodes = nodes;
            app.branches.cursor = 0;
        }
    }
}

fn load_branch_data(app: &mut ExploreApp, repo: &Repository) {
    if let Some(node) = app.branches.selected_branch() {
        let name = node.branch.name.clone();

        // Load commits for Commits sub-tab
        if app
            .branches
            .commits_cache
            .as_ref()
            .map(|(n, _)| n.as_str())
            != Some(&name)
        {
            if let Some(tip) = node.branch.tip_commit {
                let mut revwalk = match repo.revwalk() {
                    Ok(rw) => rw,
                    Err(_) => return,
                };
                let _ = revwalk.push(tip);
                let _ = revwalk.set_sorting(git2::Sort::TIME);
                let commits: Vec<String> = revwalk
                    .take(50)
                    .filter_map(|oid| oid.ok())
                    .filter_map(|oid| {
                        repo.find_commit(oid).ok().map(|c| {
                            let short = oid.to_string()[..7].to_string();
                            let msg = c.summary().unwrap_or("").to_string();
                            format!("{} {}", short, msg)
                        })
                    })
                    .collect();
                app.branches.commits_cache = Some((name.clone(), commits));
            }
        }

        // Load diff for Diff sub-tab (branch vs main)
        if app
            .branches
            .diff_cache
            .as_ref()
            .map(|(n, _)| n.as_str())
            != Some(&name)
        {
            let main_branch = app
                .branches
                .tree_nodes
                .iter()
                .find(|n| n.depth == 0)
                .map(|n| n.branch.name.clone());

            if let Some(main_name) = main_branch {
                if name != main_name {
                    if let (Ok(branch_ref), Ok(main_ref)) = (
                        repo.find_branch(&name, git2::BranchType::Local),
                        repo.find_branch(&main_name, git2::BranchType::Local),
                    ) {
                        if let (Some(b_oid), Some(m_oid)) =
                            (branch_ref.get().target(), main_ref.get().target())
                        {
                            if let (Ok(b_commit), Ok(m_commit)) =
                                (repo.find_commit(b_oid), repo.find_commit(m_oid))
                            {
                                if let (Ok(b_tree), Ok(m_tree)) =
                                    (b_commit.tree(), m_commit.tree())
                                {
                                    if let Ok(diff) = repo
                                        .diff_tree_to_tree(Some(&m_tree), Some(&b_tree), None)
                                    {
                                        let mut text = String::new();
                                        let _ = diff.print(
                                            git2::DiffFormat::Patch,
                                            |_delta, _hunk, line| {
                                                let prefix = match line.origin() {
                                                    '+' => "+",
                                                    '-' => "-",
                                                    ' ' => " ",
                                                    _ => "",
                                                };
                                                if !prefix.is_empty() {
                                                    text.push_str(prefix);
                                                }
                                                if let Ok(content) =
                                                    std::str::from_utf8(line.content())
                                                {
                                                    text.push_str(content);
                                                }
                                                true
                                            },
                                        );
                                        app.branches.diff_cache = Some((name, text));
                                    }
                                }
                            }
                        }
                    }
                } else {
                    app.branches.diff_cache =
                        Some((name, "(this is the main branch)".to_string()));
                }
            }
        }
    }
}

fn load_commit_data(app: &mut ExploreApp, repo: &Repository) {
    if let Some(oid) = app.history.selected_oid() {
        if app.history.diff_cache.as_ref().map(|(id, _)| *id) != Some(oid) {
            match crate::git::get_commit_diff(repo, oid) {
                Ok(diff) => app.history.diff_cache = Some((oid, diff)),
                Err(e) => app.status_message = format!("diff error: {}", e),
            }
        }
        if app.history.files_cache.as_ref().map(|(id, _)| *id) != Some(oid) {
            match get_commit_files(repo, oid) {
                Ok(files) => app.history.files_cache = Some((oid, files)),
                Err(e) => app.status_message = format!("files error: {}", e),
            }
        }
    }
}

fn get_commit_files(repo: &Repository, oid: git2::Oid) -> Result<Vec<(String, char)>> {
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
    let mut files = Vec::new();

    for delta in diff.deltas() {
        let status = match delta.status() {
            git2::Delta::Added => 'A',
            git2::Delta::Deleted => 'D',
            git2::Delta::Modified => 'M',
            git2::Delta::Renamed => 'R',
            _ => '?',
        };
        let path = delta
            .new_file()
            .path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        files.push((path, status));
    }

    Ok(files)
}

// ── Rendering ──

fn render(f: &mut Frame, app: &mut ExploreApp) {
    let area = f.size();
    app.layout = LayoutMode::from_size(area);

    let show_status = !app.layout.hide_status_bar(area.height);

    let mut constraints = vec![Constraint::Length(3)];
    constraints.push(Constraint::Min(5));
    if show_status {
        constraints.push(Constraint::Length(1));
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    render_tab_bar(f, app, chunks[0]);

    match app.active_tab {
        Tab::Summary => render_summary_tab(f, app, chunks[1]),
        Tab::History => render_history_tab(f, app, chunks[1]),
        Tab::Branches => render_branches_tab(f, app, chunks[1]),
    }

    if show_status {
        render_status_bar(f, app, chunks[chunks.len() - 1]);
    }

    if let Some(ref overlay) = app.overlay {
        render_overlay(f, overlay, area);
    }
}

fn render_tab_bar(f: &mut Frame, app: &ExploreApp, area: Rect) {
    // Split: tabs on left, hint on right
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(30), Constraint::Length(14)])
        .split(area);

    let titles = vec!["summary", "history", "branches"];
    let tabs = Tabs::new(titles)
        .select(app.active_tab.index())
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::White))
        .divider("  ")
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::Rgb(40, 40, 40)))
                .title(" repo ")
                .title_style(Style::default().fg(Color::DarkGray)),
        );
    f.render_widget(tabs, chunks[0]);

    let hint = Paragraph::new(Line::from(vec![
        Span::styled("tab", Style::default().fg(Color::Rgb(60, 60, 60))),
        Span::styled(" \u{21c6}  ", Style::default().fg(Color::Rgb(40, 40, 40))),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Rgb(40, 40, 40))),
    );
    f.render_widget(hint, chunks[1]);
}

fn render_summary_tab(f: &mut Frame, app: &ExploreApp, area: Rect) {
    let s = &app.summary;
    let status_text = if s.status.is_clean() {
        "clean".to_string()
    } else {
        format!(
            "{}S {}M {}U {}C",
            s.status.staged, s.status.modified, s.status.untracked, s.status.conflicted
        )
    };

    let label_style = Style::default().fg(Color::DarkGray);
    let value_style = Style::default().fg(Color::Gray);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  branch   ", label_style),
            Span::styled(
                &s.current_branch.name,
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  status   ", label_style),
            Span::styled(&status_text, value_style),
        ]),
        Line::from(vec![
            Span::styled("  commits  ", label_style),
            Span::styled(format!("{}", s.total_commits), value_style),
        ]),
        Line::from(vec![
            Span::styled("  branches ", label_style),
            Span::styled(
                format!(
                    "{} local, {} remote",
                    s.local_branches.len(),
                    s.remote_branches.len()
                ),
                value_style,
            ),
        ]),
        Line::from(vec![
            Span::styled("  stashes  ", label_style),
            Span::styled(format!("{}", s.stashes.len()), value_style),
        ]),
    ];

    let p = Paragraph::new(lines).block(Block::default().borders(Borders::NONE));
    f.render_widget(p, area);
}

fn render_history_tab(f: &mut Frame, app: &ExploreApp, area: Rect) {
    let show_filter = app.history.filter_active || !app.history.filter_input.is_empty();

    let mut constraints = Vec::new();
    if show_filter {
        constraints.push(Constraint::Length(3));
    }
    constraints.push(Constraint::Min(3));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let content_area = if show_filter {
        let filter_style = if app.history.filter_active {
            Style::default().fg(Color::Gray)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let cursor = if app.history.filter_active {
            "\u{2588}"
        } else {
            ""
        };
        let filter_text = format!("  / {}{}", app.history.filter_input, cursor);
        let filter_bar = Paragraph::new(filter_text)
            .style(filter_style)
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(Color::Rgb(40, 40, 40))),
            );
        f.render_widget(filter_bar, chunks[0]);
        chunks[1]
    } else {
        chunks[0]
    };

    if app.layout.is_small() {
        if app.history.show_detail {
            render_history_detail(f, app, content_area);
        } else {
            render_commit_list(f, app, content_area);
        }
    } else {
        let panels = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(app.layout.list_width_pct()),
                Constraint::Percentage(app.layout.detail_width_pct()),
            ])
            .split(content_area);

        render_commit_list(f, app, panels[0]);
        render_history_detail(f, app, panels[1]);
    }
}

fn render_commit_list(f: &mut Frame, app: &ExploreApp, area: Rect) {
    let hash_len = app.layout.hash_len();

    let items: Vec<ListItem> = app
        .history
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(i, &idx)| {
            let commit = &app.history.commits[idx];
            let hash = if hash_len > 0 {
                format!(
                    "{} ",
                    &commit.short_id[..hash_len.min(commit.short_id.len())]
                )
            } else {
                String::new()
            };
            let time = crate::models::format_relative_time(&commit.time);
            let overhead = hash.len() + time.len() + 5;
            let msg_width = (area.width as usize).saturating_sub(overhead);
            let msg = super::layout::truncate(&commit.message, msg_width);

            let is_selected = i == app.history.cursor;

            let hash_style = if is_selected {
                Style::default().fg(Color::Rgb(100, 100, 100)).bg(Color::Rgb(30, 30, 30))
            } else {
                Style::default().fg(Color::Rgb(80, 80, 80))
            };
            let msg_style = if is_selected {
                Style::default().fg(Color::White).bg(Color::Rgb(30, 30, 30))
            } else {
                Style::default().fg(Color::Gray)
            };
            let time_style = if is_selected {
                Style::default().fg(Color::DarkGray).bg(Color::Rgb(30, 30, 30))
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let prefix = if is_selected { " \u{2502} " } else { "   " };
            let prefix_style = if is_selected {
                Style::default().fg(Color::Rgb(80, 140, 200)).bg(Color::Rgb(30, 30, 30))
            } else {
                Style::default()
            };

            ListItem::new(Line::from(vec![
                Span::styled(prefix, prefix_style),
                Span::styled(hash, hash_style),
                Span::styled(msg, msg_style),
                Span::styled(format!(" {}", time), time_style),
            ]))
        })
        .collect();

    let count_label = format!(" {} ", app.history.visible_count());

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(Color::Rgb(40, 40, 40)))
            .title(count_label)
            .title_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(list, area);
}

fn render_history_detail(f: &mut Frame, app: &ExploreApp, area: Rect) {
    let sub_tabs = vec!["diff", "blame", "files"];
    let active = match app.history.detail_sub_tab {
        super::history::DetailSubTab::Diff => 0,
        super::history::DetailSubTab::Blame => 1,
        super::history::DetailSubTab::Files => 2,
    };

    let header_line = Line::from(
        sub_tabs
            .iter()
            .enumerate()
            .flat_map(|(i, name)| {
                let style = if i == active {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let mut spans = vec![Span::styled(format!(" {} ", name), style)];
                if i < sub_tabs.len() - 1 {
                    spans.push(Span::styled("\u{00b7}", Style::default().fg(Color::Rgb(50, 50, 50))));
                }
                spans
            })
            .collect::<Vec<Span>>(),
    );

    // Show AI result if available
    let ai_prefix = if let Some(ref result) = app.ai.last_result {
        if app.status_message.starts_with("AI:") {
            Some(format!("[AI] {}\n\n", result))
        } else {
            None
        }
    } else {
        None
    };

    // Build content lines with rich styling per sub-tab
    let content_lines: Vec<Line> = match &app.history.detail_sub_tab {
        super::history::DetailSubTab::Diff => {
            let diff_text = match &app.history.diff_cache {
                Some((_, diff)) => diff.clone(),
                None => "Loading...".to_string(),
            };
            let ext = highlight::extension_from_diff(&diff_text);
            let full = match ai_prefix {
                Some(prefix) => format!("{}{}", prefix, diff_text),
                None => diff_text,
            };
            full.lines()
                .skip(app.history.detail_scroll)
                .map(|l| {
                    if l.starts_with("[AI]") {
                        Line::from(Span::styled(
                            l.to_string(),
                            Style::default()
                                .fg(Color::Rgb(160, 120, 200))
                                .add_modifier(Modifier::BOLD),
                        ))
                    } else if ext.is_empty() {
                        // No extension detected — fall back to basic coloring
                        let style = if l.starts_with('+') && !l.starts_with("+++") {
                            Style::default().fg(Color::Rgb(100, 180, 100))
                        } else if l.starts_with('-') && !l.starts_with("---") {
                            Style::default().fg(Color::Rgb(180, 100, 100))
                        } else if l.starts_with("@@") {
                            Style::default().fg(Color::Rgb(80, 140, 200))
                        } else {
                            Style::default().fg(Color::Gray)
                        };
                        Line::from(Span::styled(l.to_string(), style))
                    } else {
                        app.highlighter.highlight_diff_line(l, &ext)
                    }
                })
                .collect()
        }
        super::history::DetailSubTab::Files => {
            let file_lines: Vec<Line> = match &app.history.files_cache {
                Some((_, files)) => files
                    .iter()
                    .enumerate()
                    .map(|(i, (path, status))| {
                        let selected = i == app.history.file_cursor;
                        let status_style = match *status {
                            'A' => Style::default().fg(Color::Rgb(100, 180, 100)),
                            'D' => Style::default().fg(Color::Rgb(180, 100, 100)),
                            'M' => Style::default().fg(Color::Rgb(180, 170, 100)),
                            'R' => Style::default().fg(Color::Rgb(80, 140, 200)),
                            _ => Style::default().fg(Color::DarkGray),
                        };
                        let path_style = if selected {
                            Style::default().fg(Color::White)
                        } else {
                            Style::default().fg(Color::Gray)
                        };
                        let marker = if selected {
                            Span::styled("\u{2502} ", Style::default().fg(Color::Rgb(80, 140, 200)))
                        } else {
                            Span::raw("  ")
                        };
                        Line::from(vec![
                            marker,
                            Span::styled(format!("{}", status), status_style),
                            Span::raw(" "),
                            Span::styled(path.to_string(), path_style),
                        ])
                    })
                    .collect(),
                None => vec![Line::from(Span::styled(
                    "Loading...",
                    Style::default().fg(Color::DarkGray),
                ))],
            };
            file_lines
                .into_iter()
                .skip(app.history.detail_scroll)
                .collect()
        }
        super::history::DetailSubTab::Blame => {
            match &app.history.files_cache {
                Some((_, files)) if !files.is_empty() => {
                    let idx = app.history.file_cursor.min(files.len().saturating_sub(1));
                    let (file_path, _) = &files[idx];
                    let commit_hash = app
                        .history
                        .selected_commit()
                        .map(|c| c.id.to_string())
                        .unwrap_or_default();
                    let ext = highlight::extension_from_path(file_path);
                    match super::history::blame::get_blame(
                        &app.repo_path,
                        file_path,
                        &commit_hash,
                    ) {
                        Ok(blame_lines) => {
                            let mut result = vec![
                                Line::from(Span::styled(
                                    format!(" {} ", file_path),
                                    Style::default().fg(Color::White),
                                )),
                                Line::from(""),
                            ];
                            let mut prev_hash = String::new();
                            for bl in blame_lines.iter().skip(app.history.detail_scroll) {
                                let new_block = bl.commit_hash != prev_hash;
                                let hash_style = if new_block {
                                    Style::default().fg(Color::Rgb(100, 100, 100))
                                } else {
                                    Style::default().fg(Color::Rgb(50, 50, 50))
                                };
                                let author_style = if new_block {
                                    Style::default().fg(Color::Rgb(80, 140, 200))
                                } else {
                                    Style::default().fg(Color::Rgb(50, 50, 50))
                                };
                                let hash_text = if new_block {
                                    bl.commit_hash.clone()
                                } else {
                                    " ".repeat(bl.commit_hash.len())
                                };
                                let author_text = if new_block {
                                    let name = &bl.author;
                                    if name.len() > 10 {
                                        format!("{:>10}", &name[..10])
                                    } else {
                                        format!("{:>10}", name)
                                    }
                                } else {
                                    " ".repeat(10)
                                };

                                let mut line_spans = vec![
                                    Span::styled(hash_text, hash_style),
                                    Span::raw(" "),
                                    Span::styled(author_text, author_style),
                                    Span::styled(
                                        " \u{2502} ",
                                        Style::default().fg(Color::Rgb(40, 40, 40)),
                                    ),
                                    Span::styled(
                                        format!("{:>3} ", bl.line_no),
                                        Style::default().fg(Color::Rgb(50, 50, 50)),
                                    ),
                                ];
                                if ext.is_empty() {
                                    line_spans.push(Span::styled(
                                        bl.content.clone(),
                                        Style::default().fg(Color::Gray),
                                    ));
                                } else {
                                    line_spans.extend(
                                        app.highlighter.highlight_blame_content(&bl.content, &ext),
                                    );
                                }
                                result.push(Line::from(line_spans));
                                prev_hash = bl.commit_hash.clone();
                            }
                            result
                        }
                        Err(e) => vec![Line::from(Span::styled(
                            format!("Blame error: {}", e),
                            Style::default().fg(Color::Red),
                        ))],
                    }
                }
                _ => vec![Line::from(Span::styled(
                    "No files loaded",
                    Style::default().fg(Color::DarkGray),
                ))],
            }
        }
    };

    let lines: Vec<Line> = std::iter::once(header_line)
        .chain(std::iter::once(Line::from("")))
        .chain(content_lines)
        .collect();

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::Rgb(40, 40, 40))),
    );

    f.render_widget(p, area);
}

fn render_branches_tab(f: &mut Frame, app: &ExploreApp, area: Rect) {
    let show_search = app.branches.search_active || !app.branches.search_input.is_empty();

    let mut constraints = Vec::new();
    if show_search {
        constraints.push(Constraint::Length(3));
    }
    constraints.push(Constraint::Min(3));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let content_area = if show_search {
        let style = if app.branches.search_active {
            Style::default().fg(Color::Gray)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let cursor = if app.branches.search_active {
            "\u{2588}"
        } else {
            ""
        };
        let bar = Paragraph::new(format!("  / {}{}", app.branches.search_input, cursor))
            .style(style)
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(Color::Rgb(40, 40, 40))),
            );
        f.render_widget(bar, chunks[0]);
        chunks[1]
    } else {
        chunks[0]
    };

    if app.layout.is_small() {
        if app.branches.show_detail {
            render_branch_detail(f, app, content_area);
        } else {
            render_branch_tree(f, app, content_area);
        }
    } else {
        let panels = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(app.layout.list_width_pct()),
                Constraint::Percentage(app.layout.detail_width_pct()),
            ])
            .split(content_area);

        render_branch_tree(f, app, panels[0]);
        render_branch_detail(f, app, panels[1]);
    }
}

fn render_branch_tree(f: &mut Frame, app: &ExploreApp, area: Rect) {
    let visible = app.branches.visible_nodes();
    let max_width = (area.width as usize).saturating_sub(8);
    let total = visible.len();

    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let is_selected = i == app.branches.cursor;
            let is_last = i + 1 >= total
                || visible
                    .get(i + 1)
                    .map(|n| n.depth <= node.depth)
                    .unwrap_or(true);

            // Build tree prefix using thin unicode chars
            let tree_prefix = if node.depth == 0 {
                String::new()
            } else {
                let connector = if is_last { "\u{2514} " } else { "\u{251c} " };
                let padding = "  ".repeat(node.depth.saturating_sub(1));
                let tree_char = if node.depth > 1 { "\u{2502} " } else { "" };
                format!("  {}{}{}", padding, tree_char, connector)
            };

            let marker = if node.branch.is_head {
                "\u{25cf} "
            } else if node.is_merged {
                "\u{2713} "
            } else if node.is_stale {
                "\u{2219} "
            } else {
                "  "
            };

            let name = super::layout::truncate_middle(
                &node.branch.name,
                max_width.saturating_sub(tree_prefix.len() + marker.len()),
            );

            let tree_style = Style::default().fg(Color::Rgb(50, 50, 50));
            let marker_style = if node.branch.is_head {
                Style::default().fg(Color::Rgb(100, 180, 100))
            } else if node.is_merged {
                Style::default().fg(Color::Rgb(80, 140, 200))
            } else if node.is_stale {
                Style::default().fg(Color::Rgb(60, 60, 60))
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let name_style = if is_selected {
                Style::default().fg(Color::White).bg(Color::Rgb(30, 30, 30))
            } else if node.branch.is_head {
                Style::default().fg(Color::White)
            } else if node.is_stale {
                Style::default().fg(Color::DarkGray)
            } else if node.is_merged {
                Style::default().fg(Color::Gray)
            } else {
                Style::default().fg(Color::Gray)
            };

            let prefix = if is_selected {
                Span::styled("\u{2502}", Style::default().fg(Color::Rgb(80, 140, 200)).bg(Color::Rgb(30, 30, 30)))
            } else {
                Span::raw(" ")
            };

            ListItem::new(Line::from(vec![
                prefix,
                Span::styled(tree_prefix, tree_style),
                Span::styled(marker, marker_style),
                Span::styled(name, name_style),
            ]))
        })
        .collect();

    let filter_label = match app.branches.filter {
        super::branches::BranchFilter::All => "all",
        super::branches::BranchFilter::Local => "local",
        super::branches::BranchFilter::Remote => "remote",
        super::branches::BranchFilter::Merged => "merged",
        super::branches::BranchFilter::Unmerged => "unmerged",
        super::branches::BranchFilter::Stale => "stale",
    };

    let title = format!(" {} \u{00b7} {} ", filter_label, visible.len());

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(Color::Rgb(40, 40, 40)))
            .title(title)
            .title_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(list, area);
}

fn render_branch_detail(f: &mut Frame, app: &ExploreApp, area: Rect) {
    let sub_tabs = vec!["info", "commits", "diff"];
    let active = match app.branches.detail_sub_tab {
        super::branches::BranchDetailSubTab::Info => 0,
        super::branches::BranchDetailSubTab::Commits => 1,
        super::branches::BranchDetailSubTab::Diff => 2,
    };

    let header = Line::from(
        sub_tabs
            .iter()
            .enumerate()
            .flat_map(|(i, name)| {
                let style = if i == active {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let mut spans = vec![Span::styled(format!(" {} ", name), style)];
                if i < sub_tabs.len() - 1 {
                    spans.push(Span::styled("\u{00b7}", Style::default().fg(Color::Rgb(50, 50, 50))));
                }
                spans
            })
            .collect::<Vec<Span>>(),
    );

    let content = match app.branches.detail_sub_tab {
        super::branches::BranchDetailSubTab::Info => {
            // Check for compare result in detail_cache
            if let Some((key, text)) = &app.branches.detail_cache {
                if key.starts_with("cmp:") {
                    text.clone()
                } else {
                    build_branch_info(app)
                }
            } else if let Some(ref result) = app.ai.last_result {
                if app.status_message.starts_with("AI:") {
                    format!("[AI] {}\n\n{}", result, build_branch_info(app))
                } else {
                    build_branch_info(app)
                }
            } else {
                build_branch_info(app)
            }
        }
        super::branches::BranchDetailSubTab::Commits => {
            match &app.branches.commits_cache {
                Some((_, commits)) => {
                    if commits.is_empty() {
                        "No unique commits (press Enter to load)".to_string()
                    } else {
                        commits.join("\n")
                    }
                }
                None => "Press Enter to load commits".to_string(),
            }
        }
        super::branches::BranchDetailSubTab::Diff => {
            match &app.branches.diff_cache {
                Some((_, diff)) => diff.clone(),
                None => "Press Enter to load diff".to_string(),
            }
        }
    };

    let branch_diff_ext = match &app.branches.diff_cache {
        Some((_, diff)) if matches!(app.branches.detail_sub_tab, super::branches::BranchDetailSubTab::Diff) => {
            highlight::extension_from_diff(diff)
        }
        _ => String::new(),
    };

    let lines: Vec<Line> = std::iter::once(header)
        .chain(std::iter::once(Line::from("")))
        .chain(
            content
                .lines()
                .skip(app.branches.detail_scroll)
                .map(|l| {
                    if !branch_diff_ext.is_empty()
                        && matches!(app.branches.detail_sub_tab, super::branches::BranchDetailSubTab::Diff)
                    {
                        app.highlighter.highlight_diff_line(l, &branch_diff_ext)
                    } else if l.starts_with("[AI]") {
                        Line::from(Span::styled(
                            l.to_string(),
                            Style::default()
                                .fg(Color::Rgb(160, 120, 200))
                                .add_modifier(Modifier::BOLD),
                        ))
                    } else {
                        Line::from(Span::styled(
                            l.to_string(),
                            Style::default().fg(Color::Gray),
                        ))
                    }
                }),
        )
        .collect();

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::Rgb(40, 40, 40))),
    );

    f.render_widget(p, area);
}

fn build_branch_info(app: &ExploreApp) -> String {
    if let Some(node) = app.branches.visible_nodes().get(app.branches.cursor).copied() {
        let upstream_str = node
            .branch
            .upstream
            .as_ref()
            .map(|u| {
                format!(
                    "tracking: {}\nahead: {} behind: {}",
                    u.name, u.ahead, u.behind
                )
            })
            .unwrap_or_else(|| "tracking: none".to_string());

        let activity = node
            .last_activity
            .map(|t| crate::models::format_relative_time(&t))
            .unwrap_or_else(|| "unknown".to_string());

        let status = if node.branch.is_head {
            "current"
        } else if node.is_merged {
            "merged"
        } else if node.is_stale {
            "stale"
        } else {
            "active"
        };

        format!(
            "{}\nlast activity: {}\nstatus: {}\nunique commits: {}\nmerged: {}",
            upstream_str,
            activity,
            status,
            node.unique_commits,
            if node.is_merged { "yes" } else { "no" },
        )
    } else {
        "No branch selected".to_string()
    }
}

fn render_status_bar(f: &mut Frame, app: &ExploreApp, area: Rect) {
    let key = Style::default().fg(Color::Rgb(80, 140, 200));
    let dim = Style::default().fg(Color::Rgb(50, 50, 50));
    let sep = Span::styled("  \u{00b7}  ", dim);

    let mut spans: Vec<Span> = Vec::new();

    if !app.status_message.is_empty() {
        spans.push(Span::styled(
            format!(" {} ", app.status_message),
            Style::default().fg(Color::Rgb(80, 80, 80)),
        ));
    } else {
        spans.push(Span::raw(" "));
        match app.active_tab {
            Tab::Summary => {
                spans.extend_from_slice(&[
                    Span::styled("q", key), Span::styled(" quit", dim),
                    sep.clone(),
                    Span::styled("?", key), Span::styled(" help", dim),
                ]);
            }
            Tab::History => {
                spans.extend_from_slice(&[
                    Span::styled("j/k", key), Span::styled(" nav", dim),
                    sep.clone(),
                    Span::styled("J/K", key), Span::styled(" scroll", dim),
                    sep.clone(),
                    Span::styled("h/l", key), Span::styled(" tabs", dim),
                    sep.clone(),
                    Span::styled("/", key), Span::styled(" filter", dim),
                    sep.clone(),
                    Span::styled("x", key), Span::styled(" actions", dim),
                    sep.clone(),
                    Span::styled("a", key), Span::styled(" ai", dim),
                ]);
            }
            Tab::Branches => {
                spans.extend_from_slice(&[
                    Span::styled("j/k", key), Span::styled(" nav", dim),
                    sep.clone(),
                    Span::styled("J/K", key), Span::styled(" scroll", dim),
                    sep.clone(),
                    Span::styled("/", key), Span::styled(" search", dim),
                    sep.clone(),
                    Span::styled("c", key), Span::styled(" compare", dim),
                    sep.clone(),
                    Span::styled("f", key), Span::styled(" filter", dim),
                    sep.clone(),
                    Span::styled("x", key), Span::styled(" actions", dim),
                ]);
            }
        }
    }

    // Right-align AI status
    let ai_status = if app.ai.has_provider() {
        format!("ai:{}", app.ai.provider_display())
    } else {
        String::new()
    };

    let left_len: usize = spans.iter().map(|s| s.content.len()).sum();
    let pad = (area.width as usize).saturating_sub(left_len + ai_status.len() + 2);
    spans.push(Span::raw(" ".repeat(pad)));
    if !ai_status.is_empty() {
        spans.push(Span::styled(
            format!("{} ", ai_status),
            Style::default().fg(Color::Rgb(50, 50, 50)),
        ));
    }

    let bar = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(Color::Rgb(15, 15, 15)));
    f.render_widget(bar, area);
}
