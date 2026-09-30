//! Drawing. Every style names its own foreground: a bare modifier inherits
//! whatever the terminal's theme has there — black on black, on some themes.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use tui_term::widget::PseudoTerminal;

use crate::tui::actions::Entry;
use crate::tui::agents::Status;
use crate::tui::app::{App, Focus, Listing, MenuHit, Workspace};

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

/// The narrowest the sidebar goes: a worktree name and its badges still fit.
pub const SIDEBAR_MIN: u16 = 24;
/// What the right pane keeps however wide the sidebar is dragged: the shells
/// stay usable.
pub const PANE_MIN: u16 = 40;

/// The widest the sidebar may be on a screen this wide.
pub fn sidebar_max(total: u16) -> u16 {
    total
        .saturating_sub(PANE_MIN)
        .max(SIDEBAR_MIN.min(total / 2))
}
/// Rows per worktree in the sidebar: its name; where it is (branch, patches,
/// commit); how it is served (PHP, database type).
const ITEM_HEIGHT: u16 = 3;

/// The first worktree the sidebar shows: just enough scroll to keep the
/// selection in view. Drawing and click hit-testing both use it, so a click
/// always lands on the row that was drawn there.
fn sidebar_offset(selected: usize, inner_height: u16) -> usize {
    let visible = (inner_height / ITEM_HEIGHT).max(1) as usize;
    (selected + 1).saturating_sub(visible)
}

/// Whether a screen cell is on the divider between the worktree list and the
/// pane: either of the two border columns that meet there, on the body rows.
pub fn divider_at(screen: Rect, app: &App, col: u16, row: u16) -> bool {
    let a = areas(screen, app.sidebar_width);
    let on_col = col + 1 == a.sidebar.right() || col == a.pane.x;
    on_col && row >= a.sidebar.y && row < a.sidebar.bottom()
}

/// The worktree drawn at a screen cell, if any.
pub fn worktree_at(screen: Rect, app: &App, col: u16, row: u16) -> Option<usize> {
    let inner = Block::bordered().inner(side(areas(screen, app.sidebar_width).sidebar, app).list);
    if !inner.contains(ratatui::layout::Position::new(col, row)) {
        return None;
    }
    let i = sidebar_offset(app.selected, inner.height) + ((row - inner.y) / ITEM_HEIGHT) as usize;
    (i < app.worktrees.len()).then_some(i)
}

/// Where everything goes. Shared with the event loop, which sizes each pane's
/// PTY to exactly the room it will be drawn in.
pub struct Areas {
    pub header: Rect,
    pub sidebar: Rect,
    pub pane: Rect,
    pub pane_inner: Rect,
    pub footer: Rect,
}

/// `sidebar`: the width asked for (`App::sidebar_width`), kept within bounds
/// here — a terminal made narrower pulls it in without losing the setting.
pub fn areas(area: Rect, sidebar: u16) -> Areas {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(area);
    let max = sidebar_max(body.width);
    let side = sidebar.clamp(SIDEBAR_MIN.min(max), max);
    let [sidebar, pane] =
        Layout::horizontal([Constraint::Length(side), Constraint::Min(10)]).areas(body);
    let (_, pane_inner) = split_tab_bar(Block::bordered().inner(pane));
    Areas {
        header,
        sidebar,
        pane,
        pane_inner,
        footer,
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let a = areas(f.area(), app.sidebar_width);
    draw_header(f, app, a.header);
    let agents = app.agents();
    let parts = side(a.sidebar, app);
    draw_sidebar(f, app, parts.list);
    if let Some(area) = parts.agents {
        draw_agents(f, &agents, area);
    }
    if let Some(area) = parts.activity {
        draw_activity(f, app, area);
    }
    draw_pane(f, app, a.pane);
    draw_footer(f, app, a.footer);
    if app.menu.is_some() {
        draw_menu(f, app);
    }
    if app.rename.is_some() {
        draw_rename(f, app, a.pane);
    }
    if app.form.is_some() {
        draw_form(f, app, f.area());
    }
    if app.confirm_close {
        draw_confirm_close(f, app, f.area());
    }
    if app.password.is_some() {
        draw_password(f, app, f.area());
    }
}

/// A job asks for a password (sudo, for DDEV's hosts file): a masked field.
fn draw_password(f: &mut Frame, app: &App, screen: Rect) {
    let Some(p) = &app.password else { return };
    let w = 60.min(screen.width);
    let area = Rect::new(
        screen.x + (screen.width - w) / 2,
        screen.y + screen.height / 3,
        w,
        8,
    )
    .intersection(screen);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme::ACCENT))
        .title(Span::styled(" Password needed ", theme::text().bold()));
    let dots = "•".repeat(p.value.chars().count());
    let lines = vec![
        Line::styled(format!(" {} asks:", p.label), theme::text()),
        Line::styled(format!(" {}", p.prompt), theme::text().bold()),
        Line::default(),
        Line::from(vec![
            Span::styled("   ", theme::text()),
            Span::styled(dots, theme::text()),
            Span::styled("▏", Style::new().fg(theme::ACCENT)),
        ]),
        Line::default(),
        Line::styled(
            " Sent to that command only — never stored or logged.",
            theme::dim(),
        ),
    ];
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(lines).block(block), area);
}

/// How many options a pick list shows at once.
const PICK_ROWS: usize = 8;

/// A form's width: the patch form takes room for votes, subject and owner.
fn form_width(form: &crate::tui::forms::Form, screen: Rect) -> u16 {
    let want = if form.kind.wants_patches().is_some() {
        110
    } else {
        64
    };
    want.min(screen.width)
}

/// One open change: tick, number, votes (Code-Review, Verified), subject, owner.
fn change_row(
    c: &crate::tui::forms::Change,
    ticked: bool,
    on: bool,
    width: usize,
) -> Line<'static> {
    const OWNER: usize = 20;
    let base = if on { theme::selected() } else { theme::text() };
    let vote = |label: &str| -> Span<'static> {
        let v = c
            .scores
            .split_whitespace()
            .find_map(|s| s.strip_prefix(label))
            .unwrap_or("");
        let colour = match v {
            "+2" | "+1" => theme::PRIMARY,
            "-2" | "-1" => theme::ERROR,
            _ => theme::MUTED,
        };
        // No vote yet is a dot, not a blank that reads like a missing column.
        let text = format!("{label}{}", if v.is_empty() { " ·" } else { v });
        Span::styled(
            format!("{text:<5}"),
            if on { base } else { Style::new().fg(colour) },
        )
    };
    let tick = if ticked { "[x]" } else { "[ ]" };
    // Indent, tick, number and the two votes take 29 columns; the owner 20 more.
    let room = width.saturating_sub(4 + 29 + OWNER + 2);
    Line::from(vec![
        Span::styled(format!("   {tick} {:<7}", c.number), base),
        vote("CR"),
        Span::styled(" ", base),
        vote("V"),
        Span::styled("  ", base),
        Span::styled(format!("{:<room$}", truncate(&c.subject, room)), base),
        Span::styled(
            format!("  {:<OWNER$}", truncate(&c.owner, OWNER)),
            if on { base } else { theme::dim() },
        ),
    ])
}

/// What a line of a form is, for a click on it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum FormLine {
    Other,
    /// The `i`th change shown.
    Change(usize),
    /// The page bar: the columns (from the form's inner left edge) of its
    /// buttons, when they can be pressed.
    Pages {
        prev: Option<(u16, u16)>,
        next: Option<(u16, u16)>,
    },
}

/// Where a click on a form landed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormHit {
    Change(usize),
    PrevPage,
    NextPage,
}

