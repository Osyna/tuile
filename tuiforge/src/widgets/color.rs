//! Color selection widgets: swatches, HSL picker, gradients and theme palettes.
//!
//! ```
//! # use tuiforge::prelude::*;
//! # let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
//! # let mut state = SwatchesState::default();
//! let colors = vec![Rgb(255, 0, 0), Rgb(0, 255, 0), Rgb(0, 0, 255)];
//! Swatches::new(&colors).render(Rect::new(0, 0, 40, 3), &mut buf, &mut state);
//! ```

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{StatefulWidget, Widget};

use crate::core::{Hit, HitBox, Interactive, Outcome, is_press, mouse_in, mouse_pos};
use crate::draw::{fill, put, put_centered, st};
use crate::theme::{self, Rgb, Theme, gradient};

// ─────────────────────────────────────────────────────────────────────────────
// Swatches
// ─────────────────────────────────────────────────────────────────────────────

/// Swatch grid state.
#[derive(Clone, Debug, Default)]
pub struct SwatchesState {
    pub selected: Option<usize>,
    pub hover: Option<usize>,
    pub hits: Vec<HitBox>,
}

impl SwatchesState {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Interactive for SwatchesState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        let n = self.hits.len();
        if n == 0 {
            return Outcome::Ignored;
        }
        let current = self.selected.unwrap_or(0);
        match key.code {
            KeyCode::Left if current > 0 => {
                self.selected = Some(current - 1);
                Outcome::Changed
            }
            KeyCode::Right if current + 1 < n => {
                self.selected = Some(current + 1);
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let mut out = Outcome::Ignored;
        for (i, hit) in self.hits.iter_mut().enumerate() {
            match hit.mouse(&m) {
                Hit::Press => {
                    self.selected = Some(i);
                    out = Outcome::Changed;
                }
                Hit::HoverChanged => {
                    self.hover = Some(i);
                    out |= Outcome::Consumed;
                }
                _ => {}
            }
        }
        out
    }
}

/// Row or grid of color swatches.
#[derive(Clone, Debug)]
pub struct Swatches {
    colors: Vec<Rgb>,
    labels: Vec<String>,
    cell_width: u16,
    focused: bool,
    enabled: bool,
    bg: Option<Rgb>,
    theme: Option<Theme>,
}

impl Swatches {
    pub fn new(colors: &[Rgb]) -> Self {
        Self { colors: colors.to_vec(), labels: Vec::new(), cell_width: 4, focused: false, enabled: true, bg: None, theme: None }
    }

