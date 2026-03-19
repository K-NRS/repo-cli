use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{self, ThemeSet},
    parsing::SyntaxSet,
};

pub struct Highlighter {
    ps: SyntaxSet,
    ts: ThemeSet,
}

impl Highlighter {
    pub fn new() -> Self {
        Self {
            ps: SyntaxSet::load_defaults_newlines(),
            ts: ThemeSet::load_defaults(),
        }
    }

    /// Highlight a line of code, returning styled spans.
    /// `extension` is the file extension (e.g. "rs", "js", "py").
    pub fn highlight_line<'a>(&self, line: &str, extension: &str) -> Vec<Span<'a>> {
        let syntax = self
            .ps
            .find_syntax_by_extension(extension)
            .unwrap_or_else(|| self.ps.find_syntax_plain_text());

        let theme = &self.ts.themes["base16-ocean.dark"];
        let mut h = HighlightLines::new(syntax, theme);

        match h.highlight_line(line, &self.ps) {
            Ok(ranges) => ranges
                .into_iter()
                .map(|(style, text)| {
                    let fg = to_ratatui_color(style.foreground);
                    Span::styled(text.to_string(), Style::default().fg(fg))
                })
                .collect(),
            Err(_) => vec![Span::styled(line.to_string(), Style::default().fg(Color::Gray))],
        }
    }

    /// Highlight a diff line — preserves +/- prefix coloring, highlights the code part.
    pub fn highlight_diff_line<'a>(&self, line: &str, extension: &str) -> Line<'a> {
        if line.starts_with("diff --git") || line.starts_with("index ") || line.starts_with("---") || line.starts_with("+++") {
            return Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::DarkGray),
            ));
        }

        if line.starts_with("@@") {
            return Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::Rgb(80, 140, 200)),
            ));
        }

        if line.starts_with('+') {
            let prefix_style = Style::default().fg(Color::Rgb(100, 180, 100));
            let code = &line[1..];
            let mut spans = vec![Span::styled("+", prefix_style)];
            for span in self.highlight_line(code, extension) {
                // Tint highlighted colors toward green
                let tinted = tint_color(span.style.fg, Color::Rgb(100, 180, 100));
                spans.push(Span::styled(span.content.to_string(), Style::default().fg(tinted)));
            }
            return Line::from(spans);
        }

        if line.starts_with('-') {
            let prefix_style = Style::default().fg(Color::Rgb(180, 100, 100));
            let code = &line[1..];
            let mut spans = vec![Span::styled("-", prefix_style)];
            for span in self.highlight_line(code, extension) {
                let tinted = tint_color(span.style.fg, Color::Rgb(180, 100, 100));
                spans.push(Span::styled(span.content.to_string(), Style::default().fg(tinted)));
            }
            return Line::from(spans);
        }

        // Context lines — just highlight normally but dimmer
        if line.starts_with(' ') {
            let code = &line[1..];
            let mut spans = vec![Span::raw(" ")];
            for span in self.highlight_line(code, extension) {
                let dimmed = dim_color(span.style.fg);
                spans.push(Span::styled(span.content.to_string(), Style::default().fg(dimmed)));
            }
            return Line::from(spans);
        }

        Line::from(Span::styled(line.to_string(), Style::default().fg(Color::Gray)))
    }

    /// Highlight a blame content line.
    pub fn highlight_blame_content<'a>(&self, content: &str, extension: &str) -> Vec<Span<'a>> {
        self.highlight_line(content, extension)
    }
}

fn to_ratatui_color(c: highlighting::Color) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

/// Blend a syntax color toward a tint (for +/- lines).
fn tint_color(fg: Option<Color>, tint: Color) -> Color {
    let (tr, tg, tb) = match tint {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => return fg.unwrap_or(Color::Gray),
    };
    match fg {
        Some(Color::Rgb(r, g, b)) => {
            // 60% original, 40% tint
            let nr = ((r as u16 * 6 + tr as u16 * 4) / 10) as u8;
            let ng = ((g as u16 * 6 + tg as u16 * 4) / 10) as u8;
            let nb = ((b as u16 * 6 + tb as u16 * 4) / 10) as u8;
            Color::Rgb(nr, ng, nb)
        }
        _ => tint,
    }
}

/// Dim a syntax color slightly for context lines.
fn dim_color(fg: Option<Color>) -> Color {
    match fg {
        Some(Color::Rgb(r, g, b)) => {
            Color::Rgb(
                (r as u16 * 7 / 10) as u8,
                (g as u16 * 7 / 10) as u8,
                (b as u16 * 7 / 10) as u8,
            )
        }
        _ => Color::Rgb(90, 90, 90),
    }
}

/// Extract file extension from a diff header line like "diff --git a/src/main.rs b/src/main.rs"
pub fn extension_from_diff(diff_text: &str) -> String {
    for line in diff_text.lines() {
        if line.starts_with("diff --git") {
            if let Some(path) = line.split_whitespace().last() {
                let path = path.strip_prefix("b/").unwrap_or(path);
                if let Some(ext) = path.rsplit('.').next() {
                    return ext.to_string();
                }
            }
        }
    }
    String::new()
}

/// Extract extension from a file path.
pub fn extension_from_path(path: &str) -> String {
    path.rsplit('.')
        .next()
        .unwrap_or("")
        .to_string()
}
