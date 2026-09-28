//! Drawing. Every style names its own foreground: a bare modifier inherits
//! whatever the terminal's theme has there, which is how the herdr panel once
//! drew black on black.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap};
use tui_term::widget::PseudoTerminal;

use crate::app::{App, Focus, Listing};

mod theme {
    use super::*;
    pub const TEXT: Color = Color::White;
    pub const DIM: Color = Color::Gray;
    pub const MUTED: Color = Color::DarkGray;
    pub const ACCENT: Color = Color::Cyan;
    pub const PRIMARY: Color = Color::Green;
    pub const SERVED: Color = Color::Blue;
    pub const DIRTY: Color = Color::Yellow;
    pub const ERROR: Color = Color::Red;

    pub fn text() -> Style {
        Style::new().fg(TEXT)
    }
    pub fn dim() -> Style {
        Style::new().fg(DIM)
    }
    pub fn border(focused: bool) -> Style {
        Style::new().fg(if focused { ACCENT } else { MUTED })
    }
    pub fn selected() -> Style {
        Style::new().fg(Color::Black).bg(ACCENT)
    }
}

const SIDEBAR_WIDTH: u16 = 34;

/// Where everything goes. Shared with the event loop, which sizes each pane's
/// PTY to exactly the room it will be drawn in.
pub struct Areas {
    pub header: Rect,
    pub sidebar: Rect,
    pub pane: Rect,
    pub pane_inner: Rect,
    pub footer: Rect,
}

pub fn areas(area: Rect) -> Areas {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(area);
    let side = SIDEBAR_WIDTH.min(body.width / 2);
    let [sidebar, pane] =
        Layout::horizontal([Constraint::Length(side), Constraint::Min(10)]).areas(body);
    let pane_inner = Block::bordered().inner(pane);
    Areas {
        header,
        sidebar,
        pane,
        pane_inner,
        footer,
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let a = areas(f.area());
    draw_header(f, app, a.header);
    draw_sidebar(f, app, a.sidebar);
    draw_pane(f, app, a.pane);
    draw_footer(f, app, a.footer);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let count = match app.listing {
        Listing::Loaded => format!("{} worktrees ", app.worktrees.len()),
        _ => String::new(),
    };
    let left = Line::from(vec![
        Span::styled(
            " tryout ",
            Style::new().fg(Color::Black).bg(theme::ACCENT).bold(),
        ),
        Span::styled(format!(" {}", app.project), theme::text().bold()),
    ]);
    f.render_widget(Paragraph::new(left), area);
    f.render_widget(
        Paragraph::new(Span::styled(count, theme::dim())).alignment(Alignment::Right),
        area,
    );
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border(app.focus == Focus::List))
        .title(Span::styled(" Worktrees ", theme::text().bold()));
    let inner = block.inner(area);
    f.render_widget(block, area);
    // Messages get a column of air; list rows carry their own leading space.
    let text_area = Block::new()
        .padding(ratatui::widgets::Padding::horizontal(1))
        .inner(inner);

    match &app.listing {
        Listing::Loading if app.worktrees.is_empty() => {
            f.render_widget(
                Paragraph::new(Span::styled("Loading worktrees…", theme::dim())),
                text_area,
            );
            return;
        }
        Listing::Failed(msg) => {
            let text = vec![
                Line::styled(
                    "Could not list worktrees",
                    Style::new().fg(theme::ERROR).bold(),
                ),
                Line::default(),
                Line::styled(msg.clone(), theme::text()),
                Line::default(),
                Line::styled("r  retry", theme::dim()),
            ];
            f.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), text_area);
            return;
        }
        _ => {}
    }

    let width = inner.width as usize;
    let items: Vec<ListItem> = app
        .worktrees
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let selected = i == app.selected;
            let base = if selected {
                theme::selected()
            } else {
                theme::text()
            };
            let sub = if selected {
                theme::selected()
            } else {
                theme::dim()
            };
            let (mark, mark_color) = if w.primary {
                ("●", theme::PRIMARY)
            } else if w.served() {
                ("◆", theme::SERVED)
            } else {
                ("○", theme::MUTED)
            };
            let mark_style = if selected {
                base
            } else {
                Style::new().fg(mark_color)
            };
            let running = app.panes.get(&w.name).is_some_and(|p| p.is_alive());
            let mut badges = String::new();
            if w.dirty {
                badges.push_str(" ±");
            }
            if running {
                badges.push_str(" ▶");
            }
            let name_room = width.saturating_sub(3 + badges.chars().count());
            let name = truncate(&w.name, name_room);
            let pad = name_room.saturating_sub(name.chars().count());
            let badge_style = if selected {
                base
            } else {
                Style::new().fg(theme::DIRTY)
            };
            let first = Line::from(vec![
                Span::styled(format!(" {mark} "), mark_style),
                Span::styled(name, base.add_modifier(Modifier::BOLD)),
                Span::styled(" ".repeat(pad), base),
                Span::styled(badges, badge_style),
            ]);
            let branch = if w.branch == "(detached)" {
                "detached"
            } else {
                &w.branch
            };
            let detail = truncate(&format!("   {branch} · {}", short(&w.head)), width);
            let second = Line::styled(format!("{detail:<width$}"), sub);
            ListItem::new(vec![first, second])
        })
        .collect();

    let mut state = ListState::default().with_selected(Some(app.selected));
    f.render_stateful_widget(List::new(items), inner, &mut state);
}