/// A form's lines, and what each one is. Drawing and clicks both come from here.
fn form_lines<'a>(
    form: &'a crate::tui::forms::Form,
    screen: Rect,
) -> (Vec<Line<'a>>, Vec<FormLine>) {
    use crate::tui::forms::Field;
    let mut lines: Vec<Line> = Vec::new();
    let mut roles: Vec<FormLine> = Vec::new();
    if form.is_confirmation() {
        for l in &form.confirm {
            lines.push(Line::styled(format!(" {l}"), theme::text()));
        }
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled(" y", Style::new().fg(theme::ERROR).bold()),
            Span::styled(" go ahead   ", theme::text()),
            Span::styled("any other key", Style::new().fg(theme::ACCENT).bold()),
            Span::styled(" cancel", theme::text()),
        ]));
    }
    for (i, field) in form.fields.iter().enumerate() {
        let focused = i == form.focus;
        let label_style = if focused {
            Style::new().fg(theme::ACCENT).bold()
        } else {
            theme::text().bold()
        };
        match field {
            Field::Text { label, value, hint } => {
                lines.push(Line::styled(format!(" {label}"), label_style));
                lines.push(Line::from(vec![
                    Span::styled("   ", theme::text()),
                    Span::styled(value.clone(), theme::text()),
                    Span::styled(
                        if focused { "▏" } else { "" },
                        Style::new().fg(theme::ACCENT),
                    ),
                ]));
                lines.push(Line::styled(format!("   {hint}"), theme::dim()));
            }
            Field::Pick {
                label,
                options,
                filter,
                selected,
            } => {
                let head = if filter.is_empty() {
                    format!(" {label}")
                } else {
                    format!(" {label}  (filter: {filter})")
                };
                lines.push(Line::styled(head, label_style));
                match options {
                    None => lines.push(Line::styled("   loading the branches…", theme::dim())),
                    Some(_) => {
                        let visible = field.visible();
                        if visible.is_empty() {
                            lines.push(Line::styled("   nothing matches", theme::dim()));
                        }
                        // Keep the selection in view.
                        let first = (*selected + 1).saturating_sub(PICK_ROWS);
                        for (j, v) in visible.iter().enumerate().skip(first).take(PICK_ROWS) {
                            let on = j == *selected;
                            let style = match (on, focused) {
                                (true, true) => theme::selected(),
                                (true, false) => theme::text().bold(),
                                _ => theme::dim(),
                            };
                            lines.push(Line::styled(
                                format!("   {} {v}", if on { "›" } else { " " }),
                                style,
                            ));
                        }
                    }
                }
            }
            Field::Check { label, value } => {
                lines.push(Line::styled(
                    format!(" [{}] {label}", if *value { "x" } else { " " }),
                    label_style,
                ));
            }
            Field::Choose {
                label,
                options,
                filter,
                selected,
                chosen,
                page,
                more,
                pending,
                in_list,
                ..
            } => {
                let mut head = vec![Span::styled(format!(" {label}"), label_style)];
                if !chosen.is_empty() {
                    head.push(Span::styled(
                        format!("  {} ticked", chosen.len()),
                        theme::dim(),
                    ));
                }
                lines.push(Line::from(head));
                // The search box: what is typed goes to Gerrit.
                lines.push(Line::from(vec![
                    Span::styled("   search ", theme::dim()),
                    Span::styled(filter.clone(), theme::text()),
                    Span::styled(
                        if focused && !*in_list { "▏" } else { "" },
                        Style::new().fg(theme::ACCENT),
                    ),
                ]));
                let width = form_width(form, screen) as usize;
                match options {
                    None => lines.push(Line::styled("   asking Gerrit…", theme::dim())),
                    Some(_) => {
                        let visible = field.visible_changes();
                        if visible.is_empty() {
                            lines.push(Line::styled("   no open change matches", theme::dim()));
                        }
                        // A whole page at once where the screen allows (25 rows),
                        // scrolling only on a terminal too short for it.
                        let fit = change_rows(screen, form.error.is_some());
                        let first = (*selected + 1).saturating_sub(fit);
                        for (j, c) in visible.iter().enumerate().skip(first).take(fit) {
                            let on = j == *selected && focused && *in_list;
                            while roles.len() < lines.len() {
                                roles.push(FormLine::Other);
                            }
                            roles.push(FormLine::Change(j));
                            lines.push(change_row(c, chosen.contains(&c.number), on, width));
                        }
                    }
                }
                // The page bar: previous and next as buttons, where the page is.
                let shown = options.as_ref().map_or(0, Vec::len) as u32;
                let from = page * crate::core::gerrit::PAGE_SIZE + 1;
                let (can_prev, can_next) =
                    (*page > 0 && pending.is_none(), *more && pending.is_none());
                let button = |label: &'static str, on: bool| {
                    Span::styled(
                        label,
                        if on {
                            Style::new().fg(theme::ACCENT).bold()
                        } else {
                            theme::dim()
                        },
                    )
                };
                let prev_label = "‹ previous";
                let where_ = if shown == 0 {
                    format!("page {}", page + 1)
                } else {
                    format!("page {} · {}–{}", page + 1, from, from + shown - 1)
                };
                let middle = format!("   {where_}   ");
                let prev_at = 3u16;
                let next_at =
                    prev_at + prev_label.chars().count() as u16 + middle.chars().count() as u16;
                let mut spans = vec![
                    Span::styled("   ", theme::dim()),
                    button(prev_label, can_prev),
                    Span::styled(middle, theme::text()),
                    button("next ›", can_next),
                ];
                if pending.is_some() {
                    spans.push(Span::styled("   searching…", theme::dim()));
                }
                while roles.len() < lines.len() {
                    roles.push(FormLine::Other);
                }
                roles.push(FormLine::Pages {
                    prev: can_prev
                        .then_some((prev_at, prev_at + prev_label.chars().count() as u16)),
                    next: can_next.then_some((next_at, next_at + 6)),
                });
                lines.push(Line::from(spans));
            }
        }
        lines.push(Line::default());
    }
    if let Some(e) = &form.error {
        lines.push(Line::styled(
            format!(" {e}"),
            Style::new().fg(theme::ERROR).bold(),
        ));
    }
    while roles.len() < lines.len() {
        roles.push(FormLine::Other);
    }
    (lines, roles)
}

/// Where a form with `lines` lines sits on the screen.
fn form_area(form: &crate::tui::forms::Form, screen: Rect, lines: usize) -> Rect {
    let w = form_width(form, screen);
    let h = (lines as u16 + 2).min(screen.height);
    Rect::new(
        screen.x + (screen.width - w) / 2,
        screen.y + (screen.height.saturating_sub(h)) / 3,
        w,
        h,
    )
}

/// How many changes the patch form can show without scrolling: a whole page,
/// unless the terminal is too short — then what fits beside the form's other
/// lines (title, search, page bar, borders and air).
fn change_rows(screen: Rect, error: bool) -> usize {
    let other = 7 + usize::from(error);
    (screen.height as usize)
        .saturating_sub(other)
        .clamp(3, crate::core::gerrit::PAGE_SIZE as usize)
}

/// What a click at (col, row) hits in the open form, if anything.
pub fn form_hit(screen: Rect, app: &App, col: u16, row: u16) -> Option<FormHit> {
    let form = app.form.as_ref()?;
    let (lines, roles) = form_lines(form, screen);
    let area = form_area(form, screen, lines.len());
    let inner = Block::bordered().inner(area);
    if !inner.contains(ratatui::layout::Position::new(col, row)) {
        return None;
    }
    let x = col - inner.x;
    match roles.get((row - inner.y) as usize)? {
        FormLine::Change(i) => Some(FormHit::Change(*i)),
        FormLine::Pages { prev, next } => {
            let within = |r: &Option<(u16, u16)>| r.is_some_and(|(a, b)| x >= a && x < b);
            if within(prev) {
                Some(FormHit::PrevPage)
            } else if within(next) {
                Some(FormHit::NextPage)
            } else {
                None
            }
        }
        FormLine::Other => None,
    }
}

fn draw_form(f: &mut Frame, app: &App, screen: Rect) {
    let Some(form) = &app.form else { return };
    let (lines, _) = form_lines(form, screen);
    let area = form_area(form, screen, lines.len());
    let border = if form.is_confirmation() {
        Style::new().fg(theme::ERROR)
    } else {
        theme::border(true)
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Span::styled(
            format!(" {} ", form.title),
            theme::text().bold(),
        ));
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_confirm_close(f: &mut Frame, app: &App, screen: Rect) {
    let (tabs, agents) = app.close_cost();
    let w = 52.min(screen.width);
    let area = Rect::new(
        screen.x + (screen.width - w) / 2,
        screen.y + screen.height / 3,
        w,
        6,
    )
    .intersection(screen);
    let plural = |n: usize, one: &str, many: &str| {
        if n == 1 {
            format!("1 {one}")
        } else {
            format!("{n} {many}")
        }
    };
    let what = if agents > 0 {
        format!(
            "{} and {}",
            plural(tabs, "tab", "tabs"),
            plural(agents, "agent", "agents")
        )
    } else {
        plural(tabs, "tab", "tabs")
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme::ERROR))
        .title(Span::styled(" Close the session? ", theme::text().bold()));
    let lines = vec![
        Line::styled(
            format!(" This ends {what}, and any running command."),
            theme::text(),
        ),
        Line::styled(
            " To keep them running, detach with q instead.",
            theme::dim(),
        ),
        Line::default(),
        Line::from(vec![
            Span::styled(" y", Style::new().fg(theme::ERROR).bold()),
            Span::styled(" close   ", theme::text()),
            Span::styled("any other key", Style::new().fg(theme::ACCENT).bold()),
            Span::styled(" keep", theme::text()),
        ]),
    ];
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_rename(f: &mut Frame, app: &App, over: Rect) {
    let Some(r) = &app.rename else { return };
    let w = 44.min(over.width);
    // Below the tab bar, never over it: you should see which tab you rename.
    let area = Rect::new(over.x + 2, over.y + 2, w, 5).intersection(over);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border(true))
        .title(Span::styled(" Rename tab ", theme::text().bold()));
    let room = (w as usize).saturating_sub(5);
    // Show the end of a long name: that is where the cursor is.
    let shown: String = {
        let n = r.text.chars().count();
        r.text.chars().skip(n.saturating_sub(room)).collect()
    };
    // Selected text is drawn as selected: it is what the next key replaces.
    let text_style = if r.selected {
        theme::selected()
    } else {
        theme::text().bold()
    };
    let lines = vec![
        Line::from(vec![
            Span::styled(" ", theme::text()),
            Span::styled(shown, text_style),
            Span::styled("▏", Style::new().fg(theme::ACCENT)),
        ]),
        Line::default(),
        Line::styled(" empty: its program's own title", theme::dim()),
    ];
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(lines).block(block), area);
}

