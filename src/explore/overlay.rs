use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

#[derive(Debug, Clone)]
pub enum Overlay {
    ActionMenu {
        title: String,
        items: Vec<(char, String)>,
        cursor: usize,
    },
    Confirm {
        title: String,
        message: String,
        warnings: Vec<String>,
    },
    Help,
    AiSettings,
    AiPrompt {
        action_name: String,
        provider_name: String,
    },
}

impl Overlay {
    pub fn action_menu(title: &str, items: Vec<(char, String)>) -> Self {
        Self::ActionMenu {
            title: title.to_string(),
            items,
            cursor: 0,
        }
    }

    pub fn confirm(title: &str, message: &str, warnings: Vec<String>) -> Self {
        Self::Confirm {
            title: title.to_string(),
            message: message.to_string(),
            warnings,
        }
    }
}

pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

pub fn render_overlay(f: &mut Frame, overlay: &Overlay, area: Rect) {
    match overlay {
        Overlay::ActionMenu { title, items, cursor } => {
            let height = (items.len() as u16) + 4;
            let width = items.iter().map(|(_, s)| s.len()).max().unwrap_or(20) as u16 + 10;
            let rect = centered_rect(width, height, area);

            f.render_widget(Clear, rect);

            let list_items: Vec<ListItem> = items
                .iter()
                .enumerate()
                .map(|(i, (key, label))| {
                    let is_selected = i == *cursor;
                    let key_style = if is_selected {
                        Style::default()
                            .fg(Color::Rgb(80, 140, 200))
                            .bg(Color::Rgb(30, 30, 30))
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    let label_style = if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Rgb(30, 30, 30))
                    } else {
                        Style::default().fg(Color::Gray)
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("  {} ", key), key_style),
                        Span::styled(format!(" {}", label), label_style),
                    ]))
                })
                .collect();

            let list = List::new(list_items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Rgb(50, 50, 50)))
                    .title(format!(" {} ", title))
                    .title_style(Style::default().fg(Color::DarkGray)),
            );

            f.render_widget(list, rect);
        }
        Overlay::Confirm {
            title,
            message,
            warnings,
        } => {
            let height = 4 + warnings.len() as u16 + 2;
            let width = 50;
            let rect = centered_rect(width, height, area);

            f.render_widget(Clear, rect);

            let mut lines = vec![
                Line::from(Span::styled(message.as_str(), Style::default().fg(Color::Gray))),
                Line::from(""),
            ];
            for w in warnings {
                lines.push(Line::from(Span::styled(
                    format!("  {}", w),
                    Style::default().fg(Color::Rgb(180, 170, 100)),
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    " y ",
                    Style::default()
                        .fg(Color::Rgb(100, 180, 100)),
                ),
                Span::styled("es  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    " n ",
                    Style::default()
                        .fg(Color::Rgb(180, 100, 100)),
                ),
                Span::styled("o", Style::default().fg(Color::DarkGray)),
            ]));

            let p = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Rgb(50, 50, 50)))
                    .title(format!(" {} ", title))
                    .title_style(Style::default().fg(Color::DarkGray)),
            );

            f.render_widget(p, rect);
        }
        Overlay::Help => {
            let rect = centered_rect(50, 18, area);
            f.render_widget(Clear, rect);

            let key_style = Style::default().fg(Color::Rgb(80, 140, 200));
            let desc_style = Style::default().fg(Color::Gray);

            let lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("  1/2/3  ", key_style),
                    Span::styled("switch tabs", desc_style),
                ]),
                Line::from(vec![
                    Span::styled("  /      ", key_style),
                    Span::styled("search / filter", desc_style),
                ]),
                Line::from(vec![
                    Span::styled("  a      ", key_style),
                    Span::styled("ai actions", desc_style),
                ]),
                Line::from(vec![
                    Span::styled("  A      ", key_style),
                    Span::styled("cycle ai provider", desc_style),
                ]),
                Line::from(vec![
                    Span::styled("  x      ", key_style),
                    Span::styled("quick actions", desc_style),
                ]),
                Line::from(vec![
                    Span::styled("  enter  ", key_style),
                    Span::styled("drill into", desc_style),
                ]),
                Line::from(vec![
                    Span::styled("  esc    ", key_style),
                    Span::styled("back / close", desc_style),
                ]),
                Line::from(vec![
                    Span::styled("  j/k    ", key_style),
                    Span::styled("navigate", desc_style),
                ]),
                Line::from(vec![
                    Span::styled("  h/l    ", key_style),
                    Span::styled("sub-tabs", desc_style),
                ]),
                Line::from(vec![
                    Span::styled("  tab    ", key_style),
                    Span::styled("toggle detail", desc_style),
                ]),
                Line::from(vec![
                    Span::styled("  q      ", key_style),
                    Span::styled("quit", desc_style),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  press any key to close",
                    Style::default().fg(Color::Rgb(50, 50, 50)),
                )),
            ];

            let p = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Rgb(50, 50, 50)))
                    .title(" ? ")
                    .title_style(Style::default().fg(Color::DarkGray)),
            );

            f.render_widget(p, rect);
        }
        Overlay::AiSettings | Overlay::AiPrompt { .. } => {
            // Rendered by ai module
        }
    }
}
