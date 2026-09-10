//! Shared vocabulary every widget speaks: event outcomes, interaction state (`Look`),
//! focus rings, hit boxes for mouse handling, and small key/mouse helpers.

use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};

// ───────────────────────────── outcome ─────────────────────────────

/// Result of feeding an event to a widget state.
///
/// `Ignored` → the widget did nothing, let someone else handle it.
/// `Consumed` → handled, redraw is enough (hover moved, cursor blinked, scrolled).
/// `Changed` → the *value* the widget owns changed (toggle flipped, text edited, row picked).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Outcome {
    #[default]
    Ignored,
    Consumed,
    Changed,
}

impl Outcome {
    pub fn is_ignored(self) -> bool {
        self == Outcome::Ignored
    }
    /// `Consumed` or `Changed`.
    pub fn is_consumed(self) -> bool {
        self != Outcome::Ignored
    }
    pub fn is_changed(self) -> bool {
        self == Outcome::Changed
    }
    /// `true` → `Changed`, `false` → `Consumed`.
    pub fn changed_if(flag: bool) -> Outcome {
        if flag {
            Outcome::Changed
        } else {
            Outcome::Consumed
        }
    }
}

impl std::ops::BitOr for Outcome {
    type Output = Outcome;
    fn bitor(self, rhs: Outcome) -> Outcome {
        self.max(rhs)
    }
}

impl std::ops::BitOrAssign for Outcome {
    fn bitor_assign(&mut self, rhs: Outcome) {
        *self = (*self).max(rhs);
    }
}

// ───────────────────────────── look ─────────────────────────────

/// Interaction state that drives a widget's styling (Textual `:focus`, `:hover`, `:disabled`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    pub focused: bool,
    pub hover: bool,
    pub enabled: bool,
}

impl Default for Look {
    fn default() -> Self {
        Look::PLAIN
    }
}

impl Look {
    pub const PLAIN: Look = Look {
        focused: false,
        hover: false,
        enabled: true,
    };

    pub const fn new() -> Self {
        Look::PLAIN
    }
    pub const fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }
    pub const fn hover(mut self, v: bool) -> Self {
        self.hover = v;
        self
    }
    pub const fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }
    /// Hover only counts when the widget is enabled.
    pub const fn hovering(&self) -> bool {
        self.hover && self.enabled
    }
}

// ───────────────────────────── focus ring ─────────────────────────────

/// Tab-order over a list of ids (any `Copy + PartialEq`, typically a small enum).
///
/// ```
/// # use tuiforge::core::Focus;
/// #[derive(Clone, Copy, PartialEq, Debug)] enum Id { Name, Email, Save }
/// let mut f = Focus::new([Id::Name, Id::Email, Id::Save]);
/// f.next(); assert!(f.is(Id::Email));
/// f.prev(); f.prev(); assert!(f.is(Id::Save));
/// ```
#[derive(Clone, Debug)]
pub struct Focus<T> {
    order: Vec<T>,
    current: usize,
    /// Wrap around at both ends (default `true`).
    pub wrap: bool,
}

impl<T: Copy + PartialEq> Focus<T> {
    pub fn new(order: impl Into<Vec<T>>) -> Self {
        Focus {
            order: order.into(),
            current: 0,
            wrap: true,
        }
    }

    /// Replace the tab order (e.g. when a section collapses); keeps the current id if it survives.
    pub fn set_order(&mut self, order: impl Into<Vec<T>>) {
        let cur = self.current();
        self.order = order.into();
        self.current = cur
            .and_then(|c| self.order.iter().position(|&x| x == c))
            .unwrap_or(0);
    }

    pub fn order(&self) -> &[T] {
        &self.order
    }

    pub fn current(&self) -> Option<T> {
        self.order.get(self.current).copied()
    }

    pub fn is(&self, id: T) -> bool {
        self.current() == Some(id)
    }

    pub fn set(&mut self, id: T) -> bool {
        match self.order.iter().position(|&x| x == id) {
            Some(i) => {
                self.current = i;
                true
            }
            None => false,
        }
    }

    pub fn index(&self) -> usize {
        self.current
    }

    pub fn set_index(&mut self, i: usize) {
        if !self.order.is_empty() {
            self.current = i.min(self.order.len() - 1);
        }
    }

    pub fn next(&mut self) {
        self.step(1);
    }

