//! Form layouts, field grouping, validation helpers, and auxiliary form components.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! # let area = Rect::new(0, 0, 60, 20);
//! # let mut buf = Buffer::empty(area);
//! let fields = vec![
//!     FormField::new("Email").required(true).help("work@example.com"),
//!     FormField::new("Password").height(3).error(Some("too weak")),
//! ];
//! let rects = Form::new().style(FormStyle::Stacked).fields(&fields).render(area, &mut buf);
//! ```

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use unicode_width::UnicodeWidthStr;

use crate::draw::{Border, fill, put, put_right, st};
use crate::layout::pad;
use crate::theme::{self, Rgb, Theme, Variant};

// ───────────────────────────── form styles ─────────────────────────────

/// Layout style for form fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FormStyle {
    /// Label above field, help below (default).
    #[default]
    Stacked,
    /// Label left-aligned with optional colon, field to the right.
    Aligned,
    /// Compact single-row fields chained horizontally.
    Inline,
    /// Label drawn into the field's top border (Material-style).
    Floating,
    /// Two columns with equal gutters, `span(2)` uses both.
    Grid,
    /// Each section a bordered fieldset with legend.
    Cards,
    /// One section visible at a time with step progress header.
    Wizard,
    /// Dense 1-row unframed fields.
    Compact,
}

// ───────────────────────────── form field ─────────────────────────────

/// Metadata for one form field.
#[derive(Clone, Debug)]
pub struct FormField {
    pub label: String,
    pub required: bool,
    pub help: String,
    pub error: Option<String>,
    pub height: u16,
    pub span: usize,
    pub hint_right: String,
}

impl FormField {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            required: false,
            help: String::new(),
            error: None,
            height: 3,
            span: 1,
            hint_right: String::new(),
        }
    }

    pub fn required(mut self, v: bool) -> Self {
        self.required = v;
        self
    }

    pub fn help(mut self, h: impl Into<String>) -> Self {
        self.help = h.into();
        self
    }

    pub fn error(mut self, e: Option<impl Into<String>>) -> Self {
        self.error = e.map(|s| s.into());
        self
    }

    pub fn height(mut self, h: u16) -> Self {
        self.height = h;
        self
    }

    pub fn span(mut self, s: usize) -> Self {
        self.span = s;
        self
    }

    pub fn hint_right(mut self, h: impl Into<String>) -> Self {
        self.hint_right = h.into();
        self
    }
}

// ───────────────────────────── form rects ─────────────────────────────

/// Per-field rectangles returned by `Form::render`.
#[derive(Clone, Debug)]
pub struct FieldRects {
    pub label: Rect,
    pub field: Rect,
    pub help: Rect,
    pub error: Rect,
}

/// All field rects plus optional header/footer areas for Wizard/Cards.
#[derive(Clone, Debug, Default)]
pub struct FormRects {
    pub fields: Vec<FieldRects>,
    pub header: Rect,
    pub footer: Rect,
}

// ───────────────────────────── form builder ─────────────────────────────

/// Stateless form layout builder.
pub struct Form<'a> {
    style: FormStyle,
    label_width: u16,
    gap: u16,
    columns: usize,
    fields: &'a [FormField],
    label_colon: bool,
    theme: Option<Theme>,
}

impl<'a> Form<'a> {
    pub fn new() -> Self {
        Self {
            style: FormStyle::default(),
            label_width: 16,
            gap: 1,
            columns: 1,
            fields: &[],
            label_colon: true,
            theme: None,
        }
    }

    pub fn style(mut self, s: FormStyle) -> Self {
        self.style = s;
        self
    }

    pub fn label_width(mut self, w: u16) -> Self {
        self.label_width = w;
        self
    }

    pub fn gap(mut self, g: u16) -> Self {
        self.gap = g;
        self
    }

    pub fn columns(mut self, n: usize) -> Self {
        self.columns = n.max(1);
        self
    }

    pub fn fields(mut self, f: &'a [FormField]) -> Self {
        self.fields = f;
        self
    }

