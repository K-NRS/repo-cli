#[derive(Debug, Clone)]
pub enum RebaseAction {
    Pick,
    Reword(String),
    Squash { into_idx: usize, message: Option<String> },
    Fixup { into_idx: usize },
    Drop,
    Split { groups: Vec<SplitGroup> },
    Edit,
}

#[derive(Debug, Clone)]
pub struct SplitGroup {
    pub hunk_indices: Vec<usize>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct TodoEntry {
    pub original_idx: usize,
    pub action: RebaseAction,
}

impl TodoEntry {
    pub fn pick(idx: usize) -> Self {
        Self { original_idx: idx, action: RebaseAction::Pick }
    }
}

impl std::fmt::Display for RebaseAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RebaseAction::Pick => write!(f, "pick"),
            RebaseAction::Reword(_) => write!(f, "reword"),
            RebaseAction::Squash { .. } => write!(f, "squash"),
            RebaseAction::Fixup { .. } => write!(f, "fixup"),
            RebaseAction::Drop => write!(f, "drop"),
            RebaseAction::Split { .. } => write!(f, "split"),
            RebaseAction::Edit => write!(f, "edit"),
        }
    }
}
