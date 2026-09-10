//! Checkbox, Switch, RadioGroup, CheckList, and Segmented controls — Textual-faithful toggles.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 40, 5);
//! # let mut buf = Buffer::empty(area);
//! # let key = KeyEvent::from(KeyCode::Char(' '));
//! let mut cb = CheckboxState::new(CheckState::Off);
//! Checkbox::new("Enable feature").render(area, &mut buf, &mut cb);
//! if cb.handle_key(key).is_changed() { /* toggled */ }
//!
//! let mut sw = SwitchState::new(false);
//! Switch::new().label("Dark mode").render(area, &mut buf, &mut sw);
//! ```

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::{Position, Rect};
use ratatui::style::Modifier;
use ratatui::widgets::StatefulWidget;
use unicode_width::UnicodeWidthStr;

use crate::anim::{Easing, Tween};
use crate::core::{Hit, HitBox, Interactive, Look, Outcome, is_activate, is_press};
use crate::draw::{Border, LEFT_BLOCKS, fill, put, st};
use crate::theme::{self, Theme};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState, keep_visible};

use std::time::Duration;

// ───────────────────────────── checkbox ─────────────────────────────

/// Checkbox state: off/on/indeterminate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckState {
    #[default]
    Off,
    On,
    Indeterminate,
}

/// Textual-style checkbox with `▐X▌` glyph and label.
#[derive(Clone, Debug)]
pub struct Checkbox {
    label: String,
    tri_state: bool,
    label_first: bool,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
}

impl Checkbox {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            tri_state: false,
            label_first: false,
            focused: false,
            enabled: true,
            theme: None,
        }
    }

    pub fn tri_state(mut self, v: bool) -> Self {
        self.tri_state = v;
        self
    }

    pub fn label_first(mut self, v: bool) -> Self {
        self.label_first = v;
        self
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl StatefulWidget for Checkbox {
    type State = CheckboxState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hit.set_area(area);
        if area.width < 3 || area.height == 0 {
            return;
        }

        let th = self.theme.clone().unwrap_or_else(theme::current);
        let look = Look { focused: self.focused, hover: state.hit.hover, enabled: self.enabled };
        let bg = th.background;

        let mark = match state.value {
            CheckState::On => "X",
            CheckState::Off => " ",
            CheckState::Indeterminate => "-",
        };
        let btn_bg = th.panel;
        let mut mark_fg = if state.value == CheckState::On { th.text_success } else { Theme::shade(th.panel, -2) };
        if !look.enabled {
            mark_fg = mark_fg.blend(bg, 0.5);
        }

        let (side_fg, side_bg, btn) = if look.focused && self.label.is_empty() {
            (th.cursor_bg, bg, th.cursor_bg)
        } else {
            (btn_bg, bg, btn_bg)
        };

        let (box_x, label_x) = if self.label_first {
            (area.x + self.label.width() as u16 + 1, area.x)
        } else {
            (area.x, area.x + 3)
        };

        // a painted 3-cell button: half-block ends get brightened by min-contrast terminals
        put(buf, box_x, area.y, " ", 1, st(side_bg, side_fg));
        put(buf, box_x + 1, area.y, mark, 1, st(mark_fg, btn).add_modifier(Modifier::BOLD));
        put(buf, box_x + 2, area.y, " ", 1, st(side_bg, side_fg));

        if !self.label.is_empty() {
            let style = if look.focused {
                st(th.cursor_fg, th.cursor_bg).add_modifier(Modifier::BOLD)
            } else if !look.enabled {
                st(th.text_disabled, bg)
            } else if look.hover {
                st(th.text, th.hover_bg)
            } else {
                st(th.text, bg)
            };
            let max_w = area.width.saturating_sub(if self.label_first { box_x - area.x } else { 3 });
            let text = if self.label_first {
                self.label.clone()
            } else {
                format!(" {}", self.label)
            };
            put(buf, label_x, area.y, &text, max_w, style);
        }
    }
}

/// Checkbox state.
#[derive(Clone, Debug, Default)]
pub struct CheckboxState {
    pub value: CheckState,
    pub hit: HitBox,
    tri_state: bool,
}