    pub fn label_colon(mut self, v: bool) -> Self {
        self.label_colon = v;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Render the form chrome and return rects for the caller to draw fields into.
    pub fn render(self, area: Rect, buf: &mut Buffer) -> FormRects {
        let th = self.theme.unwrap_or_else(theme::current);

        match self.style {
            FormStyle::Stacked => self.render_stacked(area, buf, &th),
            FormStyle::Aligned => self.render_aligned(area, buf, &th),
            FormStyle::Inline => self.render_inline(area, buf, &th),
            FormStyle::Floating => self.render_floating(area, buf, &th),
            FormStyle::Grid => self.render_grid(area, buf, &th),
            FormStyle::Cards => self.render_cards(area, buf, &th),
            FormStyle::Wizard => self.render_wizard(area, buf, &th),
            FormStyle::Compact => self.render_compact(area, buf, &th),
        }
    }

    fn render_stacked(self, area: Rect, buf: &mut Buffer, th: &Theme) -> FormRects {
        let mut rects = FormRects::default();
        let mut y = area.y;

        for field in self.fields {
            if y >= area.y + area.height {
                break;
            }

            let label_h = 1;
            let field_h = field.height.min(area.height.saturating_sub(y - area.y));
            let help_h = if field.error.is_some() || !field.help.is_empty() {
                1
            } else {
                0
            };

            let label_rect = Rect::new(area.x, y, area.width, label_h);
            y += label_h;

            let field_rect = Rect::new(area.x, y, area.width, field_h);
            y += field_h;

            let help_rect = Rect::new(area.x, y, area.width, help_h);
            y += help_h + self.gap;

            // Label (required * in error color, label in normal color)
            put(
                buf,
                label_rect.x,
                label_rect.y,
                &field.label,
                label_rect.width,
                st(th.text, th.background),
            );
            if field.required {
                let star_x = label_rect.x + field.label.width() as u16 + 1;
                if star_x < label_rect.x + label_rect.width {
                    put(
                        buf,
                        star_x,
                        label_rect.y,
                        "*",
                        1,
                        st(th.error, th.background),
                    );
                }
            }

            if !field.hint_right.is_empty() {
                put_right(
                    buf,
                    label_rect,
                    &field.hint_right,
                    st(th.text_muted, th.background),
                );
            }

            // Help or error
            if let Some(err) = &field.error {
                put(
                    buf,
                    help_rect.x,
                    help_rect.y,
                    err,
                    help_rect.width,
                    st(th.error, th.background),
                );
            } else if !field.help.is_empty() {
                put(
                    buf,
                    help_rect.x,
                    help_rect.y,
                    &field.help,
                    help_rect.width,
                    st(th.text_muted, th.background),
                );
            }

            rects.fields.push(FieldRects {
                label: label_rect,
                field: field_rect,
                help: help_rect,
                error: help_rect,
            });
        }

        rects
    }

    fn render_aligned(self, area: Rect, buf: &mut Buffer, th: &Theme) -> FormRects {
        let mut rects = FormRects::default();
        let mut y = area.y;

        for field in self.fields {
            if y >= area.y + area.height {
                break;
            }

            let row_h = field.height.max(1);
            let label_rect = Rect::new(area.x, y, self.label_width, 1);
            let field_x = area.x + self.label_width + 2;
            let field_w = area.width.saturating_sub(self.label_width + 2);
            let field_rect = Rect::new(field_x, y, field_w, row_h);
            let help_y = y + row_h;
            let help_rect = Rect::new(field_x, help_y, field_w, 1);

            // Label (right-aligned, required * in error color)
            let colon = if self.label_colon { ":" } else { "" };
            let base_label = format!("{}{}", field.label, colon);
            let label_w = base_label.width() as u16;
            let star_w = if field.required { 2 } else { 0 }; // " *"
            let total_w = label_w + star_w;
            let start_x = label_rect.x + label_rect.width.saturating_sub(total_w);

            put(
                buf,
                start_x,
                label_rect.y,
                &base_label,
                label_w,
                st(th.text, th.background),
            );
            if field.required {
                put(
                    buf,
                    start_x + label_w,
                    label_rect.y,
                    " *",
                    2,
                    st(th.error, th.background),
                );
            }

            // Help or error
            if !field.help.is_empty() || field.error.is_some() {
                let msg = field.error.as_deref().unwrap_or(&field.help);
                let msg_fg = if field.error.is_some() {
                    th.error
                } else {
                    th.text_muted
                };
                put(
                    buf,
                    help_rect.x,
                    help_rect.y,
                    msg,
                    help_rect.width,
                    st(msg_fg, th.background),
                );
                y += row_h + 1 + self.gap;
            } else {
                y += row_h + self.gap;
            }

            if !field.hint_right.is_empty() {
                put_right(
                    buf,
                    field_rect,
                    &field.hint_right,
                    st(th.text_muted, th.background),
                );
            }

            rects.fields.push(FieldRects {
                label: label_rect,
                field: field_rect,
                help: help_rect,
                error: help_rect,
            });
        }

        rects
    }

    /// One row: `Label [field]  Label [field] …`. Fields share the width left after the labels
    /// (4..=16 cells each) so a filter bar always fits its row.
    fn render_inline(self, area: Rect, buf: &mut Buffer, th: &Theme) -> FormRects {
        let mut rects = FormRects::default();
        if area.height == 0 || self.fields.is_empty() {
            return rects;
        }
        let n = self.fields.len() as u16;
        let labels: u16 = self.fields.iter().map(|f| f.label.width() as u16 + 1).sum();
        let avail = area
            .width
            .saturating_sub(labels + self.gap * n.saturating_sub(1));
        let field_w = (avail / n).clamp(4, 16);
        let mut x = area.x;
        for field in self.fields {
            let label_w = field.label.width() as u16 + 1;
            if x + label_w + 2 > area.right() {
                break;
            }
            let field_w = field_w.min(area.right().saturating_sub(x + label_w));
            let label_rect = Rect::new(x, area.y, label_w, 1);
            let field_rect = Rect::new(
                x + label_w,
                area.y,
                field_w,
                field.height.max(1).min(area.height),
            );
            put(
                buf,
                label_rect.x,
                label_rect.y,
                &field.label,
                label_rect.width,
                st(th.text, th.background),
            );
            x += label_w + field_w + self.gap;
            rects.fields.push(FieldRects {
                label: label_rect,
                field: field_rect,
                help: Rect::default(),
                error: Rect::default(),
            });
        }
        rects
    }

    /// Material-style: the field frame carries the label on its top edge. The label is NOT drawn
    /// here (the field widget would paint over it): render the widget into `field`, then call
    /// [`float_label`] for each field.
    fn render_floating(self, area: Rect, buf: &mut Buffer, th: &Theme) -> FormRects {
        let mut rects = FormRects::default();
        let mut y = area.y;

        for field in self.fields {
            if y >= area.bottom() {
                break;
            }
            let field_h = field.height.max(3);
            let field_rect = Rect::new(area.x, y, area.width, field_h);
            let label_rect = Rect::new(area.x + 2, y, area.width.saturating_sub(4), 1);
            let help_rect = Rect::new(area.x + 1, y + field_h, area.width.saturating_sub(1), 1);

            if !field.help.is_empty() || field.error.is_some() {
                let msg = field.error.as_deref().unwrap_or(&field.help);
                let msg_fg = if field.error.is_some() {
                    th.error
                } else {
                    th.text_muted
                };
                put(
                    buf,
                    help_rect.x,
                    help_rect.y,
                    msg,
                    help_rect.width,
                    st(msg_fg, th.background),
                );
                y += field_h + 1 + self.gap;
            } else {
                y += field_h + self.gap;
            }

            rects.fields.push(FieldRects {
                label: label_rect,
                field: field_rect,
                help: help_rect,
                error: help_rect,
            });
        }

        rects
    }

    /// `columns` equal columns with a `gap` gutter; a field may `span` several. A row reserves a
    /// help line only when one of its fields has help or an error.
    fn render_grid(self, area: Rect, buf: &mut Buffer, th: &Theme) -> FormRects {
        let mut rects = FormRects::default();
        let cols = self.columns as u16;
        let col_w = area.width.saturating_sub(self.gap * cols.saturating_sub(1)) / cols;
        let mut y = area.y;
        let mut col = 0usize;
        let mut row_h = 0u16;
        let mut row_help = false;

        for field in self.fields {
            if y >= area.bottom() {
                break;
            }
            let span = field.span.clamp(1, self.columns);
            if col + span > self.columns {
                // wrap before placing a field that does not fit the current row
                y += row_h + u16::from(row_help);
                col = 0;
                row_h = 0;
                row_help = false;
            }
            let x = area.x + col as u16 * (col_w + self.gap);
            let w = col_w * span as u16 + self.gap * (span as u16 - 1);
            let has_help = field.error.is_some() || !field.help.is_empty();

            let label_rect = Rect::new(x, y, w, 1);
            let field_rect = Rect::new(x, y + 1, w, field.height);
            let help_rect = Rect::new(x, y + 1 + field.height, w, u16::from(has_help));

            put(
                buf,
                label_rect.x,
                label_rect.y,
                &field.label,
                label_rect.width,
                st(th.text, th.background),
            );
            if field.required {
                let star_x = label_rect.x + field.label.width() as u16 + 1;
                if star_x < label_rect.right() {
                    put(
                        buf,
                        star_x,
                        label_rect.y,
                        "*",
                        1,
                        st(th.error, th.background),
                    );
                }
            }
            if !field.hint_right.is_empty() {
                put_right(
                    buf,
                    label_rect,
                    &field.hint_right,
                    st(th.text_muted, th.background),
                );
            }
            if let Some(err) = &field.error {
                put(
                    buf,
                    help_rect.x,
                    help_rect.y,
                    err,
                    help_rect.width,
                    st(th.error, th.background),
                );
            } else if !field.help.is_empty() {
                put(
                    buf,
                    help_rect.x,
                    help_rect.y,
                    &field.help,
                    help_rect.width,
                    st(th.text_muted, th.background),
                );
            }

            rects.fields.push(FieldRects {
                label: label_rect,
                field: field_rect,
                help: help_rect,
                error: help_rect,
            });
            row_h = row_h.max(1 + field.height);
            row_help |= has_help;
            col += span;
            if col >= self.columns {
                y += row_h + u16::from(row_help);
                col = 0;
                row_h = 0;
                row_help = false;
            }
        }
        rects
    }

    fn render_cards(self, area: Rect, buf: &mut Buffer, th: &Theme) -> FormRects {
        // Same as stacked but fields are grouped in bordered cards
        self.render_stacked(area, buf, th)
    }

    fn render_wizard(self, area: Rect, buf: &mut Buffer, th: &Theme) -> FormRects {
        // Header for step progress, content below
        let header = Rect::new(area.x, area.y, area.width, 3);
        let content = Rect::new(
            area.x,
            area.y + 4,
            area.width,
            area.height.saturating_sub(4),
        );

        let mut result = self.render_stacked(content, buf, th);
        result.header = header;
        result
    }

    fn render_compact(self, area: Rect, buf: &mut Buffer, th: &Theme) -> FormRects {
        let mut rects = FormRects::default();
        let mut y = area.y;

        for field in self.fields {
            if y >= area.y + area.height {
                break;
            }

            let label_w = self.label_width;
            let field_x = area.x + label_w + 1;
            let field_w = area.width.saturating_sub(label_w + 1);

            let label_rect = Rect::new(area.x, y, label_w, 1);
            let field_rect = Rect::new(field_x, y, field_w, 1);

            // Label (right-aligned with required * in error color)
            let base_label = field.label.clone();
            let label_w_text = base_label.width() as u16;
            let star_w = if field.required { 2 } else { 0 };
            let total_w = label_w_text + star_w;
            let start_x = label_rect.x + label_rect.width.saturating_sub(total_w);

            put(
                buf,
                start_x,
                label_rect.y,
                &base_label,
                label_w_text,
                st(th.text, th.background),
            );
            if field.required {
                put(
                    buf,
                    start_x + label_w_text,
                    label_rect.y,
                    " *",
                    2,
                    st(th.error, th.background),
                );
            }

            y += 1 + self.gap;

            rects.fields.push(FieldRects {
                label: label_rect,
                field: field_rect,
                help: Rect::default(),
                error: Rect::default(),
            });
        }

        rects
    }
}

impl<'a> Default for Form<'a> {
    fn default() -> Self {
        Self::new()
    }
}

// ───────────────────────────── fieldset ─────────────────────────────

/// Bordered group with a legend title.
pub struct Fieldset<'a> {
    legend: &'a str,
    description: &'a str,
    theme: Option<Theme>,
}