    /// Focused: the selection bracket uses the cursor colour.
    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }
    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }
    /// Background behind the swatches (default `th.surface`).
    pub fn bg(mut self, bg: Rgb) -> Self {
        self.bg = Some(bg);
        self
    }

    pub fn labels(mut self, l: &[&str]) -> Self {
        self.labels = l.iter().map(|s| s.to_string()).collect();
        self
    }
    pub fn cell_width(mut self, w: u16) -> Self {
        self.cell_width = w.max(2);
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl StatefulWidget for Swatches {
    type State = SwatchesState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < self.cell_width || area.height == 0 || self.colors.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let bg = self.bg.unwrap_or(th.surface);
        fill(buf, area, bg);

        state.hits.clear();
        state.hits.resize(self.colors.len(), HitBox::default());

        let per_row = (area.width / self.cell_width).max(1) as usize;
        let has_labels = !self.labels.is_empty();
        let row_h = if has_labels { 2 } else { 1 };

        for (i, color) in self.colors.iter().enumerate() {
            let row = i / per_row;
            let col = i % per_row;
            let y = area.y + (row as u16 * row_h);
            if y >= area.y + area.height {
                break;
            }
            let x = area.x + (col as u16 * self.cell_width);
            let cell_rect = Rect { x, y, width: self.cell_width, height: 1 };
            state.hits[i].set_area(cell_rect);

            // swatch: ██ (dimmed when disabled)
            let swatch_w = self.cell_width.min(2);
            let c = if self.enabled { *color } else { color.blend(bg, 0.6) };
            for sx in 0..swatch_w {
                put(buf, x + sx, y, "█", 1, st(c, bg));
            }

            // bracket cursor: cursor colour when focused, text colour otherwise, hover hint
            let bracket = if state.selected == Some(i) {
                Some(if self.focused { th.cursor_bg } else { th.text })
            } else if state.hover == Some(i) && self.enabled {
                Some(th.text_muted)
            } else {
                None
            };
            if let Some(bc) = bracket {
                if x > area.x {
                    put(buf, x.saturating_sub(1), y, "[", 1, st(bc, bg).add_modifier(Modifier::BOLD));
                }
                if x + swatch_w < area.x + area.width {
                    put(buf, x + swatch_w, y, "]", 1, st(bc, bg).add_modifier(Modifier::BOLD));
                }
            }

            // label
            if has_labels && y + 1 < area.y + area.height {
                let label = self.labels.get(i).map(|s| s.as_str()).unwrap_or("");
                let label_rect = Rect { x, y: y + 1, width: self.cell_width, height: 1 };
                let display = crate::draw::truncate(label, self.cell_width as usize);
                put_centered(buf, label_rect, &display, st(th.text_muted, bg));
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ColorPicker
// ─────────────────────────────────────────────────────────────────────────────

/// HSL color picker state.
#[derive(Clone, Debug)]
pub struct ColorPickerState {
    pub h: f32,    // 0..360
    pub s: f32,    // 0..1
    pub l: f32,    // 0..1
    pub hits: Vec<HitBox>,
    pub dragging: Option<usize>, // 0=hue, 1=sl
    hue_rect: Rect,
    sl_rect: Rect,
}

impl Default for ColorPickerState {
    fn default() -> Self {
        Self {
            h: 0.0,
            s: 1.0,
            l: 0.5,
            hits: vec![HitBox::default(), HitBox::default()],
            dragging: None,
            hue_rect: Rect::default(),
            sl_rect: Rect::default(),
        }
    }
}

impl ColorPickerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn value(&self) -> Rgb {
        hsl_to_rgb(self.h, self.s, self.l)
    }

    pub fn set_rgb(&mut self, rgb: Rgb) {
        let (h, s, l) = rgb_to_hsl(rgb);
        self.h = h;
        self.s = s;
        self.l = l;
    }
}

impl Interactive for ColorPickerState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Left => {
                self.h = (self.h - if shift { 1.0 } else { 5.0 }).rem_euclid(360.0);
                Outcome::Changed
            }
            KeyCode::Right => {
                self.h = (self.h + if shift { 1.0 } else { 5.0 }).rem_euclid(360.0);
                Outcome::Changed
            }
            KeyCode::Up => {
                self.l = (self.l + 0.05).clamp(0.0, 1.0);
                Outcome::Changed
            }
            KeyCode::Down => {
                self.l = (self.l - 0.05).clamp(0.0, 1.0);
                Outcome::Changed
            }
            KeyCode::Char('[') => {
                self.s = (self.s - 0.05).clamp(0.0, 1.0);
                Outcome::Changed
            }
            KeyCode::Char(']') => {
                self.s = (self.s + 0.05).clamp(0.0, 1.0);
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        use ratatui::crossterm::event::{MouseButton, MouseEventKind};

        let pos = mouse_pos(&m);

        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if mouse_in(self.hue_rect, &m) {
                    self.dragging = Some(0);
                    let frac = ((pos.x - self.hue_rect.x) as f32 / self.hue_rect.width as f32).clamp(0.0, 1.0);
                    self.h = frac * 360.0;
                    return Outcome::Changed;
                } else if mouse_in(self.sl_rect, &m) {
                    self.dragging = Some(1);
                    let s_frac = ((pos.x - self.sl_rect.x) as f32 / self.sl_rect.width as f32).clamp(0.0, 1.0);
                    let l_frac = 1.0 - ((pos.y - self.sl_rect.y) as f32 / self.sl_rect.height as f32).clamp(0.0, 1.0);
                    self.s = s_frac;
                    self.l = l_frac;
                    return Outcome::Changed;
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(0) = self.dragging {
                    if mouse_in(self.hue_rect, &m) {
                        let frac = ((pos.x - self.hue_rect.x) as f32 / self.hue_rect.width as f32).clamp(0.0, 1.0);
                        self.h = frac * 360.0;
                        return Outcome::Changed;
                    }
                } else if let Some(1) = self.dragging
                    && mouse_in(self.sl_rect, &m) {
                        let s_frac = ((pos.x - self.sl_rect.x) as f32 / self.sl_rect.width as f32).clamp(0.0, 1.0);
                        let l_frac = 1.0 - ((pos.y - self.sl_rect.y) as f32 / self.sl_rect.height as f32).clamp(0.0, 1.0);
                        self.s = s_frac;
                        self.l = l_frac;
                        return Outcome::Changed;
                    }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.dragging = None;
                return Outcome::Consumed;
            }
            _ => {}
        }

        Outcome::Ignored
    }
}

/// HSL color picker with hue strip and SL grid.
#[derive(Clone, Debug)]
pub struct ColorPicker {
    theme: Option<Theme>,
}

impl ColorPicker {
    pub fn new() -> Self {
        Self { theme: None }
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl Default for ColorPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for ColorPicker {
    type State = ColorPickerState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < 20 || area.height < 10 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.surface);

        // Layout: hue strip top, SL grid below, preview + text at bottom
        let hue_h = 1;
        let preview_h = 3;
        let sl_h = area.height.saturating_sub(hue_h + preview_h + 2);

        let hue_rect = Rect { x: area.x, y: area.y, width: area.width, height: hue_h };
        let sl_rect = Rect { x: area.x, y: area.y + hue_h + 1, width: area.width, height: sl_h };
        let preview_rect = Rect { x: area.x, y: area.y + hue_h + 1 + sl_h + 1, width: area.width, height: preview_h };

        state.hue_rect = hue_rect;
        state.sl_rect = sl_rect;

        // Hue strip
        for x in 0..hue_rect.width {
            let frac = x as f32 / hue_rect.width as f32;
            let hue = frac * 360.0;
            let color = hsl_to_rgb(hue, 1.0, 0.5);
            put(buf, hue_rect.x + x, hue_rect.y, "█", 1, st(color, th.surface));
        }

        // Hue cursor
        let hue_cursor_x = hue_rect.x + ((state.h / 360.0) * hue_rect.width as f32) as u16;
        if hue_cursor_x < hue_rect.x + hue_rect.width {
            put(buf, hue_cursor_x, hue_rect.y, "┼", 1, st(th.cursor_fg, th.surface).add_modifier(Modifier::BOLD));
        }

        // SL grid using half blocks
        let _base_hue_color = hsl_to_rgb(state.h, 1.0, 0.5);
        for row in 0..sl_rect.height {
            for col in 0..sl_rect.width {
                let s_frac = col as f32 / sl_rect.width.max(1) as f32;
                let l_frac = 1.0 - (row as f32 / sl_rect.height.max(1) as f32);
                let color = hsl_to_rgb(state.h, s_frac, l_frac);
                put(buf, sl_rect.x + col, sl_rect.y + row, "▀", 1, st(color, color));
            }
        }

        // SL cursor
        let sl_cursor_x = sl_rect.x + (state.s * sl_rect.width as f32) as u16;
        let sl_cursor_y = sl_rect.y + ((1.0 - state.l) * sl_rect.height as f32) as u16;
        if sl_cursor_x < sl_rect.x + sl_rect.width && sl_cursor_y < sl_rect.y + sl_rect.height {
            put(buf, sl_cursor_x, sl_cursor_y, "◆", 1, st(th.cursor_fg, th.surface).add_modifier(Modifier::BOLD));
        }

        // Preview + text
        let rgb = state.value();
        let preview_w = 6;
        for py in 0..preview_h {
            for px in 0..preview_w {
                put(buf, preview_rect.x + px, preview_rect.y + py, "█", 1, st(rgb, th.surface));
            }
        }

        let hex = format!("#{:02X}{:02X}{:02X}", rgb.0, rgb.1, rgb.2);
        put(buf, preview_rect.x + preview_w + 2, preview_rect.y, &hex, hex.len() as u16, st(th.text, th.surface));
        let hsl_str = format!("hsl({:.0}, {:.0}%, {:.0}%)", state.h, state.s * 100.0, state.l * 100.0);
        put(buf, preview_rect.x + preview_w + 2, preview_rect.y + 1, &hsl_str, hsl_str.len() as u16, st(th.text_muted, th.surface));
    }
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Rgb {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());
    let (r1, g1, b1) = if h_prime < 1.0 {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = l - c / 2.0;
    Rgb(
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    )
}

fn rgb_to_hsl(rgb: Rgb) -> (f32, f32, f32) {
    let r = rgb.0 as f32 / 255.0;
    let g = rgb.1 as f32 / 255.0;
    let b = rgb.2 as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let l = (max + min) / 2.0;
    if delta < 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = if l < 0.5 { delta / (max + min) } else { delta / (2.0 - max - min) };
    let h = if (max - r).abs() < 1e-6 {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < 1e-6 {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    (h, s, l)
}

// ─────────────────────────────────────────────────────────────────────────────
// GradientBar
// ─────────────────────────────────────────────────────────────────────────────

/// Horizontal gradient bar with labels and marker.
#[derive(Clone, Debug)]
pub struct GradientBar {
    stops: Vec<Rgb>,
    labels: Option<(String, String)>,
    marker: Option<f32>, // 0..1
    theme: Option<Theme>,
}

impl GradientBar {
    pub fn new(stops: &[Rgb]) -> Self {
        Self {
            stops: stops.to_vec(),
            labels: None,
            marker: None,
            theme: None,
        }
    }

    pub fn labels(mut self, min: impl Into<String>, max: impl Into<String>) -> Self {
        self.labels = Some((min.into(), max.into()));
        self
    }
    pub fn marker(mut self, pos: f32) -> Self {
        self.marker = Some(pos);
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl Widget for GradientBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 5 || area.height == 0 || self.stops.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.surface);

        let bar_y = area.y;
        for x in 0..area.width {
            let frac = x as f32 / area.width.max(1) as f32;
            let color = gradient(&self.stops, frac);
            put(buf, area.x + x, bar_y, "█", 1, st(color, th.surface));
        }

        // marker
        if let Some(pos) = self.marker {
            let marker_x = area.x + (pos.clamp(0.0, 1.0) * area.width as f32) as u16;
            if marker_x < area.x + area.width {
                put(buf, marker_x, bar_y, "▼", 1, st(th.cursor_fg, th.surface).add_modifier(Modifier::BOLD));
            }
        }

        // labels
        if let Some((min, max)) = &self.labels
            && area.height > 1 {
                let label_y = area.y + 1;
                put(buf, area.x, label_y, min, min.len() as u16, st(th.text_muted, th.surface));
                let max_w = max.len() as u16;
                if area.width > max_w {
                    put(buf, area.x + area.width - max_w, label_y, max, max_w, st(th.text_muted, th.surface));
                }
            }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ThemePalette
// ─────────────────────────────────────────────────────────────────────────────

/// Renders every role of a Theme as labeled swatches.
#[derive(Clone, Debug)]
pub struct ThemePalette {
    theme: Option<Theme>,
}

impl ThemePalette {
    pub fn new() -> Self {
        Self { theme: None }
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(th.clone());
        self
    }
}

impl Default for ThemePalette {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ThemePalette {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 15 || area.height < 2 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.surface);

        let roles = [
            ("primary", th.primary),
            ("secondary", th.secondary),
            ("accent", th.accent),
            ("success", th.success),
            ("warning", th.warning),
            ("error", th.error),
            ("surface", th.surface),
            ("panel", th.panel),
            ("text", th.text),
            ("text_muted", th.text_muted),
            ("border", th.border),
            ("cursor_bg", th.cursor_bg),
        ];

        let swatch_w = 15u16;
        let swatch_h = 2u16;
        let per_row = (area.width / swatch_w).max(1) as usize;

        for (i, (label, color)) in roles.iter().enumerate() {
            let row = i / per_row;
            let col = i % per_row;
            let y = area.y + (row as u16 * swatch_h);
            if y + swatch_h > area.y + area.height {
                break;
            }
            let x = area.x + (col as u16 * swatch_w);

            // color block (2 cells wide, 2 rows)
            for py in 0..2 {
                for px in 0..2 {
                    if x + px < area.x + area.width && y + py < area.y + area.height {
                        put(buf, x + px, y + py, "█", 1, st(*color, th.surface));
                    }
                }
            }

            // label
            let label_x = x + 3;
            if label_x < area.x + area.width {
                let label_w = swatch_w.saturating_sub(3);
                put(buf, label_x, y, label, label_w, st(th.text_muted, th.surface));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_rgb_roundtrip() {
        let rgb = Rgb(255, 0, 0);
        let (h, s, l) = rgb_to_hsl(rgb);
        let back = hsl_to_rgb(h, s, l);
        assert!((back.0 as i16 - rgb.0 as i16).abs() <= 1);
    }

    #[test]
    fn hsl_to_rgb_pure_colors() {
        let red = hsl_to_rgb(0.0, 1.0, 0.5);
        assert!(red.0 > 250 && red.1 < 5 && red.2 < 5);
        let green = hsl_to_rgb(120.0, 1.0, 0.5);
        assert!(green.0 < 5 && green.1 > 250 && green.2 < 5);
    }
}