fn draw_pane(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Pane;
    let Some(w) = app.selected() else {
        f.render_widget(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(theme::border(false)),
            area,
        );
        return;
    };
    let mut title = vec![Span::styled(format!(" {} ", w.name), theme::text().bold())];
    if let Some(url) = &w.url {
        title.push(Span::styled(
            format!("{url} "),
            Style::new().fg(theme::ACCENT),
        ));
    }
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border(focused))
        .title(Line::from(title));

    match app.panes.get(&w.name) {
        Some(pane) => {
            let alive = pane.is_alive();
            let block = if alive {
                block
            } else {
                block.title_bottom(Span::styled(
                    " shell exited — Enter starts a new one ",
                    Style::new().fg(theme::DIRTY),
                ))
            };
            pane.with_screen(|screen| {
                let mut term = PseudoTerminal::new(screen).block(block);
                // Show the cursor only where keys actually go.
                if !(focused && alive) {
                    let mut cursor = tui_term::widget::Cursor::default();
                    cursor.hide();
                    term = term.cursor(cursor);
                }
                f.render_widget(term, area);
            });
        }
        None => {
            let inner = block.inner(area);
            f.render_widget(block, area);
            f.render_widget(placeholder(app, w), inner);
        }
    }
}

fn placeholder<'a>(app: &App, w: &'a crate::worktrees::Worktree) -> Paragraph<'a> {
    let dir = app.checkout_dir(&w.name);
    let dir = dir
        .strip_prefix(&app.root)
        .ok()
        .filter(|d| !d.as_os_str().is_empty());
    let row = |label: &str, value: String| {
        Line::from(vec![
            Span::styled(format!("{label:<9}"), theme::dim()),
            Span::styled(value, theme::text()),
        ])
    };
    let mut lines = vec![
        Line::default(),
        row("branch", w.branch.clone()),
        row("head", w.head.clone()),
        row(
            "state",
            if w.dirty {
                "uncommitted changes".into()
            } else {
                "clean".into()
            },
        ),
        row(
            "dir",
            dir.map_or("project root".into(), |d| d.display().to_string()),
        ),
    ];
    match (&w.url, &w.php) {
        (Some(url), php) => {
            lines.push(row("site", url.clone()));
            lines.push(row(
                "runtime",
                format!(
                    "PHP {} · {}",
                    php.as_deref().unwrap_or("-"),
                    w.db.as_deref().unwrap_or("-")
                ),
            ));
        }
        (None, _) => lines.push(row("site", "not served".into())),
    }
    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled("Enter", Style::new().fg(theme::ACCENT).bold()),
        Span::styled("  open a shell in this worktree", theme::text()),
    ]));
    Paragraph::new(lines).block(Block::new().padding(ratatui::widgets::Padding::horizontal(2)))
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let hints: &[(&str, &str)] = match app.focus {
        Focus::List => &[
            ("↑↓", "select"),
            ("⏎", "shell"),
            ("r", "reload"),
            ("q", "quit"),
        ],
        Focus::Pane => &[
            ("^G", "back to the list"),
            ("", "every other key goes to the shell"),
        ],
    };
    let mut spans = vec![Span::raw(" ")];
    for (key, what) in hints {
        if !key.is_empty() {
            spans.push(Span::styled(*key, Style::new().fg(theme::ACCENT).bold()));
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(format!("{what}   "), theme::dim()));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
    if let Some(notice) = &app.notice {
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("{notice} "),
                Style::new().fg(theme::DIRTY),
            ))
            .alignment(Alignment::Right),
            area,
        );
    }
}

fn short(head: &str) -> &str {
    &head[..head.len().min(7)]
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let keep = max.saturating_sub(1);
    s.chars().take(keep).chain(std::iter::once('…')).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::fixture;
    use ratatui::{Terminal, backend::TestBackend};
    use std::path::PathBuf;

    fn render(app: &App, w: u16, h: u16) -> Terminal<TestBackend> {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        t
    }

    fn loaded() -> App {
        let mut a = App::new(PathBuf::from("/p/demo"));
        a.set_worktrees(Ok(fixture()));
        a
    }

    #[test]
    fn layout_at_80x24() {
        insta::assert_snapshot!(render(&loaded(), 80, 24).backend());
    }

    #[test]
    fn layout_at_200x50() {
        insta::assert_snapshot!(render(&loaded(), 200, 50).backend());
    }

    #[test]
    fn loading_and_failure_states() {
        let mut a = App::new(PathBuf::from("/p/demo"));
        insta::assert_snapshot!("loading", render(&a, 60, 12).backend());
        a.set_worktrees(Err(anyhow::anyhow!(
            "ddev tryout worktree list failed: project not running"
        )));
        insta::assert_snapshot!("failed", render(&a, 60, 12).backend());
    }

    #[test]
    fn every_drawn_cell_names_its_own_foreground() {
        let t = render(&loaded(), 80, 24);
        let buf = t.backend().buffer();
        for (i, cell) in buf.content.iter().enumerate() {
            if cell.symbol().trim().is_empty() {
                continue;
            }
            assert_ne!(
                cell.fg,
                Color::Reset,
                "cell {i} ({:?}) inherits the terminal's foreground",
                cell.symbol()
            );
        }
    }

    #[test]
    fn the_pane_leaves_room_for_its_border() {
        let a = areas(Rect::new(0, 0, 80, 24));
        assert_eq!(a.pane_inner.width, a.pane.width - 2);
        assert_eq!(a.pane_inner.height, a.pane.height - 2);
    }
}
