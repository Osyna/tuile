//! AI harness composer: slash commands, mentions, attachments, mode badges, status, questions, plans, queue.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 60, 15);
//! # let mut buf = Buffer::empty(area);
//! # let now = Instant::now();
//! let mut state = SlashMenuState::default();
//! SlashMenu::new().commands(&[SlashCommand::new("help", "Show help")]).render(area, &mut buf, &mut state);
//! ```

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{StatefulWidget, Widget};
use unicode_width::UnicodeWidthStr;

use crate::anim::{self, Easing};
use crate::core::*;
use crate::draw::{Border, bold, fill, hbar, put, put_centered, st, truncate, wrap};
use crate::fuzzy;
use crate::theme::{self, Rgb, Theme};
use crate::widgets::ai::fmt_tokens;
use crate::widgets::spinner::spinners;

// slash menu

/// Slash command descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub args: Option<String>,
    pub category: Option<String>,
}

impl SlashCommand {
    /// Create a slash command.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            args: None,
            category: None,
        }
    }
    /// Set args hint.
    pub fn args(mut self, a: impl Into<String>) -> Self {
        self.args = Some(a.into());
        self
    }
    /// Set category.
    pub fn category(mut self, c: impl Into<String>) -> Self {
        self.category = Some(c.into());
        self
    }
}

/// Popup query, ranked matches, hit boxes, and selection; the widget writes `ranked` and `names` each render, reads `selected` when the user picks a command.
#[derive(Clone, Debug, Default)]
pub struct SlashMenuState {
    pub open: bool,
    pub query: String,
    pub cursor: usize,
    pub hits: Vec<HitBox>,
    pub selected: Option<String>,
    /// `(command index, score, matched positions)` in display order, from the last render.
    ranked: Vec<(usize, i32, Vec<usize>)>,
    /// Command names from the last render, so selection can resolve `ranked` indices.
    names: Vec<String>,
}

impl SlashMenuState {
    /// Take selected command name.
    pub fn take_selected(&mut self) -> Option<String> {
        self.selected.take()
    }

    /// Select the ranked row at `row` (by display position) and close.
    fn select_row(&mut self, row: usize) {
        if let Some(&(idx, _, _)) = self.ranked.get(row) {
            self.selected = self.names.get(idx).cloned();
        }
        self.open = false;
    }
}

impl Interactive for SlashMenuState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) || !self.open {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Outcome::Consumed
            }
            KeyCode::Down => {
                if !self.ranked.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.ranked.len() - 1);
                }
                Outcome::Consumed
            }
            KeyCode::Tab | KeyCode::Enter => {
                self.select_row(self.cursor);
                Outcome::Changed
            }
            KeyCode::Esc => {
                self.open = false;
                Outcome::Consumed
            }
            KeyCode::Backspace => {
                if self.query.pop().is_none() {
                    // erasing past the trigger character closes the popup
                    self.open = false;
                    return Outcome::Ignored;
                }
                self.cursor = 0;
                Outcome::Consumed
            }
            KeyCode::Char(c) => {
                self.query.push(c);
                self.cursor = 0;
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        if !self.open {
            return Outcome::Ignored;
        }
        let mut out = Outcome::Ignored;
        let mut pressed = None;
        for (i, hit) in self.hits.iter_mut().enumerate() {
            match hit.mouse(&m) {
                Hit::Press => pressed = Some(i),
                Hit::HoverChanged if hit.hover => {
                    self.cursor = i;
                    out = Outcome::Consumed;
                }
                _ => {}
            }
        }
        if let Some(i) = pressed {
            self.cursor = i;
            self.select_row(i);
            return Outcome::Changed;
        }
        if let Some(delta) = wheel_delta(&m) {
            if delta > 0 {
                if !self.ranked.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.ranked.len() - 1);
                }
            } else {
                self.cursor = self.cursor.saturating_sub(1);
            }
            return Outcome::Consumed;
        }
        out
    }
}

/// Slash command popup menu.
pub struct SlashMenu<'a> {
    commands: &'a [SlashCommand],
    anchor: Rect,
    max_rows: u16,
    width: u16,
    theme: Option<Theme>,
}

impl<'a> SlashMenu<'a> {
    /// Create slash menu.
    pub fn new() -> Self {
        Self {
            commands: <&[SlashCommand]>::default(),
            anchor: Rect::default(),
            max_rows: 8,
            width: 48,
            theme: None,
        }
    }
    /// Set commands.
    pub fn commands(mut self, c: &'a [SlashCommand]) -> Self {
        self.commands = c;
        self
    }
    /// Set anchor rect (composer field).
    pub fn anchor(mut self, a: Rect) -> Self {
        self.anchor = a;
        self
    }
    /// Max visible rows.
    pub fn max_rows(mut self, n: u16) -> Self {
        self.max_rows = n;
        self
    }
    /// Popup width.
    pub fn width(mut self, w: u16) -> Self {
        self.width = w;
        self
    }
    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl<'a> Default for SlashMenu<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> StatefulWidget for SlashMenu<'a> {
    type State = SlashMenuState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if !state.open || area.width < 10 || area.height < 3 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);

        // rank
        let names: Vec<&str> = self.commands.iter().map(|c| c.name.as_str()).collect();
        state.names = names.iter().map(|n| n.to_string()).collect();
        state.ranked = if state.query.is_empty() {
            names
                .iter()
                .enumerate()
                .map(|(i, _)| (i, 0, Vec::new()))
                .collect()
        } else {
            fuzzy::rank(&state.query, names.iter().copied())
        };

        if state.ranked.is_empty() {
            // nothing matches: no popup, no stale hits
            state.hits.clear();
            return;
        }
        if state.cursor >= state.ranked.len() {
            state.cursor = state.ranked.len() - 1;
        }

        // group by category
        // A local (index, score, positions) grouping; a named type would not make it clearer.
        #[allow(clippy::type_complexity)]
        let mut sections: Vec<(Option<String>, Vec<(usize, i32, Vec<usize>)>)> = Vec::new();
        for (idx, score, positions) in &state.ranked {
            let cmd = &self.commands[*idx];
            if let Some(cat) = &cmd.category {
                if let Some((last_cat, items)) = sections.last_mut()
                    && last_cat.as_ref() == Some(cat)
                {
                    items.push((*idx, *score, positions.clone()));
                    continue;
                }
                sections.push((Some(cat.clone()), vec![(*idx, *score, positions.clone())]));
            } else {
                if let Some((None, items)) = sections.last_mut() {
                    items.push((*idx, *score, positions.clone()));
                } else {
                    sections.push((None, vec![(*idx, *score, positions.clone())]));
                }
            }
        }

        // count total rows (category headers + command rows + footer)
        let cat_count = sections.iter().filter(|(c, _)| c.is_some()).count() as u16;
        let cmd_count = state.ranked.len() as u16;
        let footer = 1;
        let total_rows = cat_count + cmd_count + footer;
        let content_h = total_rows.min(self.max_rows);
        let popup_h = content_h + 2; // border

        let popup_w = self.width.min(self.anchor.width).min(area.width);
        let popup_x = self.anchor.x.min(area.right().saturating_sub(popup_w));
        // above the anchor when there is room, else below; always inside `area`
        let popup_y = if self.anchor.y >= area.y + popup_h {
            self.anchor.y - popup_h
        } else {
            self.anchor
                .bottom()
                .min(area.bottom().saturating_sub(popup_h))
        }
        .max(area.y);

