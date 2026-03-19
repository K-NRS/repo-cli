pub mod blame;
pub mod filters;
pub mod search;

use git2::{Oid, Repository};

use crate::models::{CommitInfo, FilterExpr};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetailSubTab {
    Diff,
    Blame,
    Files,
}

#[derive(Debug)]
pub struct HistoryState {
    pub commits: Vec<CommitInfo>,
    pub filtered_indices: Vec<usize>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub detail_sub_tab: DetailSubTab,
    pub detail_scroll: usize,
    pub filters: Vec<FilterExpr>,
    pub filter_input: String,
    pub filter_active: bool,
    pub diff_cache: Option<(Oid, String)>,
    pub files_cache: Option<(Oid, Vec<(String, char)>)>,
    pub file_cursor: usize,
    pub page_size: usize,
    pub all_loaded: bool,
    pub show_detail: bool,
}

impl HistoryState {
    pub fn new(commits: Vec<CommitInfo>, page_size: usize) -> Self {
        let len = commits.len();
        let all_loaded = len < page_size;
        let filtered_indices = (0..len).collect();
        Self {
            commits,
            filtered_indices,
            cursor: 0,
            scroll_offset: 0,
            detail_sub_tab: DetailSubTab::Diff,
            detail_scroll: 0,
            filters: Vec::new(),
            filter_input: String::new(),
            filter_active: false,
            diff_cache: None,
            files_cache: None,
            file_cursor: 0,
            page_size,
            all_loaded,
            show_detail: false,
        }
    }

    pub fn selected_commit(&self) -> Option<&CommitInfo> {
        self.filtered_indices
            .get(self.cursor)
            .and_then(|&idx| self.commits.get(idx))
    }

    pub fn selected_oid(&self) -> Option<Oid> {
        self.selected_commit().map(|c| c.id)
    }

    pub fn visible_count(&self) -> usize {
        self.filtered_indices.len()
    }

    pub fn move_down(&mut self) {
        let max = self.visible_count().saturating_sub(1);
        self.cursor = (self.cursor + 1).min(max);
        self.invalidate_cache();
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
        self.invalidate_cache();
    }

    pub fn cycle_detail_tab(&mut self) {
        self.detail_sub_tab = match self.detail_sub_tab {
            DetailSubTab::Diff => DetailSubTab::Blame,
            DetailSubTab::Blame => DetailSubTab::Files,
            DetailSubTab::Files => DetailSubTab::Diff,
        };
        self.detail_scroll = 0;
    }

    pub fn load_more(&mut self, repo: &Repository) {
        if self.all_loaded {
            return;
        }
        let target = self.commits.len() + self.page_size;
        match crate::git::get_recent_commits(repo, target) {
            Ok(all_commits) => {
                if all_commits.len() <= self.commits.len() {
                    self.all_loaded = true;
                } else {
                    self.commits = all_commits;
                    if self.filters.is_empty() {
                        self.filtered_indices = (0..self.commits.len()).collect();
                    } else {
                        self.filtered_indices =
                            filters::apply_client_filters(&self.commits, &self.filters);
                    }
                }
            }
            Err(_) => {
                self.all_loaded = true;
            }
        }
    }

    fn invalidate_cache(&mut self) {
        self.diff_cache = None;
        self.files_cache = None;
        self.detail_scroll = 0;
        self.file_cursor = 0;
    }
}
