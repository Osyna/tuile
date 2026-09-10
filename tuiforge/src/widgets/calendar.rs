//! Calendar and date picker widgets.
//!
//! ```
//! # use tuiforge::prelude::*;
//! # let mut buf = Buffer::empty(Rect::new(0, 0, 25, 10));
//! # let mut state = CalendarState::default();
//! Calendar::new().render(Rect::new(0, 0, 25, 10), &mut buf, &mut state);
//! ```

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::StatefulWidget;

use crate::core::{Hit, HitBox, Interactive, Outcome, ctrl, is_activate, is_press, mouse_in};
use crate::draw::{fill, put, put_centered, st};
use crate::layout::popup_below;
use crate::runtime::{civil_from_days, days_from_civil, local_ymd};
use crate::theme::{self, Theme, Variant};

// ─────────────────────────────────────────────────────────────────────────────
// Calendar
// ─────────────────────────────────────────────────────────────────────────────

/// Day of week for `.week_start`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weekday {
    Sunday,
    Monday,
}

/// Calendar state with cursor and selection.
#[derive(Clone, Debug)]
pub struct CalendarState {
    pub year: i32,
    pub month: u32,
    pub cursor_day: u32,
    pub selected: Option<(i32, u32, u32)>,
    pub today: (i32, u32, u32),
    pub hits: Vec<HitBox>, // header arrows + day cells
    prev_arrow: Rect,
    next_arrow: Rect,
    day_rects: Vec<Rect>,
}

impl Default for CalendarState {
    fn default() -> Self {
        let today = local_ymd();
        Self {
            year: today.0,
            month: today.1,
            cursor_day: today.2,
            selected: None,
            today,
            hits: Vec::new(),
            prev_arrow: Rect::default(),
            next_arrow: Rect::default(),
            day_rects: Vec::new(),
        }
    }
}

impl CalendarState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_today(&mut self, y: i32, m: u32, d: u32) {
        self.today = (y, m, d);
    }

    fn prev_month(&mut self) {
        if self.month == 1 {
            self.month = 12;
            self.year -= 1;
        } else {
            self.month -= 1;
        }
        self.cursor_day = self.cursor_day.min(days_in_month(self.year, self.month));
    }

    fn next_month(&mut self) {
        if self.month == 12 {
            self.month = 1;
            self.year += 1;
        } else {
            self.month += 1;
        }
        self.cursor_day = self.cursor_day.min(days_in_month(self.year, self.month));
    }

    fn prev_year(&mut self) {
        self.year -= 1;
        self.cursor_day = self.cursor_day.min(days_in_month(self.year, self.month));
    }

    fn next_year(&mut self) {
        self.year += 1;
        self.cursor_day = self.cursor_day.min(days_in_month(self.year, self.month));
    }

    fn move_cursor(&mut self, dy: i32) {
        let current_days = days_from_civil(self.year, self.month, self.cursor_day);
        let new_days = current_days + dy as i64;
        let (y, m, d) = civil_from_days(new_days);
        self.year = y;
        self.month = m;
        self.cursor_day = d;
    }

    fn select_cursor(&mut self) -> Outcome {
        self.selected = Some((self.year, self.month, self.cursor_day));
        Outcome::Changed
    }
}

