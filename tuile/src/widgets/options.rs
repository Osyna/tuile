//! Settings menu: grouped `label  value` rows with a cursor, in-place value cycling and a group
//! index for a sidebar - the shape of omp's settings screen.
//!
//! Left/Right (or `h`/`l`, or Space) cycle the value at the cursor and report it through
//! [`OptionListState::take_changed`]. Enter never cycles: it reports the row through
//! [`OptionListState::take_activated`], which is how an app opens a dropdown, a text field or
//! runs a command for the row the user is on. A row whose value the widget cannot cycle -
//! [`OptionValue::Text`], [`OptionValue::Action`] - only ever activates.
//!
//! ```no_run
//! use tuile::prelude::*;
//! # let area = Rect::new(0, 0, 60, 20);
//! # let mut buf = Buffer::empty(area);
//! let mut state = OptionListState::new(vec![
//!     OptionGroup::new("Theme", vec![
//!         OptionItem::choice("theme", "Dark Theme", &["titanium", "nord", "dracula"], 0),
//!         OptionItem::bool("colorblind", "Color-Blind Mode", false)
//!             .trail(["default", "next session"])
//!             .dim(true),
//!     ]),
//!     OptionGroup::new("Display", vec![OptionItem::int("fps", "Frame rate", 60, 15, 120, 15)]),
//! ]);
//! OptionList::new().focused(true).render(area, &mut buf, &mut state);
//! if let Some(key) = state.take_changed() { let _ = state.choice("theme"); }
//! if let Some(key) = state.take_activated() { /* open the editor for `key` */ }
//! ```

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Modifier;
use ratatui_core::widgets::StatefulWidget;
use unicode_width::UnicodeWidthStr;

use crate::core::{
    HitBox, Interactive, MinSize, Outcome, is_left_down, is_press, mouse_pos, wheel_delta,
};
use crate::draw::{fill, put, put_right, refuse, st, truncate};
use crate::theme::{self, Theme};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState, keep_visible};

/// Value of one option row.
#[derive(Clone, Debug, PartialEq)]
pub enum OptionValue {
    Bool(bool),
    /// One of `options`, `index` selected.
    Choice {
        options: Vec<String>,
        index: usize,
    },
    Int {
        value: i64,
        min: i64,
        max: i64,
        step: i64,
    },
    /// Read-only text: the widget never cycles it, Enter activates the row and the app opens
    /// whatever editor the value needs.
    Text(String),
    /// A command. Enter activates the row; there is nothing to cycle.
    Action,
}

impl OptionValue {
    /// Value as the string drawn in the right column; `Action` renders as empty.
    pub fn display(&self) -> String {
        match self {
            OptionValue::Bool(b) => b.to_string(),
            OptionValue::Choice { options, index } => {
                options.get(*index).cloned().unwrap_or_default()
            }
            OptionValue::Int { value, .. } => value.to_string(),
            OptionValue::Text(t) => t.clone(),
            OptionValue::Action => String::new(),
        }
    }

    /// Cycle by `dir` (+1 / -1). Returns `true` when the value changed or an action fired.
    fn step(&mut self, dir: i64) -> bool {
        match self {
            OptionValue::Bool(b) => {
                *b = !*b;
                true
            }
            OptionValue::Choice { options, index } if !options.is_empty() => {
                let n = options.len() as i64;
                *index = ((*index as i64 + dir).rem_euclid(n)) as usize;
                true
            }
            OptionValue::Int {
                value,
                min,
                max,
                step,
            } => {
                let next = (*value + dir * *step).clamp(*min, *max);
                let moved = next != *value;
                *value = next;
                moved
            }
            OptionValue::Text(_) | OptionValue::Action => false,
            OptionValue::Choice { .. } => false,
        }
    }
}

/// One settings row: stable `key`, visible `label`, the value the user cycles, and optional help.
#[derive(Clone, Debug)]
pub struct OptionItem {
    pub key: String,
    pub label: String,
    pub value: OptionValue,
    /// Muted help text shown after the value on the cursor row.
    pub hint: Option<String>,
    /// Trailing columns - where a value came from, when a change takes effect - as wide as
    /// their widest cell across every row, so they read as columns and not as trailing words.
    pub trail: Vec<String>,
    /// This row is showing something the user did not choose: a default, an unset key, a
    /// placeholder. The value and the trailing columns are drawn muted.
    pub dim: bool,
}