        let popup = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_w,
            height: popup_h,
        };

        if !area.intersects(popup) {
            return;
        }

        Border::Round.draw(buf, popup, th.border, th.panel);
        let inner = Border::Round.inner(popup);
        fill(buf, inner, th.panel);

        state.hits.clear();
        state.hits.resize(state.ranked.len(), HitBox::default());

        let mut y = inner.y;
        let mut cursor_pos = 0;

        for (cat, items) in &sections {
            if y >= inner.bottom() - 1 {
                break;
            }
            if let Some(cat_name) = cat {
                put(
                    buf,
                    inner.x + 1,
                    y,
                    &cat_name.to_uppercase(),
                    inner.width.saturating_sub(2),
                    st(th.text_muted, th.panel),
                );
                y += 1;
            }

            for (idx, _score, positions) in items {
                if y >= inner.bottom() - 1 {
                    break;
                }
                let cmd = &self.commands[*idx];
                let is_cursor = cursor_pos == state.cursor;
                let bg = if is_cursor { th.cursor_bg } else { th.panel };
                fill(
                    buf,
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: 1,
                    },
                    bg,
                );

                state.hits[cursor_pos].set_area(Rect {
                    x: inner.x,
                    y,
                    width: inner.width,
                    height: 1,
                });

                let mut x = inner.x + 1;
                let name_with_args = if let Some(args) = &cmd.args {
                    format!("/{} {}", cmd.name, args)
                } else {
                    format!("/{}", cmd.name)
                };

                // highlight matched chars
                if !positions.is_empty() {
                    let mut last = 0;
                    for &p in positions {
                        if p >= last && p < name_with_args.len() {
                            let before = &name_with_args[last..p];
                            x += put(
                                buf,
                                x,
                                y,
                                before,
                                inner.width.saturating_sub(2),
                                st(th.text, bg),
                            );
                            if let Some(c) = name_with_args.chars().nth(p) {
                                let char_str = c.to_string();
                                x += put(
                                    buf,
                                    x,
                                    y,
                                    &char_str,
                                    inner.width.saturating_sub(2),
                                    bold(st(th.accent, bg)),
                                );
                                last = p + c.len_utf8();
                            }
                        }
                    }
                    let rest = &name_with_args[last..];
                    x += put(
                        buf,
                        x,
                        y,
                        rest,
                        inner.width.saturating_sub(2),
                        st(th.text, bg),
                    );
                } else {
                    x += put(
                        buf,
                        x,
                        y,
                        &name_with_args,
                        inner.width.saturating_sub(2),
                        st(th.text, bg),
                    );
                }

                x += 2;
                let desc_w = inner.right().saturating_sub(x + 1);
                if desc_w > 0 {
                    let desc = truncate(&cmd.description, desc_w as usize);
                    put(buf, x, y, &desc, desc_w, st(th.text_muted, bg));
                }

                y += 1;
                cursor_pos += 1;
            }
        }

        // footer hint
        let hint = "↑↓ navigate · Tab/Enter select · Esc close";
        let hint_y = inner.bottom() - 1;
        if hint_y >= inner.y {
            put_centered(
                buf,
                Rect {
                    x: inner.x,
                    y: hint_y,
                    width: inner.width,
                    height: 1,
                },
                hint,
                st(th.text_muted, th.panel),
            );
        }
    }
}

// mention picker

/// File, directory, symbol, URL, or agent reference types for the mention picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MentionKind {
    File,
    Dir,
    Symbol,
    Url,
    Agent,
}

impl MentionKind {
    fn glyph(self) -> &'static str {
        match self {
            MentionKind::File => "¶",
            MentionKind::Dir => "▸",
            MentionKind::Symbol => "◆",
            MentionKind::Url => "⊕",
            MentionKind::Agent => "◈",
        }
    }
    fn color(self, th: &Theme) -> Rgb {
        match self {
            MentionKind::File => th.text,
            MentionKind::Dir => th.accent,
            MentionKind::Symbol => th.secondary,
            MentionKind::Url => th.primary,
            MentionKind::Agent => th.success,
        }
    }
}

/// Label, kind, optional detail line, and recent flag for one mention picker row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MentionItem {
    pub label: String,
    pub kind: MentionKind,
    pub detail: Option<String>,
    pub recent: bool,
}

impl MentionItem {
    /// Create mention item.
    pub fn new(label: impl Into<String>, kind: MentionKind) -> Self {
        Self {
            label: label.into(),
            kind,
            detail: None,
            recent: false,
        }
    }
    /// Set detail.
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }
    /// Mark as recent.
    pub fn recent(mut self, r: bool) -> Self {
        self.recent = r;
        self
    }
}

/// Popup query, ranked matches, hit boxes, and selection; the widget writes `ranked` and `names` each render, reads `selected` when the user picks an item.
#[derive(Clone, Debug, Default)]
pub struct MentionPickerState {
    pub open: bool,
    pub query: String,
    pub cursor: usize,
    pub hits: Vec<HitBox>,
    pub selected: Option<String>,
    /// `(item index, score, matched positions)` in display order, from the last render.
    ranked: Vec<(usize, i32, Vec<usize>)>,
    /// Item labels from the last render, so selection can resolve `ranked` indices.
    names: Vec<String>,
}

impl MentionPickerState {
    /// Take selected label.
    pub fn take_selected(&mut self) -> Option<String> {
        self.selected.take()
    }

    /// Select the ranked row at `row` (by display position) and close.
    fn select_row(&mut self, row: usize) {
        if let Some(&(idx, _, _)) = self.ranked.get(row) {
            self.selected = self.names.get(idx).cloned();
        }
        self.open = false;
    }
}

impl Interactive for MentionPickerState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) || !self.open {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Outcome::Consumed
            }
            KeyCode::Down => {
                if !self.ranked.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.ranked.len() - 1);
                }
                Outcome::Consumed
            }
            KeyCode::Tab | KeyCode::Enter => {
                self.select_row(self.cursor);
                Outcome::Changed
            }
            KeyCode::Esc => {
                self.open = false;
                Outcome::Consumed
            }
            KeyCode::Backspace => {
                if self.query.pop().is_none() {
                    // erasing past the trigger character closes the popup
                    self.open = false;
                    return Outcome::Ignored;
                }
                self.cursor = 0;
                Outcome::Consumed
            }
            KeyCode::Char(c) => {
                self.query.push(c);
                self.cursor = 0;
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        if !self.open {
            return Outcome::Ignored;
        }
        let mut out = Outcome::Ignored;
        let mut pressed = None;
        for (i, hit) in self.hits.iter_mut().enumerate() {
            match hit.mouse(&m) {
                Hit::Press => pressed = Some(i),
                Hit::HoverChanged if hit.hover => {
                    self.cursor = i;
                    out = Outcome::Consumed;
                }
                _ => {}
            }
        }
        if let Some(i) = pressed {
            self.cursor = i;
            self.select_row(i);
            return Outcome::Changed;
        }
        if let Some(delta) = wheel_delta(&m) {
            if delta > 0 {
                if !self.ranked.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.ranked.len() - 1);
                }
            } else {
                self.cursor = self.cursor.saturating_sub(1);
            }
            return Outcome::Consumed;
        }
        out
    }
}

/// Mention picker popup.
pub struct MentionPicker<'a> {
    items: &'a [MentionItem],
    anchor: Rect,
    max_rows: u16,
    width: u16,
    theme: Option<Theme>,
}

impl<'a> MentionPicker<'a> {
    /// Create mention picker.
    pub fn new() -> Self {
        Self {
            items: <&[MentionItem]>::default(),
            anchor: Rect::default(),
            max_rows: 8,
            width: 48,
            theme: None,
        }
    }
    /// Set items.
    pub fn items(mut self, i: &'a [MentionItem]) -> Self {
        self.items = i;
        self
    }
    /// Set anchor rect.
    pub fn anchor(mut self, a: Rect) -> Self {
        self.anchor = a;
        self
    }
    /// Max visible rows.
    pub fn max_rows(mut self, n: u16) -> Self {
        self.max_rows = n;
        self
    }
    /// Popup width.
    pub fn width(mut self, w: u16) -> Self {
        self.width = w;
        self
    }
    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl<'a> Default for MentionPicker<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> StatefulWidget for MentionPicker<'a> {
    type State = MentionPickerState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if !state.open || area.width < 10 || area.height < 3 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);