impl Interactive for CalendarState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        match key.code {
            KeyCode::Left => {
                self.move_cursor(-1);
                Outcome::Consumed
            }
            KeyCode::Right => {
                self.move_cursor(1);
                Outcome::Consumed
            }
            KeyCode::Up => {
                self.move_cursor(-7);
                Outcome::Consumed
            }
            KeyCode::Down => {
                self.move_cursor(7);
                Outcome::Consumed
            }
            KeyCode::PageUp if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.prev_year();
                Outcome::Consumed
            }
            KeyCode::PageUp => {
                self.prev_month();
                Outcome::Consumed
            }
            KeyCode::PageDown if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.next_year();
                Outcome::Consumed
            }
            KeyCode::PageDown => {
                self.next_month();
                Outcome::Consumed
            }
            KeyCode::Home => {
                self.cursor_day = 1;
                Outcome::Consumed
            }
            KeyCode::End => {
                self.cursor_day = days_in_month(self.year, self.month);
                Outcome::Consumed
            }
            KeyCode::Char('t') => {
                self.year = self.today.0;
                self.month = self.today.1;
                self.cursor_day = self.today.2;
                Outcome::Consumed
            }
            _ if is_activate(&key) => self.select_cursor(),
            _ if ctrl(&key, 'p') => {
                self.prev_month();
                Outcome::Consumed
            }
            _ if ctrl(&key, 'n') => {
                self.next_month();
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let down = matches!(m.kind, ratatui::crossterm::event::MouseEventKind::Down(_));

        // check arrows
        if down && mouse_in(self.prev_arrow, &m) {
            self.prev_month();
            return Outcome::Consumed;
        }
        if down && mouse_in(self.next_arrow, &m) {
            self.next_month();
            return Outcome::Consumed;
        }

        // day cells are pushed in day order during render: index + 1 == day of month
        if down && let Some(idx) = self.day_rects.iter().position(|r| mouse_in(*r, &m)) {
            self.cursor_day = idx as u32 + 1;
            return self.select_cursor();
        }

        // wheel changes month
        if let Some(delta) = crate::core::wheel_delta(&m) {
            if delta > 0 {
                self.next_month();
            } else {
                self.prev_month();
            }
            return Outcome::Consumed;
        }

        Outcome::Ignored
    }
}

/// Calendar widget.
#[derive(Clone, Debug)]
pub struct Calendar {
    week_start: Weekday,
    show_adjacent: bool,
    show_week_numbers: bool,
    min_date: Option<(i32, u32, u32)>,
    max_date: Option<(i32, u32, u32)>,
    disabled_fn: Option<fn(i32, u32, u32) -> bool>,
    marks: Vec<(i32, u32, u32, Variant)>,
    focused: bool,
    theme: Option<Theme>,
}

impl Calendar {
    pub fn new() -> Self {
        Self {
            week_start: Weekday::Sunday,
            show_adjacent: true,
            show_week_numbers: false,
            min_date: None,
            max_date: None,
            disabled_fn: None,
            marks: Vec::new(),
            focused: false,
            theme: None,
        }
    }

    pub fn week_start(mut self, w: Weekday) -> Self {
        self.week_start = w;
        self
    }
    pub fn show_adjacent(mut self, v: bool) -> Self {
        self.show_adjacent = v;
        self
    }
    pub fn show_week_numbers(mut self, v: bool) -> Self {
        self.show_week_numbers = v;
        self
    }
    pub fn min(mut self, y: i32, m: u32, d: u32) -> Self {
        self.min_date = Some((y, m, d));
        self
    }
    pub fn max(mut self, y: i32, m: u32, d: u32) -> Self {
        self.max_date = Some((y, m, d));
        self
    }
    pub fn disabled(mut self, f: fn(i32, u32, u32) -> bool) -> Self {
        self.disabled_fn = Some(f);
        self
    }
    pub fn marks(mut self, m: &[(i32, u32, u32, Variant)]) -> Self {
        self.marks = m.to_vec();
        self
    }
    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Default for Calendar {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for Calendar {
    type State = CalendarState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < 20 || area.height < 8 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.surface);

        state.day_rects.clear();

        // Header: ◀ September 2026 ▶
        let month_name = month_name(state.month);
        let header_text = format!("{} {}", month_name, state.year);
        let header_w = header_text.len() as u16;
        let header_x = area.x + (area.width / 2).saturating_sub(header_w / 2);
        put(buf, header_x, area.y, &header_text, header_w.min(area.width), st(th.text, th.surface));

        // arrows
        let arrow_y = area.y;
        let left_arrow_x = area.x + 1;
        let right_arrow_x = area.x + area.width.saturating_sub(2);
        put(buf, left_arrow_x, arrow_y, "◀", 1, st(th.text_muted, th.surface));
        put(buf, right_arrow_x, arrow_y, "▶", 1, st(th.text_muted, th.surface));
        state.prev_arrow = Rect { x: left_arrow_x, y: arrow_y, width: 1, height: 1 };
        state.next_arrow = Rect { x: right_arrow_x, y: arrow_y, width: 1, height: 1 };

        // Weekday header
        let weekday_y = area.y + 1;
        let weekdays = if self.week_start == Weekday::Monday {
            ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
        } else {
            ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]
        };

