use chrono::{DateTime, Local, NaiveDate, TimeZone};

use crate::models::{CommitInfo, FilterExpr};

pub fn parse_filters(input: &str) -> Vec<FilterExpr> {
    let mut filters = Vec::new();
    let mut remaining = input.trim();

    while !remaining.is_empty() {
        if let Some(rest) = remaining.strip_prefix("msg:") {
            let (value, next) = extract_value(rest);
            filters.push(FilterExpr::Message(value));
            remaining = next;
        } else if let Some(rest) = remaining.strip_prefix("author:") {
            let (value, next) = extract_value(rest);
            filters.push(FilterExpr::Author(value));
            remaining = next;
        } else if let Some(rest) = remaining.strip_prefix("date:") {
            let (value, next) = extract_value(rest);
            let (from, to) = parse_date_range(&value);
            filters.push(FilterExpr::DateRange { from, to });
            remaining = next;
        } else if let Some(rest) = remaining.strip_prefix("path:") {
            let (value, next) = extract_value(rest);
            filters.push(FilterExpr::Path(value));
            remaining = next;
        } else if let Some(rest) = remaining.strip_prefix("diff:") {
            let (value, next) = extract_value(rest);
            filters.push(FilterExpr::DiffContent(value));
            remaining = next;
        } else {
            let (value, next) = extract_value(remaining);
            if !value.is_empty() {
                filters.push(FilterExpr::Message(value));
            }
            remaining = next;
        }
    }

    filters
}

fn extract_value(s: &str) -> (String, &str) {
    let prefixes = ["msg:", "author:", "date:", "path:", "diff:"];
    let end = prefixes
        .iter()
        .filter_map(|p| s.find(p))
        .filter(|&pos| pos > 0)
        .min()
        .unwrap_or(s.len());

    (s[..end].trim().to_string(), &s[end..])
}

fn parse_date_range(s: &str) -> (Option<DateTime<Local>>, Option<DateTime<Local>>) {
    let parts: Vec<&str> = s.split("..").collect();
    let from = parts.first().and_then(|s| parse_date(s));
    let to = parts.get(1).and_then(|s| parse_date(s));
    (from, to)
}

fn parse_date(s: &str) -> Option<DateTime<Local>> {
    let s = s.trim();
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Local
            .from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
            .single();
    }
    if let Ok(d) = NaiveDate::parse_from_str(&format!("{}-01", s), "%Y-%m-%d") {
        return Local
            .from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
            .single();
    }
    None
}

pub fn apply_client_filters(commits: &[CommitInfo], filters: &[FilterExpr]) -> Vec<usize> {
    commits
        .iter()
        .enumerate()
        .filter(|(_, commit)| {
            filters.iter().all(|f| match f {
                FilterExpr::Message(term) => commit
                    .message
                    .to_lowercase()
                    .contains(&term.to_lowercase()),
                FilterExpr::Author(name) => {
                    commit.author.to_lowercase().contains(&name.to_lowercase())
                }
                FilterExpr::DateRange { from, to } => {
                    let after_from = from.map_or(true, |f| commit.time >= f);
                    let before_to = to.map_or(true, |t| commit.time <= t);
                    after_from && before_to
                }
                FilterExpr::Path(_) | FilterExpr::DiffContent(_) => true,
            })
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_filters_message() {
        let filters = parse_filters("msg:fix auth");
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0], FilterExpr::Message("fix auth".to_string()));
    }

    #[test]
    fn test_parse_filters_combined() {
        let filters = parse_filters("msg:bug author:kn path:src/");
        assert_eq!(filters.len(), 3);
    }

    #[test]
    fn test_parse_filters_bare_text() {
        let filters = parse_filters("some search term");
        assert_eq!(filters.len(), 1);
        assert_eq!(
            filters[0],
            FilterExpr::Message("some search term".to_string())
        );
    }
}