impl CheckboxState {
    pub fn new(value: CheckState) -> Self {
        Self { value, hit: HitBox::default(), tri_state: false }
    }

    pub fn toggle(&mut self) {
        self.value = match self.value {
            CheckState::Off => CheckState::On,
            CheckState::On => if self.tri_state { CheckState::Indeterminate } else { CheckState::Off },
            CheckState::Indeterminate => CheckState::Off,
        };
    }

    pub fn set_tri_state(&mut self, v: bool) {
        self.tri_state = v;
    }
}

impl Interactive for CheckboxState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        if is_activate(&key) {
            self.toggle();
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        match self.hit.mouse(&m) {
            Hit::Click => {
                self.toggle();
                Outcome::Changed
            }
            Hit::HoverChanged | Hit::Press | Hit::Cancel => Outcome::Consumed,
            Hit::Drag | Hit::Wheel(_) | Hit::None => Outcome::Ignored,
        }
    }
}

// ───────────────────────────── switch ─────────────────────────────

const SWITCH_W: u16 = 14;

/// Textual `Switch`: 8-cell track with sliding 4-cell thumb.
#[derive(Clone, Debug)]
pub struct Switch {
    label: Option<String>,
    compact: bool,
    duration: Option<Duration>,
    focused: bool,
    enabled: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl Switch {
    pub fn new() -> Self {
        Self {
            label: None,
            compact: false,
            duration: None,
            focused: false,
            enabled: true,
            now: None,
            theme: None,
        }
    }

    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }

