//! Checkbox, Switch, RadioGroup, CheckList and Segmented: Textual-faithful toggles.
//!
//! ```no_run
//! use tuile::prelude::*;
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

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Position, Rect};
use ratatui_core::style::Modifier;
use ratatui_core::widgets::StatefulWidget;
use unicode_width::UnicodeWidthStr;

use crate::anim::{Easing, Tween};
use crate::core::{Hit, HitBox, Interactive, Look, MinSize, Outcome, is_activate, is_press};
use crate::draw::{Border, LEFT_BLOCKS, fill, put, refuse, st};
use crate::theme::{self, Theme};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState, keep_visible};

use std::time::Duration;

// check styles

/// Visual style for checkboxes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckStyle {
    /// Painted `[X]` pill (default).
    #[default]
    Pill,
    /// `[ ]` `[x]` `[-]` bracket style.
    Bracket,
    /// `☐` `☑` `☒` box symbols.
    Box,
    /// `○` `●` `◐` circle symbols.
    Circle,
    /// Bare `✓` / `−` when on/indeterminate, blank when off.
    Check,
    /// `□` `■` `▣` square symbols.
    Square,
}
// checkbox

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
    style: CheckStyle,
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
            style: CheckStyle::default(),
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
        self.theme = Some(*th);
        self
    }

    pub fn style(mut self, s: CheckStyle) -> Self {
        self.style = s;
        self
    }
}

impl MinSize for Checkbox {
    fn min_size(&self) -> (u16, u16) {
        (3, 1)
    }
}

impl StatefulWidget for Checkbox {
    type State = CheckboxState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hit.set_area(area);
        let th = self.theme.unwrap_or_else(theme::current);
        if refuse(buf, area, self.min_size(), th.text_disabled) {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let look = Look {
            focused: self.focused,
            hover: state.hit.hover,
            enabled: self.enabled,
        };
        let bg = th.background;

        let (mark_w, box_x, label_x) = match self.style {
            CheckStyle::Pill => {
                let w = 3u16;
                let bx = if self.label_first {
                    area.x + self.label.width() as u16 + 1
                } else {
                    area.x
                };
                let lx = if self.label_first { area.x } else { area.x + w };
                (w, bx, lx)
            }
            CheckStyle::Bracket => {
                let w = 3u16;
                let bx = if self.label_first {
                    area.x + self.label.width() as u16 + 1
                } else {
                    area.x
                };
                let lx = if self.label_first { area.x } else { area.x + w };
                (w, bx, lx)
            }
            CheckStyle::Box | CheckStyle::Circle | CheckStyle::Check | CheckStyle::Square => {
                let w = 2u16;
                let bx = if self.label_first {
                    area.x + self.label.width() as u16 + 1
                } else {
                    area.x
                };
                let lx = if self.label_first { area.x } else { area.x + w };
                (w, bx, lx)
            }
        };

        match self.style {
            CheckStyle::Pill => {
                let mark = match state.value {
                    CheckState::On => "X",
                    CheckState::Off => " ",
                    CheckState::Indeterminate => "-",
                };
                let btn_bg = th.panel;
                let mut mark_fg = if state.value == CheckState::On {
                    th.text_success
                } else {
                    Theme::shade(th.panel, -2)
                };
                if !look.enabled {
                    mark_fg = mark_fg.blend(bg, 0.5);
                }

                let (side_fg, side_bg, btn) = if look.focused && self.label.is_empty() {
                    (th.cursor_bg, bg, th.cursor_bg)
                } else {
                    (btn_bg, bg, btn_bg)
                };

                put(buf, box_x, area.y, " ", 1, st(side_bg, side_fg));
                put(
                    buf,
                    box_x + 1,
                    area.y,
                    mark,
                    1,
                    st(mark_fg, btn).add_modifier(Modifier::BOLD),
                );
                put(buf, box_x + 2, area.y, " ", 1, st(side_bg, side_fg));
            }
            CheckStyle::Bracket => {
                let mark = match state.value {
                    CheckState::On => "[x]",
                    CheckState::Off => "[ ]",
                    CheckState::Indeterminate => "[-]",
                };
                let mut fg = if state.value == CheckState::On {
                    th.text_success
                } else {
                    th.text
                };
                if look.focused && self.label.is_empty() {
                    fg = th.cursor_fg;
                }
                if !look.enabled {
                    fg = fg.blend(bg, 0.5);
                }
                let mark_bg = if look.focused && self.label.is_empty() {
                    th.cursor_bg
                } else {
                    bg
                };
                let mark_style = if look.focused || state.value != CheckState::Off {
                    st(fg, mark_bg).add_modifier(Modifier::BOLD)
                } else {
                    st(fg, mark_bg)
                };
                put(buf, box_x, area.y, mark, 3, mark_style);
            }
            CheckStyle::Box => {
                let mark = match state.value {
                    CheckState::On => "☑ ",
                    CheckState::Off => "☐ ",
                    CheckState::Indeterminate => "☒ ",
                };
                let mut fg = if state.value == CheckState::On {
                    th.text_success
                } else {
                    th.text
                };
                if look.focused && self.label.is_empty() {
                    fg = th.cursor_fg;
                }
                if !look.enabled {
                    fg = fg.blend(bg, 0.5);
                }
                let mark_bg = if look.focused && self.label.is_empty() {
                    th.cursor_bg
                } else {
                    bg
                };
                put(
                    buf,
                    box_x,
                    area.y,
                    mark,
                    2,
                    st(fg, mark_bg).add_modifier(Modifier::BOLD),
                );
            }
            CheckStyle::Circle => {
                let mark = match state.value {
                    CheckState::On => "● ",
                    CheckState::Off => "○ ",
                    CheckState::Indeterminate => "◐ ",
                };
                let mut fg = if state.value == CheckState::On {
                    th.text_success
                } else {
                    th.text
                };
                if look.focused && self.label.is_empty() {
                    fg = th.cursor_fg;
                }
                if !look.enabled {
                    fg = fg.blend(bg, 0.5);
                }
                let mark_bg = if look.focused && self.label.is_empty() {
                    th.cursor_bg
                } else {
                    bg
                };
                put(
                    buf,
                    box_x,
                    area.y,
                    mark,
                    2,
                    st(fg, mark_bg).add_modifier(Modifier::BOLD),
                );
            }
            CheckStyle::Check => {
                let mark = match state.value {
                    CheckState::On => "✓ ",
                    CheckState::Off => "  ",
                    CheckState::Indeterminate => "− ",
                };
                let mut fg = if state.value != CheckState::Off {
                    th.text_success
                } else {
                    bg
                };
                if look.focused && self.label.is_empty() {
                    fg = th.cursor_fg;
                }
                if !look.enabled {
                    fg = fg.blend(bg, 0.5);
                }
                let mark_bg = if look.focused && self.label.is_empty() {
                    th.cursor_bg
                } else {
                    bg
                };
                put(
                    buf,
                    box_x,
                    area.y,
                    mark,
                    2,
                    st(fg, mark_bg).add_modifier(Modifier::BOLD),
                );
            }
            CheckStyle::Square => {
                let mark = match state.value {
                    CheckState::On => "■ ",
                    CheckState::Off => "□ ",
                    CheckState::Indeterminate => "▣ ",
                };
                let mut fg = if state.value == CheckState::On {
                    th.text_success
                } else {
                    th.text
                };
                if look.focused && self.label.is_empty() {
                    fg = th.cursor_fg;
                }
                if !look.enabled {
                    fg = fg.blend(bg, 0.5);
                }
                let mark_bg = if look.focused && self.label.is_empty() {
                    th.cursor_bg
                } else {
                    bg
                };
                put(
                    buf,
                    box_x,
                    area.y,
                    mark,
                    2,
                    st(fg, mark_bg).add_modifier(Modifier::BOLD),
                );
            }
        }

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
            let max_w = area.width.saturating_sub(if self.label_first {
                box_x - area.x
            } else {
                mark_w
            });
            let text = if self.label_first {
                self.label.clone()
            } else {
                format!(" {}", self.label)
            };
            put(buf, label_x, area.y, &text, max_w, style);
        }
    }
}