    pub fn prev(&mut self) {
        self.step(-1);
    }

    fn step(&mut self, d: i32) {
        let n = self.order.len() as i32;
        if n == 0 {
            return;
        }
        let next = self.current as i32 + d;
        self.current = if self.wrap {
            next.rem_euclid(n)
        } else {
            next.clamp(0, n - 1)
        } as usize;
    }

    /// `Tab` / `Shift+Tab` navigation; returns `Consumed` when the key was one of those.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Tab => {
                self.next();
                Outcome::Consumed
            }
            KeyCode::BackTab => {
                self.prev();
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }
}

// ───────────────────────────── mouse ─────────────────────────────

pub fn mouse_pos(m: &MouseEvent) -> Position {
    Position {
        x: m.column,
        y: m.row,
    }
}

pub fn mouse_in(area: Rect, m: &MouseEvent) -> bool {
    area.contains(mouse_pos(m))
}

pub fn is_left_down(m: &MouseEvent) -> bool {
    m.kind == MouseEventKind::Down(MouseButton::Left)
}

pub fn is_left_up(m: &MouseEvent) -> bool {
    m.kind == MouseEventKind::Up(MouseButton::Left)
}

pub fn is_left_drag(m: &MouseEvent) -> bool {
    m.kind == MouseEventKind::Drag(MouseButton::Left)
}

pub fn is_move(m: &MouseEvent) -> bool {
    m.kind == MouseEventKind::Moved
}

/// `+1` for wheel down, `-1` for wheel up, `None` otherwise.
pub fn wheel_delta(m: &MouseEvent) -> Option<i32> {
    match m.kind {
        MouseEventKind::ScrollDown => Some(1),
        MouseEventKind::ScrollUp => Some(-1),
        _ => None,
    }
}

/// What a [`HitBox`] observed for one mouse event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    None,
    /// Pointer entered or left the box (hover flag flipped).
    HoverChanged,
    /// Left button pressed inside.
    Press,
    /// Left button dragged while pressed here.
    Drag,
    /// Left button released inside after a press inside — a click.
    Click,
    /// Press was cancelled (released outside).
    Cancel,
    /// Mouse wheel over the box, `+1` down / `-1` up.
    Wheel(i32),
}

/// Hover/press tracker for one rectangle. Store it in a widget state, call
/// [`HitBox::set_area`] on render and [`HitBox::mouse`] on every mouse event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HitBox {
    pub area: Rect,
    pub hover: bool,
    pub pressed: bool,
}

impl HitBox {
    pub fn set_area(&mut self, area: Rect) {
        self.area = area;
    }

    pub fn contains(&self, m: &MouseEvent) -> bool {
        mouse_in(self.area, m)
    }