        // rank, then reorder into display order (RECENT first) so cursor == display row
        let labels: Vec<&str> = self.items.iter().map(|i| i.label.as_str()).collect();
        state.names = labels.iter().map(|l| l.to_string()).collect();
        let ranked = if state.query.is_empty() {
            labels
                .iter()
                .enumerate()
                .map(|(i, _)| (i, 0, Vec::new()))
                .collect()
        } else {
            fuzzy::rank(&state.query, labels.iter().copied())
        };
        let (recent, all): (Vec<_>, Vec<_>) = ranked
            .into_iter()
            .partition(|(idx, _, _)| self.items[*idx].recent);
        state.ranked = recent.iter().chain(all.iter()).cloned().collect();
        if state.ranked.is_empty() {
            // nothing matches: no popup, no stale hits
            state.hits.clear();
            return;
        }
        if state.cursor >= state.ranked.len() {
            state.cursor = state.ranked.len() - 1;
        }

        let has_recent = !recent.is_empty();
        // A local (index, score, positions) grouping; a named type would not make it clearer.
        #[allow(clippy::type_complexity)]
        let sections: Vec<(Option<&str>, Vec<(usize, i32, Vec<usize>)>)> = if has_recent {
            vec![(Some("RECENT"), recent), (Some("ALL"), all)]
        } else {
            vec![(None, all)]
        };

        let cat_count = sections.iter().filter(|(c, _)| c.is_some()).count() as u16;
        let cmd_count = state.ranked.len() as u16;
        let footer = 1;
        let total_rows = cat_count + cmd_count + footer;
        let content_h = total_rows.min(self.max_rows);
        let popup_h = content_h + 2;

        let popup_w = self.width.min(self.anchor.width).min(area.width);
        let popup_x = self.anchor.x.min(area.right().saturating_sub(popup_w));
        // above the anchor when there is room, else below; always inside `area`
        let popup_y = if self.anchor.y >= area.y + popup_h {
            self.anchor.y - popup_h
        } else {
            self.anchor
                .bottom()
                .min(area.bottom().saturating_sub(popup_h))
        }
        .max(area.y);

        let popup = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_w,
            height: popup_h,
        };

        if !area.intersects(popup) {
            return;
        }

        Border::Round.draw(buf, popup, th.border, th.panel);
        let inner = Border::Round.inner(popup);
        fill(buf, inner, th.panel);

        state.hits.clear();
        state.hits.resize(state.ranked.len(), HitBox::default());

        let mut y = inner.y;
        let mut cursor_pos = 0;

        for (cat, items) in &sections {
            if y >= inner.bottom() - 1 {
                break;
            }
            if let Some(cat_name) = cat {
                put(
                    buf,
                    inner.x + 1,
                    y,
                    cat_name,
                    inner.width.saturating_sub(2),
                    st(th.text_muted, th.panel),
                );
                y += 1;
            }

            for (idx, _score, positions) in items {
                if y >= inner.bottom() - 1 {
                    break;
                }
                let item = &self.items[*idx];
                let is_cursor = cursor_pos == state.cursor;
                let bg = if is_cursor { th.cursor_bg } else { th.panel };
                fill(
                    buf,
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: 1,
                    },
                    bg,
                );

                state.hits[cursor_pos].set_area(Rect {
                    x: inner.x,
                    y,
                    width: inner.width,
                    height: 1,
                });

                let mut x = inner.x + 1;
                let glyph = item.kind.glyph();
                let glyph_color = item.kind.color(&th);
                x += put(buf, x, y, glyph, 1, st(glyph_color, bg));
                x += 1;

                // highlight matched chars
                let label = &item.label;
                if !positions.is_empty() {
                    let mut last = 0;
                    for &p in positions {
                        if p >= last && p < label.len() {
                            let before = &label[last..p];
                            x += put(
                                buf,
                                x,
                                y,
                                before,
                                inner.width.saturating_sub(2),
                                st(th.text, bg),
                            );
                            if let Some(c) = label.chars().nth(p) {
                                let char_str = c.to_string();
                                x += put(
                                    buf,
                                    x,
                                    y,
                                    &char_str,
                                    inner.width.saturating_sub(2),
                                    bold(st(th.accent, bg)),
                                );
                                last = p + c.len_utf8();
                            }
                        }
                    }
                    let rest = &label[last..];
                    x += put(
                        buf,
                        x,
                        y,
                        rest,
                        inner.width.saturating_sub(2),
                        st(th.text, bg),
                    );
                } else {
                    x += put(
                        buf,
                        x,
                        y,
                        label,
                        inner.width.saturating_sub(2),
                        st(th.text, bg),
                    );
                }

                x += 2;
                if let Some(detail) = &item.detail {
                    let desc_w = inner.right().saturating_sub(x + 1);
                    if desc_w > 0 {
                        let desc = truncate(detail, desc_w as usize);
                        put(buf, x, y, &desc, desc_w, st(th.text_muted, bg));
                    }
                }

                y += 1;
                cursor_pos += 1;
            }
        }

        let hint = "↑↓ navigate · Tab/Enter select · Esc close";
        let hint_y = inner.bottom() - 1;
        if hint_y >= inner.y {
            put_centered(
                buf,
                Rect {
                    x: inner.x,
                    y: hint_y,
                    width: inner.width,
                    height: 1,
                },
                hint,
                st(th.text_muted, th.panel),
            );
        }
    }
}

// attachment chips

/// Image, file, snippet, or URL attachment types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentKind {
    Image,
    File,
    Snippet,
    Url,
}

impl AttachmentKind {
    fn glyph(self) -> &'static str {
        match self {
            AttachmentKind::Image => "◧",
            AttachmentKind::File => "¶",
            AttachmentKind::Snippet => "≡",
            AttachmentKind::Url => "⊕",
        }
    }
}

/// Name, kind, and optional size label for one attachment chip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub name: String,
    pub kind: AttachmentKind,
    pub size: Option<String>,
}

impl Attachment {
    /// Create attachment.
    pub fn new(name: impl Into<String>, kind: AttachmentKind) -> Self {
        Self {
            name: name.into(),
            kind,
            size: None,
        }
    }
    /// Set size.
    pub fn size(mut self, s: impl Into<String>) -> Self {
        self.size = Some(s.into());
        self
    }
}

/// Hover index, hit boxes, remove button hits, and removal flag; the widget writes hit state each render, reads `removed` when the user deletes a chip.
#[derive(Clone, Debug, Default)]
pub struct AttachmentChipsState {
    pub hover: Option<usize>,
    pub hits: Vec<HitBox>,
    pub remove_hits: Vec<HitBox>,
    pub removed: Option<usize>,
}

impl AttachmentChipsState {
    /// Take removed index.
    pub fn take_removed(&mut self) -> Option<usize> {
        self.removed.take()
    }
}

impl Interactive for AttachmentChipsState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        if k.code == KeyCode::Backspace && !self.hits.is_empty() {
            self.removed = Some(self.hits.len() - 1);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let mut out = Outcome::Ignored;
        for (i, hit) in self.remove_hits.iter_mut().enumerate() {
            match hit.mouse(&m) {
                Hit::Press => {
                    self.removed = Some(i);
                    return Outcome::Changed;
                }
                Hit::HoverChanged if hit.hover => {
                    self.hover = Some(i);
                    out = Outcome::Consumed;
                }
                _ => {}
            }
        }
        for (i, hit) in self.hits.iter_mut().enumerate() {
            if hit.mouse(&m) == Hit::HoverChanged && hit.hover {
                self.hover = Some(i);
                out = Outcome::Consumed;
            }
        }
        if out == Outcome::Ignored {
            self.hover = None;
        }
        out
    }
}

/// Horizontal row of removable attachment chips with hover and focus states.
pub struct AttachmentChips<'a> {
    attachments: &'a [Attachment],
    focused: bool,
    theme: Option<Theme>,
}