    pub fn compact(mut self, v: bool) -> Self {
        self.compact = v;
        self
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn duration(mut self, d: Duration) -> Self {
        self.duration = Some(d);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl Default for Switch {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for Switch {
    type State = SwitchState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let min_w = if self.compact { SWITCH_W - 2 } else { SWITCH_W };
        let min_h = if self.compact { 1 } else { 3 };
        state.hit.set_area(area);
        state.duration = self.duration.unwrap_or(Duration::from_millis(200));
        if area.width < min_w || area.height < min_h {
            return;
        }

        let th = self.theme.clone().unwrap_or_else(theme::current);
        let look = Look { focused: self.focused, hover: state.hit.hover, enabled: self.enabled };
        let t = state.anim.value(self.now.unwrap_or_else(Instant::now));

        let bg = if !self.compact && look.focused {
            th.surface.blend(th.foreground, 0.05)
        } else {
            th.background
        };
        fill(buf, area, bg);

        if !self.compact {
            let border = if look.focused { th.border } else { th.border_blurred };
            Border::Tall.draw(buf, area, border, bg);
        }

        let track_y = if self.compact { area.y } else { area.y + 1 };
        let track_x = if self.compact { area.x } else { area.x + 3 };
        let track = if !self.compact && look.focused { Theme::shade(th.panel, -2) } else { Theme::shade(bg.blend(th.foreground, 0.1), -2) };
        let mut thumb = if state.on { th.success } else { th.panel };
        if look.hover && look.enabled {
            thumb = Theme::shade(thumb, 1);
        }
        if !look.enabled {
            thumb = thumb.blend(bg, 0.5);
        }

        let start = t * 4.0;
        let end = start + 4.0;
        for i in 0..8u16 {
            let (cl, cr) = (i as f32, i as f32 + 1.0);
            let Some(cell) = buf.cell_mut((track_x + i, track_y)) else { continue };
            if end <= cl || start >= cr {
                cell.set_symbol(" ").set_bg(track.color());
            } else if start <= cl && end >= cr {
                cell.set_symbol(" ").set_bg(thumb.color());
            } else if start > cl {
                let idx = ((start - cl) * 8.0).round() as usize;
                cell.set_symbol(LEFT_BLOCKS[idx.min(8)]).set_fg(track.color()).set_bg(thumb.color());
            } else {
                let idx = ((end - cl) * 8.0).round() as usize;
                cell.set_symbol(LEFT_BLOCKS[idx.min(8)]).set_fg(thumb.color()).set_bg(track.color());
            }
        }

        if let Some(label) = &self.label {
            let label_x = track_x + 9;
            let max_w = area.width.saturating_sub(label_x - area.x);
            let fg = if look.enabled { th.text } else { th.text_disabled };
            put(buf, label_x, track_y, label, max_w, st(fg, bg));
        }
    }
}
/// Switch state with animation.
#[derive(Clone, Debug)]
pub struct SwitchState {
    pub on: bool,
    pub anim: Tween,
    pub hit: HitBox,
    pub duration: Duration,
}

impl Default for SwitchState {
    fn default() -> Self {
        Self::new(false)
    }
}

impl SwitchState {
    pub fn new(on: bool) -> Self {
        Self {
            on,
            anim: Tween::new(if on { 1.0 } else { 0.0 }),
            hit: HitBox::default(),
            duration: Duration::from_millis(200),
        }
    }

    pub fn set(&mut self, on: bool, now: Instant, dur: Duration) {
        if self.on != on {
            self.on = on;
            self.anim.go_with(if on { 1.0 } else { 0.0 }, now, dur, Easing::InOutCubic);
        }
    }

    pub fn toggle(&mut self, now: Instant, dur: Duration) {
        self.set(!self.on, now, dur);
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.anim.active(now)
    }
}

impl Interactive for SwitchState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        if is_activate(&key) {
            self.toggle(Instant::now(), self.duration);
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        match self.hit.mouse(&m) {
            Hit::Click => {
                self.toggle(Instant::now(), self.duration);
                Outcome::Changed
            }
            Hit::HoverChanged | Hit::Press | Hit::Cancel => Outcome::Consumed,
            Hit::Drag | Hit::Wheel(_) | Hit::None => Outcome::Ignored,
        }
    }
}

// ───────────────────────────── radio group ─────────────────────────────

/// Textual-style radio button group.
#[derive(Clone, Debug)]
pub struct RadioGroup {
    options: Vec<String>,
    horizontal: bool,
    bordered: bool,
    title: Option<String>,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
}

impl RadioGroup {
    pub fn new(options: Vec<String>) -> Self {
        Self {
            options,
            horizontal: false,
            bordered: false,
            title: None,
            focused: false,
            enabled: true,
            theme: None,
        }
    }

    pub fn horizontal(mut self, v: bool) -> Self {
        self.horizontal = v;
        self
    }

    pub fn bordered(mut self, v: bool) -> Self {
        self.bordered = v;
        self
    }

    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl StatefulWidget for RadioGroup {
    type State = RadioState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hits.clear();
        if area.width < 3 || area.height == 0 || self.options.is_empty() {
            return;
        }

        let th = self.theme.clone().unwrap_or_else(theme::current);
        let look = Look { focused: self.focused, hover: false, enabled: self.enabled };
        let bg = th.background;

        let content = if self.bordered {
            let border_color = if look.focused { th.border } else { th.border_blurred };
            if let Some(title) = &self.title {
                let title_style = st(if look.focused { th.text } else { th.text_muted }, bg).add_modifier(Modifier::BOLD);
                Border::Tall.draw_titled_with(buf, area, border_color, bg, title, ratatui::layout::Alignment::Left, title_style)
            } else {
                Border::Tall.draw(buf, area, border_color, bg);
                Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(2), height: area.height.saturating_sub(2) }
            }
        } else {
            area
        };

        if self.horizontal {
            let mut x = content.x;
            for (idx, opt) in self.options.iter().enumerate() {
                let w = 4 + opt.width() as u16;
                if x + w > content.right() {
                    break;
                }
                let r = Rect { x, y: content.y, width: w, height: 1 };
                state.hits.push(r);
                self.render_option(buf, r, idx, opt, state, &th, &look);
                x += w + 1;
            }
        } else {
            for (y, (idx, opt)) in (content.y..).zip(self.options.iter().enumerate()) {
                if y >= content.bottom() {
                    break;
                }
                let r = Rect { x: content.x, y, width: content.width, height: 1 };
                state.hits.push(r);
                self.render_option(buf, r, idx, opt, state, &th, &look);
            }
        }
    }
}

impl RadioGroup {
    fn render_option(&self, buf: &mut Buffer, area: Rect, idx: usize, label: &str, state: &RadioState, th: &Theme, look: &Look) {
        let bg = th.background;
        let selected = state.selected == Some(idx);
        let is_cursor = state.cursor == idx;

        let mark = if selected { "●" } else { " " };
        let btn_bg = th.panel;
        let mut mark_fg = if selected { th.text_success } else { Theme::shade(th.panel, -2) };
        if !look.enabled {
            mark_fg = mark_fg.blend(bg, 0.5);
        }

        let (side_fg, side_bg, btn) = if is_cursor && look.focused {
            (th.cursor_bg, bg, th.cursor_bg)
        } else {
            (btn_bg, bg, btn_bg)
        };

        put(buf, area.x, area.y, " ", 1, st(side_bg, side_fg));
        put(buf, area.x + 1, area.y, mark, 1, st(mark_fg, btn).add_modifier(Modifier::BOLD));
        put(buf, area.x + 2, area.y, " ", 1, st(side_bg, side_fg));

        let style = if is_cursor && look.focused {
            st(th.cursor_fg, th.cursor_bg).add_modifier(Modifier::BOLD)
        } else if !look.enabled {
            st(th.text_disabled, bg)
        } else {
            st(th.text, bg)
        };
        let max_w = area.width.saturating_sub(3);
        put(buf, area.x + 3, area.y, &format!(" {}", label), max_w, style);
    }
}

/// Radio group state.
#[derive(Clone, Debug, Default)]
pub struct RadioState {
    pub selected: Option<usize>,
    pub cursor: usize,
    pub hits: Vec<Rect>,
}

impl RadioState {
    pub fn new(selected: Option<usize>) -> Self {
        Self { selected, cursor: selected.unwrap_or(0), hits: vec![] }
    }