/// Off/on/indeterminate value and cached hit area; `tri_state` enables three-state cycling.
#[derive(Clone, Debug, Default)]
pub struct CheckboxState {
    pub value: CheckState,
    pub hit: HitBox,
    tri_state: bool,
}

impl CheckboxState {
    pub fn new(value: CheckState) -> Self {
        Self {
            value,
            hit: HitBox::default(),
            tri_state: false,
        }
    }

    pub fn toggle(&mut self) {
        self.value = match self.value {
            CheckState::Off => CheckState::On,
            CheckState::On => {
                if self.tri_state {
                    CheckState::Indeterminate
                } else {
                    CheckState::Off
                }
            }
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

// switch styles

/// Visual style for switches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwitchStyle {
    /// 4-cell animated track (default).
    #[default]
    Pill,
    /// 1-row 3-cell painted track with 1-cell painted thumb, animated.
    Slim,
    /// `━━●` / `●━━` box-drawing track, thumb `●`, on-colour vs muted.
    Line,
    /// `(●  )` ↔ `(  ●)` in parens, animated position.
    Round,
    /// Painted pill reading ` ON ` (success) / ` OFF ` (muted).
    Text,
    /// `✓` (success) / `✗` (muted) glyph.
    Check,
}

const SWITCH_W: u16 = 14;
// switch

pub struct Switch {
    label: Option<String>,
    compact: bool,
    duration: Option<Duration>,
    focused: bool,
    enabled: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
    style: SwitchStyle,
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
            style: SwitchStyle::default(),
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
        self.theme = Some(*th);
        self
    }

    pub fn style(mut self, s: SwitchStyle) -> Self {
        self.style = s;
        self
    }
}

impl Default for Switch {
    fn default() -> Self {
        Self::new()
    }
}
impl MinSize for Switch {
    fn min_size(&self) -> (u16, u16) {
        match self.style {
            SwitchStyle::Pill if self.compact => (SWITCH_W - 2, 1),
            SwitchStyle::Pill => (SWITCH_W, 3),
            SwitchStyle::Slim => (3, 1),
            SwitchStyle::Line => (3, 1),
            SwitchStyle::Round => (5, 1),
            SwitchStyle::Text => (6, 1),
            SwitchStyle::Check => (1, 1),
        }
    }
}

impl StatefulWidget for Switch {
    type State = SwitchState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hit.set_area(area);
        state.duration = self.duration.unwrap_or(Duration::from_millis(200));