impl<'a> AttachmentChips<'a> {
    /// Create attachment chips.
    pub fn new() -> Self {
        Self {
            attachments: <&[Attachment]>::default(),
            focused: false,
            theme: None,
        }
    }
    /// Set attachments.
    pub fn attachments(mut self, a: &'a [Attachment]) -> Self {
        self.attachments = a;
        self
    }
    /// Set focus.
    pub fn focused(mut self, f: bool) -> Self {
        self.focused = f;
        self
    }
    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl<'a> Default for AttachmentChips<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> StatefulWidget for AttachmentChips<'a> {
    type State = AttachmentChipsState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < 5 || area.height < 1 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);

        fill(buf, area, th.background);

        state.hits.clear();
        state.remove_hits.clear();
        state.hits.resize(self.attachments.len(), HitBox::default());
        state
            .remove_hits
            .resize(self.attachments.len(), HitBox::default());

        let mut x = area.x;
        let mut visible = 0;

        for (i, attach) in self.attachments.iter().enumerate() {
            let glyph = attach.kind.glyph();
            let label = if let Some(size) = &attach.size {
                format!(" {} {} {} × ", glyph, attach.name, size)
            } else {
                format!(" {} {} × ", glyph, attach.name)
            };
            let w = label.width() as u16;

            if x + w > area.right() {
                break;
            }

            let is_hover = state.hover == Some(i);
            let bg = if is_hover { th.hover_bg } else { th.surface };

            let chip_rect = Rect {
                x,
                y: area.y,
                width: w,
                height: 1,
            };
            fill(buf, chip_rect, bg);

            state.hits[i].set_area(chip_rect);

            let mut cx = x;
            let parts: Vec<&str> = label.split('×').collect();
            if parts.len() == 2 {
                cx += put(buf, cx, area.y, parts[0], w, st(th.text, bg));
                let remove_rect = Rect {
                    x: cx,
                    y: area.y,
                    width: 1,
                    height: 1,
                };
                state.remove_hits[i].set_area(remove_rect);
                let remove_color = if is_hover { th.error } else { th.text_muted };
                cx += put(buf, cx, area.y, "×", 1, st(remove_color, bg));
                put(buf, cx, area.y, parts[1], w, st(th.text, bg));
            } else {
                put(buf, cx, area.y, &label, w, st(th.text, bg));
            }

            x += w + 1;
            visible += 1;
        }

        if visible < self.attachments.len() {
            let overflow = format!("+{}", self.attachments.len() - visible);
            let ow = overflow.width() as u16 + 2;
            if x + ow <= area.right() {
                let chip_rect = Rect {
                    x,
                    y: area.y,
                    width: ow,
                    height: 1,
                };
                fill(buf, chip_rect, th.surface);
                put(
                    buf,
                    x + 1,
                    area.y,
                    &overflow,
                    ow - 2,
                    st(th.text_muted, th.surface),
                );
            }
        }
    }
}

// mode badge

/// Plan, Act, Ask, or Auto modes for the AI harness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarnessMode {
    Plan,
    Act,
    Ask,
    Auto,
}

impl HarnessMode {
    /// Mode label.
    pub fn label(self) -> &'static str {
        match self {
            HarnessMode::Plan => "Plan",
            HarnessMode::Act => "Act",
            HarnessMode::Ask => "Ask",
            HarnessMode::Auto => "Auto",
        }
    }
    /// Mode color.
    pub fn color(self, th: &Theme) -> Rgb {
        match self {
            HarnessMode::Plan => th.secondary,
            HarnessMode::Act => th.primary,
            HarnessMode::Ask => th.warning,
            HarnessMode::Auto => th.success,
        }
    }
}

/// Mode badge with animated color transition.
pub struct ModeBadge {
    mode: HarnessMode,
    hint: Option<String>,
    from: Option<(HarnessMode, Instant)>,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl ModeBadge {
    /// Create mode badge.
    pub fn new() -> Self {
        Self {
            mode: HarnessMode::Auto,
            hint: None,
            from: None,
            now: None,
            theme: None,
        }
    }
    /// Set mode.
    pub fn mode(mut self, m: HarnessMode) -> Self {
        self.mode = m;
        self
    }
    /// Set hint.
    pub fn hint(mut self, h: &str) -> Self {
        self.hint = Some(h.to_string());
        self
    }
    /// Set transition source.
    pub fn from(mut self, f: Option<(HarnessMode, Instant)>) -> Self {
        self.from = f;
        self
    }
    /// Set time.
    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }
    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
    /// Width of the badge.
    pub fn width(&self) -> u16 {
        let label_w = self.mode.label().width() as u16 + 2;
        let hint_w = self
            .hint
            .as_ref()
            .map(|h| h.width() as u16 + 1)
            .unwrap_or(0);
        label_w + hint_w
    }
}

impl Default for ModeBadge {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ModeBadge {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height < 1 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let now = self.now.unwrap_or_else(Instant::now);

        let target_color = self.mode.color(&th);
        let pill_color = if let Some((from_mode, start)) = self.from {
            let elapsed = now.saturating_duration_since(start).as_secs_f32();
            let duration = 0.25;
            let t = (elapsed / duration).min(1.0);
            let t_eased = Easing::OutCubic.apply(t);
            from_mode.color(&th).blend(target_color, t_eased)
        } else {
            target_color
        };

        let label = format!(" {} ", self.mode.label());
        let label_w = label.width() as u16;

        let mut x = area.x;
        let text_color = pill_color.text_on(0.9);

        let pill_rect = Rect {
            x,
            y: area.y,
            width: label_w,
            height: 1,
        };
        fill(buf, pill_rect, pill_color);
        put(
            buf,
            x,
            area.y,
            &label,
            label_w,
            bold(st(text_color, pill_color)),
        );
        x += label_w;

        if let Some(hint) = &self.hint {
            x += 1;
            let hint_w = hint.width() as u16;
            if x + hint_w <= area.right() {
                put(
                    buf,
                    x,
                    area.y,
                    hint,
                    hint_w,
                    st(th.text_muted, th.background),
                );
            }
        }
    }
}

// harness status