/// Where the menu is drawn: at the pointer for a right-click (kept on screen),
/// else at the top of the pane. Drawing and click hit-testing both use it.
pub fn menu_rect(screen: Rect, app: &App) -> Option<Rect> {
    let menu = app.menu.as_ref()?;
    let (label_w, hint_w) = widths(menu.items.iter().map(|e| (e.label(), e.hint())));
    // Room for the label, the hint, the ▸ column and the borders.
    let w = ((label_w + hint_w + 9) as u16).min(screen.width);
    let h = (menu.items.len() as u16 + 2).min(screen.height);
    let (x, y) = match menu.anchor {
        Some((x, y)) => (x, y),
        None => {
            let pane = areas(screen, app.sidebar_width).pane;
            (pane.x + 2, pane.y + 1)
        }
    };
    let x = x.min(screen.right().saturating_sub(w));
    let y = y.min(screen.bottom().saturating_sub(h));
    Some(Rect::new(x, y, w, h))
}

/// The open submenu's box: beside its row, to the right when there is room,
/// else to the left — never over the entry that opened it.
pub fn submenu_rect(screen: Rect, app: &App) -> Option<Rect> {
    let menu = app.menu.as_ref()?;
    let items = menu.sub_items()?;
    let top = menu_rect(screen, app)?;
    let (label_w, hint_w) = widths(items.iter().map(|a| (a.label.as_str(), a.hint.as_str())));
    let w = ((label_w + hint_w + 7) as u16).min(screen.width);
    let h = (items.len() as u16 + 2).min(screen.height);
    let x = if top.right() + w <= screen.right() {
        top.right()
    } else if top.x >= screen.x + w {
        top.x - w
    } else {
        // Room on neither side: against the right edge, over the menu's end.
        screen.right().saturating_sub(w)
    };
    // Its first item level with the entry that opened it.
    let y = (top.y + menu.selected as u16).min(screen.bottom().saturating_sub(h));
    Some(Rect::new(x, y, w, h))
}

fn widths<'a>(rows: impl Iterator<Item = (&'a str, &'a str)> + Clone) -> (usize, usize) {
    let max = |f: fn((&str, &str)) -> usize| rows.clone().map(f).max().unwrap_or(0);
    (
        max(|(l, _)| l.chars().count()),
        max(|(_, h)| h.chars().count()),
    )
}

/// A click while the menu is open: the entry under it, submenu first.
pub fn menu_at(screen: Rect, app: &App, col: u16, row: u16) -> Option<MenuHit> {
    let at = ratatui::layout::Position::new(col, row);
    let menu = app.menu.as_ref()?;
    if let (Some(r), Some(items)) = (submenu_rect(screen, app), menu.sub_items()) {
        let inner = Block::bordered().inner(r);
        if inner.contains(at) {
            let i = (row - inner.y) as usize;
            return (i < items.len()).then_some(MenuHit::Sub(i));
        }
    }
    let inner = Block::bordered().inner(menu_rect(screen, app)?);
    if !inner.contains(at) {
        return None;
    }
    let i = (row - inner.y) as usize;
    (i < menu.items.len()).then_some(MenuHit::Top(i))
}

fn menu_line<'a>(
    label: &str,
    hint: &str,
    (label_w, hint_w): (usize, usize),
    selected: bool,
    sub: bool,
) -> ListItem<'a> {
    let (l, h) = if selected {
        (theme::selected().bold(), theme::selected())
    } else {
        (theme::text().bold(), theme::dim())
    };
    ListItem::new(Line::from(vec![
        Span::styled(format!(" {label:<label_w$}  "), l),
        Span::styled(format!("{hint:<hint_w$} "), h),
        Span::styled(if sub { "▸ " } else { "  " }, l),
    ]))
}

fn draw_menu(f: &mut Frame, app: &App) {
    let Some(menu) = &app.menu else { return };
    let Some(area) = menu_rect(f.area(), app) else {
        return;
    };
    let w = widths(menu.items.iter().map(|e| (e.label(), e.hint())));
    let rule = "─".repeat(area.width.saturating_sub(2) as usize);
    let items: Vec<ListItem> = menu
        .items
        .iter()
        .enumerate()
        .map(|(i, e)| match e {
            Entry::Separator => {
                ListItem::new(Line::styled(rule.clone(), Style::new().fg(theme::MUTED)))
            }
            _ => menu_line(
                e.label(),
                e.hint(),
                w,
                i == menu.selected,
                matches!(e, Entry::Sub { .. }),
            ),
        })
        .collect();
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border(menu.sub.is_none()))
        .title(Span::styled(
            format!(" {} ", menu.worktree),
            theme::text().bold(),
        ));
    f.render_widget(Clear, area);
    let mut state = ListState::default().with_selected(Some(menu.selected));
    f.render_stateful_widget(List::new(items).block(block), area, &mut state);

    let (Some(items), Some(sel), Some(area)) =
        (menu.sub_items(), menu.sub, submenu_rect(f.area(), app))
    else {
        return;
    };
    let w = widths(items.iter().map(|a| (a.label.as_str(), a.hint.as_str())));
    let rows: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, a)| menu_line(&a.label, &a.hint, w, i == sel, false))
        .collect();
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border(true));
    f.render_widget(Clear, area);
    let mut state = ListState::default().with_selected(Some(sel));
    f.render_stateful_widget(List::new(rows).block(block), area, &mut state);
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

/// The button at the top right of the worktree list: `ddev tryout worktree add`.
const NEW_BUTTON: &str = " + new ";

/// Whether a screen cell is on the "+ new" button. ratatui ends a right-aligned
/// title one cell in from the corner.
pub fn new_button_at(screen: Rect, app: &App, col: u16, row: u16) -> bool {
    let list = side(areas(screen, app.sidebar_width).sidebar, app).list;
    let end = list.right().saturating_sub(1);
    let start = end.saturating_sub(NEW_BUTTON.chars().count() as u16);
    row == list.y && col >= start && col < end
}

/// The sidebar's three stacked parts, bottom up: the Activity block (while
/// there are jobs), the agents block (while an agent runs), and the worktree
/// list in what is left. Drawing and every click hit-test use it.
pub struct Side {
    pub list: Rect,
    pub agents: Option<Rect>,
    pub activity: Option<Rect>,
}

fn sidebar_parts(sidebar: Rect, agents: usize, jobs: usize) -> Side {
    let mut rest = sidebar;
    let mut take = |rows: usize, cap: u16| {
        (rows > 0).then(|| {
            let height = (rows as u16 + 2).min(cap).max(3.min(rest.height));
            let [top, bottom] =
                Layout::vertical([Constraint::Min(0), Constraint::Length(height)]).areas(rest);
            rest = top;
            bottom
        })
    };
    let activity = take(jobs, 7);
    let agents = take(agents, sidebar.height * 2 / 5);
    Side {
        list: rest,
        agents,
        activity,
    }
}

pub fn side(sidebar: Rect, app: &App) -> Side {
    sidebar_parts(sidebar, app.agents().len(), app.jobs.list().len())
}