        let cell_w = 3u16;
        let grid_w = cell_w * 7;
        let grid_x = area.x + (area.width / 2).saturating_sub(grid_w / 2);

        for (i, day) in weekdays.iter().enumerate() {
            let x = grid_x + (i as u16 * cell_w);
            put_centered(buf, Rect { x, y: weekday_y, width: cell_w, height: 1 }, day, st(th.text_muted, th.surface));
        }

        // Calendar grid
        let grid_y = weekday_y + 1;
        let first_day = days_from_civil(state.year, state.month, 1);
        let weekday_offset = ((first_day + 4).rem_euclid(7)) as u32; // 0 = Sunday
        let offset = if self.week_start == Weekday::Monday {
            (weekday_offset + 6) % 7
        } else {
            weekday_offset
        };

        let days_in = days_in_month(state.year, state.month);
        let total_cells = (offset + days_in).div_ceil(7) * 7;

        for cell_idx in 0..total_cells.min(42) {
            let row = cell_idx / 7;
            let col = cell_idx % 7;
            let y = grid_y + row as u16;
            if y >= area.y + area.height {
                break;
            }
            let x = grid_x + (col as u16 * cell_w);
            let cell_rect = Rect { x, y, width: cell_w, height: 1 };

            if cell_idx < offset {
                // prev month
                if self.show_adjacent {
                    let prev_month_days = if state.month == 1 {
                        days_in_month(state.year - 1, 12)
                    } else {
                        days_in_month(state.year, state.month - 1)
                    };
                    let day = prev_month_days - (offset - cell_idx - 1);
                    put_centered(buf, cell_rect, &day.to_string(), st(th.text_disabled, th.surface));
                }
            } else if cell_idx < offset + days_in {
                let day = cell_idx - offset + 1;
                state.day_rects.push(cell_rect);

                let is_today = state.today.0 == state.year && state.today.1 == state.month && state.today.2 == day;
                let is_selected = state.selected == Some((state.year, state.month, day));
                let is_cursor = day == state.cursor_day;

                let mut fg = th.text;
                let mut bg = th.surface;
                let mut style_mod = Modifier::empty();

                // Check for marks first to tint the foreground
                let mut has_mark = false;
                for (my, mm, md, v) in &self.marks {
                    if *my == state.year && *mm == state.month && *md == day {
                        fg = th.text_variant(*v);
                        has_mark = true;
                        break;
                    }
                }

                if is_selected {
                    bg = th.cursor_bg;
                    fg = th.cursor_fg;
                    style_mod |= Modifier::BOLD;
                } else if is_today {
                    fg = th.accent;
                    style_mod |= Modifier::BOLD;
                } else if has_mark {
                    style_mod |= Modifier::BOLD;
                }

                if is_cursor && self.focused {
                    bg = th.focus_bg();
                    style_mod |= Modifier::UNDERLINED;
                }

                fill(buf, cell_rect, bg);
                let day_str = day.to_string();
                put_centered(buf, cell_rect, &day_str, st(fg, bg).add_modifier(style_mod));
            } else if self.show_adjacent {
                // next month
                let day = cell_idx - offset - days_in + 1;
                put_centered(buf, cell_rect, &day.to_string(), st(th.text_disabled, th.surface));
            }
        }
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn month_name(m: u32) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "?",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DatePicker
// ─────────────────────────────────────────────────────────────────────────────

/// Date picker state with popup calendar.
#[derive(Clone, Debug, Default)]
pub struct DatePickerState {
    pub open: bool,
    pub cal: CalendarState,
    pub hit: HitBox,
    /// Popup rect from the last `render_overlay`; clicks outside it close the picker.
    pub popup: Rect,
}

impl DatePickerState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Currently selected date, if any.
    pub fn selected(&self) -> Option<(i32, u32, u32)> {
        self.cal.selected
    }
}