/// Harness status line.
pub struct HarnessStatus {
    mode: HarnessMode,
    model: String,
    branch: String,
    dirty: bool,
    context_pct: f32,
    tokens: u32,
    cost: f32,
    elapsed: std::time::Duration,
    busy: bool,
    queued: u32,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl HarnessStatus {
    /// Create status line.
    pub fn new() -> Self {
        Self {
            mode: HarnessMode::Auto,
            model: String::new(),
            branch: String::new(),
            dirty: false,
            context_pct: 0.0,
            tokens: 0,
            cost: 0.0,
            elapsed: std::time::Duration::ZERO,
            busy: false,
            queued: 0,
            now: None,
            theme: None,
        }
    }
    /// Set mode.
    pub fn mode(mut self, m: HarnessMode) -> Self {
        self.mode = m;
        self
    }
    /// Set model.
    pub fn model(mut self, m: &str) -> Self {
        self.model = m.to_string();
        self
    }
    /// Set branch.
    pub fn branch(mut self, b: &str) -> Self {
        self.branch = b.to_string();
        self
    }
    /// Set dirty.
    pub fn dirty(mut self, d: bool) -> Self {
        self.dirty = d;
        self
    }
    /// Set context percentage.
    pub fn context_pct(mut self, p: f32) -> Self {
        self.context_pct = p;
        self
    }
    /// Set tokens.
    pub fn tokens(mut self, t: u32) -> Self {
        self.tokens = t;
        self
    }
    /// Set cost.
    pub fn cost(mut self, c: f32) -> Self {
        self.cost = c;
        self
    }
    /// Set elapsed.
    pub fn elapsed(mut self, e: std::time::Duration) -> Self {
        self.elapsed = e;
        self
    }
    /// Set busy.
    pub fn busy(mut self, b: bool) -> Self {
        self.busy = b;
        self
    }
    /// Set queued count.
    pub fn queued(mut self, q: u32) -> Self {
        self.queued = q;
        self
    }
    /// Set time.
    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }
    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Default for HarnessStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for HarnessStatus {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 1 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let now = self.now.unwrap_or_else(Instant::now);

        fill(buf, area, th.panel);

        let mut parts: Vec<String> = Vec::new();

        // spinner or dot
        let spinner_glyph = if self.busy {
            spinners::DOTS.frame(anim::since(now))
        } else {
            "●"
        };
        parts.push(spinner_glyph.to_string());

        // mode
        parts.push(self.mode.label().to_string());

        // model
        if !self.model.is_empty() {
            parts.push(self.model.clone());
        }

        // context bar + pct
        let ctx_bar = format!("{}%", (self.context_pct * 100.0) as u32);
        parts.push(ctx_bar.clone());

        // tokens
        if self.tokens > 0 {
            parts.push(fmt_tokens(self.tokens));
        }

        // cost
        if self.cost > 0.0 {
            parts.push(format!("${:.2}", self.cost));
        }

        // branch
        if !self.branch.is_empty() {
            let branch_str = if self.dirty {
                format!("{}*", self.branch)
            } else {
                self.branch.clone()
            };
            parts.push(branch_str);
        }

        // elapsed
        if !self.elapsed.is_zero() {
            let secs = self.elapsed.as_secs();
            parts.push(format!("{}s", secs));
        }

        // queued
        if self.queued > 0 {
            parts.push(format!("{} queued", self.queued));
        }

        // measure and drop parts if needed
        let sep = " · ";
        let sep_w = sep.width() as u16;

        // priority order (keep first, drop last): spinner, mode, model, context; drop: queued, elapsed, cost, tokens, branch
        let keep_priority = [0, 1, 2, 3]; // spinner, mode, model, context
        let drop_priority = vec![8, 7, 6, 5, 4]; // queued, elapsed, cost, tokens, branch

        loop {
            let total_w: usize = parts.iter().map(|p| p.width()).sum::<usize>()
                + sep_w as usize * parts.len().saturating_sub(1);
            if total_w <= area.width as usize {
                break;
            }
            let mut dropped = false;
            for &idx in &drop_priority {
                if idx < parts.len() && !keep_priority.contains(&idx) {
                    parts.remove(idx);
                    dropped = true;
                    break;
                }
            }
            if !dropped {
                break;
            }
        }

        let mut x = area.x + 1;
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                x += put(buf, x, area.y, sep, sep_w, st(th.text_muted, th.panel));
            }
            let color = if i == 0 {
                if self.busy { th.primary } else { th.text_muted }
            } else if i == 1 {
                self.mode.color(&th)
            } else if part.contains('*') {
                th.warning
            } else {
                th.text
            };
            let pw = part.width() as u16;
            x += put(buf, x, area.y, part, pw, st(color, th.panel));

            // draw context bar after the percentage
            if i < parts.len() && part.ends_with('%') {
                x += 1;
                let bar_w = 6;
                if x + bar_w <= area.right() {
                    // paint track background first
                    fill(
                        buf,
                        Rect {
                            x,
                            y: area.y,
                            width: bar_w,
                            height: 1,
                        },
                        th.surface,
                    );
                    hbar(
                        buf,
                        x,
                        area.y,
                        bar_w,
                        self.context_pct,
                        th.primary,
                        th.surface,
                    );
                    x += bar_w;
                }
            }
        }
    }
}

// question card

/// Label and description for one question card option row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

impl QuestionOption {
    /// Create option.
    pub fn new(label: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: description.into(),
        }
    }
}

/// Cursor position, selected flags, submission indices, cancellation flag, and hit boxes; the widget writes hits each render, reads `submitted` or `cancelled` when the user answers or cancels.
#[derive(Clone, Debug, Default)]
pub struct QuestionCardState {
    pub cursor: usize,
    pub selected: Vec<bool>,
    pub submitted: Option<Vec<usize>>,
    pub cancelled: bool,
    pub hits: Vec<HitBox>,
}

impl QuestionCardState {
    /// Returns selected option indices and clears the submission flag.
    pub fn take_answer(&mut self) -> Option<Vec<usize>> {
        self.submitted.take()
    }
    /// Returns whether the card was cancelled and clears the flag.
    pub fn take_cancelled(&mut self) -> bool {
        std::mem::replace(&mut self.cancelled, false)
    }
}

impl Interactive for QuestionCardState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Outcome::Consumed
            }
            KeyCode::Down => {
                if !self.selected.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.selected.len() - 1);
                }
                Outcome::Consumed
            }
            KeyCode::Char(' ') if self.selected.len() > 1 => {
                if self.cursor < self.selected.len() {
                    self.selected[self.cursor] = !self.selected[self.cursor];
                }
                Outcome::Consumed
            }
            KeyCode::Enter => {
                let indices: Vec<usize> = self
                    .selected
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &s)| if s { Some(i) } else { None })
                    .collect();
                self.submitted = Some(indices);
                Outcome::Changed
            }
            KeyCode::Esc => {
                self.cancelled = true;
                Outcome::Changed
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if let Some(digit) = c.to_digit(10) {
                    let digit = digit as usize;
                    if digit > 0 && digit <= self.selected.len() {
                        let idx = digit - 1;
                        if self.selected.len() == 1 {
                            // single mode: submit
                            self.selected[idx] = true;
                            self.submitted = Some(vec![idx]);
                            return Outcome::Changed;
                        } else {
                            // multi mode: toggle
                            self.selected[idx] = !self.selected[idx];
                            return Outcome::Consumed;
                        }
                    }
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        for (i, hit) in self.hits.iter_mut().enumerate() {
            match hit.mouse(&m) {
                Hit::Press => {
                    self.cursor = i;
                    if self.selected.len() == 1 {
                        self.selected[i] = true;
                        self.submitted = Some(vec![i]);
                        return Outcome::Changed;
                    } else {
                        self.selected[i] = !self.selected[i];
                        return Outcome::Consumed;
                    }
                }
                Hit::HoverChanged if hit.hover => {
                    self.cursor = i;
                    return Outcome::Consumed;
                }
                _ => {}
            }
        }
        Outcome::Ignored
    }
}

/// Interactive card presenting a question with single or multi-select options, keyboard shortcuts, and a recommended hint.
pub struct QuestionCard<'a> {
    question: String,
    options: &'a [QuestionOption],
    multi: bool,
    recommended: Option<usize>,
    focused: bool,
    theme: Option<Theme>,
}

impl<'a> QuestionCard<'a> {
    /// Create question card.
    pub fn new() -> Self {
        Self {
            question: String::new(),
            options: <&[QuestionOption]>::default(),
            multi: false,
            recommended: None,
            focused: false,
            theme: None,
        }
    }
    /// Set question.
    pub fn question(mut self, q: &str) -> Self {
        self.question = q.to_string();
        self
    }
    /// Set options.
    pub fn options(mut self, o: &'a [QuestionOption]) -> Self {
        self.options = o;
        self
    }
    /// Set multi-select.
    pub fn multi(mut self, m: bool) -> Self {
        self.multi = m;
        self
    }
    /// Set recommended option.
    pub fn recommended(mut self, r: Option<usize>) -> Self {
        self.recommended = r;
        self
    }
    /// Set focus.
    pub fn focused(mut self, f: bool) -> Self {
        self.focused = f;
        self
    }
    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl<'a> Default for QuestionCard<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> StatefulWidget for QuestionCard<'a> {
    type State = QuestionCardState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < 10 || area.height < 3 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);

        // ensure selected matches options
        if state.selected.len() != self.options.len() {
            state.selected = vec![false; self.options.len()];
        }

        let border_color = if self.focused {
            th.primary
        } else {
            th.border_blurred
        };
        Border::Round.draw(buf, area, border_color, th.background);
        let inner = Border::Round.inner(area);

        let mut y = inner.y;