        let th = self.theme.unwrap_or_else(theme::current);
        let look = Look {
            focused: self.focused,
            hover: state.hit.hover,
            enabled: self.enabled,
        };
        let t = state.anim.value(self.now.unwrap_or_else(Instant::now));

        if refuse(buf, area, self.min_size(), th.text_disabled) {
            return;
        }

        let bg = if !self.compact && look.focused && self.style == SwitchStyle::Pill {
            th.surface.blend(th.foreground, 0.05)
        } else {
            th.background
        };

        if self.style == SwitchStyle::Pill {
            fill(buf, area, bg);
            if !self.compact {
                let border = if look.focused {
                    th.border
                } else {
                    th.border_blurred
                };
                Border::Tall.draw(buf, area, border, bg);
            }
        }

        let track_y = if self.style == SwitchStyle::Pill && !self.compact {
            area.y + 1
        } else {
            area.y
        };
        let track_x = if self.style == SwitchStyle::Pill && !self.compact {
            area.x + 3
        } else {
            area.x
        };

        match self.style {
            SwitchStyle::Pill => {
                let track = if !self.compact && look.focused {
                    Theme::shade(th.panel, -2)
                } else {
                    Theme::shade(bg.blend(th.foreground, 0.1), -2)
                };
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
                    let Some(cell) = buf.cell_mut((track_x + i, track_y)) else {
                        continue;
                    };
                    if end <= cl || start >= cr {
                        cell.set_symbol(" ").set_bg(track.color());
                    } else if start <= cl && end >= cr {
                        cell.set_symbol(" ").set_bg(thumb.color());
                    } else if start > cl {
                        let idx = ((start - cl) * 8.0).round() as usize;
                        cell.set_symbol(LEFT_BLOCKS[idx.min(8)])
                            .set_fg(track.color())
                            .set_bg(thumb.color());
                    } else {
                        let idx = ((end - cl) * 8.0).round() as usize;
                        cell.set_symbol(LEFT_BLOCKS[idx.min(8)])
                            .set_fg(thumb.color())
                            .set_bg(track.color());
                    }
                }
            }
            SwitchStyle::Slim => {
                let track_bg = Theme::shade(th.panel, -2);
                let mut thumb_bg = if state.on { th.success } else { th.panel };
                if look.hover && look.enabled {
                    thumb_bg = Theme::shade(thumb_bg, 1);
                }
                if !look.enabled {
                    thumb_bg = thumb_bg.blend(bg, 0.5);
                }

                let thumb_pos = (t * 2.0).round() as u16;
                for i in 0..3u16 {
                    let cell_bg = if i == thumb_pos { thumb_bg } else { track_bg };
                    put(buf, track_x + i, track_y, " ", 1, st(cell_bg, cell_bg));
                }
            }
            SwitchStyle::Line => {
                let mut on_fg = th.success;
                let mut off_fg = th.text_muted;
                if !look.enabled {
                    on_fg = on_fg.blend(bg, 0.5);
                    off_fg = off_fg.blend(bg, 0.5);
                }
                let line_fg = if state.on { on_fg } else { off_fg };

                let thumb_pos = (t * 2.0).round() as u16;
                for i in 0..3u16 {
                    let sym = if i == thumb_pos { "●" } else { "━" };
                    put(buf, track_x + i, track_y, sym, 1, st(line_fg, bg));
                }
            }
            SwitchStyle::Round => {
                let mut fg = if state.on { th.success } else { th.text };
                if !look.enabled {
                    fg = fg.blend(bg, 0.5);
                }

                let thumb_pos = (t * 2.0).round() as u16 + 1;
                put(buf, track_x, track_y, "(", 1, st(fg, bg));
                for i in 1..4u16 {
                    let sym = if i == thumb_pos { "●" } else { " " };
                    put(buf, track_x + i, track_y, sym, 1, st(fg, bg));
                }
                put(buf, track_x + 4, track_y, ")", 1, st(fg, bg));
            }
            SwitchStyle::Text => {
                let (text, variant_bg) = if state.on {
                    (" ON ", th.success)
                } else {
                    (" OFF ", th.text_muted)
                };
                let mut pill_bg = variant_bg;
                if !look.enabled {
                    pill_bg = pill_bg.blend(bg, 0.5);
                }
                let fg = pill_bg.text_on(0.9);
                put(
                    buf,
                    track_x,
                    track_y,
                    text,
                    5,
                    st(fg, pill_bg).add_modifier(Modifier::BOLD),
                );
            }
            SwitchStyle::Check => {
                let (sym, mut fg) = if state.on {
                    ("✓", th.success)
                } else {
                    ("✗", th.text_muted)
                };
                if !look.enabled {
                    fg = fg.blend(bg, 0.5);
                }
                put(
                    buf,
                    track_x,
                    track_y,
                    sym,
                    1,
                    st(fg, bg).add_modifier(Modifier::BOLD),
                );
            }
        }