impl Interactive for DatePickerState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        if self.open {
            if key.code == KeyCode::Esc {
                self.open = false;
                return Outcome::Consumed;
            }
            let out = self.cal.handle_key(key);
            if out.is_changed() {
                self.open = false;
            }
            out
        } else {
            if is_activate(&key) {
                self.open = !self.open;
                Outcome::Consumed
            } else {
                Outcome::Ignored
            }
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        if self.open {
            let out = self.cal.handle_mouse(m);
            if out.is_changed() {
                self.open = false;
                return out;
            }
            // click outside the popup and the field closes it
            let down = matches!(m.kind, ratatui::crossterm::event::MouseEventKind::Down(_));
            if out == Outcome::Ignored && down && !mouse_in(self.popup, &m) && !mouse_in(self.hit.area, &m) {
                self.open = false;
                return Outcome::Consumed;
            }
            out
        } else if self.hit.mouse(&m) == Hit::Click {
            self.open = true;
            Outcome::Consumed
        } else {
            Outcome::Ignored
        }
    }
}

/// Date picker with popup calendar.
#[derive(Clone, Debug)]
pub struct DatePicker {
    focused: bool,
    theme: Option<Theme>,
}

impl DatePicker {
    pub fn new() -> Self {
        Self { focused: false, theme: None }
    }
    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Render the overlay calendar. Call this at the end of the page draw.
    pub fn render_overlay(&self, state: &mut DatePickerState, buf: &mut Buffer, bounds: Rect) {
        if !state.open {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let cal_w = 25;
        let cal_h = 10;
        let popup_area = popup_below(state.hit.area, cal_w, cal_h, bounds);
        state.popup = popup_area;

        // dim background
        crate::draw::blend_area(buf, popup_area, th.panel, 0.9);

        Calendar::new().focused(self.focused).theme(&th).render(popup_area, buf, &mut state.cal);
    }
}

impl Default for DatePicker {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for DatePicker {
    type State = DatePickerState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < 12 || area.height < 1 {
            return;
        }
        state.hit.set_area(area);
        let th = self.theme.unwrap_or_else(theme::current);
        let bg = th.surface;
        let border = if self.focused || state.open { th.border } else { th.border_blurred };

        fill(buf, area, bg);
        let text_y = if area.height >= 3 {
            crate::draw::Border::Tall.draw(buf, area, border, bg);
            area.y + area.height / 2
        } else {
            area.y
        };

        let text = if let Some((y, m, d)) = state.cal.selected {
            format!("{:04}-{:02}-{:02}", y, m, d)
        } else {
            "Select date".to_string()
        };
        let fg = if state.cal.selected.is_some() { th.text } else { th.text_muted };
        let display = crate::draw::truncate(&text, area.width.saturating_sub(5) as usize);
        put(buf, area.x + 2, text_y, &display, area.width.saturating_sub(5), st(fg, bg));
        put(buf, area.right().saturating_sub(3), text_y, "▾", 1, st(th.text_muted, bg));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_in_month_correct() {
        assert_eq!(days_in_month(2024, 2), 29); // leap
        assert_eq!(days_in_month(2023, 2), 28);
        assert_eq!(days_in_month(2023, 1), 31);
        assert_eq!(days_in_month(2023, 4), 30);
    }

    #[test]
    fn calendar_nav() {
        let mut state = CalendarState {
            year: 2024,
            month: 1,
            cursor_day: 15,
            ..Default::default()
        };
        state.next_month();
        assert_eq!(state.month, 2);
        state.prev_month();
        assert_eq!(state.month, 1);
    }

    #[test]
    fn calendar_year_wrap() {
        let mut state = CalendarState {
            year: 2024,
            month: 1,
            ..Default::default()
        };
        state.prev_month();
        assert_eq!(state.year, 2023);
        assert_eq!(state.month, 12);
    }

    #[test]
    fn calendar_day_click_selects_that_day() {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};
        let mut state = CalendarState { year: 2024, month: 3, cursor_day: 1, ..Default::default() };
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 10));
        Calendar::new().render(buf.area, &mut buf, &mut state);
        let cell = state.day_rects[9]; // 10 March
        let press = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: cell.x, row: cell.y, modifiers: KeyModifiers::NONE };
        assert_eq!(state.handle_mouse(press), Outcome::Changed);
        assert_eq!(state.selected, Some((2024, 3, 10)));
    }
}