        // question (wrapped, bold)
        if !self.question.is_empty() {
            let lines = wrap(&self.question, inner.width.saturating_sub(2) as usize);
            for line in lines {
                if y >= inner.bottom() - 1 {
                    break;
                }
                put(
                    buf,
                    inner.x + 1,
                    y,
                    &line,
                    inner.width.saturating_sub(2),
                    bold(st(th.text, th.background)),
                );
                y += 1;
            }
            y += 1;
        }

        state.hits.clear();
        state.hits.resize(self.options.len(), HitBox::default());

        // options
        for (i, opt) in self.options.iter().enumerate() {
            if y >= inner.bottom() - 1 {
                break;
            }

            let is_cursor = i == state.cursor;
            let marker = if is_cursor { "❯" } else { " " };
            let marker_color = if is_cursor { th.primary } else { th.text };

            let digit = format!("{} ", i + 1);
            let checkbox = if self.multi {
                if state.selected[i] { "[x] " } else { "[ ] " }
            } else {
                ""
            };

            let prefix = format!("{} {}{}{}", marker, digit, checkbox, opt.label);
            let _prefix_w = prefix.width() as u16;

            state.hits[i].set_area(Rect {
                x: inner.x,
                y,
                width: inner.width,
                height: 1,
            });

            let mut x = inner.x + 1;
            x += put(buf, x, y, marker, 1, st(marker_color, th.background));
            x += 1;
            x += put(
                buf,
                x,
                y,
                &digit,
                digit.width() as u16,
                st(th.text, th.background),
            );
            if self.multi {
                x += put(
                    buf,
                    x,
                    y,
                    checkbox,
                    checkbox.width() as u16,
                    st(th.text, th.background),
                );
            }
            x += put(
                buf,
                x,
                y,
                &opt.label,
                opt.label.width() as u16,
                st(th.text, th.background),
            );

            // recommended chip
            if self.recommended == Some(i) {
                x += 1;
                let chip = " recommended ";
                let chip_w = chip.width() as u16;
                if x + chip_w < inner.right() {
                    fill(
                        buf,
                        Rect {
                            x,
                            y,
                            width: chip_w,
                            height: 1,
                        },
                        th.primary,
                    );
                    put(
                        buf,
                        x,
                        y,
                        chip,
                        chip_w,
                        st(th.primary.text_on(0.9), th.primary),
                    );
                }
            }

            y += 1;

            // description (wrapped, muted, indented)
            if !opt.description.is_empty() {
                let desc_lines = wrap(&opt.description, inner.width.saturating_sub(6) as usize);
                for line in desc_lines {
                    if y >= inner.bottom() - 1 {
                        break;
                    }
                    put(
                        buf,
                        inner.x + 5,
                        y,
                        &line,
                        inner.width.saturating_sub(6),
                        st(th.text_muted, th.background),
                    );
                    y += 1;
                }
            }
        }

        // footer hint
        let hint = if self.multi {
            "Enter answer · Esc cancel · Space toggle"
        } else {
            "Enter answer · Esc cancel"
        };
        let hint_y = inner.bottom() - 1;
        if hint_y >= inner.y {
            put_centered(
                buf,
                Rect {
                    x: inner.x,
                    y: hint_y,
                    width: inner.width,
                    height: 1,
                },
                hint,
                st(th.text_muted, th.background),
            );
        }
    }
}

// plan view

/// Pending, in-progress, done, blocked, or dropped states for plan tasks; each has a glyph, color, and modifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    InProgress,
    Done,
    Blocked,
    Dropped,
}

impl TaskState {
    fn glyph(self, now: Instant) -> &'static str {
        match self {
            TaskState::Pending => "○",
            TaskState::InProgress => spinners::DOTS.frame(anim::since(now)),
            TaskState::Done => "✓",
            TaskState::Blocked => "◔",
            TaskState::Dropped => "✗",
        }
    }
    fn color(self, th: &Theme) -> Rgb {
        match self {
            TaskState::Pending => th.text_muted,
            TaskState::InProgress => th.primary,
            TaskState::Done => th.success,
            TaskState::Blocked => th.warning,
            TaskState::Dropped => th.text_muted,
        }
    }
    fn modifier(self) -> Modifier {
        match self {
            TaskState::Dropped => Modifier::CROSSED_OUT,
            _ => Modifier::empty(),
        }
    }
}

/// Text and state for one task row in a plan phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanTask {
    pub text: String,
    pub state: TaskState,
}

impl PlanTask {
    /// Create task.
    pub fn new(text: impl Into<String>, state: TaskState) -> Self {
        Self {
            text: text.into(),
            state,
        }
    }
}

/// Named phase with a task list and collapse flag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanPhase {
    pub name: String,
    pub tasks: Vec<PlanTask>,
    pub collapsed: bool,
}

impl PlanPhase {
    /// Create phase.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tasks: Vec::new(),
            collapsed: false,
        }
    }
    /// Add task.
    pub fn task(mut self, text: impl Into<String>, state: TaskState) -> Self {
        self.tasks.push(PlanTask::new(text, state));
        self
    }
    /// Set collapsed.
    pub fn collapsed(mut self, c: bool) -> Self {
        self.collapsed = c;
        self
    }
}

/// Cursor position, scroll offset, hit boxes, and per-phase collapse state; the widget writes hits each render, reads `collapsed` to show or hide phase tasks.
#[derive(Clone, Debug, Default)]
pub struct PlanViewState {
    pub cursor: usize,
    pub scroll: usize,
    pub hits: Vec<HitBox>,
    pub collapsed: Vec<bool>,
}

impl Interactive for PlanViewState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Outcome::Consumed
            }
            KeyCode::Down => {
                if !self.hits.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.hits.len() - 1);
                }
                Outcome::Consumed
            }
            KeyCode::Left | KeyCode::Char('c') => {
                if self.cursor < self.collapsed.len() {
                    self.collapsed[self.cursor] = true;
                }
                Outcome::Consumed
            }
            KeyCode::Right | KeyCode::Enter | KeyCode::Char('e') => {
                if self.cursor < self.collapsed.len() {
                    self.collapsed[self.cursor] = false;
                }
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        for (i, hit) in self.hits.iter_mut().enumerate() {
            match hit.mouse(&m) {
                Hit::Press => {
                    self.cursor = i;
                    if i < self.collapsed.len() {
                        self.collapsed[i] = !self.collapsed[i];
                    }
                    return Outcome::Consumed;
                }
                Hit::HoverChanged if hit.hover => {
                    self.cursor = i;
                    return Outcome::Consumed;
                }
                _ => {}
            }
        }
        if let Some(delta) = wheel_delta(&m) {
            if delta > 0 {
                if !self.hits.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.hits.len() - 1);
                }
            } else {
                self.cursor = self.cursor.saturating_sub(1);
            }
            return Outcome::Consumed;
        }
        Outcome::Ignored
    }
}