        if let Some(label) = &self.label {
            let label_x = track_x
                + match self.style {
                    SwitchStyle::Pill => 9,
                    SwitchStyle::Slim => 4,
                    SwitchStyle::Line => 4,
                    SwitchStyle::Round => 6,
                    SwitchStyle::Text => 6,
                    SwitchStyle::Check => 2,
                };
            let max_w = area.width.saturating_sub(label_x.saturating_sub(area.x));
            let fg = if look.enabled {
                th.text
            } else {
                th.text_disabled
            };
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
            self.anim
                .go_with(if on { 1.0 } else { 0.0 }, now, dur, Easing::InOutCubic);
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

// radio styles

/// Visual style for radio buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RadioStyle {
    /// Dot in painted pill (default).
    #[default]
    Dot,
    /// `( )` `(•)` bracket style.
    Bracket,
    /// `✓` on the selected row.
    Check,
    /// `❯` on the selected row, others indented.
    Arrow,
}

// radio group

pub struct RadioGroup {
    options: Vec<String>,
    horizontal: bool,
    bordered: bool,
    title: Option<String>,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
    style: RadioStyle,
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
            style: RadioStyle::default(),
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
        self.theme = Some(*th);
        self
    }

    pub fn style(mut self, s: RadioStyle) -> Self {
        self.style = s;
        self
    }
}

impl MinSize for RadioGroup {
    fn min_size(&self) -> (u16, u16) {
        (3, 1)
    }
}

impl StatefulWidget for RadioGroup {
    type State = RadioState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hits.clear();
        let th = self.theme.unwrap_or_else(theme::current);
        if self.options.is_empty() || refuse(buf, area, self.min_size(), th.text_disabled) {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let look = Look {
            focused: self.focused,
            hover: false,
            enabled: self.enabled,
        };
        let bg = th.background;

        let content = if self.bordered {
            let border_color = if look.focused {
                th.border
            } else {
                th.border_blurred
            };
            if let Some(title) = &self.title {
                let title_style = st(if look.focused { th.text } else { th.text_muted }, bg)
                    .add_modifier(Modifier::BOLD);
                Border::Tall.draw_titled_with(
                    buf,
                    area,
                    border_color,
                    bg,
                    title,
                    ratatui_core::layout::Alignment::Left,
                    title_style,
                )
            } else {
                Border::Tall.draw(buf, area, border_color, bg);
                Rect {
                    x: area.x + 1,
                    y: area.y + 1,
                    width: area.width.saturating_sub(2),
                    height: area.height.saturating_sub(2),
                }
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
                let r = Rect {
                    x,
                    y: content.y,
                    width: w,
                    height: 1,
                };
                state.hits.push(r);
                self.render_option(buf, r, idx, opt, state, &th, &look);
                x += w + 1;
            }
        } else {
            for (y, (idx, opt)) in (content.y..).zip(self.options.iter().enumerate()) {
                if y >= content.bottom() {
                    break;
                }
                let r = Rect {
                    x: content.x,
                    y,
                    width: content.width,
                    height: 1,
                };
                state.hits.push(r);
                self.render_option(buf, r, idx, opt, state, &th, &look);
            }
        }
    }
}