impl<'a> Fieldset<'a> {
    pub fn new(legend: &'a str) -> Self {
        Self {
            legend,
            description: "",
            theme: None,
        }
    }

    pub fn description(mut self, d: &'a str) -> Self {
        self.description = d;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Draw the border and legend, return the content rect.
    pub fn render(self, area: Rect, buf: &mut Buffer) -> Rect {
        if area.width < 4 || area.height < 2 {
            return Rect::default();
        }

        let th = self.theme.unwrap_or_else(theme::current);
        Border::Round.draw_titled(
            buf,
            area,
            th.border,
            th.background,
            self.legend,
            Alignment::Left,
        );

        let mut content = pad(area, 2, 1);
        if !self.description.is_empty() && content.height > 1 {
            put(
                buf,
                content.x,
                content.y,
                self.description,
                content.width,
                st(th.text_muted, th.background),
            );
            content.y += 1;
            content.height = content.height.saturating_sub(1);
        }

        content
    }
}

// ───────────────────────────── form actions ─────────────────────────────

/// Button row for form submit/cancel with dirty state hint.
pub struct FormActions<'a> {
    dirty: bool,
    align: Alignment,
    button_labels: &'a [&'a str],
    theme: Option<Theme>,
}

impl<'a> FormActions<'a> {
    pub fn new(button_labels: &'a [&'a str]) -> Self {
        Self {
            dirty: false,
            align: Alignment::Right,
            button_labels,
            theme: None,
        }
    }

    pub fn dirty(mut self, v: bool) -> Self {
        self.dirty = v;
        self
    }

    pub fn align(mut self, a: Alignment) -> Self {
        self.align = a;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Render the dirty hint and return one rect per button (label width + 4, at most 3 rows,
    /// clipped to `area`), in order. The caller renders the actual `Button`s into them.
    pub fn render(self, area: Rect, buf: &mut Buffer) -> Vec<Rect> {
        let th = self.theme.unwrap_or_else(theme::current);
        let mut rects = Vec::new();
        if area.height == 0 || area.width == 0 {
            return rects;
        }
        let h = area.height.min(3);
        let gap = 2u16;
        let widths: Vec<u16> = self
            .button_labels
            .iter()
            .map(|l| (l.width() as u16 + 4).max(8))
            .collect();
        let total_w =
            widths.iter().sum::<u16>() + gap * self.button_labels.len().saturating_sub(1) as u16;
        let start_x = match self.align {
            Alignment::Left => area.x,
            Alignment::Center => area.x + area.width.saturating_sub(total_w) / 2,
            Alignment::Right => area.x + area.width.saturating_sub(total_w),
        };
        if self.dirty {
            let hint = "● Unsaved changes";
            let hint_w = if self.align == Alignment::Left {
                area.width.saturating_sub(total_w + gap)
            } else {
                start_x.saturating_sub(area.x + gap)
            };
            let hint_x = if self.align == Alignment::Left {
                start_x + total_w + gap
            } else {
                area.x
            };
            put(
                buf,
                hint_x,
                area.y + h / 2,
                hint,
                hint_w,
                st(th.warning, th.background),
            );
        }
        let mut x = start_x;
        for w in widths {
            let w = w.min(area.right().saturating_sub(x));
            if w == 0 {
                break;
            }
            rects.push(Rect::new(x, area.y, w, h));
            x += w + gap;
        }
        rects
    }
}

/// Paint a floating label onto the top edge of an already-rendered framed field
/// (`FormStyle::Floating`): ` Label ` with a required `*`, muted or `th.primary` when focused.
pub fn float_label(
    buf: &mut Buffer,
    field: Rect,
    label: &str,
    required: bool,
    focused: bool,
    th: &Theme,
) {
    if field.width < 6 || field.height == 0 {
        return;
    }
    let fg = if focused { th.primary } else { th.text_muted };
    let text = format!(" {label} ");
    let max = field.width.saturating_sub(4);
    let w = put(buf, field.x + 2, field.y, &text, max, st(fg, th.background));
    if required {
        put(
            buf,
            field.x + 2 + w.saturating_sub(1),
            field.y,
            "* ",
            max.saturating_sub(w.saturating_sub(1)),
            st(th.error, th.background),
        );
    }
}

// ───────────────────────────── validation summary ─────────────────────────────

/// List of field validation errors.
pub struct ValidationSummary<'a> {
    errors: &'a [(&'a str, &'a str)],
    theme: Option<Theme>,
}