/// Collapsible tree of plan phases and tasks with keyboard navigation and animated state icons.
pub struct PlanView<'a> {
    title: Option<String>,
    phases: &'a [PlanPhase],
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl<'a> PlanView<'a> {
    /// Create plan view.
    pub fn new() -> Self {
        Self {
            title: None,
            phases: <&[PlanPhase]>::default(),
            now: None,
            theme: None,
        }
    }
    /// Set title.
    pub fn title(mut self, t: &str) -> Self {
        self.title = Some(t.to_string());
        self
    }
    /// Set phases.
    pub fn phases(mut self, p: &'a [PlanPhase]) -> Self {
        self.phases = p;
        self
    }
    /// Set time.
    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }
    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl<'a> Default for PlanView<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> StatefulWidget for PlanView<'a> {
    type State = PlanViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < 10 || area.height < 2 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let now = self.now.unwrap_or_else(Instant::now);

        fill(buf, area, th.background);

        // ensure collapsed matches phases
        if state.collapsed.len() != self.phases.len() {
            state.collapsed = self.phases.iter().map(|p| p.collapsed).collect();
        }

        let mut y = area.y;

        // header with progress
        if let Some(title) = &self.title {
            let total: usize = self.phases.iter().map(|p| p.tasks.len()).sum();
            let done: usize = self
                .phases
                .iter()
                .flat_map(|p| &p.tasks)
                .filter(|t| t.state == TaskState::Done)
                .count();
            let header = format!("{}  {}/{}", title, done, total);
            put(
                buf,
                area.x,
                y,
                &header,
                area.width,
                bold(st(th.text, th.background)),
            );

            let bar_w = 10;
            let hx = area.right().saturating_sub(bar_w + 1);
            if hx > area.x + header.width() as u16 + 2 && total > 0 {
                let progress = done as f32 / total as f32;
                // paint track
                fill(
                    buf,
                    Rect {
                        x: hx,
                        y,
                        width: bar_w,
                        height: 1,
                    },
                    th.surface,
                );
                // filled portion
                hbar(buf, hx, y, bar_w, progress, th.success, th.surface);
            }

            y += 1;
        }

        state.hits.clear();
        state.hits.resize(self.phases.len(), HitBox::default());

        for (i, phase) in self.phases.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }

            let is_collapsed = state.collapsed[i];
            let arrow = if is_collapsed { "▸" } else { "▾" };

            let done = phase
                .tasks
                .iter()
                .filter(|t| t.state == TaskState::Done)
                .count();
            let total = phase.tasks.len();
            let phase_label = format!("{} {}  {}/{}", arrow, phase.name, done, total);

            state.hits[i].set_area(Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            });

            let mut x = area.x;
            x += put(
                buf,
                x,
                y,
                &phase_label,
                area.width,
                bold(st(th.text, th.background)),
            );

            x += 1;
            let bar_w = 6;
            let bar_x = area.right().saturating_sub(bar_w + 1);
            if bar_x > x && total > 0 {
                let progress = done as f32 / total as f32;
                // paint track
                fill(
                    buf,
                    Rect {
                        x: bar_x,
                        y,
                        width: bar_w,
                        height: 1,
                    },
                    th.surface,
                );
                // filled portion
                hbar(buf, bar_x, y, bar_w, progress, th.primary, th.surface);
            }

            y += 1;

            if !is_collapsed {
                for task in &phase.tasks {
                    if y >= area.bottom() {
                        break;
                    }

                    let glyph = task.state.glyph(now);
                    let color = task.state.color(&th);
                    let modifier = task.state.modifier();

                    let mut x = area.x + 2;
                    x += put(buf, x, y, glyph, 1, st(color, th.background));
                    x += 1;
                    let text_style = st(th.text, th.background).add_modifier(modifier);
                    put(
                        buf,
                        x,
                        y,
                        &task.text,
                        area.width.saturating_sub(x - area.x),
                        text_style,
                    );

                    y += 1;
                }
            }
        }
    }
}

// message queue

/// Text and timestamp label for one queued message row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedMessage {
    pub text: String,
    pub when: String,
}

impl QueuedMessage {
    /// Create message.
    pub fn new(text: impl Into<String>, when: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            when: when.into(),
        }
    }
}

/// Cursor position, hit boxes, and removal index; the widget writes hits each render, reads `removed` when the user deletes a message.
#[derive(Clone, Debug, Default)]
pub struct MessageQueueState {
    pub cursor: usize,
    pub hits: Vec<HitBox>,
    pub removed: Option<usize>,
}

impl MessageQueueState {
    /// Take removed index.
    pub fn take_removed(&mut self) -> Option<usize> {
        self.removed.take()
    }
}

impl Interactive for MessageQueueState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Outcome::Consumed
            }
            KeyCode::Down => {
                if !self.hits.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.hits.len() - 1);
                }
                Outcome::Consumed
            }
            KeyCode::Char('d') | KeyCode::Backspace | KeyCode::Delete => {
                if self.cursor < self.hits.len() {
                    self.removed = Some(self.cursor);
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        for (i, hit) in self.hits.iter_mut().enumerate() {
            match hit.mouse(&m) {
                Hit::Press => {
                    self.cursor = i;
                    return Outcome::Consumed;
                }
                Hit::HoverChanged if hit.hover => {
                    self.cursor = i;
                    return Outcome::Consumed;
                }
                _ => {}
            }
        }
        if let Some(delta) = wheel_delta(&m) {
            if delta > 0 {
                if !self.hits.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.hits.len() - 1);
                }
            } else {
                self.cursor = self.cursor.saturating_sub(1);
            }
            return Outcome::Consumed;
        }
        Outcome::Ignored
    }
}

/// Scrollable list of queued messages with delete controls.
pub struct MessageQueue<'a> {
    messages: &'a [QueuedMessage],
    theme: Option<Theme>,
}

impl<'a> MessageQueue<'a> {
    /// Create message queue.
    pub fn new() -> Self {
        Self {
            messages: <&[QueuedMessage]>::default(),
            theme: None,
        }
    }
    /// Set messages.
    pub fn messages(mut self, m: &'a [QueuedMessage]) -> Self {
        self.messages = m;
        self
    }
    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl<'a> Default for MessageQueue<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> StatefulWidget for MessageQueue<'a> {
    type State = MessageQueueState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < 10 || area.height < 2 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);

        fill(buf, area, th.background);

        let mut y = area.y;

        // header
        if self.messages.is_empty() {
            put(
                buf,
                area.x,
                y,
                "No queued messages",
                area.width,
                st(th.text_muted, th.background),
            );
            return;
        } else {
            let header = format!("{} queued · sent after this turn", self.messages.len());
            put(
                buf,
                area.x,
                y,
                &header,
                area.width,
                st(th.text_muted, th.background),
            );
            y += 1;
        }

        state.hits.clear();
        state.hits.resize(self.messages.len(), HitBox::default());

        for (i, msg) in self.messages.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }

            state.hits[i].set_area(Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            });

            let is_cursor = i == state.cursor;
            let bg = if is_cursor {
                th.cursor_bg
            } else {
                th.background
            };

            if is_cursor {
                fill(
                    buf,
                    Rect {
                        x: area.x,
                        y,
                        width: area.width,
                        height: 1,
                    },
                    bg,
                );
            }

            let num = format!("{}  ", i + 1);
            let mut x = area.x;
            x += put(buf, x, y, &num, num.width() as u16, st(th.text, bg));

            let text_w = area
                .width
                .saturating_sub(x - area.x + msg.when.width() as u16 + 3);
            let text = truncate(&msg.text, text_w as usize);
            x += put(buf, x, y, &text, text_w, st(th.text, bg));

            x += 2;
            let when_w = msg.when.width() as u16;
            if x + when_w <= area.right() {
                put(buf, x, y, &msg.when, when_w, st(th.text_muted, bg));
            }

            y += 1;
        }
    }
}

// suggestions

/// Cursor position, hit boxes, and activation index; the widget writes hits each render, reads `activated` when the user picks a suggestion.
#[derive(Clone, Debug, Default)]
pub struct SuggestionsState {
    pub cursor: usize,
    pub hits: Vec<HitBox>,
    pub activated: Option<usize>,
}

impl SuggestionsState {
    /// Take activated index.
    pub fn take_activated(&mut self) -> Option<usize> {
        self.activated.take()
    }
}

impl Interactive for SuggestionsState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        match k.code {
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                Outcome::Consumed
            }
            KeyCode::Right => {
                if !self.hits.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.hits.len() - 1);
                }
                Outcome::Consumed
            }
            KeyCode::Enter => {
                if self.cursor < self.hits.len() {
                    self.activated = Some(self.cursor);
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if let Some(digit) = c.to_digit(10) {
                    let digit = digit as usize;
                    if digit > 0 && digit <= self.hits.len() {
                        self.activated = Some(digit - 1);
                        return Outcome::Changed;
                    }
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        for (i, hit) in self.hits.iter_mut().enumerate() {
            match hit.mouse(&m) {
                Hit::Press => {
                    self.cursor = i;
                    self.activated = Some(i);
                    return Outcome::Changed;
                }
                Hit::HoverChanged if hit.hover => {
                    self.cursor = i;
                    return Outcome::Consumed;
                }
                _ => {}
            }
        }
        Outcome::Ignored
    }
}

/// Row of clickable suggestion chips; the cursor moves with left/right and Enter takes the
/// selected text out of the state.
pub struct Suggestions<'a> {
    items: &'a [&'a str],
    theme: Option<Theme>,
}