    pub fn select(&mut self, idx: usize) {
        self.selected = Some(idx);
        self.cursor = idx;
    }
}

impl Interactive for RadioState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        let total = self.hits.len();
        if total == 0 {
            return Outcome::Ignored;
        }
        match key.code {
            KeyCode::Up | KeyCode::Left => {
                self.cursor = if self.cursor == 0 { total - 1 } else { self.cursor - 1 };
                Outcome::Consumed
            }
            KeyCode::Down | KeyCode::Right => {
                self.cursor = (self.cursor + 1) % total;
                Outcome::Consumed
            }
            _ if is_activate(&key) => {
                if self.selected != Some(self.cursor) {
                    self.selected = Some(self.cursor);
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                }
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let pos = Position { x: m.column, y: m.row };
        if let Some(idx) = self.hits.iter().position(|r| r.contains(pos)) {
            match m.kind {
                ratatui::crossterm::event::MouseEventKind::Down(_) => {
                    self.cursor = idx;
                    if self.selected != Some(idx) {
                        self.selected = Some(idx);
                        Outcome::Changed
                    } else {
                        Outcome::Consumed
                    }
                }
                _ => Outcome::Consumed,
            }
        } else {
            Outcome::Ignored
        }
    }
}

// ───────────────────────────── checklist ─────────────────────────────

/// Scrollable multi-select checkbox list.
#[derive(Clone, Debug)]
pub struct CheckList {
    options: Vec<String>,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
    now: Option<Instant>,
}

impl CheckList {
    pub fn new(options: Vec<String>) -> Self {
        Self {
            options,
            focused: false,
            enabled: true,
            theme: None,
            now: None,
        }
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }
}

impl StatefulWidget for CheckList {
    type State = CheckListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hits.clear();
        if area.width < 5 || area.height < 3 || self.options.is_empty() {
            return;
        }

        let th = self.theme.clone().unwrap_or_else(theme::current);
        let look = Look { focused: self.focused, hover: false, enabled: self.enabled };
        let bg = th.background;

        let border_color = if look.focused { th.border } else { th.border_blurred };
        Border::Tall.draw(buf, area, border_color, bg);

        let content = Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(3), height: area.height.saturating_sub(2) };
        let viewport = content.height as usize;
        state.scroll = keep_visible(state.scroll, state.cursor, viewport);

