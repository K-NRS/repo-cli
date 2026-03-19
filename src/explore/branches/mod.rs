pub mod actions;
pub mod compare;
pub mod tree;

use crate::models::{BranchTreeNode, RemoteBranchInfo};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BranchDetailSubTab {
    Info,
    Commits,
    Diff,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BranchFilter {
    All,
    Local,
    Remote,
    Merged,
    Unmerged,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BranchSort {
    Activity,
    Name,
    AheadBehind,
}

#[derive(Debug)]
pub struct BranchesState {
    pub tree_nodes: Vec<BranchTreeNode>,
    pub remote_branches: Vec<RemoteBranchInfo>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub detail_sub_tab: BranchDetailSubTab,
    pub detail_scroll: usize,
    pub filter: BranchFilter,
    pub sort: BranchSort,
    pub search_input: String,
    pub search_active: bool,
    pub compare_mode: bool,
    pub compare_source: Option<usize>,
    pub detail_cache: Option<(String, String)>, // (branch_name, detail_text)
    pub commits_cache: Option<(String, Vec<String>)>,
    pub diff_cache: Option<(String, String)>,
    pub show_detail: bool,
}

impl BranchesState {
    pub fn new(tree_nodes: Vec<BranchTreeNode>, remote_branches: Vec<RemoteBranchInfo>) -> Self {
        Self {
            tree_nodes,
            remote_branches,
            cursor: 0,
            scroll_offset: 0,
            detail_sub_tab: BranchDetailSubTab::Info,
            detail_scroll: 0,
            filter: BranchFilter::All,
            sort: BranchSort::Activity,
            search_input: String::new(),
            search_active: false,
            compare_mode: false,
            compare_source: None,
            detail_cache: None,
            commits_cache: None,
            diff_cache: None,
            show_detail: false,
        }
    }

    pub fn selected_branch(&self) -> Option<&BranchTreeNode> {
        let visible = self.visible_nodes();
        visible.get(self.cursor).copied()
    }

    pub fn selected_branch_name(&self) -> Option<String> {
        self.selected_branch().map(|n| n.branch.name.clone())
    }

    pub fn visible_nodes(&self) -> Vec<&BranchTreeNode> {
        self.tree_nodes
            .iter()
            .filter(|n| self.matches_filter(n) && self.matches_search(n))
            .collect()
    }

    fn matches_filter(&self, node: &BranchTreeNode) -> bool {
        match self.filter {
            BranchFilter::All => true,
            BranchFilter::Local => true,
            BranchFilter::Remote => false,
            BranchFilter::Merged => node.is_merged,
            BranchFilter::Unmerged => !node.is_merged,
            BranchFilter::Stale => node.is_stale,
        }
    }

    fn matches_search(&self, node: &BranchTreeNode) -> bool {
        if self.search_input.is_empty() {
            return true;
        }
        node.branch
            .name
            .to_lowercase()
            .contains(&self.search_input.to_lowercase())
    }

    pub fn move_down(&mut self) {
        let max = self.visible_nodes().len().saturating_sub(1);
        self.cursor = (self.cursor + 1).min(max);
        self.invalidate_cache();
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
        self.invalidate_cache();
    }

    pub fn cycle_detail_tab(&mut self) {
        self.detail_sub_tab = match self.detail_sub_tab {
            BranchDetailSubTab::Info => BranchDetailSubTab::Commits,
            BranchDetailSubTab::Commits => BranchDetailSubTab::Diff,
            BranchDetailSubTab::Diff => BranchDetailSubTab::Info,
        };
        self.detail_scroll = 0;
    }

    pub fn cycle_filter(&mut self) {
        self.filter = match self.filter {
            BranchFilter::All => BranchFilter::Merged,
            BranchFilter::Local => BranchFilter::Merged,
            BranchFilter::Merged => BranchFilter::Unmerged,
            BranchFilter::Unmerged => BranchFilter::Stale,
            BranchFilter::Stale => BranchFilter::All,
            BranchFilter::Remote => BranchFilter::All,
        };
        self.cursor = 0;
    }

    pub fn cycle_sort(&mut self) {
        self.sort = match self.sort {
            BranchSort::Activity => BranchSort::Name,
            BranchSort::Name => BranchSort::AheadBehind,
            BranchSort::AheadBehind => BranchSort::Activity,
        };
    }

    fn invalidate_cache(&mut self) {
        self.detail_cache = None;
        self.commits_cache = None;
        self.diff_cache = None;
        self.detail_scroll = 0;
    }
}