impl<'a> Suggestions<'a> {
    /// Create suggestions.
    pub fn new() -> Self {
        Self {
            items: <&[&str]>::default(),
            theme: None,
        }
    }
    /// Set items.
    pub fn items(mut self, i: &'a [&'a str]) -> Self {
        self.items = i;
        self
    }
    /// Set theme.
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl<'a> Default for Suggestions<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> StatefulWidget for Suggestions<'a> {
    type State = SuggestionsState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < 5 || area.height < 1 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);

        fill(buf, area, th.background);

        state.hits.clear();
        state.hits.resize(self.items.len(), HitBox::default());

        let mut x = area.x;
        let mut y = area.y;
        let mut visible = 0;

        for (i, item) in self.items.iter().enumerate() {
            let label = format!(" {} {} ", i + 1, item);
            let w = label.width() as u16;

            if x + w > area.right() {
                // wrap to next row
                x = area.x;
                y += 1;
                if y >= area.bottom() {
                    break;
                }
            }

            let is_cursor = i == state.cursor;
            let bg = if is_cursor { th.cursor_bg } else { th.surface };

            let chip_rect = Rect {
                x,
                y,
                width: w,
                height: 1,
            };
            fill(buf, chip_rect, bg);
            state.hits[i].set_area(chip_rect);

            put(buf, x, y, &label, w, st(th.text, bg));

            x += w + 1;
            visible += 1;
        }

        if visible < self.items.len() {
            let overflow = format!("+{}", self.items.len() - visible);
            let ow = overflow.width() as u16 + 2;
            if x + ow > area.right() {
                x = area.x;
                y += 1;
            }
            if y < area.bottom() && x + ow <= area.right() {
                let chip_rect = Rect {
                    x,
                    y,
                    width: ow,
                    height: 1,
                };
                fill(buf, chip_rect, th.surface);
                put(
                    buf,
                    x + 1,
                    y,
                    &overflow,
                    ow - 2,
                    st(th.text_muted, th.surface),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_menu_filtering() {
        let commands = [
            SlashCommand::new("compact", "Compact output"),
            SlashCommand::new("commit", "Commit changes"),
            SlashCommand::new("cost", "Show cost"),
            SlashCommand::new("help", "Show help"),
        ];
        let state = SlashMenuState {
            open: true,
            query: "co".to_string(),
            ..Default::default()
        };
        let names: Vec<&str> = commands.iter().map(|c| c.name.as_str()).collect();
        let ranked = fuzzy::rank(&state.query, names.iter().copied());
        assert!(!ranked.is_empty());
        // should have compact, commit, cost
        assert!(
            ranked
                .iter()
                .any(|(i, _, _)| commands[*i].name == "compact")
        );
        assert!(ranked.iter().any(|(i, _, _)| commands[*i].name == "commit"));
        assert!(ranked.iter().any(|(i, _, _)| commands[*i].name == "cost"));
    }

    #[test]
    fn slash_menu_tab_selects_the_cursor_command_by_name() {
        let commands = [
            SlashCommand::new("help", "h").category("Info"),
            SlashCommand::new("compact", "c").category("View"),
            SlashCommand::new("commit", "c").category("Action"),
        ];
        let mut state = SlashMenuState {
            open: true,
            query: "com".into(),
            ..Default::default()
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 20));
        SlashMenu::new()
            .commands(&commands)
            .anchor(Rect::new(0, 15, 60, 3))
            .render(buf.area, &mut buf, &mut state);
        state.handle_key(KeyEvent::from(KeyCode::Down));
        let outcome = state.handle_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(outcome, Outcome::Changed);
        assert_eq!(state.take_selected().as_deref(), Some("commit"));
        assert!(!state.open);
    }

    #[test]
    fn question_card_digit_submit_single() {
        let mut state = QuestionCardState {
            selected: vec![false], // single mode has 1 option
            ..Default::default()
        };
        let outcome = state.handle_key(KeyEvent::from(KeyCode::Char('1')));
        assert_eq!(outcome, Outcome::Changed);
        assert_eq!(state.submitted, Some(vec![0]));
    }

    #[test]
    fn question_card_digit_toggle_multi() {
        let mut state = QuestionCardState {
            selected: vec![false, false, false],
            ..Default::default()
        };
        let outcome = state.handle_key(KeyEvent::from(KeyCode::Char('2')));
        assert_eq!(outcome, Outcome::Consumed);
        assert!(state.selected[1]);
        let outcome = state.handle_key(KeyEvent::from(KeyCode::Char('2')));
        assert_eq!(outcome, Outcome::Consumed);
        assert!(!state.selected[1]);
    }

    #[test]
    fn harness_status_drops_low_priority_segments_when_narrow() {
        let row = |width: u16| {
            let mut buf = Buffer::empty(Rect::new(0, 0, width, 1));
            HarnessStatus::new()
                .mode(HarnessMode::Act)
                .model("claude")
                .context_pct(0.5)
                .tokens(1000)
                .cost(0.5)
                .branch("main")
                .elapsed(std::time::Duration::from_secs(10))
                .queued(2)
                .render(buf.area, &mut buf);
            (0..width)
                .map(|x| buf[(x, 0)].symbol().to_string())
                .collect::<String>()
        };
        let wide = row(120);
        assert!(wide.contains("queued") && wide.contains("main") && wide.contains("$0.50"));
        let narrow = row(36);
        assert!(narrow.contains("Act") && narrow.contains("claude") && narrow.contains("50%"));
        assert!(
            !narrow.contains("queued") && !narrow.contains("main") && !narrow.contains("$0.50")
        );
    }

    #[test]
    fn attachment_chips_remove() {
        let mut state = AttachmentChipsState {
            hits: vec![HitBox::default(), HitBox::default()],
            ..Default::default()
        };
        let outcome = state.handle_key(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(outcome, Outcome::Changed);
        assert_eq!(state.removed, Some(1));
    }

    #[test]
    fn plan_view_collapse() {
        let mut state = PlanViewState {
            collapsed: vec![false, false],
            cursor: 0,
            ..Default::default()
        };
        let outcome = state.handle_key(KeyEvent::from(KeyCode::Char('c')));
        assert_eq!(outcome, Outcome::Consumed);
        assert!(state.collapsed[0]);

        let outcome = state.handle_key(KeyEvent::from(KeyCode::Char('e')));
        assert_eq!(outcome, Outcome::Consumed);
        assert!(!state.collapsed[0]);
    }

    #[test]
    fn size_sweep_no_panic() {
        let sizes = [(1, 1), (3, 2), (10, 3), (60, 16), (130, 42), (250, 70)];
        for (w, h) in sizes {
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);

            let mut state = SlashMenuState {
                open: true,
                ..Default::default()
            };
            SlashMenu::new().render(area, &mut buf, &mut state);

            let mut state = MentionPickerState {
                open: true,
                ..Default::default()
            };
            MentionPicker::new().render(area, &mut buf, &mut state);

            let mut state = AttachmentChipsState::default();
            AttachmentChips::new().render(area, &mut buf, &mut state);

            ModeBadge::new().render(area, &mut buf);

            HarnessStatus::new().render(area, &mut buf);

            let mut state = QuestionCardState::default();
            QuestionCard::new().render(area, &mut buf, &mut state);

            let mut state = PlanViewState::default();
            PlanView::new().render(area, &mut buf, &mut state);

            let mut state = MessageQueueState::default();
            MessageQueue::new().render(area, &mut buf, &mut state);

            let mut state = SuggestionsState::default();
            Suggestions::new().render(area, &mut buf, &mut state);
        }
    }
}