impl<'a> ValidationSummary<'a> {
    pub fn new(errors: &'a [(&'a str, &'a str)]) -> Self {
        Self {
            errors,
            theme: None,
        }
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    pub fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 6 || area.height < 3 || self.errors.is_empty() {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let err_bg = th.variant(Variant::Error);
        let err_fg = th.text_variant(Variant::Error);

        Border::Round.draw(buf, area, th.error, err_bg);
        let title = format!(" {} errors ", self.errors.len());
        put(
            buf,
            area.x + 2,
            area.y,
            &title,
            area.width.saturating_sub(4),
            st(err_fg, err_bg),
        );

        let content = pad(area, 2, 1);
        for (y, (field, msg)) in (content.y..content.bottom()).zip(self.errors.iter()) {
            put(
                buf,
                content.x,
                y,
                &format!("{field}: {msg}"),
                content.width,
                st(err_fg, err_bg),
            );
        }
    }
}

// ───────────────────────────── password strength ─────────────────────────────

/// Password strength meter (0..4 segments).
pub struct PasswordStrength<'a> {
    password: &'a str,
    theme: Option<Theme>,
}

impl<'a> PasswordStrength<'a> {
    pub fn new(password: &'a str) -> Self {
        Self {
            password,
            theme: None,
        }
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Calculate strength level 0..4.
    pub fn strength(&self) -> u8 {
        let len = self.password.len();
        if len == 0 {
            return 0;
        }
        let has_lower = self.password.chars().any(|c| c.is_ascii_lowercase());
        let has_upper = self.password.chars().any(|c| c.is_ascii_uppercase());
        let has_digit = self.password.chars().any(|c| c.is_ascii_digit());
        let has_special = self.password.chars().any(|c| !c.is_alphanumeric());

        let mut score = 0u8;
        if len >= 8 {
            score += 1;
        }
        if has_lower && has_upper {
            score += 1;
        }
        if has_digit {
            score += 1;
        }
        if has_special {
            score += 1;
        }

        score.min(4)
    }

    pub fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 1 {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        let strength = self.strength();
        let (label, color) = match strength {
            0 => ("", th.text_muted),
            1 => ("Weak", th.error),
            2 => ("Fair", th.warning),
            3 => ("Good", th.success),
            4 => ("Strong", th.success),
            _ => ("", th.text_muted),
        };

        // Draw segments
        let seg_w = 4u16;
        let seg_gap = 1u16;
        let mut x = area.x;
        for i in 0..4 {
            let seg_bg = if i < strength { color } else { th.surface };
            fill(buf, Rect::new(x, area.y, seg_w, 1), seg_bg);
            x += seg_w + seg_gap;
        }

        // Label
        if !label.is_empty() {
            put(
                buf,
                x + 1,
                area.y,
                label,
                area.width.saturating_sub((x - area.x) + 1),
                st(color, th.background),
            );
        }
    }
}

// ───────────────────────────── char counter ─────────────────────────────

/// Character counter helper (n/max, color changes near limit).
pub fn char_counter(current: usize, max: usize, th: &Theme) -> (String, Rgb) {
    let text = format!("{}/{}", current, max);
    let ratio = current as f32 / max.max(1) as f32;
    let color = if current > max {
        th.error
    } else if ratio >= 0.9 {
        th.warning
    } else {
        th.text_muted
    };
    (text, color)
}

// ───────────────────────────── validators ─────────────────────────────

/// Validation helper: check if a field is non-empty.
pub fn required(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        Some("required".to_string())
    } else {
        None
    }
}