    pub fn mouse(&mut self, m: &MouseEvent) -> Hit {
        let inside = self.contains(m);
        match m.kind {
            MouseEventKind::Moved => {
                if inside != self.hover {
                    self.hover = inside;
                    Hit::HoverChanged
                } else {
                    Hit::None
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let was = self.hover;
                self.hover = inside;
                if inside {
                    self.pressed = true;
                    Hit::Press
                } else if was {
                    Hit::HoverChanged
                } else {
                    Hit::None
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.pressed {
                    Hit::Drag
                } else if inside != self.hover {
                    self.hover = inside;
                    Hit::HoverChanged
                } else {
                    Hit::None
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if self.pressed {
                    self.pressed = false;
                    if inside { Hit::Click } else { Hit::Cancel }
                } else {
                    Hit::None
                }
            }
            MouseEventKind::ScrollDown if inside => Hit::Wheel(1),
            MouseEventKind::ScrollUp if inside => Hit::Wheel(-1),
            _ => Hit::None,
        }
    }

    /// Current interaction look, combined with an externally-owned focus flag.
    pub fn look(&self, focused: bool, enabled: bool) -> Look {
        Look {
            focused,
            hover: self.hover,
            enabled,
        }
    }
}

// ───────────────────────────── keys ─────────────────────────────

/// Key presses only (crossterm may also report repeats/releases with the kitty protocol).
pub fn is_press(k: &KeyEvent) -> bool {
    k.kind != KeyEventKind::Release
}

pub fn ctrl(k: &KeyEvent, c: char) -> bool {
    k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char(c)
}

pub fn alt(k: &KeyEvent, c: char) -> bool {
    k.modifiers.contains(KeyModifiers::ALT) && k.code == KeyCode::Char(c)
}

pub fn plain_char(k: &KeyEvent) -> Option<char> {
    match k.code {
        KeyCode::Char(c)
            if !k
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            Some(c)
        }
        _ => None,
    }
}

/// `Enter` or `Space` — the universal "activate" keys.
pub fn is_activate(k: &KeyEvent) -> bool {
    matches!(k.code, KeyCode::Enter | KeyCode::Char(' '))
}

/// Human-readable key label for footers/help (`^S`, `⇧Tab`, `F5`, `Esc`).
pub fn key_label(k: &KeyEvent) -> String {
    let mut s = String::new();
    if k.modifiers.contains(KeyModifiers::CONTROL) {
        s.push('^');
    }
    if k.modifiers.contains(KeyModifiers::ALT) {
        s.push_str("Alt+");
    }
    if k.modifiers.contains(KeyModifiers::SHIFT)
        && !matches!(k.code, KeyCode::Char(_) | KeyCode::BackTab)
    {
        s.push('⇧');
    }
    match k.code {
        KeyCode::Char(' ') => s.push_str("Space"),
        KeyCode::Char(c) => s.push(c),
        KeyCode::F(n) => s.push_str(&format!("F{n}")),
        KeyCode::Esc => s.push_str("Esc"),
        KeyCode::Enter => s.push_str("Enter"),
        KeyCode::BackTab => s.push_str("⇧Tab"),
        KeyCode::Tab => s.push_str("Tab"),
        KeyCode::Backspace => s.push_str("Bksp"),
        KeyCode::Up => s.push('↑'),
        KeyCode::Down => s.push('↓'),
        KeyCode::Left => s.push('←'),
        KeyCode::Right => s.push('→'),
        KeyCode::PageUp => s.push_str("PgUp"),
        KeyCode::PageDown => s.push_str("PgDn"),
        other => s.push_str(&format!("{other:?}")),
    }
    s
}

/// Widgets with state implement this so apps can forward raw events with one call.
pub trait Interactive {
    fn handle_key(&mut self, _key: KeyEvent) -> Outcome {
        Outcome::Ignored
    }
    fn handle_mouse(&mut self, _m: MouseEvent) -> Outcome {
        Outcome::Ignored
    }
    fn handle(&mut self, ev: &Event) -> Outcome {
        match ev {
            Event::Key(k) if is_press(k) => self.handle_key(*k),
            Event::Mouse(m) => self.handle_mouse(*m),
            _ => Outcome::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn me(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn hitbox_click_requires_press_and_release_inside() {
        let mut h = HitBox {
            area: Rect::new(5, 5, 10, 3),
            ..Default::default()
        };
        assert_eq!(h.mouse(&me(MouseEventKind::Moved, 6, 6)), Hit::HoverChanged);
        assert_eq!(h.mouse(&me(MouseEventKind::Moved, 7, 6)), Hit::None);
        assert_eq!(
            h.mouse(&me(MouseEventKind::Down(MouseButton::Left), 7, 6)),
            Hit::Press
        );
        assert_eq!(
            h.mouse(&me(MouseEventKind::Up(MouseButton::Left), 40, 40)),
            Hit::Cancel
        );
        h.mouse(&me(MouseEventKind::Down(MouseButton::Left), 7, 6));
        assert_eq!(
            h.mouse(&me(MouseEventKind::Up(MouseButton::Left), 8, 7)),
            Hit::Click
        );
        assert_eq!(h.mouse(&me(MouseEventKind::Moved, 0, 0)), Hit::HoverChanged);
        assert!(!h.hover);
    }

    #[test]
    fn outcome_combines_upwards() {
        assert_eq!(Outcome::Ignored | Outcome::Consumed, Outcome::Consumed);
        assert_eq!(Outcome::Changed | Outcome::Consumed, Outcome::Changed);
        let mut o = Outcome::Ignored;
        o |= Outcome::Changed;
        assert!(o.is_changed());
    }

    #[test]
    fn focus_wraps_and_survives_reorder() {
        let mut f = Focus::new(['a', 'b', 'c']);
        f.prev();
        assert!(f.is('c'));
        f.set_order(['c', 'd']);
        assert!(f.is('c'));
        f.wrap = false;
        f.next();
        f.next();
        assert!(f.is('d'));
    }
}
