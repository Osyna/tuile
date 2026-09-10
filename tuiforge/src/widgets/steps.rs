//! Step indicators and timelines for process flows.
//!
//! ```
//! # use tuiforge::prelude::*;
//! # let mut buf = Buffer::empty(Rect::new(0, 0, 40, 3));
//! # let mut state = StepsState::default();
//! Steps::new(&["Start", "Build", "Deploy"]).active(1).render(Rect::new(0, 0, 40, 3), &mut buf, &mut state);
//! ```

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidget;

use crate::core::{Hit, HitBox, Interactive, Outcome};
use crate::draw::{fill, put, put_centered, st};
use crate::theme::{self, Rgb, Theme, Variant};
use unicode_width::UnicodeWidthStr;

// ─────────────────────────────────────────────────────────────────────────────
// Steps
// ─────────────────────────────────────────────────────────────────────────────

/// Status of one step in a sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepStatus {
    Done,
    Active,
    Pending,
    Error,
    Skipped,
}

/// Horizontal/vertical step indicator.
#[derive(Clone, Debug, Default)]
pub struct StepsState {
    pub hits: Vec<HitBox>,
    clicked: Option<usize>,
}

impl StepsState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn take_clicked(&mut self) -> Option<usize> {
        self.clicked.take()
    }
}

impl Interactive for StepsState {
    fn handle_key(&mut self, _key: KeyEvent) -> Outcome {
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let mut out = Outcome::Ignored;
        for (i, hit) in self.hits.iter_mut().enumerate() {
            match hit.mouse(&m) {
                Hit::Press => {
                    self.clicked = Some(i);
                    out = Outcome::Changed;
                }
                Hit::HoverChanged => out |= Outcome::Consumed,
                _ => {}
            }
        }
        out
    }
}
/// Step indicator widget.
#[derive(Clone, Debug)]
pub struct Steps {
    labels: Vec<String>,
    statuses: Vec<StepStatus>,
    active: usize,
    numbered: bool,
    vertical: bool,
    compact: bool,
    theme: Option<Theme>,
}

impl Steps {
    pub fn new(labels: &[&str]) -> Self {
        let statuses = labels
            .iter()
            .enumerate()
            .map(|(i, _)| if i == 0 { StepStatus::Active } else { StepStatus::Pending })
            .collect();
        Self {
            labels: labels.iter().map(|s| s.to_string()).collect(),
            statuses,
            active: 0,
            numbered: false,
            vertical: false,
            compact: false,
            theme: None,
        }
    }

    pub fn statuses(mut self, s: &[StepStatus]) -> Self {
        self.statuses = s.to_vec();
        self
    }
    pub fn active(mut self, idx: usize) -> Self {
        self.active = idx;
        if idx < self.statuses.len() {
            for i in 0..self.statuses.len() {
                if i < idx {
                    if self.statuses[i] != StepStatus::Error && self.statuses[i] != StepStatus::Skipped {
                        self.statuses[i] = StepStatus::Done;
                    }
                } else if i == idx {
                    self.statuses[i] = StepStatus::Active;
                } else if self.statuses[i] != StepStatus::Error && self.statuses[i] != StepStatus::Skipped {
                    self.statuses[i] = StepStatus::Pending;
                }
            }
        }
        self
    }
    pub fn numbered(mut self, v: bool) -> Self {
        self.numbered = v;
        self
    }
    pub fn vertical(mut self, v: bool) -> Self {
        self.vertical = v;
        self
    }
    pub fn compact(mut self, v: bool) -> Self {
        self.compact = v;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl StatefulWidget for Steps {
    type State = StepsState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 || self.labels.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.surface);

        state.hits.clear();
        state.hits.resize(self.labels.len(), HitBox::default());

        if self.vertical {
            self.render_vertical(area, buf, &th, state);
        } else {
            self.render_horizontal(area, buf, &th, state);
        }
    }
}

impl Steps {
    fn render_horizontal(&self, area: Rect, buf: &mut Buffer, th: &Theme, state: &mut StepsState) {
        let n = self.labels.len();
        if n == 0 {
            return;
        }

        // horizontal layout: distribute space
        let step_w = area.width / n as u16;
        if step_w < 3 {
            return;
        }
        let span = |i: usize| -> (u16, u16) {
            let x = area.x + i as u16 * step_w;
            let w = if i == n - 1 { area.width - i as u16 * step_w } else { step_w };
            (x, w)
        };

        for (i, (label, status)) in self.labels.iter().zip(&self.statuses).enumerate() {
            let (x, w) = span(i);
            state.hits[i].set_area(Rect { x, y: area.y, width: w, height: area.height });

            let (glyph, color) = self.step_glyph_color(*status, th);
            let connector_color = if *status == StepStatus::Done { th.success } else { th.border_blurred };

            // Marker centred on the step column
            let glyph_str = if self.numbered { format!("({})", i + 1) } else { glyph.to_string() };
            let gw = glyph_str.width() as u16;
            let gx = (x + w / 2).saturating_sub(gw / 2);
            put(buf, gx, area.y, &glyph_str, gw, st(color, th.surface));

            // Connector from this marker to the next one
            if i < n - 1 {
                let (nx, nw) = span(i + 1);
                let start = gx + gw + 1;
                let end = (nx + nw / 2).saturating_sub(gw / 2 + 1);
                for cx in start..end {
                    put(buf, cx, area.y, "━", 1, st(connector_color, th.surface));
                }
            }

            // Label below if space
            if area.height > 1 && !self.compact {
                let label_display = crate::draw::truncate(label, w as usize);
                put_centered(buf, Rect { x, y: area.y + 1, width: w, height: 1 }, &label_display, st(th.text_muted, th.surface));
            }
        }
    }