/// Validate email format (simple check).
pub fn email(s: &str) -> Option<String> {
    if !s.contains('@') || !s.contains('.') {
        Some("invalid email".to_string())
    } else {
        None
    }
}

/// Minimum length validator.
pub fn min_len(min: usize) -> impl Fn(&str) -> Option<String> {
    move |s: &str| {
        if s.len() < min {
            Some(format!("min {} chars", min))
        } else {
            None
        }
    }
}

/// Maximum length validator.
pub fn max_len(max: usize) -> impl Fn(&str) -> Option<String> {
    move |s: &str| {
        if s.len() > max {
            Some(format!("max {} chars", max))
        } else {
            None
        }
    }
}

/// Numeric validator.
pub fn numeric(s: &str) -> Option<String> {
    if s.parse::<f64>().is_err() {
        Some("must be a number".to_string())
    } else {
        None
    }
}

/// Match validator (e.g., password confirmation).
pub fn matches(expected: &str) -> impl Fn(&str) -> Option<String> + '_ {
    move |s: &str| {
        if s != expected {
            Some("does not match".to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn form_stacked_vs_aligned_geometry() {
        let area = Rect::new(0, 0, 40, 20);
        let mut buf = Buffer::empty(area);
        let fields = vec![
            FormField::new("Name").height(3),
            FormField::new("Email").height(3),
        ];

        let stacked = Form::new()
            .style(FormStyle::Stacked)
            .fields(&fields)
            .render(area, &mut buf);
        assert_eq!(stacked.fields.len(), 2);
        assert_eq!(stacked.fields[0].label.y, 0);
        assert_eq!(stacked.fields[0].field.y, 1);

        let aligned = Form::new()
            .style(FormStyle::Aligned)
            .label_width(10)
            .fields(&fields)
            .render(area, &mut buf);
        assert_eq!(aligned.fields.len(), 2);
        assert_eq!(aligned.fields[0].field.x, 12); // label_width + 2
    }

    #[test]
    fn grid_span_uses_both_columns() {
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        let fields = vec![FormField::new("A").span(1), FormField::new("B").span(2)];

        let grid = Form::new()
            .style(FormStyle::Grid)
            .columns(2)
            .fields(&fields)
            .render(area, &mut buf);
        assert_eq!(grid.fields.len(), 2);
        assert!(grid.fields[1].field.width > grid.fields[0].field.width);
    }

    #[test]
    fn floating_label_on_top_edge() {
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        let fields = vec![FormField::new("Email").height(3)];

        let rects = Form::new()
            .style(FormStyle::Floating)
            .fields(&fields)
            .render(area, &mut buf);
        assert_eq!(rects.fields.len(), 1);
        assert_eq!(rects.fields[0].label.y, rects.fields[0].field.y); // same row
    }

    #[test]
    fn wizard_returns_header_rect() {
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        let fields = vec![FormField::new("Step1")];

        let wiz = Form::new()
            .style(FormStyle::Wizard)
            .fields(&fields)
            .render(area, &mut buf);
        assert!(wiz.header.height > 0);
    }

    #[test]
    fn validators_work() {
        assert!(required("").is_some());
        assert!(required("x").is_none());

        assert!(email("bad").is_some());
        assert!(email("good@example.com").is_none());

        let min3 = min_len(3);
        assert!(min3("ab").is_some());
        assert!(min3("abc").is_none());

        let exact = matches("secret");
        assert!(exact("wrong").is_some());
        assert!(exact("secret").is_none());
    }

    #[test]
    fn password_strength_buckets() {
        let p = PasswordStrength::new("");
        assert_eq!(p.strength(), 0);

        let p = PasswordStrength::new("password");
        assert_eq!(p.strength(), 1); // 8+ chars only

        let p = PasswordStrength::new("StrongP@ss123");
        assert_eq!(p.strength(), 4);
    }

    #[test]
    fn no_panic_on_small_sizes() {
        let fields = vec![FormField::new("Test")];
        let sizes = [
            Rect::new(0, 0, 1, 1),
            Rect::new(0, 0, 3, 2),
            Rect::new(0, 0, 20, 5),
            Rect::new(0, 0, 80, 24),
        ];

        for style in [
            FormStyle::Stacked,
            FormStyle::Aligned,
            FormStyle::Inline,
            FormStyle::Floating,
            FormStyle::Grid,
            FormStyle::Cards,
            FormStyle::Wizard,
            FormStyle::Compact,
        ] {
            for &area in &sizes {
                let mut buf = Buffer::empty(area);
                Form::new()
                    .style(style)
                    .fields(&fields)
                    .render(area, &mut buf);
            }
        }
    }
}