impl RadioGroup {
    // The render path takes buffer, position, size and style separately: bundling them into a
    // struct would cost an allocation per call.
    #[allow(clippy::too_many_arguments)]
    fn render_option(
        &self,
        buf: &mut Buffer,
        area: Rect,
        idx: usize,
        label: &str,
        state: &RadioState,
        th: &Theme,
        look: &Look,
    ) {
        let bg = th.background;
        let selected = state.selected == Some(idx);
        let is_cursor = state.cursor == idx;

        match self.style {
            RadioStyle::Dot => {
                let mark = if selected { "●" } else { " " };
                let btn_bg = th.panel;
                let mut mark_fg = if selected {
                    th.text_success
                } else {
                    Theme::shade(th.panel, -2)
                };
                if !look.enabled {
                    mark_fg = mark_fg.blend(bg, 0.5);
                }

                let (side_fg, side_bg, btn) = if is_cursor && look.focused {
                    (th.cursor_bg, bg, th.cursor_bg)
                } else {
                    (btn_bg, bg, btn_bg)
                };

                put(buf, area.x, area.y, " ", 1, st(side_bg, side_fg));
                put(
                    buf,
                    area.x + 1,
                    area.y,
                    mark,
                    1,
                    st(mark_fg, btn).add_modifier(Modifier::BOLD),
                );
                put(buf, area.x + 2, area.y, " ", 1, st(side_bg, side_fg));
            }
            RadioStyle::Bracket => {
                let mark = if selected { "(•)" } else { "( )" };
                let mut fg = if selected { th.text_success } else { th.text };
                if is_cursor && look.focused {
                    fg = th.cursor_fg;
                }
                if !look.enabled {
                    fg = fg.blend(bg, 0.5);
                }
                let mark_bg = if is_cursor && look.focused {
                    th.cursor_bg
                } else {
                    bg
                };
                let mark_style = if look.focused || selected {
                    st(fg, mark_bg).add_modifier(Modifier::BOLD)
                } else {
                    st(fg, mark_bg)
                };
                put(buf, area.x, area.y, mark, 3, mark_style);
            }
            RadioStyle::Check => {
                let mark = if selected { "✓ " } else { "  " };
                let mut fg = if selected { th.text_success } else { bg };
                if is_cursor && look.focused {
                    fg = th.cursor_fg;
                }
                if !look.enabled {
                    fg = fg.blend(bg, 0.5);
                }
                let mark_bg = if is_cursor && look.focused {
                    th.cursor_bg
                } else {
                    bg
                };
                put(
                    buf,
                    area.x,
                    area.y,
                    mark,
                    2,
                    st(fg, mark_bg).add_modifier(Modifier::BOLD),
                );
            }
            RadioStyle::Arrow => {
                let mark = if selected { "❯ " } else { "  " };
                let mut fg = if selected { th.text_success } else { bg };
                if is_cursor && look.focused {
                    fg = th.cursor_fg;
                }
                if !look.enabled {
                    fg = fg.blend(bg, 0.5);
                }
                let mark_bg = if is_cursor && look.focused {
                    th.cursor_bg
                } else {
                    bg
                };
                put(
                    buf,
                    area.x,
                    area.y,
                    mark,
                    2,
                    st(fg, mark_bg).add_modifier(Modifier::BOLD),
                );
            }
        }

        let style = if is_cursor && look.focused {
            st(th.cursor_fg, th.cursor_bg).add_modifier(Modifier::BOLD)
        } else if !look.enabled {
            st(th.text_disabled, bg)
        } else {
            st(th.text, bg)
        };
        let spacing = match self.style {
            RadioStyle::Dot | RadioStyle::Bracket => 3,
            _ => 2,
        };
        let max_w = area.width.saturating_sub(spacing);
        put(
            buf,
            area.x + spacing,
            area.y,
            &format!(" {}", label),
            max_w,
            style,
        );
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
        Self {
            selected,
            cursor: selected.unwrap_or(0),
            hits: vec![],
        }
    }

    pub fn select(&mut self, idx: usize) {
        self.selected = Some(idx);
        self.cursor = idx;
    }

