use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutMode {
    Large,
    Medium,
    Small,
}

impl LayoutMode {
    pub fn from_size(area: Rect) -> Self {
        if area.width >= 120 {
            Self::Large
        } else if area.width >= 80 {
            Self::Medium
        } else {
            Self::Small
        }
    }

    pub fn is_small(&self) -> bool {
        matches!(self, Self::Small)
    }

    pub fn hide_status_bar(&self, height: u16) -> bool {
        height < 20
    }

    pub fn hash_len(&self) -> usize {
        match self {
            Self::Large => 7,
            Self::Medium => 4,
            Self::Small => 0,
        }
    }

    pub fn list_width_pct(&self) -> u16 {
        match self {
            Self::Large => 40,
            Self::Medium => 45,
            Self::Small => 100,
        }
    }

    pub fn detail_width_pct(&self) -> u16 {
        match self {
            Self::Large => 60,
            Self::Medium => 55,
            Self::Small => 100,
        }
    }
}

pub fn truncate(s: &str, max_width: usize) -> String {
    if s.len() <= max_width {
        s.to_string()
    } else if max_width <= 3 {
        s.chars().take(max_width).collect()
    } else {
        let mut result: String = s.chars().take(max_width - 1).collect();
        result.push('\u{2026}');
        result
    }
}

pub fn truncate_middle(s: &str, max_width: usize) -> String {
    if s.len() <= max_width {
        return s.to_string();
    }
    if max_width <= 5 {
        return truncate(s, max_width);
    }
    let half = (max_width - 1) / 2;
    let prefix: String = s.chars().take(half).collect();
    let suffix: String = s.chars().skip(s.len() - half).collect();
    format!("{}\u{2026}{}", prefix, suffix)
}