        for (i, idx) in (state.scroll..).take(viewport).enumerate() {
            if idx >= self.options.len() {
                break;
            }
            let y = content.y + i as u16;
            let r = Rect { x: content.x, y, width: content.width, height: 1 };
            state.hits.push((r, idx));

            let checked = state.checked.get(idx).copied().unwrap_or(false);
            let is_cursor = state.cursor == idx;

            let mark = if checked { "X" } else { " " };
            let btn_bg = th.panel;
            let mut mark_fg = if checked { th.text_success } else { Theme::shade(th.panel, -2) };
            if !look.enabled {
                mark_fg = mark_fg.blend(bg, 0.5);
            }

            let (side_fg, side_bg, btn) = if is_cursor && look.focused {
                (th.cursor_bg, bg, th.cursor_bg)
            } else {
                (btn_bg, bg, btn_bg)
            };

            put(buf, r.x, y, " ", 1, st(side_bg, side_fg));
            put(buf, r.x + 1, y, mark, 1, st(mark_fg, btn).add_modifier(Modifier::BOLD));
            put(buf, r.x + 2, y, " ", 1, st(side_bg, side_fg));

            let style = if is_cursor && look.focused {
                st(th.cursor_fg, th.cursor_bg).add_modifier(Modifier::BOLD)
            } else if !look.enabled {
                st(th.text_disabled, bg)
            } else {
                st(th.text, bg)
            };
            let max_w = r.width.saturating_sub(3);
            put(buf, r.x + 3, y, &format!(" {}", self.options[idx]), max_w, style);
        }

        if self.options.len() > viewport {
            let sb_area = Rect { x: area.right() - 1, y: area.y + 1, width: 1, height: area.height.saturating_sub(2) };
            Scrollbar::vertical(self.options.len(), viewport).offset(state.scroll).render(sb_area, buf, &mut state.sb);
        }
    }
}

/// CheckList state.
#[derive(Clone, Debug, Default)]
pub struct CheckListState {
    pub checked: Vec<bool>,
    pub cursor: usize,
    pub scroll: usize,
    pub hits: Vec<(Rect, usize)>,
    pub sb: ScrollbarState,
}

impl CheckListState {
    pub fn new(count: usize) -> Self {
        Self {
            checked: vec![false; count],
            cursor: 0,
            scroll: 0,
            hits: vec![],
            sb: ScrollbarState::default(),
        }
    }

    pub fn toggle_current(&mut self) {
        if self.cursor < self.checked.len() {
            self.checked[self.cursor] = !self.checked[self.cursor];
        }
    }

    pub fn toggle_all(&mut self) {
        let all_on = self.checked.iter().all(|&v| v);
        self.checked.fill(!all_on);
    }
}

impl Interactive for CheckListState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        let total = self.checked.len();
        if total == 0 {
            return Outcome::Ignored;
        }
        match key.code {
            KeyCode::Up => {
                self.cursor = if self.cursor == 0 { total - 1 } else { self.cursor - 1 };
                Outcome::Consumed
            }
            KeyCode::Down => {
                self.cursor = (self.cursor + 1) % total;
                Outcome::Consumed
            }
            KeyCode::Char(' ') if is_press(&key) => {
                self.toggle_current();
                Outcome::Changed
            }
            KeyCode::Char('a') if is_press(&key) => {
                self.toggle_all();
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let sb_out = self.sb.handle_mouse(m);
        if sb_out != Outcome::Ignored {
            self.scroll = self.sb.offset;
            return sb_out;
        }

        let pos = Position { x: m.column, y: m.row };
        if let Some(&(_, idx)) = self.hits.iter().find(|(r, _)| r.contains(pos)) {
            match m.kind {
                ratatui::crossterm::event::MouseEventKind::Down(_) => {
                    self.cursor = idx;
                    if idx < self.checked.len() {
                        self.checked[idx] = !self.checked[idx];
                        Outcome::Changed
                    } else {
                        Outcome::Consumed
                    }
                }
                _ => Outcome::Consumed,
            }
        } else {
            Outcome::Ignored
        }
    }
}

// ───────────────────────────── segmented ─────────────────────────────

/// Textual-style segmented control: horizontal pill row.
#[derive(Clone, Debug)]
pub struct Segmented {
    options: Vec<String>,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
}