impl OptionItem {
    pub fn new(key: &str, label: &str, value: OptionValue) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            value,
            hint: None,
            trail: Vec::new(),
            dim: false,
        }
    }
    pub fn bool(key: &str, label: &str, v: bool) -> Self {
        Self::new(key, label, OptionValue::Bool(v))
    }
    pub fn choice(key: &str, label: &str, options: &[&str], index: usize) -> Self {
        Self::new(
            key,
            label,
            OptionValue::Choice {
                options: options.iter().map(|s| s.to_string()).collect(),
                index,
            },
        )
    }
    pub fn int(key: &str, label: &str, value: i64, min: i64, max: i64, step: i64) -> Self {
        Self::new(
            key,
            label,
            OptionValue::Int {
                value,
                min,
                max,
                step: step.max(1),
            },
        )
    }
    pub fn text(key: &str, label: &str, v: &str) -> Self {
        Self::new(key, label, OptionValue::Text(v.to_string()))
    }
    pub fn action(key: &str, label: &str) -> Self {
        Self::new(key, label, OptionValue::Action)
    }
    pub fn hint(mut self, h: &str) -> Self {
        self.hint = Some(h.to_string());
        self
    }

    /// Trailing metadata columns, aligned with every other row's.
    pub fn trail<I, S>(mut self, cells: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.trail = cells.into_iter().map(Into::into).collect();
        self
    }

    /// Draw the value and the trail muted: this row is at its default, not at a choice.
    pub fn dim(mut self, v: bool) -> Self {
        self.dim = v;
        self
    }
}

/// A titled group of rows.
#[derive(Clone, Debug)]
pub struct OptionGroup {
    pub title: String,
    pub items: Vec<OptionItem>,
}

impl OptionGroup {
    pub fn new(title: &str, items: Vec<OptionItem>) -> Self {
        Self {
            title: title.to_string(),
            items,
        }
    }
}

/// State: the groups themselves plus cursor and scroll.
#[derive(Clone, Debug, Default)]
pub struct OptionListState {
    pub groups: Vec<OptionGroup>,
    /// Flat index over all items, in group order.
    pub cursor: usize,
    pub scroll: usize,
    pub hit: HitBox,
    pub scrollbar_state: ScrollbarState,
    rows_visible: usize,
    row_hits: Vec<(Rect, usize)>,
    /// Where the value column started and ended in the last draw, so an app can put an
    /// in-place editor exactly over the cell it is editing.
    value_col: u16,
    value_end: u16,
    changed: Option<String>,
    activated: Option<String>,
}

impl OptionListState {
    pub fn new(groups: Vec<OptionGroup>) -> Self {
        Self {
            groups,
            ..Default::default()
        }
    }

    /// Screen rect of row `flat` as last drawn.
    pub fn row_rect(&self, flat: usize) -> Option<Rect> {
        self.row_hits
            .iter()
            .find(|&&(_, i)| i == flat)
            .map(|&(r, _)| r)
    }

    /// Screen rect of row `flat`'s value cell as last drawn: where an app draws the field that
    /// edits it, so the editor replaces the value instead of opening somewhere else.
    pub fn value_rect(&self, flat: usize) -> Option<Rect> {
        let row = self.row_rect(flat)?;
        Some(Rect {
            x: self.value_col,
            y: row.y,
            width: self.value_end.saturating_sub(self.value_col),
            height: 1,
        })
    }

