//! Drawing. Every style names its own foreground: a bare modifier inherits
//! whatever the terminal's theme has there, which is how the herdr panel once
//! drew black on black.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
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

/// The popup a tryout command runs in: most of the screen, centred, so a gum
/// prompt or a long report has room.
pub fn popup_rect(area: Rect) -> Rect {
    let w = (area.width * 4 / 5).max(40).min(area.width);
    let h = (area.height * 7 / 10).max(10).min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

/// Where the command's terminal goes inside the popup's border.
pub fn popup_inner(area: Rect) -> Rect {
    Block::bordered().inner(popup_rect(area))
}

pub fn draw(f: &mut Frame, app: &App) {
    let a = areas(f.area());
    draw_header(f, app, a.header);
    draw_sidebar(f, app, a.sidebar);
    draw_pane(f, app, a.pane);
    draw_footer(f, app, a.footer);
    if app.menu.is_some() {
        draw_menu(f, app, a.pane);
    }
    if app.popup.is_some() {
        draw_popup(f, app, f.area());
    }
}

fn draw_menu(f: &mut Frame, app: &App, over: Rect) {
    let Some(menu) = &app.menu else { return };
    let label_w = menu
        .items
        .iter()
        .map(|a| a.label.chars().count())
        .max()
        .unwrap_or(0);
    let hint_w = menu
        .items
        .iter()
        .map(|a| a.hint.chars().count())
        .max()
        .unwrap_or(0);
    let w = ((label_w + hint_w + 7) as u16).min(over.width);
    let h = (menu.items.len() as u16 + 2).min(over.height);
    let area = Rect::new(over.x + 2, over.y + 1, w, h).intersection(over);
    let items: Vec<ListItem> = menu
        .items
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let selected = i == menu.selected;
            let (label, hint) = if selected {
                (theme::selected().bold(), theme::selected())
            } else {
                (theme::text().bold(), theme::dim())
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {:<label_w$}  ", a.label), label),
                Span::styled(format!("{:<hint_w$} ", a.hint), hint),
            ]))
        })
        .collect();
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border(true))
        .title(Span::styled(
            format!(" {} ", menu.worktree),
            theme::text().bold(),
        ));
    f.render_widget(Clear, area);
    let mut state = ListState::default().with_selected(Some(menu.selected));
    f.render_stateful_widget(List::new(items).block(block), area, &mut state);
}

fn draw_popup(f: &mut Frame, app: &App, screen: Rect) {
    let Some(popup) = &app.popup else { return };
    let area = popup_rect(screen);
    let title = Span::styled(
        format!(" {} ", popup.action.command_line()),
        theme::text().bold(),
    );
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border(true))
        .title(title);
    let alive = popup.pane.is_alive();
    block = match popup.pane.exit_code() {
        Some(0) => block.title_bottom(Span::styled(
            " done — Enter closes ",
            Style::new().fg(theme::PRIMARY),
        )),
        Some(code) => block
            .border_style(Style::new().fg(theme::ERROR))
            .title_bottom(Span::styled(
                format!(" failed (exit {code}) — Enter closes "),
                Style::new().fg(theme::ERROR).bold(),
            )),
        None => block,
    };
    f.render_widget(Clear, area);
    popup.pane.with_screen(|screen| {
        let mut term = PseudoTerminal::new(screen).block(block);
        if !alive {
            let mut cursor = tui_term::widget::Cursor::default();
            cursor.hide();
            term = term.cursor(cursor);
        }
        f.render_widget(term, area);
    });
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
            if w.dirty() {
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
            // The branch it is on, or for a detached checkout the one it came
            // from; "+N" is the patches on top.
            let at = w
                .branch
                .as_deref()
                .or(w.base.as_deref())
                .unwrap_or("detached");
            let patches = if w.patches > 0 {
                format!(" +{}", w.patches)
            } else {
                String::new()
            };
            let detail = truncate(&format!("   {at}{patches} · {}", short(&w.head)), width);
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
            f.render_widget(placeholder(w, inner.width), inner);
        }
    }
}

fn placeholder<'a>(w: &'a crate::worktrees::Worktree, width: u16) -> Paragraph<'a> {
    // Label column plus the horizontal padding.
    let room = (width as usize).saturating_sub(9 + 4);
    let row = |label: &str, value: String| {
        Line::from(vec![
            Span::styled(format!("{label:<9}"), theme::dim()),
            Span::styled(value, theme::text()),
        ])
    };
    let changes = match (w.modified, w.untracked) {
        (0, 0) => "clean".to_string(),
        (m, 0) => format!("{m} modified"),
        (0, u) => format!("{u} untracked"),
        (m, u) => format!("{m} modified, {u} untracked"),
    };
    let mut lines = vec![
        Line::default(),
        row("where", w.position()),
        row(
            "head",
            truncate(
                &format!("{} {}", short(&w.head), w.subject.as_deref().unwrap_or("")),
                room,
            ),
        ),
    ];
    if w.patches > 0 {
        let noun = if w.patches == 1 { "patch" } else { "patches" };
        lines.push(row(
            "patches",
            format!(
                "{} {noun} on top of {}",
                w.patches,
                w.base.as_deref().unwrap_or("its base")
            ),
        ));
    }
    lines.push(row("changes", changes));
    lines.push(row(
        "dir",
        if w.dir == "." {
            "project root".into()
        } else {
            w.dir.clone()
        },
    ));
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
    let running = app.popup.as_ref().map(|p| p.pane.is_alive());
    let hints: &[(&str, &str)] = match (running, app.menu.is_some(), app.focus) {
        (Some(true), ..) => &[("", "the command has the keyboard until it ends")],
        (Some(false), ..) => &[("⏎", "close")],
        (None, true, _) => &[("↑↓", "select"), ("⏎", "run"), ("esc", "cancel")],
        (None, false, Focus::List) => &[
            ("↑↓", "select"),
            ("⏎", "shell"),
            ("a", "actions"),
            ("r", "reload"),
            ("q", "quit"),
        ],
        (None, false, Focus::Pane) => &[
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
    fn the_action_menu_over_a_served_worktree() {
        let mut a = loaded();
        a.selected = 1;
        a.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('a'),
            crossterm::event::KeyModifiers::NONE,
        ));
        insta::assert_snapshot!(render(&a, 100, 26).backend());
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