    pub fn take_activated(&mut self) -> Option<usize> {
        self.selected.take()
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
                self.cursor = if self.cursor == 0 {
                    total - 1
                } else {
                    self.cursor - 1
                };
                Outcome::Consumed
            }
            KeyCode::Down | KeyCode::Right => {
                self.cursor = (self.cursor + 1) % total;
                Outcome::Consumed
            }
            _ if is_activate(&key) => {
                if self.selected != Some(self.cursor) {
                    self.selected = Some(self.cursor);
                    Outcome::Submitted
                } else {
                    Outcome::Consumed
                }
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let pos = Position {
            x: m.column,
            y: m.row,
        };
        if let Some(idx) = self.hits.iter().position(|r| r.contains(pos)) {
            match m.kind {
                crossterm::event::MouseEventKind::Down(_) => {
                    self.cursor = idx;
                    if self.selected != Some(idx) {
                        self.selected = Some(idx);
                        Outcome::Submitted
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

// checklist

/// Scrollable multi-select checkbox list.
#[derive(Clone, Debug)]
pub struct CheckList {
    options: Vec<String>,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
    now: Option<Instant>,
    style: CheckStyle,
}

impl CheckList {
    pub fn new(options: Vec<String>) -> Self {
        Self {
            options,
            focused: false,
            enabled: true,
            theme: None,
            now: None,
            style: CheckStyle::default(),
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
        self.theme = Some(*th);
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn style(mut self, s: CheckStyle) -> Self {
        self.style = s;
        self
    }
}

impl MinSize for CheckList {
    fn min_size(&self) -> (u16, u16) {
        (5, 3)
    }
}

impl StatefulWidget for CheckList {
    type State = CheckListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hits.clear();
        let th = self.theme.unwrap_or_else(theme::current);
        if self.options.is_empty() || refuse(buf, area, self.min_size(), th.text_disabled) {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let look = Look {
            focused: self.focused,
            hover: false,
            enabled: self.enabled,
        };
        let bg = th.background;

        let border_color = if look.focused {
            th.border
        } else {
            th.border_blurred
        };
        Border::Tall.draw(buf, area, border_color, bg);

        let content = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(3),
            height: area.height.saturating_sub(2),
        };
        let viewport = content.height as usize;
        state.scroll = keep_visible(state.scroll, state.cursor, viewport);

        for (i, idx) in (state.scroll..).take(viewport).enumerate() {
            if idx >= self.options.len() {
                break;
            }
            let y = content.y + i as u16;
            let r = Rect {
                x: content.x,
                y,
                width: content.width,
                height: 1,
            };
            state.hits.push((r, idx));

            let checked = state.checked.get(idx).copied().unwrap_or(false);
            let is_cursor = state.cursor == idx;

            match self.style {
                CheckStyle::Pill => {
                    let mark = if checked { "X" } else { " " };
                    let btn_bg = th.panel;
                    let mut mark_fg = if checked {
                        th.text_success
                    } else {
                        Theme::shade(th.panel, -2)
                    };
                    if !look.enabled {
                        mark_fg = mark_fg.blend(bg, 0.5);
                    }

                    let (side_fg, side_bg, btn) = if is_cursor && look.focused {
                        (th.cursor_bg, bg, th.cursor_bg)
                    } else {
                        (btn_bg, bg, btn_bg)
                    };

                    put(buf, r.x, y, " ", 1, st(side_bg, side_fg));
                    put(
                        buf,
                        r.x + 1,
                        y,
                        mark,
                        1,
                        st(mark_fg, btn).add_modifier(Modifier::BOLD),
                    );
                    put(buf, r.x + 2, y, " ", 1, st(side_bg, side_fg));
                }
                CheckStyle::Bracket => {
                    let mark = if checked { "[x]" } else { "[ ]" };
                    let mut fg = if checked { th.text_success } else { th.text };
                    if is_cursor && look.focused {
                        fg = th.cursor_fg;
                    }
                    if !look.enabled {
                        fg = fg.blend(bg, 0.5);
                    }
                    let mark_bg = if is_cursor && look.focused {
                        th.cursor_bg
                    } else {
                        bg
                    };
                    let mark_style = if look.focused || checked {
                        st(fg, mark_bg).add_modifier(Modifier::BOLD)
                    } else {
                        st(fg, mark_bg)
                    };
                    put(buf, r.x, y, mark, 3, mark_style);
                }
                CheckStyle::Box | CheckStyle::Circle | CheckStyle::Check | CheckStyle::Square => {
                    let mark = match self.style {
                        CheckStyle::Box => {
                            if checked {
                                "☑ "
                            } else {
                                "☐ "
                            }
                        }
                        CheckStyle::Circle => {
                            if checked {
                                "● "
                            } else {
                                "○ "
                            }
                        }
                        CheckStyle::Check => {
                            if checked {
                                "✓ "
                            } else {
                                "  "
                            }
                        }
                        CheckStyle::Square => {
                            if checked {
                                "■ "
                            } else {
                                "□ "
                            }
                        }
                        _ => unreachable!(),
                    };
                    let mut fg = if checked { th.text_success } else { th.text };
                    if is_cursor && look.focused {
                        fg = th.cursor_fg;
                    }
                    if !look.enabled {
                        fg = fg.blend(bg, 0.5);
                    }
                    let mark_bg = if is_cursor && look.focused {
                        th.cursor_bg
                    } else {
                        bg
                    };
                    put(
                        buf,
                        r.x,
                        y,
                        mark,
                        2,
                        st(fg, mark_bg).add_modifier(Modifier::BOLD),
                    );
                }
            }

            let style = if is_cursor && look.focused {
                st(th.cursor_fg, th.cursor_bg).add_modifier(Modifier::BOLD)
            } else if !look.enabled {
                st(th.text_disabled, bg)
            } else {
                st(th.text, bg)
            };
            let spacing = match self.style {
                CheckStyle::Pill | CheckStyle::Bracket => 3,
                _ => 2,
            };
            let max_w = r.width.saturating_sub(spacing);
            put(
                buf,
                r.x + spacing,
                y,
                &format!(" {}", self.options[idx]),
                max_w,
                style,
            );
        }

        if self.options.len() > viewport {
            let sb_area = Rect {
                x: area.right() - 1,
                y: area.y + 1,
                width: 1,
                height: area.height.saturating_sub(2),
            };
            Scrollbar::vertical(self.options.len(), viewport)
                .offset(state.scroll)
                .render(sb_area, buf, &mut state.sb);
        }
    }
}

/// One `checked` flag per option plus cursor, scroll offset and the row hits cached by `render`.
/// `checked` is resized to the option count on the first render.
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
                self.cursor = if self.cursor == 0 {
                    total - 1
                } else {
                    self.cursor - 1
                };
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

        let pos = Position {
            x: m.column,
            y: m.row,
        };
        if let Some(&(_, idx)) = self.hits.iter().find(|(r, _)| r.contains(pos)) {
            match m.kind {
                crossterm::event::MouseEventKind::Down(_) => {
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

// segmented styles

/// Visual style for segmented controls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentedStyle {
    /// Painted segments (default).
    #[default]
    Filled,
    /// `[ Day ] [ Week ]` brackets, selected bold + accent.
    Outline,
    /// Labels with a 1-row underline (`▔` or painted) under the selected one; needs 2 rows, falls back to Filled when only 1 row.
    Underline,
    /// Plain labels, selected in accent bold, separated by ` │ `.
    Text,
}
// segmented

/// Textual-style segmented control: horizontal pill row.
#[derive(Clone, Debug)]
pub struct Segmented {
    options: Vec<String>,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
    style: SegmentedStyle,
}

impl Segmented {
    pub fn new(options: Vec<String>) -> Self {
        Self {
            options,
            focused: false,
            enabled: true,
            theme: None,
            style: SegmentedStyle::default(),
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
        self.theme = Some(*th);
        self
    }

    pub fn style(mut self, s: SegmentedStyle) -> Self {
        self.style = s;
        self
    }
}

impl MinSize for Segmented {
    fn min_size(&self) -> (u16, u16) {
        (3, 1)
    }
}

impl StatefulWidget for Segmented {
    type State = SegmentedState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.hits.clear();
        let th = self.theme.unwrap_or_else(theme::current);
        if self.options.is_empty() || refuse(buf, area, self.min_size(), th.text_disabled) {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let look = Look {
            focused: self.focused,
            hover: false,
            enabled: self.enabled,
        };
        let bg = th.background;

        let effective_style = if self.style == SegmentedStyle::Underline && area.height < 2 {
            SegmentedStyle::Filled
        } else {
            self.style
        };

        match effective_style {
            SegmentedStyle::Filled => {
                let seg_bg = Theme::shade(th.surface, 1);
                let mut x = area.x;
                for (idx, opt) in self.options.iter().enumerate() {
                    let w = opt.width() as u16 + 2;
                    if x + w > area.right() {
                        break;
                    }
                    let r = Rect {
                        x,
                        y: area.y,
                        width: w,
                        height: 1,
                    };
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
                    put(
                        buf,
                        x,
                        area.y,
                        &format!(" {} ", opt),
                        w,
                        st(fg, item_bg).add_modifier(Modifier::BOLD),
                    );

                    x += w;
                    if idx + 1 < self.options.len() {
                        put(buf, x, area.y, " ", 1, st(bg, bg));
                        x += 1;
                    }
                }
            }
            SegmentedStyle::Outline => {
                let mut x = area.x;
                for (idx, opt) in self.options.iter().enumerate() {
                    let w = opt.width() as u16 + 4;
                    if x + w > area.right() {
                        break;
                    }
                    let r = Rect {
                        x,
                        y: area.y,
                        width: w,
                        height: 1,
                    };
                    state.hits.push(r);

                    let selected = state.selected == idx;
                    let mut fg = if selected { th.primary } else { th.text };
                    if !look.enabled {
                        fg = th.text_disabled;
                    }
                    let text = format!("[ {} ]", opt);
                    let style = if selected {
                        st(fg, bg).add_modifier(Modifier::BOLD)
                    } else {
                        st(fg, bg)
                    };
                    put(buf, x, area.y, &text, w, style);

                    x += w;
                    if idx + 1 < self.options.len() {
                        put(buf, x, area.y, " ", 1, st(bg, bg));
                        x += 1;
                    }
                }
            }
            SegmentedStyle::Underline => {
                let mut x = area.x;
                for (idx, opt) in self.options.iter().enumerate() {
                    let w = opt.width() as u16 + 2;
                    if x + w > area.right() {
                        break;
                    }
                    let r = Rect {
                        x,
                        y: area.y,
                        width: w,
                        height: 2,
                    };
                    state.hits.push(r);

                    let selected = state.selected == idx;
                    let mut fg = if selected { th.primary } else { th.text };
                    if !look.enabled {
                        fg = th.text_disabled;
                    }
                    let label_style = if selected {
                        st(fg, bg).add_modifier(Modifier::BOLD)
                    } else {
                        st(fg, bg)
                    };
                    put(buf, x, area.y, &format!(" {} ", opt), w, label_style);

                    if selected {
                        for i in 0..w {
                            put(buf, x + i, area.y + 1, "▔", 1, st(th.primary, bg));
                        }
                    }

                    x += w;
                    if idx + 1 < self.options.len() {
                        put(buf, x, area.y, " ", 1, st(bg, bg));
                        x += 1;
                    }
                }
            }
            SegmentedStyle::Text => {
                let mut x = area.x;
                for (idx, opt) in self.options.iter().enumerate() {
                    let w = opt.width() as u16;
                    if x + w > area.right() {
                        break;
                    }
                    let r = Rect {
                        x,
                        y: area.y,
                        width: w,
                        height: 1,
                    };
                    state.hits.push(r);

                    let selected = state.selected == idx;
                    let mut fg = if selected { th.primary } else { th.text };
                    if !look.enabled {
                        fg = th.text_disabled;
                    }
                    let style = if selected {
                        st(fg, bg).add_modifier(Modifier::BOLD)
                    } else {
                        st(fg, bg)
                    };
                    put(buf, x, area.y, opt, w, style);

                    x += w;
                    if idx + 1 < self.options.len() {
                        put(buf, x, area.y, " │ ", 3, st(th.text_muted, bg));
                        x += 3;
                    }
                }
            }
        }
    }
}

/// Selected segment index and per-segment hit areas from the last render.
#[derive(Clone, Debug, Default)]
pub struct SegmentedState {
    pub selected: usize,
    pub hits: Vec<Rect>,
}

impl SegmentedState {
    pub fn new(selected: usize) -> Self {
        Self {
            selected,
            hits: vec![],
        }
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
        let pos = Position {
            x: m.column,
            y: m.row,
        };
        if let Some(idx) = self.hits.iter().position(|r| r.contains(pos)) {
            match m.kind {
                crossterm::event::MouseEventKind::Down(_) => {
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

    #[test]
    fn checkbox_styles_render_without_panic() {
        let mut state = CheckboxState::new(CheckState::On);
        let area = Rect::new(0, 0, 20, 2);
        let mut buf = Buffer::empty(area);

        for style in [
            CheckStyle::Pill,
            CheckStyle::Bracket,
            CheckStyle::Box,
            CheckStyle::Circle,
            CheckStyle::Check,
            CheckStyle::Square,
        ] {
            buf.reset();
            Checkbox::new("Test")
                .style(style)
                .render(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn checkbox_bracket_shows_correct_marks() {
        let area = Rect::new(0, 0, 12, 1);
        let mut buf = Buffer::empty(area);

        let mut state = CheckboxState::new(CheckState::On);
        Checkbox::new("")
            .style(CheckStyle::Bracket)
            .render(area, &mut buf, &mut state);
        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "[");
        assert_eq!(buf.cell((1, 0)).unwrap().symbol(), "x");
        assert_eq!(buf.cell((2, 0)).unwrap().symbol(), "]");

        buf.reset();
        state.value = CheckState::Indeterminate;
        Checkbox::new("")
            .style(CheckStyle::Bracket)
            .render(area, &mut buf, &mut state);
        assert_eq!(buf.cell((1, 0)).unwrap().symbol(), "-");
    }

    #[test]
    fn switch_styles_render_without_panic() {
        let mut state = SwitchState::new(true);
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);

        for style in [
            SwitchStyle::Pill,
            SwitchStyle::Slim,
            SwitchStyle::Line,
            SwitchStyle::Round,
            SwitchStyle::Text,
            SwitchStyle::Check,
        ] {
            buf.reset();
            Switch::new()
                .style(style)
                .compact(true)
                .render(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn switch_text_shows_on_off() {
        let area = Rect::new(0, 0, 6, 1);
        let mut buf = Buffer::empty(area);

        let mut state = SwitchState::new(true);
        Switch::new()
            .style(SwitchStyle::Text)
            .render(area, &mut buf, &mut state);
        let text: String = (0..5).map(|x| buf.cell((x, 0)).unwrap().symbol()).collect();
        assert!(text.contains("ON"));

        buf.reset();
        state.on = false;
        Switch::new()
            .style(SwitchStyle::Text)
            .render(area, &mut buf, &mut state);
        let text: String = (0..5).map(|x| buf.cell((x, 0)).unwrap().symbol()).collect();
        assert!(text.contains("OFF"));
    }

    #[test]
    fn radio_styles_render_without_panic() {
        let mut state = RadioState::new(Some(1));
        let area = Rect::new(0, 0, 20, 3);
        let mut buf = Buffer::empty(area);

        for style in [
            RadioStyle::Dot,
            RadioStyle::Bracket,
            RadioStyle::Check,
            RadioStyle::Arrow,
        ] {
            buf.reset();
            RadioGroup::new(vec!["A".into(), "B".into(), "C".into()])
                .style(style)
                .render(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn segmented_styles_render_without_panic() {
        let mut state = SegmentedState::new(1);
        let area = Rect::new(0, 0, 30, 2);
        let mut buf = Buffer::empty(area);

        for style in [
            SegmentedStyle::Filled,
            SegmentedStyle::Outline,
            SegmentedStyle::Underline,
            SegmentedStyle::Text,
        ] {
            buf.reset();
            Segmented::new(vec!["Day".into(), "Week".into(), "Month".into()])
                .style(style)
                .render(area, &mut buf, &mut state);
        }
    }

    fn painted(buf: &Buffer) -> bool {
        buf.content().iter().any(|c| c.symbol() != " ")
    }

    #[test]
    fn checkbox_draws_at_minimum_and_refuses_below_it() {
        let cb = Checkbox::new("");
        let (w, h) = cb.min_size();
        assert_eq!((w, h), (3, 1));

        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        let mut state = CheckboxState::new(CheckState::On);
        Checkbox::new("").render(buf.area, &mut buf, &mut state);
        assert!(painted(&buf), "should draw at minimum");

        let mut buf = Buffer::empty(Rect::new(0, 0, w - 1, h));
        Checkbox::new("").render(buf.area, &mut buf, &mut state);
        assert!(
            buf.content().iter().any(|c| c.symbol() == "⋯"),
            "one col short must refuse"
        );
    }

    #[test]
    fn radio_activation_returns_submitted() {
        let mut state = RadioState {
            hits: vec![Rect::new(0, 0, 5, 1)],
            ..Default::default()
        };

        let out = state.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(out.is_submitted());
        assert_eq!(state.take_activated(), Some(0));
    }
}