    pub fn len(&self) -> usize {
        self.groups.iter().map(|g| g.items.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Key of the row whose value changed (or whose action fired) since the last call.
    pub fn take_changed(&mut self) -> Option<String> {
        self.changed.take()
    }

    /// Key of the row the user asked to open (Enter, or a second click on the cursor row).
    pub fn take_activated(&mut self) -> Option<String> {
        self.activated.take()
    }

    pub fn get(&self, key: &str) -> Option<&OptionValue> {
        self.groups
            .iter()
            .flat_map(|g| &g.items)
            .find(|i| i.key == key)
            .map(|i| &i.value)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut OptionValue> {
        self.groups
            .iter_mut()
            .flat_map(|g| &mut g.items)
            .find(|i| i.key == key)
            .map(|i| &mut i.value)
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        match self.get(key)? {
            OptionValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Selected option text of a `Choice` row.
    pub fn choice(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            OptionValue::Choice { options, index } => options.get(*index).map(String::as_str),
            _ => None,
        }
    }

    pub fn int(&self, key: &str) -> Option<i64> {
        match self.get(key)? {
            OptionValue::Int { value, .. } => Some(*value),
            _ => None,
        }
    }

    fn item_mut(&mut self, flat: usize) -> Option<&mut OptionItem> {
        self.groups.iter_mut().flat_map(|g| &mut g.items).nth(flat)
    }

    /// Index of the group the cursor is in (for a sidebar).
    pub fn group_index(&self) -> usize {
        let mut seen = 0;
        for (gi, g) in self.groups.iter().enumerate() {
            if self.cursor < seen + g.items.len() {
                return gi;
            }
            seen += g.items.len();
        }
        self.groups.len().saturating_sub(1)
    }

    /// Move the cursor to the first row of group `gi`.
    pub fn jump_to_group(&mut self, gi: usize) {
        self.cursor = self.groups.iter().take(gi).map(|g| g.items.len()).sum();
        // put the header at the top of the view
        self.scroll = self.row_of_group(gi);
    }

    /// Row (header rows included) where group `gi` starts.
    fn row_of_group(&self, gi: usize) -> usize {
        self.groups.iter().take(gi).map(|g| g.items.len() + 1).sum()
    }

    /// Row of item `flat` (header rows included).
    fn row_of_item(&self, flat: usize) -> usize {
        let mut row = 0;
        let mut seen = 0;
        for g in &self.groups {
            row += 1;
            if flat < seen + g.items.len() {
                return row + (flat - seen);
            }
            row += g.items.len();
            seen += g.items.len();
        }
        row
    }

    fn total_rows(&self) -> usize {
        self.groups.iter().map(|g| g.items.len() + 1).sum()
    }

    /// Cycle the value at the cursor by `dir` (+1 / -1); reports the key through
    /// [`Self::take_changed`]. Rows the widget cannot cycle are left alone.
    pub fn cycle(&mut self, dir: i64) -> Outcome {
        let cursor = self.cursor;
        let Some(item) = self.item_mut(cursor) else {
            return Outcome::Ignored;
        };
        if item.value.step(dir) {
            let key = item.key.clone();
            self.changed = Some(key);
            Outcome::Changed
        } else {
            Outcome::Consumed
        }
    }

    /// Hand the cursor row to the app to open. Reported by [`Self::take_activated`].
    fn activate(&mut self) -> Outcome {
        match self.groups.iter().flat_map(|g| &g.items).nth(self.cursor) {
            Some(item) => {
                self.activated = Some(item.key.clone());
                Outcome::Changed
            }
            None => Outcome::Ignored,
        }
    }

    fn move_cursor(&mut self, delta: i64) -> Outcome {
        let n = self.len();
        if n == 0 {
            return Outcome::Ignored;
        }
        let before = self.cursor;
        self.cursor = (self.cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
        Outcome::changed_if(before != self.cursor)
    }
}

impl Interactive for OptionListState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        let page = self.rows_visible.max(1) as i64;
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::PageUp => self.move_cursor(-page),
            KeyCode::PageDown => self.move_cursor(page),
            KeyCode::Home => self.move_cursor(i64::MIN / 2),
            KeyCode::End => self.move_cursor(i64::MAX / 2),
            KeyCode::Left | KeyCode::Char('h') => self.cycle(-1),
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') => self.cycle(1),
            KeyCode::Enter => self.activate(),
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        self.hit.mouse(&m);
        if let Some(d) = wheel_delta(&m) {
            if !self.hit.contains(&m) {
                return Outcome::Ignored;
            }
            let max = self.total_rows().saturating_sub(self.rows_visible);
            self.scroll = (self.scroll as i64 + d as i64 * 2).clamp(0, max as i64) as usize;
            return Outcome::Consumed;
        }
        let pos = mouse_pos(&m);
        let over = self
            .row_hits
            .iter()
            .find(|(r, _)| r.contains(pos))
            .map(|&(_, i)| i);
        let sb = self.scrollbar_state.handle_mouse(m);
        if sb.is_changed() {
            self.scroll = self.scrollbar_state.offset;
            return Outcome::Consumed;
        }
        match over {
            Some(i) if is_left_down(&m) => {
                let was = self.cursor;
                self.cursor = i;
                if was == i {
                    self.activate()
                } else {
                    Outcome::Consumed
                }
            }
            Some(i)
                if i != self.cursor
                    && matches!(m.kind, crossterm::event::MouseEventKind::Moved) =>
            {
                self.cursor = i;
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }
}

/// Renders an [`OptionListState`].
#[derive(Clone, Debug)]
pub struct OptionList {
    focused: bool,
    value_column: Option<u16>,
    cursor_glyph: &'static str,
    theme: Option<Theme>,
}

impl OptionList {
    pub fn new() -> Self {
        Self {
            focused: false,
            value_column: None,
            cursor_glyph: "❯",
            theme: None,
        }
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    /// Column where values start (default: widest label + 4).
    pub fn value_column(mut self, c: u16) -> Self {
        self.value_column = Some(c);
        self
    }

    pub fn cursor_glyph(mut self, g: &'static str) -> Self {
        self.cursor_glyph = g;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Default for OptionList {
    fn default() -> Self {
        Self::new()
    }
}

impl MinSize for OptionList {
    /// Minimum size: 8 wide (label + value), 1 tall (one row).
    fn min_size(&self) -> (u16, u16) {
        (8, 1)
    }
}

/// Gap before each trailing column.
const TRAIL_GAP: u16 = 2;
/// Cells the value is guaranteed before the trail is dropped for being too expensive.
const MIN_VALUE_W: u16 = 6;

/// Width of each trailing column: its widest cell over every row, so the columns line up.
fn trail_widths(state: &OptionListState) -> Vec<u16> {
    let mut w: Vec<u16> = Vec::new();
    for item in state.groups.iter().flat_map(|g| &g.items) {
        for (i, cell) in item.trail.iter().enumerate() {
            let cw = cell.width() as u16;
            match w.get_mut(i) {
                Some(slot) => *slot = (*slot).max(cw),
                None => w.push(cw),
            }
        }
    }
    w
}

impl StatefulWidget for OptionList {
    type State = OptionListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hit.set_area(area);
        state.row_hits.clear();
        let th = self.theme.unwrap_or_else(theme::current);
        if refuse(buf, area, self.min_size(), th.text_disabled) {
            return;
        }
        let bg = th.background;
        fill(buf, area, bg);

        let total = state.total_rows();
        let viewport = area.height as usize;
        state.rows_visible = viewport;
        state.scroll = keep_visible(state.scroll, state.row_of_item(state.cursor), viewport)
            .min(total.saturating_sub(viewport));

        let label_w = state
            .groups
            .iter()
            .flat_map(|g| &g.items)
            .map(|i| i.label.width())
            .max()
            .unwrap_or(0) as u16;
        let value_x = area.x + self.value_column.unwrap_or(label_w + 4).min(area.width / 2);
        let has_sb = total > viewport;
        let right = area.right() - u16::from(has_sb);

        // Every trailing column is as wide as its widest cell, so they read as columns down the
        // pane rather than as words trailing each value. The block starts just past the widest
        // value instead of flush right: columns that are read together belong together, and a
        // pane 110 cells wide would otherwise leave 45 blank ones between a value and where it
        // came from. It slides left only when the row cannot hold both.
        let cols = trail_widths(state);
        let trail_w: u16 = cols.iter().map(|w| w + TRAIL_GAP).sum();
        let value_w = state
            .groups
            .iter()
            .flat_map(|g| &g.items)
            .map(|i| i.value.display().width())
            .max()
            .unwrap_or(0) as u16;
        let trail_x = (value_x + value_w + TRAIL_GAP).min(right.saturating_sub(trail_w));
        let trailing = trail_w > 0 && trail_x >= value_x + MIN_VALUE_W;
        let value_right = if trailing { trail_x } else { right };
        state.value_col = value_x;
        state.value_end = value_right;

        let mut row = 0usize;
        let mut flat = 0usize;
        for g in &state.groups {
            // header
            if row >= state.scroll && row < state.scroll + viewport {
                let y = area.y + (row - state.scroll) as u16;
                put(
                    buf,
                    area.x + 2,
                    y,
                    &g.title,
                    right.saturating_sub(area.x + 2),
                    st(th.text_primary, bg).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                );
            }
            row += 1;
            for item in &g.items {
                if row >= state.scroll && row < state.scroll + viewport {
                    let y = area.y + (row - state.scroll) as u16;
                    let line = Rect {
                        x: area.x,
                        y,
                        width: right - area.x,
                        height: 1,
                    };
                    let is_cursor = flat == state.cursor;
                    // A row the user never set reads muted - the value is a default, and so is
                    // whatever the trail says about where it came from.
                    let (row_bg, label_fg, value_fg, trail_fg) = match (is_cursor, self.focused) {
                        (true, true) => (
                            th.cursor_bg,
                            th.cursor_fg,
                            th.cursor_fg,
                            th.cursor_fg.blend(th.cursor_bg, 0.4),
                        ),
                        (true, false) => (th.selection_bg, th.text, th.text, th.text_muted),
                        _ if item.dim => (bg, th.text_muted, th.text_disabled, th.text_disabled),
                        _ => (bg, th.text, th.text, th.text_muted),
                    };
                    if row_bg != bg {
                        fill(buf, line, row_bg);
                    }
                    if is_cursor {
                        put(
                            buf,
                            area.x,
                            y,
                            self.cursor_glyph,
                            1,
                            st(label_fg, row_bg).add_modifier(Modifier::BOLD),
                        );
                    }
                    let label_style = if is_cursor {
                        st(label_fg, row_bg).add_modifier(Modifier::BOLD)
                    } else {
                        st(label_fg, row_bg)
                    };
                    put(
                        buf,
                        area.x + 2,
                        y,
                        &truncate(&item.label, value_x.saturating_sub(area.x + 3) as usize),
                        value_x.saturating_sub(area.x + 3),
                        label_style,
                    );
                    let value = item.value.display();
                    let mut vx = value_x;
                    if !value.is_empty() {
                        vx += put(
                            buf,
                            value_x,
                            y,
                            &truncate(&value, value_right.saturating_sub(value_x) as usize),
                            value_right.saturating_sub(value_x),
                            st(value_fg, row_bg),
                        );
                    }
                    if trailing {
                        let mut x = trail_x;
                        for (cell, &w) in item.trail.iter().zip(&cols) {
                            x += TRAIL_GAP;
                            put(
                                buf,
                                x,
                                y,
                                &truncate(cell, w as usize),
                                w,
                                st(trail_fg, row_bg),
                            );
                            x += w;
                        }
                    }
                    if let (true, Some(h)) = (is_cursor, &item.hint) {
                        let slot = Rect {
                            x: vx + 2,
                            y,
                            width: value_right.saturating_sub(vx + 2),
                            height: 1,
                        };
                        if slot.width > 4 {
                            put_right(
                                buf,
                                slot,
                                &truncate(h, slot.width as usize),
                                st(trail_fg, row_bg),
                            );
                        }
                    }
                    state.row_hits.push((line, flat));
                }
                row += 1;
                flat += 1;
            }
        }

        if has_sb {
            let sb = Rect {
                x: area.right() - 1,
                width: 1,
                ..area
            };
            Scrollbar::vertical(total, viewport)
                .offset(state.scroll)
                .theme(&th)
                .render(sb, buf, &mut state.scrollbar_state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> OptionListState {
        OptionListState::new(vec![
            OptionGroup::new(
                "A",
                vec![
                    OptionItem::choice("shape", "Shape", &["field", "bars", "rule"], 0),
                    OptionItem::bool("on", "On", false),
                ],
            ),
            OptionGroup::new("B", vec![OptionItem::int("fps", "FPS", 60, 15, 60, 15)]),
        ])
    }

    #[test]
    fn cursor_spans_groups_and_reports_group() {
        let mut s = state();
        assert_eq!(s.group_index(), 0);
        s.handle_key(KeyEvent::from(KeyCode::Down));
        s.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!((s.cursor, s.group_index()), (2, 1));
        s.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(s.cursor, 2, "clamps at the last row");
        s.jump_to_group(0);
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn values_cycle_and_report_key() {
        let mut s = state();
        assert!(s.handle_key(KeyEvent::from(KeyCode::Left)).is_changed());
        assert_eq!(s.choice("shape"), Some("rule"), "Left wraps backwards");
        assert_eq!(s.take_changed().as_deref(), Some("shape"));
        assert_eq!(s.take_changed(), None);
        s.cursor = 2;
        assert!(
            s.handle_key(KeyEvent::from(KeyCode::Right)).is_consumed(),
            "already at max: no change"
        );
        assert!(s.handle_key(KeyEvent::from(KeyCode::Left)).is_changed());
        assert_eq!(s.int("fps"), Some(45));
    }

    #[test]
    fn renders_cursor_and_values() {
        let mut s = state();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 6));
        OptionList::new()
            .focused(true)
            .render(buf.area, &mut buf, &mut s);
        assert_eq!(buf[(0, 1)].symbol(), "❯");
        let row: String = (0..40).map(|x| buf[(x, 1)].symbol().to_string()).collect();
        assert!(row.contains("Shape") && row.contains("field"), "{row}");
    }

    #[test]
    fn enter_opens_the_row_and_never_cycles_it() {
        let mut s = state();
        assert!(s.handle_key(KeyEvent::from(KeyCode::Enter)).is_changed());
        assert_eq!(s.take_activated().as_deref(), Some("shape"));
        assert_eq!(
            s.choice("shape"),
            Some("field"),
            "Enter left the value alone"
        );
        assert_eq!(s.take_changed(), None, "activation is not a write");
        assert_eq!(s.take_activated(), None, "taken once");
    }

    #[test]
    fn a_row_with_nothing_to_cycle_reports_no_change() {
        let mut s = OptionListState::new(vec![OptionGroup::new(
            "A",
            vec![
                OptionItem::text("path", "Path", "~/skills"),
                OptionItem::action("login", "Log in"),
            ],
        )]);
        for row in 0..2 {
            s.cursor = row;
            for key in [KeyCode::Left, KeyCode::Right, KeyCode::Char(' ')] {
                assert!(
                    s.handle_key(KeyEvent::from(key)).is_consumed(),
                    "row {row} claimed {key:?} changed something"
                );
                assert_eq!(s.take_changed(), None, "row {row}, {key:?}");
            }
        }
    }

    #[test]
    fn trailing_columns_line_up_and_defaults_read_muted() {
        let th = Theme::default();
        let mut s = OptionListState::new(vec![OptionGroup::new(
            "A",
            vec![
                OptionItem::choice("provider", "Provider", &["claude"], 0)
                    .trail(["project", "next session"]),
                OptionItem::text("timeout", "Timeout", "900")
                    .trail(["default", "now"])
                    .dim(true),
            ],
        )]);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 3));
        OptionList::new()
            .focused(true)
            .theme(&th)
            .render(buf.area, &mut buf, &mut s);

        let row = |y: u16| -> String { (0..60).map(|x| buf[(x, y)].symbol()).collect() };
        let (top, bottom) = (row(1), row(2));
        let at = |line: &str, word: &str| line.find(word).map(|b| line[..b].chars().count());
        assert_eq!(
            at(&top, "project"),
            at(&bottom, "default"),
            "first trail column is one column: {top}|{bottom}"
        );
        assert_eq!(
            at(&top, "next session"),
            at(&bottom, "now"),
            "second trail column is one column: {top}|{bottom}"
        );

        let value_at = at(&bottom, "900").expect("the value drew") as u16;
        assert_eq!(
            buf[(value_at, 2)].fg,
            ratatui_core::style::Color::from(th.text_disabled),
            "a default value is muted, not stated as a choice"
        );
        let chosen = at(&top, "claude").expect("the value drew") as u16;
        assert_ne!(
            buf[(chosen, 1)].fg,
            ratatui_core::style::Color::from(th.text_disabled),
            "a value the user set is not muted"
        );
    }
}