fn draw_agents(f: &mut Frame, rows: &[crate::tui::app::AgentRow], area: Rect) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border(false))
        .title(Span::styled(" Agents ", theme::text().bold()));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let width = inner.width as usize;
    let lines: Vec<Line> = rows
        .iter()
        .take(inner.height as usize)
        .map(|r| {
            let (glyph, style) = match r.status {
                Status::Working => ("◐", Style::new().fg(theme::DIRTY)),
                Status::Waiting => ("●", Style::new().fg(theme::ERROR).bold()),
                Status::Idle => ("○", Style::new().fg(theme::MUTED)),
            };
            let text = truncate(
                &format!("{} · {}", r.worktree, r.title),
                width.saturating_sub(3),
            );
            let text_style = if r.status == Status::Waiting {
                theme::text().bold()
            } else {
                theme::text()
            };
            Line::from(vec![
                Span::styled(format!(" {glyph} "), style),
                Span::styled(text, text_style),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

/// The agent row at a screen cell, if any: its index in `app.agents()`.
pub fn agent_at(screen: Rect, app: &App, col: u16, row: u16) -> Option<usize> {
    let n = app.agents().len();
    let inner =
        Block::bordered().inner(side(areas(screen, app.sidebar_width).sidebar, app).agents?);
    if !inner.contains(ratatui::layout::Position::new(col, row)) {
        return None;
    }
    let i = (row - inner.y) as usize;
    (i < n).then_some(i)
}

/// Is the pointer on a failed Activity row's ↻, its last column? The job's id.
pub fn activity_retry_at(screen: Rect, app: &App, col: u16, row: u16) -> Option<u64> {
    let inner =
        Block::bordered().inner(side(areas(screen, app.sidebar_width).sidebar, app).activity?);
    if col + 2 != inner.x + inner.width || !inner.contains(ratatui::layout::Position::new(col, row))
    {
        return None;
    }
    app.jobs
        .list()
        .get((row - inner.y) as usize)
        .filter(|j| matches!(j.state, crate::tui::jobs::JobState::Done { ok: false, .. }))
        .map(|j| j.id)
}

/// The job whose Activity row is at a screen cell, if any.
pub fn activity_at(screen: Rect, app: &App, col: u16, row: u16) -> Option<u64> {
    let inner =
        Block::bordered().inner(side(areas(screen, app.sidebar_width).sidebar, app).activity?);
    if !inner.contains(ratatui::layout::Position::new(col, row)) {
        return None;
    }
    app.jobs.list().get((row - inner.y) as usize).map(|j| j.id)
}

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn took(d: std::time::Duration) -> String {
    let s = d.as_secs();
    if s >= 60 {
        format!("{}m{:02}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}

fn draw_activity(f: &mut Frame, app: &App, area: Rect) {
    use crate::tui::jobs::JobState;
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border(false))
        .title(Span::styled(" Activity ", theme::text().bold()));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let width = inner.width as usize;
    let lines: Vec<Line> = app
        .jobs
        .list()
        .iter()
        .take(inner.height as usize)
        .map(|j| {
            let (glyph, glyph_style, tail) = match &j.state {
                JobState::Running { .. } if j.waiting.is_some() => (
                    "?",
                    Style::new().fg(theme::ACCENT).bold(),
                    "waiting for password".into(),
                ),
                JobState::Running { step, since } => (
                    SPINNER[(since.elapsed().as_millis() / 100) as usize % SPINNER.len()],
                    Style::new().fg(theme::ACCENT),
                    step.clone().unwrap_or_else(|| "starting…".into()),
                ),
                // What it waits for: two jobs on one worktree, or one that runs alone.
                JobState::Queued => (
                    "…",
                    Style::new().fg(theme::MUTED),
                    app.jobs
                        .blocked_by(j.id)
                        .map_or("waiting".into(), |b| format!("after {}", b.label)),
                ),
                JobState::Done {
                    ok: true, took: t, ..
                } => ("✓", Style::new().fg(theme::PRIMARY), took(*t)),
                JobState::Done {
                    ok: false, code, ..
                } => (
                    "✗",
                    Style::new().fg(theme::ERROR).bold(),
                    code.map_or("failed".into(), |c| format!("exit {c}")),
                ),
            };
            // A failed row ends in a ↻ to click, its own column.
            let failed = matches!(j.state, JobState::Done { ok: false, .. });
            let room = width.saturating_sub(if failed { 3 } else { 0 });
            let label = truncate(&j.label, room.saturating_sub(4).min(24));
            let rest = truncate(&tail, room.saturating_sub(cols(&label) + 6));
            let mut spans = vec![
                Span::styled(format!(" {glyph} "), glyph_style),
                Span::styled(label.clone(), theme::text()),
                Span::styled(format!(" · {rest}"), theme::dim()),
            ];
            if failed {
                let used = 3 + cols(&label) + 3 + cols(&rest);
                spans.push(Span::raw(" ".repeat(width.saturating_sub(used + 2))));
                spans.push(Span::styled("↻ ", Style::new().fg(theme::ACCENT).bold()));
            }
            // The row whose log is open, drawn as a selection — so stepping
            // through the logs shows where you are.
            if app.log_view == Some(j.id) {
                let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
                Line::styled(format!("{text:<width$}"), theme::selected())
            } else {
                Line::from(spans)
            }
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

/// The clickable retry in a finished job's log border.
const RETRY_BUTTON: &str = " ↻ retry (r) ";

/// Where the log's text goes: the pane inside its border.
pub fn log_inner(screen: Rect, app: &App) -> Rect {
    let pane = areas(screen, app.sidebar_width).pane;
    Rect::new(
        pane.x + 1,
        pane.y + 1,
        pane.width.saturating_sub(2),
        pane.height.saturating_sub(2),
    )
}

/// A job's log run through a terminal emulator the size of `inner`.
fn log_parser(job: &crate::tui::jobs::Job, inner: Rect) -> vt100::Parser {
    let mut parser = vt100::Parser::new(inner.height.max(1), inner.width.max(1), 4000);
    for line in &job.log {
        parser.process(line.as_bytes());
        parser.process(b"\r\n");
    }
    parser
}

/// What a parsed log is valid for: the job, the size, and the log's ends (it
/// only ever grows at the back, and drops lines at the front once full).
type LogKey = (u64, u16, u16, usize, Option<String>, Option<String>);

thread_local! {
    /// The open log, parsed once rather than on every frame: a running job's
    /// spinner redraws ten times a second, and a log is up to 3000 lines.
    static LOG_CACHE: std::cell::RefCell<Option<(LogKey, vt100::Parser)>> =
        const { std::cell::RefCell::new(None) };
}

/// Run `f` on the log's screen, scrolled back `scroll` rows (clamped).
fn with_log_screen<R>(
    job: &crate::tui::jobs::Job,
    inner: Rect,
    scroll: usize,
    f: impl FnOnce(&vt100::Screen) -> R,
) -> R {
    let key: LogKey = (
        job.id,
        inner.width,
        inner.height,
        job.log.len(),
        job.log.front().cloned(),
        job.log.back().cloned(),
    );
    LOG_CACHE.with_borrow_mut(|cache| {
        if cache.as_ref().is_none_or(|(k, _)| *k != key) {
            *cache = Some((key, log_parser(job, inner)));
        }
        let parser = &mut cache.as_mut().expect("just filled").1;
        parser.screen_mut().set_scrollback(scroll);
        f(parser.screen())
    })
}

/// How far back the log can scroll at this size: its rows beyond one screen,
/// wrapped lines counted as the rows they take.
pub fn log_max_scroll(job: &crate::tui::jobs::Job, inner: Rect) -> usize {
    with_log_screen(job, inner, usize::MAX, vt100::Screen::scrollback)
}

/// Is the pointer on the retry button of the open log?
pub fn log_retry_at(screen: Rect, app: &App, col: u16, row: u16) -> bool {
    if !app.log_retryable() {
        return false;
    }
    let pane = areas(screen, app.sidebar_width).pane;
    let w = RETRY_BUTTON.chars().count() as u16;
    let right = pane.x + pane.width.saturating_sub(1);
    row == pane.y + pane.height.saturating_sub(1) && col >= right.saturating_sub(w) && col < right
}

/// A job's log in the right pane: its output through a terminal emulator, so
/// colours come through, scrolled back `app.log_scroll` lines from the end.
fn draw_log(f: &mut Frame, app: &App, job: &crate::tui::jobs::Job, area: Rect) {
    use crate::tui::jobs::JobState;
    let state = match &job.state {
        JobState::Running { .. } => Span::styled(" running ", Style::new().fg(theme::ACCENT)),
        JobState::Queued => Span::styled(" waiting ", theme::dim()),
        JobState::Done {
            ok: true, took: t, ..
        } => Span::styled(
            format!(" ✓ done in {} ", took(*t)),
            Style::new().fg(theme::PRIMARY),
        ),
        JobState::Done {
            ok: false, code, ..
        } => Span::styled(
            format!(
                " ✗ failed{} ",
                code.map_or(String::new(), |c| format!(" (exit {c})"))
            ),
            Style::new().fg(theme::ERROR).bold(),
        ),
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border(true))
        .title(Line::from(vec![
            Span::styled(format!(" {} ", job.command_line()), theme::text().bold()),
            Span::styled(
                app.log_position()
                    .map(|(i, n)| format!("{i}/{n} "))
                    .unwrap_or_default(),
                theme::dim(),
            ),
        ]))
        .title_bottom(Line::from(vec![
            state,
            Span::styled(
                " esc closes · ↑↓ other logs · k j wheel PgUp PgDn scroll ",
                theme::dim(),
            ),
        ]));
    let block = if matches!(job.state, JobState::Done { .. }) {
        block.title_bottom(
            Line::from(Span::styled(
                RETRY_BUTTON,
                Style::new().fg(theme::ACCENT).bold(),
            ))
            .right_aligned(),
        )
    } else {
        block
    };
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut cursor = tui_term::widget::Cursor::default();
    cursor.hide();
    with_log_screen(job, inner, app.log_scroll, |screen| {
        f.render_widget(PseudoTerminal::new(screen).cursor(cursor), inner);
    });
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border(app.focus == Focus::List || app.resizing()))
        .title(Span::styled(" Worktrees ", theme::text().bold()))
        .title_top(
            Line::from(Span::styled(
                NEW_BUTTON,
                Style::new().fg(theme::ACCENT).bold(),
            ))
            .right_aligned(),
        );
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
            let tabs = app.workspaces.get(&w.name).map_or(0, |ws| ws.tabs.len());
            let mut badges = String::new();
            if w.dirty() {
                badges.push_str(" ±");
            }
            match tabs {
                0 => {}
                1 => badges.push_str(" ▶"),
                n => badges.push_str(&format!(" ▶{n}")),
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
            // Where it is: the branch it is on, or for a detached checkout the
            // one it came from; the patches on top; the commit — whole, or not
            // at all, since a cut hash says nothing.
            let at = w
                .branch
                .as_deref()
                .or(w.base.as_deref())
                .unwrap_or("detached");
            let place = format!("   {at}{}", patch_badge(w));
            let with_head = format!("{place} · {}", short(&w.head));
            let place = if cols(&with_head) <= width {
                with_head
            } else {
                truncate(&place, width)
            };
            let second = Line::styled(format!("{place:<width$}"), sub);
            // How it is served: its PHP and database type.
            let config = if w.served() {
                let mut c = format!("   PHP {}", w.php.as_deref().unwrap_or("?"));
                if let Some(e) = w
                    .db_engine
                    .as_deref()
                    .and_then(crate::core::db::Db::parse_recorded)
                {
                    c.push_str(&format!(" · {}", e.label()));
                }
                c
            } else {
                "   not served".to_string()
            };
            let config = truncate(&config, width);
            let third = Line::styled(format!("{config:<width$}"), sub);
            ListItem::new(vec![first, second, third])
        })
        .collect();

    let mut state = ListState::default()
        .with_selected(Some(app.selected))
        .with_offset(sidebar_offset(app.selected, inner.height));
    f.render_stateful_widget(List::new(items), inner, &mut state);
}

fn draw_pane(f: &mut Frame, app: &App, area: Rect) {
    if let Some(job) = app.log_view.and_then(|id| app.jobs.get(id)) {
        draw_log(f, app, job, area);
        return;
    }
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
    let mut title = vec![Span::styled(pane_title_name(w), theme::text().bold())];
    if let Some(url) = &w.url {
        // Underlined: it is a link, and a click opens it.
        title.push(Span::styled(
            url.clone(),
            Style::new().fg(theme::ACCENT).underlined(),
        ));
        title.push(Span::styled(" ", theme::text()));
    }
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border(focused || app.resizing()))
        .title(Line::from(title));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let (bar, body) = split_tab_bar(inner);
    let ws = app.selected_workspace();
    draw_tab_bar(f, ws, bar, focused);

    match ws.and_then(Workspace::active_tab) {
        Some(tab) => tab.pane.with_screen(|screen| {
            let mut term = PseudoTerminal::new(screen);
            // Show the cursor only where keys actually go.
            if !focused {
                let mut cursor = tui_term::widget::Cursor::default();
                cursor.hide();
                term = term.cursor(cursor);
            }
            f.render_widget(term, body);
        }),
        None => f.render_widget(placeholder(w, body.width), body),
    }
}

/// The first part of the pane box's title; the URL follows it.
fn pane_title_name(w: &crate::tui::worktrees::Worktree) -> String {
    format!(" {} ", w.name)
}

/// Whether a screen cell is on the selected worktree's URL in the pane title.
/// Built from the same pieces the title is drawn from, which ratatui starts one
/// cell in from the corner.
pub fn url_at(screen: Rect, app: &App, col: u16, row: u16) -> bool {
    let Some(w) = app.selected() else {
        return false;
    };
    let Some(url) = &w.url else {
        return false;
    };
    let pane = areas(screen, app.sidebar_width).pane;
    let start = pane.x + 1 + pane_title_name(w).chars().count() as u16;
    let end = (start + url.chars().count() as u16).min(pane.right().saturating_sub(1));
    row == pane.y && col >= start && col < end
}

/// The pane box's first inner row is the tab bar; the rest is the terminal.
/// Reserved even with no tab open, so a shell's size never jumps by a row.
fn split_tab_bar(inner: Rect) -> (Rect, Rect) {
    let [bar, body] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);
    (bar, body)
}

/// What a click on the tab bar means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabHit {
    Tab(u64),
    New,
}

/// One entry in the tab bar: where it is drawn and what it is.
pub struct TabSlot {
    pub x: u16,
    pub width: u16,
    pub label: String,
    pub hit: TabHit,
    pub active: bool,
}

/// Longest title a tab shows; an agent's task title can be a whole sentence.
const TAB_TITLE_MAX: usize = 20;

/// Lay the tab bar out. Drawing and click hit-testing both use it, so a click
/// always lands on the tab drawn there. `+` is always kept on screen; tabs that
/// do not fit before it are left out.
pub fn tab_bar_layout(ws: Option<&Workspace>, bar: Rect) -> Vec<TabSlot> {
    let plus = " + ";
    let room = bar.width.saturating_sub(plus.len() as u16);
    let mut slots = Vec::new();
    let mut x = bar.x;
    for (i, tab) in ws
        .map(|w| w.tabs.as_slice())
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        let title = tab.label();
        let label = format!(" {} {} ", i + 1, truncate(&title, TAB_TITLE_MAX));
        let width = cols(&label) as u16;
        if x + width > bar.x + room {
            break;
        }
        slots.push(TabSlot {
            x,
            width,
            label,
            hit: TabHit::Tab(tab.id),
            active: ws.is_some_and(|w| w.active == i),
        });
        x += width + 1;
    }
    slots.push(TabSlot {
        x,
        width: plus.len() as u16,
        label: plus.into(),
        hit: TabHit::New,
        active: false,
    });
    slots
}

fn draw_tab_bar(f: &mut Frame, ws: Option<&Workspace>, bar: Rect, focused: bool) {
    let mut spans = Vec::new();
    let mut at = bar.x;
    for slot in tab_bar_layout(ws, bar) {
        if slot.x > at {
            spans.push(Span::styled(
                " ".repeat((slot.x - at) as usize),
                theme::dim(),
            ));
        }
        let style = match (slot.hit, slot.active) {
            (TabHit::New, _) => Style::new().fg(theme::ACCENT).bold(),
            (_, true) if focused => theme::selected().bold(),
            (_, true) => Style::new().fg(Color::Black).bg(theme::DIM).bold(),
            (_, false) => theme::dim(),
        };
        at = slot.x + slot.width;
        spans.push(Span::styled(slot.label, style));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), bar);
}

/// The tab bar entry at a screen cell, if any.
pub fn tab_at(screen: Rect, app: &App, col: u16, row: u16) -> Option<TabHit> {
    let (bar, _) = split_tab_bar(Block::bordered().inner(areas(screen, app.sidebar_width).pane));
    if row != bar.y {
        return None;
    }
    tab_bar_layout(app.selected_workspace(), bar)
        .into_iter()
        .find(|s| col >= s.x && col < s.x + s.width)
        .map(|s| s.hit)
}

/// Whether a screen cell is inside the terminal area below the tab bar.
pub fn in_pane_body(screen: Rect, app: &App, col: u16, row: u16) -> bool {
    areas(screen, app.sidebar_width)
        .pane_inner
        .contains(ratatui::layout::Position::new(col, row))
}

/// The patches on top, as the list shows them: the Gerrit changes tryout
/// applied by number, any others as "+N".
fn patch_badge(w: &crate::tui::worktrees::Worktree) -> String {
    let named: String = w.changes.iter().map(|n| format!(" #{n}")).collect();
    match (w.patches as usize).saturating_sub(w.changes.len()) {
        0 => named,
        n => format!("{named} +{n}"),
    }
}

fn placeholder<'a>(w: &'a crate::tui::worktrees::Worktree, width: u16) -> Paragraph<'a> {
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
        let named: Vec<String> = w.changes.iter().map(|n| format!("#{n}")).collect();
        let by_hand = (w.patches as usize).saturating_sub(w.changes.len());
        let which = match (named.is_empty(), by_hand) {
            (true, _) => String::new(),
            (false, 0) => format!(": {}", named.join(" ")),
            (false, n) => format!(": {} and {n} more", named.join(" ")),
        };
        lines.push(row(
            "patches",
            truncate(
                &format!(
                    "{} {noun} on top of {}{which}",
                    w.patches,
                    w.base.as_deref().unwrap_or("its base")
                ),
                room,
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
                    "PHP {} · {}{}",
                    php.as_deref().unwrap_or("-"),
                    w.db.as_deref().unwrap_or("-"),
                    w.db_engine
                        .as_deref()
                        .and_then(crate::core::db::Db::parse_recorded)
                        .map(|e| format!(" ({})", e.label()))
                        .unwrap_or_default()
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
    let hints: &[(&str, &str)] = match (app.menu.is_some(), app.focus) {
        _ if app.password.is_some() => &[("⏎", "send"), ("esc", "cancel the command")],
        _ if app.rename.is_some() => &[("⏎", "keep"), ("esc", "cancel")],
        _ if app
            .form
            .as_ref()
            .is_some_and(|f| f.kind.wants_patches().is_some() && f.in_change_list()) =>
        {
            &[
                ("↑↓", "choose"),
                ("space", "tick"),
                ("←→", "pages"),
                ("tab", "search"),
                ("⏎", "apply"),
                ("esc", "cancel"),
            ]
        }
        _ if app
            .form
            .as_ref()
            .is_some_and(|f| f.kind.wants_patches().is_some()) =>
        {
            &[
                ("type", "search Gerrit"),
                ("↓ tab", "to the list"),
                ("PgUp PgDn", "pages"),
                ("⏎", "apply"),
                ("esc", "cancel"),
            ]
        }
        _ if app.form.as_ref().is_some_and(|f| !f.is_confirmation()) => &[
            ("tab", "next field"),
            ("↑↓", "choose"),
            ("space", "tick"),
            ("⏎", "go"),
            ("esc", "cancel"),
        ],
        _ if app.form.is_some() => &[("y", "go ahead"), ("", "any other key cancels")],
        _ if app.log_retryable() => &[
            ("esc", "close the log"),
            ("↑↓ wheel", "scroll"),
            ("r", "retry"),
        ],
        _ if app.log_view.is_some() => &[
            ("esc", "close the log"),
            ("↑↓", "other logs"),
            ("k j wheel", "scroll"),
        ],
        _ if app.focus == Focus::List && app.jobs.last_failed().is_some() => &[
            ("↑↓", "select"),
            ("⏎", "shell"),
            ("a", "actions"),
            ("R", "retry the failed command"),
            ("L", "its log"),
        ],
        (true, _) => &[("↑↓", "select"), ("⏎", "run"), ("esc", "cancel")],
        (false, Focus::List) => &[
            ("↑↓", "select"),
            ("⏎", "shell"),
            ("t", "new tab"),
            ("n", "next agent"),
            ("a", "actions"),
            ("q", "detach"),
        ],
        (false, Focus::Pane) => &[
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

/// Columns `s` takes on screen: an emoji or a CJK character takes two, which
/// counting chars would miss — and clicks would land beside what is drawn.
fn cols(s: &str) -> usize {
    Span::raw(s).width()
}

/// At most `max` columns, an ellipsis marking what was cut.
fn truncate(s: &str, max: usize) -> String {
    if cols(s) <= max {
        return s.to_string();
    }
    let room = max.saturating_sub(1);
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = cols(c.encode_utf8(&mut [0; 4]));
        if used + w > room {
            break;
        }
        used += w;
        out.push(c);
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::tests::fixture;
    use ratatui::{Terminal, backend::TestBackend};
    use std::path::PathBuf;

    #[test]
    fn truncation_counts_screen_columns_not_characters() {
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abc", 4), "abc");
        // Each of these takes two columns.
        assert_eq!(truncate("日本語", 4), "日…");
        assert_eq!(cols(&truncate("🙂🙂🙂🙂", 5)), 5);
    }

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
    fn a_finished_logs_retry_button_is_where_it_is_drawn() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = loaded();
        a.root = dir.path().to_path_buf();
        a.jobs = crate::tui::jobs::Jobs::new(crate::tui::jobs::tests::fake(dir.path()));
        let action = crate::tui::actions::Action {
            label: "Serve".into(),
            hint: String::new(),
            args: vec!["fail".into()],
            run: crate::tui::actions::Run::Background,
        };
        let id = a.jobs.enqueue(&action, false);
        let t = std::time::Instant::now();
        while !a
            .jobs
            .get(id)
            .is_some_and(|j| matches!(j.state, crate::tui::jobs::JobState::Done { .. }))
        {
            a.tick();
            assert!(t.elapsed().as_secs() < 10);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        a.open_log(id);
        a.screen = Rect::new(0, 0, 100, 24);
        let backend = render(&a, 100, 24);
        let bottom: String = (0..100)
            .map(|x| backend.backend().buffer()[(x, 22)].symbol().to_string())
            .collect();
        let at = bottom
            .find("↻")
            .map(|b| bottom[..b].chars().count() as u16)
            .expect("the button is drawn");
        assert!(log_retry_at(a.screen, &a, at, 22), "{bottom}");
        assert!(!log_retry_at(a.screen, &a, at, 21));
        assert!(!log_retry_at(a.screen, &a, 40, 22));
    }

    #[test]
    fn a_served_worktree_shows_its_php_and_its_gerrit_changes() {
        let mut a = loaded();
        a.selected = 1;
        let t = render(&a, 120, 24);
        let text: String = t
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("13.4 #91234 +1 · 32d1f51"), "where it is");
        assert!(text.contains("PHP 8.4 · MariaDB"), "how it is served");
        assert!(
            text.contains("2 patches on top of 13.4: #91234 and 1 more"),
            "the details"
        );
    }

    #[test]
    fn the_patch_form_shows_votes_owner_and_where_in_the_pages() {
        let mut a = loaded();
        a.open_form(crate::tui::forms::FormKind::Patch("v13".into()));
        a.tick();
        let c = |n, subject: &str, owner: &str, scores: &str| crate::tui::forms::Change {
            number: n,
            subject: subject.into(),
            owner: owner.into(),
            scores: scores.into(),
        };
        a.set_patches(
            "",
            0,
            Ok((
                vec![
                    c(
                        91234,
                        "[BUGFIX] Keep the page tree open after a move",
                        "Ada Lovelace",
                        "CR+2 V+1",
                    ),
                    c(
                        91000,
                        "WIP [FEATURE] A new cache backend",
                        "Frédéric",
                        "CR-1 V-1",
                    ),
                    c(90001, "[TASK] Raise phpstan", "?", ""),
                ],
                true,
            )),
        );
        insta::assert_snapshot!(render(&a, 120, 30).backend());
    }

    /// The patch form on page 2 of a search, with a whole page of 25 changes.
    fn full_page(screen_h: u16) -> (App, Rect) {
        let mut a = loaded();
        a.open_form(crate::tui::forms::FormKind::Patch("v13".into()));
        a.tick();
        a.set_patches("", 0, Ok((Vec::new(), true)));
        if let Some(f) = a.form.as_mut() {
            f.turn_page(1);
        }
        a.tick();
        let page: Vec<_> = (0..25)
            .map(|i| crate::tui::forms::Change {
                number: 91000 + i,
                subject: format!("[TASK] Change number {i}"),
                owner: "Ada".into(),
                scores: if i % 2 == 0 {
                    "CR+1".into()
                } else {
                    String::new()
                },
            })
            .collect();
        a.set_patches("", 1, Ok((page, true)));
        (a, Rect::new(0, 0, 120, screen_h))
    }

    #[test]
    fn a_whole_page_of_changes_fits_and_the_page_bar_is_clickable() {
        let (a, screen) = full_page(40);
        let t = render(&a, screen.width, screen.height);
        let text: String = t
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        for i in 0..25 {
            assert!(
                text.contains(&format!("{}", 91000 + i)),
                "change {i} is on screen"
            );
        }
        assert!(text.contains("page 2 · 26–50"));
        insta::assert_snapshot!(t.backend());

        // Find the page bar and a change row on screen, then click them.
        let row_of = |needle: &str| {
            (0..screen.height)
                .find(|&y| {
                    let line: String = (0..screen.width)
                        .map(|x| t.backend().buffer()[(x, y)].symbol().to_string())
                        .collect();
                    line.contains(needle)
                })
                .unwrap()
        };
        let col_of = |y: u16, needle: &str| {
            let line: String = (0..screen.width)
                .map(|x| t.backend().buffer()[(x, y)].symbol().to_string())
                .collect();
            line[..line.find(needle).unwrap()].chars().count() as u16
        };
        let bar = row_of("‹ previous");
        assert_eq!(
            form_hit(screen, &a, col_of(bar, "‹ previous") + 2, bar),
            Some(FormHit::PrevPage)
        );
        assert_eq!(
            form_hit(screen, &a, col_of(bar, "next ›") + 1, bar),
            Some(FormHit::NextPage)
        );
        assert_eq!(form_hit(screen, &a, col_of(bar, "page 2"), bar), None);
        let fifth = row_of("91004");
        assert_eq!(form_hit(screen, &a, 20, fifth), Some(FormHit::Change(4)));
    }

    #[test]
    fn a_short_terminal_scrolls_the_page_instead_of_cutting_it() {
        let (mut a, screen) = full_page(20);
        let f = a.form.as_mut().unwrap();
        f.move_selection(24);
        let t = render(&a, screen.width, screen.height);
        let text: String = t
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("91024"), "the selection is kept in view");
        assert!(text.contains("next ›"), "the page bar stays");
    }

    #[test]
    fn a_password_popup_masks_what_is_typed() {
        let mut a = loaded();
        a.password = Some(crate::tui::app::PasswordPrompt {
            job: 1,
            prompt: "Password:".into(),
            label: "Serve v13".into(),
            value: "hunter2".into(),
        });
        let text = format!("{}", render(&a, 90, 24).backend());
        assert!(text.contains("•••••••"), "{text}");
        assert!(!text.contains("hunter2"));
        insta::assert_snapshot!(render(&a, 90, 24).backend());
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
    fn a_click_lands_on_the_worktree_drawn_there() {
        let a = loaded();
        let screen = Rect::new(0, 0, 80, 24);
        // Header on row 0, sidebar border on row 1; each worktree is three rows.
        assert_eq!(worktree_at(screen, &a, 5, 2), Some(0));
        assert_eq!(
            worktree_at(screen, &a, 5, 4),
            Some(0),
            "its third line counts too"
        );
        assert_eq!(worktree_at(screen, &a, 5, 5), Some(1));
        assert_eq!(worktree_at(screen, &a, 5, 8), Some(2));
        assert_eq!(
            worktree_at(screen, &a, 5, 11),
            None,
            "below the last worktree"
        );
        assert_eq!(worktree_at(screen, &a, 0, 2), None, "on the border");
        assert_eq!(worktree_at(screen, &a, 50, 2), None, "in the pane");
    }

    #[test]
    fn a_click_on_a_scrolled_list_follows_the_scroll() {
        let mut a = App::new(PathBuf::from("/p/demo"));
        let mut many = fixture();
        for i in 0..20 {
            let mut w = fixture()[2].clone();
            w.name = format!("wt{i:02}");
            many.push(w);
        }
        a.set_worktrees(Ok(many));
        a.selected = 22; // the last one; a 24-row screen shows ten
        let screen = Rect::new(0, 0, 80, 24);
        let t = render(&a, 80, 24);
        let drawn = t.backend().buffer().content[(2 * 80 + 3)..(2 * 80 + 12)]
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        let hit = worktree_at(screen, &a, 5, 2).unwrap();
        assert!(
            drawn.contains(&a.worktrees[hit].name),
            "drew {drawn:?}, hit {hit}"
        );
        // The selected last one is on screen, and a click on it finds it.
        assert!((2..23).any(|row| worktree_at(screen, &a, 5, row) == Some(22)));
    }

    #[test]
    fn the_pane_leaves_room_for_its_border_and_the_tab_bar() {
        let a = areas(Rect::new(0, 0, 80, 24), crate::tui::app::SIDEBAR_DEFAULT);
        assert_eq!(a.pane_inner.width, a.pane.width - 2);
        assert_eq!(a.pane_inner.height, a.pane.height - 3);
    }

    fn quiet() -> crate::tui::pane::Pane {
        let mut cmd = portable_pty::CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "cat"]);
        crate::tui::pane::Pane::spawn(cmd, 5, 30).unwrap()
    }

    #[test]
    fn a_click_lands_on_the_tab_drawn_there() {
        let mut a = loaded();
        a.add_tab("main", quiet());
        a.add_tab("main", quiet());
        let ids: Vec<u64> = a
            .selected_workspace()
            .unwrap()
            .tabs
            .iter()
            .map(|t| t.id)
            .collect();
        let screen = Rect::new(0, 0, 80, 24);
        let bar_y = areas(screen, a.sidebar_width).pane.y + 1;
        let slots = tab_bar_layout(
            a.selected_workspace(),
            split_tab_bar(Block::bordered().inner(areas(screen, a.sidebar_width).pane)).0,
        );
        assert_eq!(slots.len(), 3, "two tabs and the +");
        // First and last cell of each slot, and nothing in the gaps.
        for s in &slots {
            assert_eq!(tab_at(screen, &a, s.x, bar_y), Some(s.hit));
            assert_eq!(tab_at(screen, &a, s.x + s.width - 1, bar_y), Some(s.hit));
        }
        assert_eq!(slots[0].hit, TabHit::Tab(ids[0]));
        assert_eq!(slots[1].hit, TabHit::Tab(ids[1]));
        assert_eq!(slots[2].hit, TabHit::New);
        assert_eq!(
            tab_at(screen, &a, slots[2].x + slots[2].width + 2, bar_y),
            None
        );
        assert_eq!(
            tab_at(screen, &a, slots[0].x, bar_y + 1),
            None,
            "below the bar"
        );
    }

    #[test]
    fn plus_stays_on_screen_however_many_tabs() {
        let mut a = loaded();
        for _ in 0..12 {
            a.add_tab("main", quiet());
        }
        let bar = Rect::new(0, 0, 40, 1);
        let slots = tab_bar_layout(a.selected_workspace(), bar);
        let plus = slots.last().unwrap();
        assert_eq!(plus.hit, TabHit::New);
        assert!(plus.x + plus.width <= 40);
    }

    /// Two worktrees with an agent tab each, one of them rung.
    fn with_agents() -> App {
        let mut a = loaded();
        a.add_tab("v13", quiet());
        a.add_tab("bugfix", quiet());
        let ids: Vec<(String, u64)> = a
            .workspaces
            .iter()
            .map(|(n, ws)| (n.clone(), ws.tabs[0].id))
            .collect();
        a.set_agent_kinds(ids.iter().map(|(_, id)| (*id, "claude")).collect());
        a.selected = 0; // looking at main, so neither agent is on screen
        a.focus = crate::tui::app::Focus::List;
        a
    }

    #[test]
    fn the_agents_pane_appears_under_the_worktrees_when_an_agent_runs() {
        let screen = Rect::new(0, 0, 80, 24);
        let a = loaded();
        assert_eq!(agent_at(screen, &a, 5, 20), None, "no agents, no pane");
        let a = with_agents();
        let t = render(&a, 80, 24);
        let text: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("Agents"), "no agents block drawn");
        // The two rows are the last two inner rows above the bottom border.
        let bottom_inner = areas(screen, a.sidebar_width).sidebar.bottom() - 2;
        assert_eq!(agent_at(screen, &a, 5, bottom_inner), Some(1));
        assert_eq!(agent_at(screen, &a, 5, bottom_inner - 1), Some(0));
        assert_eq!(
            agent_at(screen, &a, 5, bottom_inner - 2),
            None,
            "the block's border"
        );
        // The worktree list above still maps its own rows.
        assert_eq!(worktree_at(screen, &a, 5, 2), Some(0));
    }

    #[test]
    fn a_tab_you_named_shows_your_name_and_the_prompt_draws() {
        let mut a = loaded();
        a.add_tab("main", quiet());
        a.add_tab("main", quiet());
        a.workspaces.get_mut("main").unwrap().tabs[0].name = Some("tests".into());
        a.focus = crate::tui::app::Focus::List;
        a.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(','),
            crossterm::event::KeyModifiers::NONE,
        ));
        insta::assert_snapshot!(render(&a, 80, 12).backend());
    }

    #[test]
    fn a_click_lands_on_the_url_in_the_pane_title_and_nowhere_else() {
        let a = loaded(); // main, https://demo.ddev.site
        let screen = Rect::new(0, 0, 80, 24);
        let top = areas(screen, a.sidebar_width).pane.y;
        let t = render(&a, 80, 24);
        let row: String = (0..80)
            .map(|x| t.backend().buffer()[(x, top)].symbol().to_string())
            .collect();
        let start = row.find("https://").unwrap();
        let start = row[..start].chars().count() as u16;
        let len = "https://demo.ddev.site".len() as u16;
        assert!(url_at(screen, &a, start, top));
        assert!(url_at(screen, &a, start + len - 1, top));
        assert!(
            !url_at(screen, &a, start - 2, top),
            "the name is not the link"
        );
        assert!(!url_at(screen, &a, start + len + 1, top), "past its end");
        assert!(!url_at(screen, &a, start, top + 1), "the row below");
        let mut unserved = loaded();
        unserved.selected = 2;
        assert!(!url_at(screen, &unserved, start, top), "no URL, no link");
    }

    #[test]
    fn a_click_lands_on_the_new_button_drawn_there() {
        let a = loaded();
        let screen = Rect::new(0, 0, 80, 24);
        let t = render(&a, 80, 24);
        let top = areas(screen, a.sidebar_width).sidebar.y;
        let row: String = (0..40)
            .map(|x| t.backend().buffer()[(x, top)].symbol().to_string())
            .collect();
        let at = row[..row.find("+ new").expect("no button drawn")]
            .chars()
            .count() as u16;
        assert!(new_button_at(screen, &a, at, top), "the + itself");
        assert!(new_button_at(screen, &a, at + 4, top), "the w of new");
        assert!(!new_button_at(screen, &a, 3, top), "the title");
        assert!(!new_button_at(screen, &a, at, top + 1), "the row below");
    }

    #[test]
    fn a_right_click_menu_opens_at_the_pointer_and_stays_on_screen() {
        let screen = Rect::new(0, 0, 80, 24);
        let mut a = loaded();
        a.context_menu(1, (5, 4));
        let r = menu_rect(screen, &a).unwrap();
        // At the pointer — or as near it as a menu this tall still fits.
        assert_eq!(r.x, 5);
        assert!(r.y <= 4 && r.bottom() <= 24, "{r:?}");
        assert_eq!(
            menu_at(screen, &a, 7, r.y + 1),
            Some(MenuHit::Top(0)),
            "first item"
        );
        assert_eq!(menu_at(screen, &a, 7, r.y), None, "its border");
        // Near the bottom-right corner it is pulled back inside the screen.
        a.context_menu(1, (78, 23));
        let r = menu_rect(screen, &a).unwrap();
        assert!(r.right() <= 80 && r.bottom() <= 24);
        insta::assert_snapshot!(render(&a, 80, 24).backend());
    }

    #[test]
    fn a_click_on_a_php_entry_opens_its_versions_beside_it() {
        let screen = Rect::new(0, 0, 100, 30);
        let mut a = loaded();
        a.worktrees[1].php_versions = vec!["8.2".into(), "8.3".into(), "8.4".into()];
        a.context_menu(1, (5, 4)); // v13: PHP 8.4 ▸ first
        let top = menu_rect(screen, &a).unwrap();
        assert_eq!(menu_at(screen, &a, 7, top.y + 1), Some(MenuHit::Top(0)));
        assert_eq!(
            a.click_menu(Some(MenuHit::Top(0))),
            crate::tui::app::Effect::None
        );
        let sub = submenu_rect(screen, &a).expect("the versions open");
        assert_eq!(sub.x, top.right(), "beside the menu, not over it");
        assert_eq!(
            menu_at(screen, &a, sub.x + 2, sub.y + 1),
            Some(MenuHit::Sub(0))
        );
        assert_eq!(
            menu_at(screen, &a, sub.x + 2, sub.y + 3),
            Some(MenuHit::Sub(2))
        );
        insta::assert_snapshot!(render(&a, 100, 30).backend());
    }

    #[test]
    fn a_queued_row_says_what_it_waits_for() {
        use crate::tui::actions::{Action, Run};
        let dir = tempfile::tempdir().unwrap();
        let mut a = loaded();
        a.root = dir.path().to_path_buf();
        a.jobs = crate::tui::jobs::Jobs::new(crate::tui::jobs::tests::fake(dir.path()));
        let act = |label: &str, args: &str| Action {
            label: label.into(),
            hint: String::new(),
            args: args.split_whitespace().map(String::from).collect(),
            run: Run::Job { reveal: false },
        };
        a.jobs.enqueue(&act("Reset main", "reset main"), false);
        a.jobs
            .enqueue(&act("Serve v13", "worktree serve v13"), false);
        a.jobs.tick(dir.path());
        let t = render(&a, 100, 30);
        let text: String = t
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("Serve v13 · after Reset main"), "{text}");
    }

    #[test]
    fn the_activity_block_lists_jobs_and_a_click_opens_one() {
        use crate::tui::actions::{Action, Run};
        let dir = tempfile::tempdir().unwrap();
        let mut a = loaded();
        a.root = dir.path().to_path_buf();
        a.jobs = crate::tui::jobs::Jobs::new(crate::tui::jobs::tests::fake(dir.path()));
        let act = |args: &str| Action {
            label: args.into(),
            hint: String::new(),
            args: args.split_whitespace().map(String::from).collect(),
            run: Run::Job { reveal: false },
        };
        a.jobs.enqueue(&act("fail"), false);
        a.jobs.enqueue(&act("worktree use v13"), false);
        let t = std::time::Instant::now();
        while a
            .jobs
            .list()
            .iter()
            .any(|j| !matches!(j.state, crate::tui::jobs::JobState::Done { .. }))
        {
            assert!(t.elapsed().as_secs() < 10);
            a.tick();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let screen = Rect::new(0, 0, 80, 24);
        let area = side(areas(screen, a.sidebar_width).sidebar, &a)
            .activity
            .expect("no Activity block");
        // Newest first: the second job on the first row.
        let first = activity_at(screen, &a, area.x + 2, area.y + 1).unwrap();
        assert_eq!(a.jobs.get(first).unwrap().label, "worktree use v13");
        assert!(
            activity_at(screen, &a, area.x + 2, area.y).is_none(),
            "its border"
        );
        assert_eq!(
            worktree_at(screen, &a, 5, 2),
            Some(0),
            "the list above still maps"
        );
        // The failed row's ↻ retries it; the rest of the row opens its log.
        let retry_col = area.x + area.width - 3;
        let failed_row = (1..area.height - 1)
            .map(|r| area.y + r)
            .find(|&y| activity_retry_at(screen, &a, retry_col, y).is_some())
            .expect("a failed row with its ↻");
        let failed = activity_retry_at(screen, &a, retry_col, failed_row).unwrap();
        assert_eq!(a.jobs.get(failed).unwrap().label, "fail");
        assert_eq!(
            activity_retry_at(screen, &a, retry_col - 1, failed_row),
            None
        );
        assert_eq!(
            activity_retry_at(screen, &a, retry_col, area.y + 1),
            None,
            "a success has none"
        );
        insta::assert_snapshot!(render(&a, 80, 24).backend());
        // And the log it opens.
        a.open_log(first);
        let t = render(&a, 80, 24);
        let text: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(
            text.contains("plain output of worktree"),
            "the log is not shown"
        );
        assert!(text.contains("done in"));
    }

    #[test]
    fn the_new_worktree_form_and_a_confirmation_draw() {
        let mut a = loaded();
        a.open_form(crate::tui::forms::FormKind::NewWorktree);
        a.set_branches(Ok(vec!["main".into(), "13.4".into(), "12.4".into()]));
        for c in "fix-9".chars() {
            a.handle_key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(c),
                crossterm::event::KeyModifiers::NONE,
            ));
        }
        insta::assert_snapshot!("new_worktree_form", render(&a, 80, 30).backend());
        a.form = None;
        a.open_form(crate::tui::forms::FormKind::Remove("v13".into()));
        insta::assert_snapshot!("remove_confirmation", render(&a, 80, 24).backend());
    }

    #[test]
    fn the_sidebar_takes_the_width_it_is_given_within_bounds() {
        let screen = Rect::new(0, 0, 120, 30);
        assert_eq!(areas(screen, 50).sidebar.width, 50);
        assert_eq!(
            areas(screen, 5).sidebar.width,
            SIDEBAR_MIN,
            "never too narrow to read"
        );
        assert_eq!(
            areas(screen, 200).sidebar.width,
            120 - PANE_MIN,
            "the shells keep room"
        );
        // A narrow terminal pulls it in; the pane still gets the rest.
        let a = areas(Rect::new(0, 0, 70, 24), 50);
        assert_eq!(a.sidebar.width, 70 - PANE_MIN);
        assert_eq!(a.pane.x, a.sidebar.right());
    }

    #[test]
    fn the_divider_is_the_two_border_columns_between_list_and_pane() {
        let mut a = loaded();
        a.sidebar_width = 40;
        let screen = Rect::new(0, 0, 120, 30);
        assert!(divider_at(screen, &a, 39, 10), "the sidebar's right border");
        assert!(divider_at(screen, &a, 40, 10), "the pane's left border");
        assert!(!divider_at(screen, &a, 38, 10));
        assert!(!divider_at(screen, &a, 41, 10));
        assert!(!divider_at(screen, &a, 39, 0), "the header row");
        assert!(!divider_at(screen, &a, 39, 29), "the footer row");
        // Everything else follows the new width.
        assert_eq!(worktree_at(screen, &a, 5, 2), Some(0));
        assert_eq!(areas(screen, a.sidebar_width).pane.x, 40);
        insta::assert_snapshot!(render(&a, 120, 20).backend());
    }

    #[test]
    fn a_worktree_with_tabs_shows_its_tab_bar() {
        let mut a = loaded();
        a.add_tab("main", quiet());
        a.add_tab("main", quiet());
        insta::assert_snapshot!(render(&a, 80, 12).backend());
    }
}