impl Segmented {
    pub fn new(options: Vec<String>) -> Self {
        Self {
            options,
            focused: false,
            enabled: true,
            theme: None,
        }
    }

    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl StatefulWidget for Segmented {
    type State = SegmentedState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hits.clear();
        if area.width < 3 || area.height == 0 || self.options.is_empty() {
            return;
        }

        let th = self.theme.clone().unwrap_or_else(theme::current);
        let look = Look { focused: self.focused, hover: false, enabled: self.enabled };
        let bg = th.background;

        // flat painted segments (no half-block ends: fg glyphs near their bg colour get
        // brightened by terminals with a minimum-contrast setting)
        let seg_bg = Theme::shade(th.surface, 1);
        let mut x = area.x;
        for (idx, opt) in self.options.iter().enumerate() {
            let w = opt.width() as u16 + 2;
            if x + w > area.right() {
                break;
            }
            let r = Rect { x, y: area.y, width: w, height: 1 };
            state.hits.push(r);

            let selected = state.selected == idx;
            let (fg, item_bg) = if selected && look.focused {
                (th.cursor_fg, th.cursor_bg)
            } else if selected {
                (th.primary.text_on(0.9), th.primary)
            } else if !look.enabled {
                (th.text_disabled, seg_bg)
            } else {
                (th.text, seg_bg)
            };
            put(buf, x, area.y, &format!(" {} ", opt), w, st(fg, item_bg).add_modifier(Modifier::BOLD));

            x += w;
            if idx + 1 < self.options.len() {
                put(buf, x, area.y, " ", 1, st(bg, bg));
                x += 1;
            }
        }
    }
}

/// Segmented state.
#[derive(Clone, Debug, Default)]
pub struct SegmentedState {
    pub selected: usize,
    pub hits: Vec<Rect>,
}

impl SegmentedState {
    pub fn new(selected: usize) -> Self {
        Self { selected, hits: vec![] }
    }
}

impl Interactive for SegmentedState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        let total = self.hits.len();
        if total == 0 {
            return Outcome::Ignored;
        }
        match key.code {
            KeyCode::Left => {
                if self.selected > 0 {
                    self.selected -= 1;
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                }
            }
            KeyCode::Right => {
                if self.selected + 1 < total {
                    self.selected += 1;
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                }
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let pos = Position { x: m.column, y: m.row };
        if let Some(idx) = self.hits.iter().position(|r| r.contains(pos)) {
            match m.kind {
                ratatui::crossterm::event::MouseEventKind::Down(_) => {
                    if self.selected != idx {
                        self.selected = idx;
                        Outcome::Changed
                    } else {
                        Outcome::Consumed
                    }
                }
                _ => Outcome::Consumed,
            }
        } else {
            Outcome::Ignored
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkbox_toggles() {
        let mut state = CheckboxState::new(CheckState::Off);
        state.toggle();
        assert_eq!(state.value, CheckState::On);
        state.toggle();
        assert_eq!(state.value, CheckState::Off);
    }

    #[test]
    fn checkbox_tri_state() {
        let mut state = CheckboxState::new(CheckState::Off);
        state.set_tri_state(true);
        state.toggle();
        assert_eq!(state.value, CheckState::On);
        state.toggle();
        assert_eq!(state.value, CheckState::Indeterminate);
        state.toggle();
        assert_eq!(state.value, CheckState::Off);
    }

    #[test]
    fn switch_animates() {
        let mut state = SwitchState::new(false);
        let now = Instant::now();
        state.toggle(now, Duration::from_millis(200));
        assert!(state.on);
        assert!(state.animating(now));
    }

    #[test]
    fn radio_select() {
        let mut state = RadioState::new(None);
        state.select(2);
        assert_eq!(state.selected, Some(2));
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn checklist_toggle() {
        let mut state = CheckListState::new(5);
        state.cursor = 2;
        state.toggle_current();
        assert!(state.checked[2]);
        state.toggle_current();
        assert!(!state.checked[2]);
    }

    #[test]
    fn segmented_selects() {
        let mut state = SegmentedState::new(0);
        state.hits = vec![Rect::default(); 3];
        let key = KeyEvent::from(KeyCode::Right);
        assert!(state.handle_key(key).is_changed());
        assert_eq!(state.selected, 1);
    }
}