    fn render_vertical(&self, area: Rect, buf: &mut Buffer, th: &Theme, state: &mut StepsState) {
        let n = self.labels.len();
        if n == 0 {
            return;
        }

        let step_h = if self.compact { 1 } else { 2 };
        let total_h = n as u16 * step_h;
        if total_h > area.height {
            // fallback: compact
            for (i, (label, status)) in self.labels.iter().zip(&self.statuses).enumerate() {
                let y = area.y + i as u16;
                if y >= area.y + area.height {
                    break;
                }
                let step_area = Rect { x: area.x, y, width: area.width, height: 1 };
                state.hits[i].set_area(step_area);

                let (glyph, color) = self.step_glyph_color(*status, th);
                put(buf, area.x, y, glyph, 1, st(color, th.surface));
                let label_display = crate::draw::truncate(label, area.width.saturating_sub(3) as usize);
                put(buf, area.x + 2, y, &label_display, area.width.saturating_sub(2), st(th.text, th.surface));
            }
        } else {
            for (i, (label, status)) in self.labels.iter().zip(&self.statuses).enumerate() {
                let y = area.y + (i as u16 * step_h);
                let step_area = Rect { x: area.x, y, width: area.width, height: step_h };
                state.hits[i].set_area(step_area);

                let (glyph, color) = self.step_glyph_color(*status, th);
                put(buf, area.x, y, glyph, 1, st(color, th.surface));

                let label_display = crate::draw::truncate(label, area.width.saturating_sub(3) as usize);
                put(buf, area.x + 2, y, &label_display, area.width.saturating_sub(2), st(th.text, th.surface));

                // connector down (except last)
                if i < n - 1 && !self.compact {
                    let connector_color = if *status == StepStatus::Done { th.success } else { th.border_blurred };
                    put(buf, area.x, y + 1, "│", 1, st(connector_color, th.surface));
                }
            }
        }
    }

    fn step_glyph_color(&self, status: StepStatus, th: &Theme) -> (&'static str, Rgb) {
        match status {
            StepStatus::Done => ("✓", th.success),
            StepStatus::Active => ("●", th.primary),
            StepStatus::Pending => ("○", th.text_muted),
            StepStatus::Error => ("✖", th.error),
            StepStatus::Skipped => ("◌", th.text_muted),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Timeline
// ─────────────────────────────────────────────────────────────────────────────

/// One entry in a timeline.
#[derive(Clone, Debug)]
pub struct TimelineEntry {
    pub time: String,
    pub title: String,
    pub description: Option<String>,
    pub variant: Variant,
}

/// Vertical timeline widget.
#[derive(Clone, Debug)]
pub struct Timeline {
    entries: Vec<TimelineEntry>,
    compact: bool,
    reverse: bool,
    theme: Option<Theme>,
}

impl Timeline {
    pub fn new(entries: Vec<TimelineEntry>) -> Self {
        Self { entries, compact: false, reverse: false, theme: None }
    }

    pub fn compact(mut self, v: bool) -> Self {
        self.compact = v;
        self
    }
    pub fn reverse(mut self, v: bool) -> Self {
        self.reverse = v;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl ratatui::widgets::Widget for Timeline {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height == 0 || self.entries.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.surface);

        let entries: Vec<&TimelineEntry> = if self.reverse {
            self.entries.iter().rev().collect()
        } else {
            self.entries.iter().collect()
        };

        let mut y = area.y;
        for (i, entry) in entries.iter().enumerate() {
            if y >= area.y + area.height {
                break;
            }

            let color = th.variant(entry.variant);
            let time_w = entry.time.len() as u16;
            put(buf, area.x, y, &entry.time, time_w.min(area.width), st(th.text_muted, th.surface));

            let glyph_x = area.x + time_w + 1;
            if glyph_x < area.x + area.width {
                put(buf, glyph_x, y, "│●", 2, st(color, th.surface));
            }

            let title_x = glyph_x + 3;
            if title_x < area.x + area.width {
                let title_w = area.width.saturating_sub(title_x - area.x);
                put(buf, title_x, y, &entry.title, title_w, st(th.text, th.surface));
            }
            y += 1;

            if !self.compact
                && let Some(desc) = &entry.description
                    && y < area.y + area.height {
                        put(buf, glyph_x, y, "│", 1, st(th.border_blurred, th.surface));
                        let desc_x = glyph_x + 3;
                        if desc_x < area.x + area.width {
                            let desc_w = area.width.saturating_sub(desc_x - area.x);
                            put(buf, desc_x, y, desc, desc_w, st(th.text_muted, th.surface));
                        }
                        y += 1;
                    }

            // connector to next (except last)
            if i < entries.len() - 1 && y < area.y + area.height {
                put(buf, glyph_x, y, "│", 1, st(th.border_blurred, th.surface));
                y += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_sets_active() {
        let s = Steps::new(&["A", "B", "C"]).active(1);
        assert_eq!(s.statuses[0], StepStatus::Done);
        assert_eq!(s.statuses[1], StepStatus::Active);
        assert_eq!(s.statuses[2], StepStatus::Pending);
    }

    #[test]
    fn steps_preserves_errors() {
        let s = Steps::new(&["A", "B", "C"]).statuses(&[StepStatus::Done, StepStatus::Error, StepStatus::Pending]).active(2);
        assert_eq!(s.statuses[1], StepStatus::Error);
    }
}
