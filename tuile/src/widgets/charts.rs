//! Charts: sparklines, bar/line/scatter/heatmap graphs, activity grids, gauges.
//!
//! ```no_run
//! use tuile::prelude::*;
//! # let area = Rect::new(0, 0, 80, 24);
//! # let mut buf = Buffer::empty(area);
//! # let th = theme::current();
//! // Live sparkline
//! let mut spark_data = vec![1.0, 2.0, 5.0, 3.0];
//! SparkChart::new(&spark_data)
//!     .gradient(&[th.primary, th.accent])
//!     .baseline(true)
//!     .render(area, &mut buf);
//! ```

use std::f64::consts::PI;

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::widgets::Widget;

use crate::draw::{
    bold, fill, hbar, put, put_centered, put_right, st, truncate, vbar, width as text_width,
};
use crate::theme::{self, Rgb, Theme, Variant, gradient as color_gradient};

// helpers

/// Nice bounds for axis scaling: expands (min, max) to round numbers and picks a step.
pub fn nice_bounds(min: f64, max: f64) -> (f64, f64, f64) {
    if !min.is_finite() || !max.is_finite() || min >= max {
        return (0.0, 1.0, 0.2);
    }
    let range = max - min;
    let exp = 10_f64.powf(range.log10().floor());
    let frac = range / exp;
    let nice_range = if frac <= 1.0 {
        1.0
    } else if frac <= 2.0 {
        2.0
    } else if frac <= 5.0 {
        5.0
    } else {
        10.0
    } * exp;
    let tick_spacing = nice_range / 5.0;
    let nice_min = (min / tick_spacing).floor() * tick_spacing;
    let nice_max = (max / tick_spacing).ceil() * tick_spacing;
    (nice_min, nice_max, tick_spacing)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn map_range(v: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    if (in_max - in_min).abs() < 1e-9 {
        return out_min;
    }
    let t = (v - in_min) / (in_max - in_min);
    lerp(out_min, out_max, t.clamp(0.0, 1.0))
}

// braille canvas

/// 2×4 dot rasterizer for line graphs and scatter plots. Each cell holds 8 dots arranged:
/// ```text
/// ⠁ ⠈  (dots 0-3 left column, 4-7 right column, bottom to top)
/// ⠂ ⠐
/// ⠄ ⠠
/// ⡀ ⢀
/// ```
/// Braille unicode: U+2800 + bit pattern where bit 0 = top-left, bit 7 = bottom-right.
pub struct BrailleCanvas {
    width: usize,
    height: usize,
    /// (dot_mask, color) per cell; last-set-wins for color.
    cells: Vec<(u8, Rgb)>,
}

impl BrailleCanvas {
    /// Create a canvas `width_cells` × `height_cells`.
    pub fn new(width_cells: usize, height_cells: usize) -> Self {
        Self {
            width: width_cells,
            height: height_cells,
            cells: vec![(0, Rgb(0, 0, 0)); width_cells * height_cells],
        }
    }

    /// Set a single dot at pixel coordinates (x, y); coords are in dots (width*2, height*4).
    pub fn set(&mut self, x: usize, y: usize, color: Rgb) {
        let cx = x / 2;
        let cy = y / 4;
        if cx >= self.width || cy >= self.height {
            return;
        }
        let dx = x % 2;
        let dy = y % 4;
        // Braille bit layout: column 0: bits 0,1,2,6; column 1: bits 3,4,5,7 (bottom to top)
        let bit = match (dx, dy) {
            (0, 0) => 0,
            (0, 1) => 1,
            (0, 2) => 2,
            (0, 3) => 6,
            (1, 0) => 3,
            (1, 1) => 4,
            (1, 2) => 5,
            (1, 3) => 7,
            _ => return,
        };
        let idx = cy * self.width + cx;
        self.cells[idx].0 |= 1 << bit;
        self.cells[idx].1 = color; // last-set-wins
    }

    /// Bresenham line from (x0, y0) to (x1, y1) in dot coordinates.
    pub fn line(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, color: Rgb) {
        let (mut x0, mut y0, x1, y1) = (x0 as isize, y0 as isize, x1 as isize, y1 as isize);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            if x0 >= 0 && y0 >= 0 {
                self.set(x0 as usize, y0 as usize, color);
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// Render into the buffer area with the given background color.
    pub fn render(&self, area: Rect, buf: &mut Buffer, bg: Rgb) {
        let area = area.intersection(buf.area);
        for cy in 0..self.height.min(area.height as usize) {
            for cx in 0..self.width.min(area.width as usize) {
                let idx = cy * self.width + cx;
                let (mask, fg) = self.cells[idx];
                let glyph = if mask == 0 {
                    ' '
                } else {
                    char::from_u32(0x2800 + mask as u32).unwrap_or(' ')
                };
                let x = area.x + cx as u16;
                let y = area.y + cy as u16;
                if buf.area.contains(ratatui_core::layout::Position { x, y }) {
                    buf[(x, y)]
                        .set_symbol(&glyph.to_string())
                        .set_fg(fg.color())
                        .set_bg(bg.color());
                }
            }
        }
    }
}

// spark chart

/// Style for sparkline rendering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SparkStyle {
    /// Block bars `▁▂▃▄▅▆▇█`.
    #[default]
    Bars,
    /// Braille line connecting values.
    Line,
    /// Braille line with a dimmed area fill below.
    Area,
    /// btop-style braille dot field: every dot under the value lit, coloured by its own height
    /// when a gradient is set.
    Field,
}

/// Compact sparkline: `▁▂▃▄▅▆▇█` bars, braille line, area or dot field. Fits width by showing
/// the last N values. `.mirrored(true)` hangs the graph from the top edge (btop's upload half).
#[derive(Clone, Debug)]
pub struct SparkChart<'a> {
    values: &'a [f64],
    min: Option<f64>,
    max: Option<f64>,
    color: Option<Rgb>,
    gradient_stops: Option<&'a [Rgb]>,
    baseline: bool,
    mirrored: bool,
    style: SparkStyle,
    show_last: bool,
    theme: Option<Theme>,
}

impl<'a> SparkChart<'a> {
    pub fn new(values: &'a [f64]) -> Self {
        Self {
            values,
            min: None,
            max: None,
            color: None,
            gradient_stops: None,
            baseline: false,
            mirrored: false,
            style: SparkStyle::Bars,
            show_last: false,
            theme: None,
        }
    }

    pub fn min(mut self, v: f64) -> Self {
        self.min = Some(v);
        self
    }
    pub fn max(mut self, v: f64) -> Self {
        self.max = Some(v);
        self
    }
    pub fn color(mut self, c: Rgb) -> Self {
        self.color = Some(c);
        self
    }
    pub fn gradient(mut self, stops: &'a [Rgb]) -> Self {
        self.gradient_stops = Some(stops);
        self
    }
    pub fn baseline(mut self, v: bool) -> Self {
        self.baseline = v;
        self
    }
    /// Grow downward from the top edge instead of up from the bottom.
    pub fn mirrored(mut self, v: bool) -> Self {
        self.mirrored = v;
        self
    }
    pub fn style(mut self, s: SparkStyle) -> Self {
        self.style = s;
        self
    }
    pub fn show_last_value(mut self, v: bool) -> Self {
        self.show_last = v;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for SparkChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.background);

        let label_w = if self.show_last { 6 } else { 0 };
        let chart_w = area.width.saturating_sub(label_w) as usize;
        if chart_w == 0 {
            return;
        }

        let vals: Vec<f64> = self
            .values
            .iter()
            .rev()
            .take(chart_w)
            .rev()
            .copied()
            .collect();
        if vals.is_empty() {
            if self.baseline && area.height > 0 {
                for x in 0..area.width {
                    put(
                        buf,
                        area.x + x,
                        area.bottom().saturating_sub(1),
                        "▁",
                        1,
                        st(th.text_disabled, th.background),
                    );
                }
            }
            return;
        }

        let min = self
            .min
            .unwrap_or_else(|| vals.iter().copied().fold(f64::INFINITY, f64::min));
        let max = self
            .max
            .unwrap_or_else(|| vals.iter().copied().fold(f64::NEG_INFINITY, f64::max));

        let frac_of = |v: f64| {
            if (max - min).abs() < 1e-9 {
                0.5
            } else {
                ((v - min) / (max - min)).clamp(0.0, 1.0)
            }
        };
        let color_at = |frac: f64| {
            self.gradient_stops
                .map_or(self.color.unwrap_or(th.primary), |stops| {
                    color_gradient(stops, frac as f32)
                })
        };

        match self.style {
            SparkStyle::Bars => {
                for (i, &v) in vals.iter().enumerate() {
                    let frac = frac_of(v);
                    let x = area.x + i as u16;
                    if self.mirrored {
                        // hang from the top: full cells painted, the partial cell uses upper eighths
                        let cells = frac as f32 * area.height as f32;
                        let full = cells.floor() as u16;
                        fill(
                            buf,
                            Rect {
                                x,
                                y: area.y,
                                width: 1,
                                height: full.min(area.height),
                            },
                            color_at(frac),
                        );
                        if full < area.height {
                            let idx = ((cells - full as f32) * 8.0).round() as usize;
                            if idx > 0 {
                                // an upper partial is the inverse of a lower one: paint fg=bg, bg=color
                                put(
                                    buf,
                                    x,
                                    area.y + full,
                                    crate::draw::LOWER_BLOCKS[8 - idx.min(8)],
                                    1,
                                    st(th.background, color_at(frac)),
                                );
                            }
                        }
                    } else {
                        vbar(
                            buf,
                            x,
                            area.y,
                            area.height,
                            frac as f32,
                            color_at(frac),
                            th.background,
                        );
                    }
                }
            }
            SparkStyle::Line | SparkStyle::Area | SparkStyle::Field => {
                let mut canvas = BrailleCanvas::new(chart_w, area.height as usize);
                let h_dots = (area.height as usize) * 4;
                // dot row for a fraction; mirrored charts hang from row 0
                let row_of = |frac: f64| {
                    let r = (frac * (h_dots as f64 - 1.0)).round() as usize;
                    if self.mirrored { r } else { h_dots - 1 - r }
                };
                let mut prev: Option<usize> = None;
                for (i, &v) in vals.iter().enumerate() {
                    let frac = frac_of(v);
                    let y = row_of(frac);
                    let color = color_at(frac);
                    match self.style {
                        SparkStyle::Field => {
                            // every dot between the edge and the value, coloured by its own height
                            let (lo, hi) = if self.mirrored {
                                (0, y)
                            } else {
                                (y, h_dots - 1)
                            };
                            for dy in lo..=hi {
                                let h = if self.mirrored {
                                    dy as f64
                                } else {
                                    (h_dots - 1 - dy) as f64
                                } / (h_dots as f64 - 1.0).max(1.0);
                                let c = if self.gradient_stops.is_some() {
                                    color_at(h)
                                } else {
                                    color
                                };
                                canvas.set(i * 2, dy, c);
                                canvas.set(i * 2 + 1, dy, c);
                            }
                        }
                        _ => {
                            match prev {
                                Some(py) => canvas.line(i * 2 - 2, py, i * 2, y, color),
                                None => {
                                    canvas.set(i * 2, y, color);
                                    canvas.set(i * 2 + 1, y, color);
                                }
                            }
                            if self.style == SparkStyle::Area {
                                let (lo, hi) = if self.mirrored {
                                    (0, y)
                                } else {
                                    (y, h_dots - 1)
                                };
                                for dy in lo..=hi {
                                    canvas.set(i * 2, dy, color.blend(th.background, 0.3));
                                    canvas.set(i * 2 + 1, dy, color.blend(th.background, 0.3));
                                }
                            }
                        }
                    }
                    prev = Some(y);
                }
                canvas.render(
                    Rect {
                        width: chart_w as u16,
                        ..area
                    },
                    buf,
                    th.background,
                );
            }
        }

        if self.show_last && !vals.is_empty() {
            let last = vals[vals.len() - 1];
            let label = format!("{last:.1}");
            put_right(buf, area, &label, st(th.text, th.background));
        }
    }
}

// bar graph

/// One group of bars in a [`BarGraph`].
#[derive(Clone, Debug)]
pub struct BarGroup {
    pub label: String,
    pub values: Vec<f64>,
}

/// Grouped bar chart (vertical or horizontal).
#[derive(Clone, Debug)]
pub struct BarGraph<'a> {
    groups: &'a [BarGroup],
    series_names: &'a [&'a str],
    colors: Option<&'a [Rgb]>,
    horizontal: bool,
    show_values: bool,
    bar_width: u16,
    gap: u16,
    max_override: Option<f64>,
    axis: bool,
    theme: Option<Theme>,
}

impl<'a> BarGraph<'a> {
    pub fn new(groups: &'a [BarGroup]) -> Self {
        Self {
            groups,
            series_names: &[],
            colors: None,
            horizontal: false,
            show_values: false,
            bar_width: 4,
            gap: 1,
            max_override: None,
            axis: true,
            theme: None,
        }
    }

    pub fn series_names(mut self, names: &'a [&'a str]) -> Self {
        self.series_names = names;
        self
    }
    pub fn colors(mut self, c: &'a [Rgb]) -> Self {
        self.colors = Some(c);
        self
    }
    pub fn horizontal(mut self, v: bool) -> Self {
        self.horizontal = v;
        self
    }
    pub fn show_values(mut self, v: bool) -> Self {
        self.show_values = v;
        self
    }
    pub fn bar_width(mut self, w: u16) -> Self {
        self.bar_width = w.max(1);
        self
    }
    pub fn gap(mut self, g: u16) -> Self {
        self.gap = g;
        self
    }
    pub fn max(mut self, m: f64) -> Self {
        self.max_override = Some(m);
        self
    }
    pub fn axis(mut self, v: bool) -> Self {
        self.axis = v;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for BarGraph<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.groups.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.background);

        let default_colors = [
            th.primary,
            th.secondary,
            th.accent,
            th.success,
            th.warning,
            th.error,
        ];
        let colors = self.colors.unwrap_or(&default_colors);

        let max_val = self.max_override.unwrap_or_else(|| {
            self.groups
                .iter()
                .flat_map(|g| &g.values)
                .copied()
                .fold(0.0, f64::max)
        });
        if max_val <= 0.0 {
            return;
        }

        if self.horizontal {
            // Horizontal bars: label column + bars
            let label_w = self
                .groups
                .iter()
                .map(|g| text_width(&g.label))
                .max()
                .unwrap_or(0)
                .min(20) as u16;
            let chart_area = Rect {
                x: area.x + label_w + 1,
                width: area.width.saturating_sub(label_w + 1),
                ..area
            };
            let row_h = 1 + self.gap;
            for (i, group) in self.groups.iter().enumerate() {
                let y = area.y + (i as u16) * row_h;
                if y >= area.bottom() {
                    break;
                }
                put(
                    buf,
                    area.x,
                    y,
                    &group.label,
                    label_w,
                    st(th.text_muted, th.background),
                );
                for (j, &val) in group.values.iter().enumerate() {
                    let color = colors[j % colors.len()];
                    let frac = (val / max_val).clamp(0.0, 1.0);
                    let bar_y = y + j as u16;
                    if bar_y < area.bottom() && chart_area.width > 0 {
                        hbar(
                            buf,
                            chart_area.x,
                            bar_y,
                            chart_area.width,
                            frac as f32,
                            color,
                            th.background,
                        );
                        if self.show_values && chart_area.width > 6 {
                            let label = format!("{val:.0}");
                            let lx = chart_area.x
                                + ((frac * chart_area.width as f64) as u16)
                                    .saturating_sub(label.len() as u16 + 1);
                            put(
                                buf,
                                lx,
                                bar_y,
                                &label,
                                chart_area.width,
                                st(th.background, color),
                            );
                        }
                    }
                }
            }
        } else {
            // Vertical bars
            let axis_h = if self.axis { 1 } else { 0 };
            let label_h = 1;
            let value_h = if self.show_values { 1 } else { 0 };
            let chart_h = area.height.saturating_sub(axis_h + label_h + value_h);
            if chart_h == 0 {
                return;
            }
            let chart_y = area.y + value_h;

            let n_series = self.groups.first().map(|g| g.values.len()).unwrap_or(0);
            let group_w =
                self.bar_width * n_series as u16 + self.gap * (n_series.saturating_sub(1)) as u16;
            let total_w = self.groups.len() as u16 * (group_w + self.gap);
            let start_x = area.x + area.width.saturating_sub(total_w.min(area.width)) / 2;

            for (gi, group) in self.groups.iter().enumerate() {
                let gx = start_x + (gi as u16) * (group_w + self.gap);
                if gx >= area.right() {
                    break;
                }
                for (si, &val) in group.values.iter().enumerate() {
                    let color = colors[si % colors.len()];
                    let bx = gx + (si as u16) * (self.bar_width + self.gap);
                    if bx + self.bar_width > area.right() {
                        break;
                    }
                    let frac = (val / max_val).clamp(0.0, 1.0);
                    for dx in 0..self.bar_width {
                        vbar(
                            buf,
                            bx + dx,
                            chart_y,
                            chart_h,
                            frac as f32,
                            color,
                            th.background,
                        );
                    }
                    if self.show_values {
                        let label = format!("{val:.0}");
                        let filled = (frac * chart_h as f64).ceil() as u16;
                        // one row above the bar's top cell; the reserved row keeps this inside `area`
                        let ly = (chart_y + chart_h - filled).saturating_sub(1).max(area.y);
                        put_centered(
                            buf,
                            Rect {
                                x: bx,
                                y: ly,
                                width: self.bar_width,
                                height: 1,
                            },
                            &label,
                            st(th.text, th.background),
                        );
                    }
                }
                // Group label
                let ly = area.bottom().saturating_sub(label_h);
                let label = &group.label;
                let truncated = truncate(label, group_w as usize);
                put_centered(
                    buf,
                    Rect {
                        x: gx,
                        y: ly,
                        width: group_w,
                        height: 1,
                    },
                    &truncated,
                    st(th.text_muted, th.background),
                );
            }

            if self.axis {
                let baseline_y = chart_y + chart_h;
                for x in area.x..area.right() {
                    put(
                        buf,
                        x,
                        baseline_y,
                        "▔",
                        1,
                        st(th.border_blurred, th.background),
                    );
                }
            }
        }
    }
}

// line graph

/// Line style for a series in [`LineGraph`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineStyle {
    #[default]
    Line,
    Area,
    Points,
}

/// Legend position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LegendPos {
    Hidden,
    TopLeft,
    #[default]
    TopRight,
    Bottom,
}

/// One data series for [`LineGraph`].
#[derive(Clone, Debug)]
pub struct LineSeries {
    pub name: String,
    pub points: Vec<(f64, f64)>,
    pub color: Option<Rgb>,
    pub style: LineStyle,
}

/// Multi-series line graph with axes, grid, and legend.
#[derive(Clone, Debug)]
pub struct LineGraph<'a> {
    series: &'a [LineSeries],
    x_bounds: Option<(f64, f64)>,
    y_bounds: Option<(f64, f64)>,
    x_labels: usize,
    y_labels: usize,
    label_fmt: Option<fn(f64) -> String>,
    grid: bool,
    legend: LegendPos,
    title: Option<&'a str>,
    theme: Option<Theme>,
}

impl<'a> LineGraph<'a> {
    pub fn new(series: &'a [LineSeries]) -> Self {
        Self {
            series,
            x_bounds: None,
            y_bounds: None,
            x_labels: 5,
            y_labels: 5,
            label_fmt: None,
            grid: false,
            legend: LegendPos::TopRight,
            title: None,
            theme: None,
        }
    }

    pub fn x_bounds(mut self, min: f64, max: f64) -> Self {
        self.x_bounds = Some((min, max));
        self
    }
    pub fn y_bounds(mut self, min: f64, max: f64) -> Self {
        self.y_bounds = Some((min, max));
        self
    }
    pub fn x_labels(mut self, n: usize) -> Self {
        self.x_labels = n;
        self
    }
    pub fn y_labels(mut self, n: usize) -> Self {
        self.y_labels = n;
        self
    }
    pub fn label_fmt(mut self, f: fn(f64) -> String) -> Self {
        self.label_fmt = Some(f);
        self
    }
    pub fn grid(mut self, v: bool) -> Self {
        self.grid = v;
        self
    }
    pub fn legend(mut self, p: LegendPos) -> Self {
        self.legend = p;
        self
    }
    pub fn title(mut self, t: &'a str) -> Self {
        self.title = Some(t);
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for LineGraph<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 5 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.background);

        // Compute bounds
        let (x_min, x_max) = self.x_bounds.unwrap_or_else(|| {
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            for s in self.series {
                for &(x, _) in &s.points {
                    min = min.min(x);
                    max = max.max(x);
                }
            }
            if !min.is_finite() || !max.is_finite() || min >= max {
                (0.0, 1.0)
            } else {
                let (nm, nx, _) = nice_bounds(min, max);
                (nm, nx)
            }
        });
        let (y_min, y_max) = self.y_bounds.unwrap_or_else(|| {
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            for s in self.series {
                for &(_, y) in &s.points {
                    min = min.min(y);
                    max = max.max(y);
                }
            }
            if !min.is_finite() || !max.is_finite() || min >= max {
                (0.0, 1.0)
            } else {
                let (nm, nx, _) = nice_bounds(min, max);
                (nm, nx)
            }
        });

        // Layout: title, y-axis labels, chart, x-axis labels
        let title_h = if self.title.is_some() { 1 } else { 0 };
        let y_label_w = 8;
        let x_label_h = 1;
        let chart_area = Rect {
            x: area.x + y_label_w,
            y: area.y + title_h,
            width: area.width.saturating_sub(y_label_w),
            height: area.height.saturating_sub(title_h + x_label_h),
        };
        if chart_area.width == 0 || chart_area.height == 0 {
            return;
        }

        // Title
        if let Some(t) = self.title {
            put_centered(
                buf,
                Rect { height: 1, ..area },
                t,
                bold(st(th.text, th.background)),
            );
        }

        // Grid
        if self.grid && chart_area.height > 2 {
            for i in 0..=self.y_labels {
                let y = chart_area.y
                    + ((i as f64 / self.y_labels as f64) * chart_area.height as f64) as u16;
                if y >= chart_area.bottom() {
                    continue;
                }
                for x in chart_area.x..chart_area.right() {
                    put(buf, x, y, "·", 1, st(th.border_blurred, th.background));
                }
            }
        }

        // Axes
        let axis_x = chart_area.x.saturating_sub(1);
        let axis_y = chart_area.bottom();
        for y in chart_area.y..chart_area.bottom() {
            put(buf, axis_x, y, "│", 1, st(th.text_muted, th.background));
        }
        for x in chart_area.x..chart_area.right() {
            put(buf, x, axis_y, "─", 1, st(th.text_muted, th.background));
        }

        // Y labels
        let fmt = self.label_fmt.unwrap_or(|v| format!("{v:.1}"));
        for i in 0..=self.y_labels {
            let frac = i as f64 / self.y_labels as f64;
            let val = lerp(y_max, y_min, frac);
            let label = fmt(val);
            let y = chart_area.y + (frac * chart_area.height as f64) as u16;
            if y < chart_area.bottom() {
                put(
                    buf,
                    area.x,
                    y,
                    &label,
                    y_label_w.saturating_sub(1),
                    st(th.text_muted, th.background),
                );
            }
        }

        // X labels
        for i in 0..=self.x_labels {
            let frac = i as f64 / self.x_labels as f64;
            let val = lerp(x_min, x_max, frac);
            let label = fmt(val);
            let x = chart_area.x + (frac * chart_area.width as f64) as u16;
            if x < chart_area.right() {
                put(
                    buf,
                    x,
                    axis_y + 1,
                    &label,
                    6,
                    st(th.text_muted, th.background),
                );
            }
        }

        // Series
        let mut canvas = BrailleCanvas::new(chart_area.width as usize, chart_area.height as usize);
        let w_dots = (chart_area.width as usize) * 2;
        let h_dots = (chart_area.height as usize) * 4;

        let default_colors = [th.primary, th.secondary, th.accent];
        for (si, series) in self.series.iter().enumerate() {
            if series.points.is_empty() {
                continue;
            }
            let color = series
                .color
                .unwrap_or(default_colors[si % default_colors.len()]);
            for i in 0..series.points.len() {
                let (x, y) = series.points[i];
                let px = map_range(x, x_min, x_max, 0.0, (w_dots - 1) as f64) as usize;
                let py = map_range(y, y_min, y_max, (h_dots - 1) as f64, 0.0) as usize;
                if series.style == LineStyle::Points {
                    canvas.set(px, py, color);
                } else if i > 0 {
                    let (px0, py0) = series.points[i - 1];
                    let px0 = map_range(px0, x_min, x_max, 0.0, (w_dots - 1) as f64) as usize;
                    let py0 = map_range(py0, y_min, y_max, (h_dots - 1) as f64, 0.0) as usize;
                    canvas.line(px0, py0, px, py, color);
                }
                if series.style == LineStyle::Area {
                    for dy in py..h_dots {
                        canvas.set(px, dy, color.blend(th.background, 0.5));
                    }
                }
            }
        }
        canvas.render(chart_area, buf, th.background);

        // Legend
        if self.legend != LegendPos::Hidden && !self.series.is_empty() {
            let items: Vec<_> = self.series.iter().map(|s| s.name.as_str()).collect();
            let max_w = items
                .iter()
                .map(|s| text_width(s))
                .max()
                .unwrap_or(0)
                .min(20);
            let leg_w = (max_w + 4) as u16;
            let leg_h = (items.len() as u16).min(chart_area.height.saturating_sub(2));
            let (lx, ly) = match self.legend {
                LegendPos::TopLeft => (chart_area.x + 2, chart_area.y + 1),
                LegendPos::TopRight => (
                    chart_area.right().saturating_sub(leg_w + 2),
                    chart_area.y + 1,
                ),
                LegendPos::Bottom => (
                    chart_area.x + (chart_area.width.saturating_sub(leg_w)) / 2,
                    chart_area.bottom().saturating_sub(leg_h + 1),
                ),
                LegendPos::Hidden => return,
            };
            let leg_area = Rect {
                x: lx,
                y: ly,
                width: leg_w,
                height: leg_h,
            };
            fill(buf, leg_area, th.panel);
            for (i, series) in self.series.iter().take(leg_h as usize).enumerate() {
                let color = series
                    .color
                    .unwrap_or(default_colors[i % default_colors.len()]);
                put(buf, lx + 1, ly + i as u16, "●", 1, st(color, th.panel));
                put(
                    buf,
                    lx + 3,
                    ly + i as u16,
                    &series.name,
                    leg_w.saturating_sub(4),
                    st(th.text, th.panel),
                );
            }
        }
    }
}

// scatter plot

/// Scatter plot (thin wrapper over [`LineGraph`] with `Points` style).
#[derive(Clone, Debug)]
pub struct ScatterPlot<'a> {
    series: &'a [LineSeries],
    x_bounds: Option<(f64, f64)>,
    y_bounds: Option<(f64, f64)>,
    theme: Option<Theme>,
}

impl<'a> ScatterPlot<'a> {
    pub fn new(series: &'a [LineSeries]) -> Self {
        Self {
            series,
            x_bounds: None,
            y_bounds: None,
            theme: None,
        }
    }

    pub fn x_bounds(mut self, min: f64, max: f64) -> Self {
        self.x_bounds = Some((min, max));
        self
    }
    pub fn y_bounds(mut self, min: f64, max: f64) -> Self {
        self.y_bounds = Some((min, max));
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for ScatterPlot<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut graph = LineGraph::new(self.series).legend(LegendPos::TopRight);
        if let Some((min, max)) = self.x_bounds {
            graph = graph.x_bounds(min, max);
        }
        if let Some((min, max)) = self.y_bounds {
            graph = graph.y_bounds(min, max);
        }
        if let Some(th) = &self.theme {
            graph = graph.theme(th);
        }
        graph.render(area, buf);
    }
}

// heatmap

/// 2D heatmap with gradient coloring.
#[derive(Clone, Debug)]
pub struct Heatmap<'a> {
    values: &'a [Vec<f64>],
    row_labels: &'a [&'a str],
    col_labels: &'a [&'a str],
    gradient_stops: &'a [Rgb],
    cell_width: u16,
    show_values: bool,
    legend: bool,
    null_color: Rgb,
    theme: Option<Theme>,
}

impl<'a> Heatmap<'a> {
    pub fn new(values: &'a [Vec<f64>]) -> Self {
        Self {
            values,
            row_labels: &[],
            col_labels: &[],
            gradient_stops: &[],
            cell_width: 2,
            show_values: false,
            legend: false,
            null_color: Rgb(50, 50, 50),
            theme: None,
        }
    }

    pub fn row_labels(mut self, l: &'a [&'a str]) -> Self {
        self.row_labels = l;
        self
    }
    pub fn col_labels(mut self, l: &'a [&'a str]) -> Self {
        self.col_labels = l;
        self
    }
    pub fn gradient(mut self, stops: &'a [Rgb]) -> Self {
        self.gradient_stops = stops;
        self
    }
    pub fn cell_width(mut self, w: u16) -> Self {
        self.cell_width = w.max(1);
        self
    }
    pub fn show_values(mut self, v: bool) -> Self {
        self.show_values = v;
        self
    }
    pub fn legend(mut self, v: bool) -> Self {
        self.legend = v;
        self
    }
    pub fn null_color(mut self, c: Rgb) -> Self {
        self.null_color = c;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for Heatmap<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.values.is_empty() {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.background);

        let default_gradient = [th.surface, th.primary, th.accent];
        let gradient = if self.gradient_stops.is_empty() {
            &default_gradient
        } else {
            self.gradient_stops
        };

        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for row in self.values {
            for &v in row {
                if v.is_finite() {
                    min = min.min(v);
                    max = max.max(v);
                }
            }
        }
        if !min.is_finite() {
            min = 0.0;
        }
        if !max.is_finite() {
            max = 1.0;
        }

        let row_label_w = self
            .row_labels
            .iter()
            .map(|s| text_width(s))
            .max()
            .unwrap_or(0)
            .min(12) as u16;
        let col_label_h = if self.col_labels.is_empty() { 0 } else { 1 };
        let legend_h = if self.legend { 2 } else { 0 };
        let grid_area = Rect {
            x: area.x + row_label_w,
            y: area.y + col_label_h,
            width: area.width.saturating_sub(row_label_w),
            height: area.height.saturating_sub(col_label_h + legend_h),
        };
        if grid_area.width == 0 || grid_area.height == 0 {
            return;
        }

        let _rows = self.values.len();
        let cols = self.values.first().map(|r| r.len()).unwrap_or(0);
        let cell_h = 1; // ponytail: compact mode uses half-blocks, still 1 row per cell

        // Column labels
        if !self.col_labels.is_empty() {
            for (ci, &label) in self.col_labels.iter().take(cols).enumerate() {
                let cx = grid_area.x + (ci as u16) * self.cell_width;
                if cx < grid_area.right() {
                    put_centered(
                        buf,
                        Rect {
                            x: cx,
                            y: area.y,
                            width: self.cell_width,
                            height: 1,
                        },
                        label,
                        st(th.text_muted, th.background),
                    );
                }
            }
        }

        // Grid
        for (ri, row) in self.values.iter().enumerate() {
            let ry = grid_area.y + (ri as u16) * cell_h;
            if ry >= grid_area.bottom() {
                break;
            }
            if ri < self.row_labels.len() {
                put(
                    buf,
                    area.x,
                    ry,
                    self.row_labels[ri],
                    row_label_w.saturating_sub(1),
                    st(th.text_muted, th.background),
                );
            }
            for (ci, &val) in row.iter().enumerate() {
                let cx = grid_area.x + (ci as u16) * self.cell_width;
                if cx + self.cell_width > grid_area.right() {
                    break;
                }
                let color = if val.is_nan() {
                    self.null_color
                } else {
                    let frac = if (max - min).abs() < 1e-9 {
                        0.5
                    } else {
                        ((val - min) / (max - min)).clamp(0.0, 1.0)
                    };
                    color_gradient(gradient, frac as f32)
                };
                let cell_rect = Rect {
                    x: cx,
                    y: ry,
                    width: self.cell_width,
                    height: cell_h,
                };
                fill(buf, cell_rect, color);
                if self.show_values && self.cell_width >= 4 && !val.is_nan() {
                    let label = format!("{val:.0}");
                    put_centered(buf, cell_rect, &label, st(th.background, color));
                }
            }
        }

        // Legend
        if self.legend {
            let ly = area.bottom().saturating_sub(legend_h);
            let bar_w = area.width.saturating_sub(20).min(40);
            let bar_x = area.x + (area.width.saturating_sub(bar_w)) / 2;
            for i in 0..bar_w {
                let frac = i as f32 / bar_w as f32;
                let color = color_gradient(gradient, frac);
                put(buf, bar_x + i, ly, " ", 1, st(color, color));
            }
            let min_label = format!("{min:.1}");
            let max_label = format!("{max:.1}");
            put(
                buf,
                bar_x.saturating_sub(min_label.len() as u16 + 1),
                ly,
                &min_label,
                10,
                st(th.text_muted, th.background),
            );
            put(
                buf,
                bar_x + bar_w + 1,
                ly,
                &max_label,
                10,
                st(th.text_muted, th.background),
            );
        }
    }
}

// activity graph

/// GitHub-style contribution grid: weeks as columns, days as rows. Cells are painted
/// (2 cells per week: colour + gutter) so low levels stay faithful in every terminal.
#[derive(Clone, Debug)]
pub struct ActivityGraph<'a> {
    values: &'a [u8],
    levels: &'a [Rgb],
    theme: Option<Theme>,
}

impl<'a> ActivityGraph<'a> {
    /// Values are 0..=4 intensity levels, one per day, up to 364 days (52 weeks).
    pub fn new(values: &'a [u8]) -> Self {
        Self {
            values,
            levels: &[],
            theme: None,
        }
    }

    pub fn levels(mut self, l: &'a [Rgb]) -> Self {
        self.levels = l;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for ActivityGraph<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 8 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.background);

        let default_levels = [
            th.surface,
            th.primary.blend(th.surface, 0.75),
            th.primary.blend(th.surface, 0.5),
            th.primary.blend(th.surface, 0.25),
            th.primary,
        ];
        let levels = if self.levels.is_empty() {
            &default_levels
        } else {
            self.levels
        };

        let weeks = (self.values.len() / 7).min(52);
        let cell_w = 2;
        let _cell_h = 1;
        let grid_w = weeks * cell_w;
        let start_x = area.x + (area.width.saturating_sub(grid_w as u16)) / 2;

        // Month labels (approximate)
        let months = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        for (mi, &m) in months.iter().enumerate() {
            let week_offset = (mi * 4).min(weeks.saturating_sub(1));
            let mx = start_x + (week_offset * cell_w) as u16;
            if mx < area.right() {
                put(buf, mx, area.y, m, 3, st(th.text_muted, th.background));
            }
        }

        // Grid
        for week in 0..weeks {
            for day in 0..7 {
                let idx = week * 7 + day;
                if idx >= self.values.len() {
                    break;
                }
                let level = (self.values[idx] as usize).min(levels.len() - 1);
                let color = levels[level];
                let cx = start_x + (week * cell_w) as u16;
                let cy = area.y + 2 + day as u16;
                if cx < area.right() && cy < area.bottom() {
                    fill(
                        buf,
                        Rect {
                            x: cx,
                            y: cy,
                            width: cell_w as u16,
                            height: 1,
                        },
                        color,
                    );
                }
            }
        }
    }
}

// meter

/// Meter display style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MeterStyle {
    /// Horizontal line `━━━━╺━━`.
    #[default]
    Line,
    /// Painted fill.
    Block,
    /// `n` painted segments with one-cell gaps.
    Segments(u16),
    /// btop-style LED row: one `■` per cell, empties dimmed.
    Blocks,
    /// Compact LED row of `●` dots (per-core meters).
    Dots,
}

/// Horizontal gauge/meter with threshold or gradient colouring.
///
/// ```no_run
/// use tuile::prelude::*;
/// # let area = Rect::new(0, 0, 40, 1);
/// # let mut buf = Buffer::empty(area);
/// Meter::new().value(0.73).label("Used:").show_percent(true).suffix("665 GiB")
///     .style(MeterStyle::Blocks).gradient(&[Rgb(80, 200, 120), Rgb(240, 200, 60), Rgb(230, 80, 80)])
///     .render(area, &mut buf);
/// ```
pub struct Meter<'a> {
    value: f32,
    label: Option<String>,
    suffix: Option<String>,
    show_percent: bool,
    thresholds: Vec<(f32, Variant)>,
    color: Option<Rgb>,
    gradient: Option<&'a [Rgb]>,
    style: MeterStyle,
    compact: bool,
    theme: Option<Theme>,
}

impl<'a> Meter<'a> {
    pub fn new() -> Self {
        Self {
            value: 0.0,
            label: None,
            suffix: None,
            show_percent: false,
            thresholds: Vec::new(),
            color: None,
            gradient: None,
            style: MeterStyle::Line,
            compact: false,
            theme: None,
        }
    }

    pub fn value(mut self, v: f32) -> Self {
        self.value = v.clamp(0.0, 1.0);
        self
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    /// Right-aligned text after the bar (`665 GiB`).
    pub fn suffix(mut self, s: impl Into<String>) -> Self {
        self.suffix = Some(s.into());
        self
    }
    pub fn show_percent(mut self, v: bool) -> Self {
        self.show_percent = v;
        self
    }
    pub fn thresholds(mut self, t: &[(f32, Variant)]) -> Self {
        self.thresholds = t.to_vec();
        self
    }
    /// Fixed bar colour (overrides thresholds; a gradient still wins).
    pub fn color(mut self, c: Rgb) -> Self {
        self.color = Some(c);
        self
    }
    /// Colour stops along the bar (LED styles colour each cell by position, solid styles by value).
    pub fn gradient(mut self, stops: &'a [Rgb]) -> Self {
        self.gradient = Some(stops);
        self
    }
    pub fn style(mut self, s: MeterStyle) -> Self {
        self.style = s;
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

impl Default for Meter<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Meter<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height == 0 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        let bg = th.background;
        fill(buf, area, bg);

        let label_w = self
            .label
            .as_ref()
            .map(|l| text_width(l) as u16 + 1)
            .unwrap_or(0);
        let pct_w = if self.show_percent { 5 } else { 0 };
        let suffix_w = self
            .suffix
            .as_ref()
            .map(|s| text_width(s) as u16 + 1)
            .unwrap_or(0);
        let bar_w = area.width.saturating_sub(label_w + pct_w + suffix_w);
        if bar_w == 0 {
            return;
        }

        if let Some(l) = &self.label {
            put(buf, area.x, area.y, l, label_w, st(th.text, bg));
        }
        // `Label: 73% [bar] 665 GiB` - percent sits between label and bar like btop
        let mut x = area.x + label_w;
        if self.show_percent {
            put(
                buf,
                x,
                area.y,
                &format!("{:>3.0}% ", self.value * 100.0),
                pct_w,
                st(th.text_muted, bg),
            );
            x += pct_w;
        }

        let solid = self.color.unwrap_or_else(|| {
            self.thresholds
                .iter()
                .rev()
                .find(|&&(t, _)| self.value >= t)
                .map(|&(_, v)| th.variant(v))
                .unwrap_or(th.primary)
        });
        let color = self
            .gradient
            .map_or(solid, |g| color_gradient(g, self.value));
        let empty = th.text_disabled.blend(bg, 0.5);

        match self.style {
            MeterStyle::Line => hbar(buf, x, area.y, bar_w, self.value, color, th.panel),
            MeterStyle::Block => {
                fill(
                    buf,
                    Rect {
                        x,
                        y: area.y,
                        width: bar_w,
                        height: 1,
                    },
                    th.panel,
                );
                let fill_w = (bar_w as f32 * self.value).round() as u16;
                fill(
                    buf,
                    Rect {
                        x,
                        y: area.y,
                        width: fill_w,
                        height: 1,
                    },
                    color,
                );
            }
            MeterStyle::Segments(n) => {
                let seg_w = bar_w / n.max(1);
                let active = (n as f32 * self.value).ceil() as u16;
                for i in 0..n {
                    let sx = x + i * seg_w;
                    if sx >= area.right() {
                        break;
                    }
                    let seg_color = if i < active { color } else { th.panel };
                    fill(
                        buf,
                        Rect {
                            x: sx,
                            y: area.y,
                            width: seg_w.saturating_sub(1),
                            height: 1,
                        },
                        seg_color,
                    );
                }
            }
            MeterStyle::Blocks | MeterStyle::Dots => {
                let glyph = if self.style == MeterStyle::Blocks {
                    "■"
                } else {
                    "●"
                };
                let lit = (bar_w as f32 * self.value).round() as u16;
                for i in 0..bar_w {
                    let c = if i >= lit {
                        empty
                    } else if let Some(g) = self.gradient {
                        color_gradient(g, i as f32 / bar_w.saturating_sub(1).max(1) as f32)
                    } else {
                        solid
                    };
                    put(buf, x + i, area.y, glyph, 1, st(c, bg));
                }
            }
        }

        if let Some(s) = &self.suffix {
            put_right(
                buf,
                Rect {
                    y: area.y,
                    height: 1,
                    ..area
                },
                s,
                st(th.text, bg),
            );
        }
    }
}

// radial gauge

/// Semicircular gauge using braille arc (experimental; may be removed if it doesn't look good).
pub struct RadialGauge {
    value: f32,
    label: Option<String>,
    thickness: u16,
    theme: Option<Theme>,
}

impl RadialGauge {
    pub fn new() -> Self {
        Self {
            value: 0.0,
            label: None,
            thickness: 2,
            theme: None,
        }
    }

    pub fn value(mut self, v: f32) -> Self {
        self.value = v.clamp(0.0, 1.0);
        self
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    pub fn thickness(mut self, t: u16) -> Self {
        self.thickness = t.max(1);
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Default for RadialGauge {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for RadialGauge {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ponytail: radial gauge on braille canvas at 20x10 looks sparse; keeping as a showcase curiosity
        if area.width < 12 || area.height < 6 {
            return;
        }
        let th = self.theme.unwrap_or_else(theme::current);
        fill(buf, area, th.background);

        let cx = area.width as f64 / 2.0;
        let cy = area.height as f64;
        let radius = (area.width as f64 / 2.0).min(area.height as f64) - 1.0;
        let w_dots = (area.width as usize) * 2;
        let h_dots = (area.height as usize) * 4;

        let mut canvas = BrailleCanvas::new(area.width as usize, area.height as usize);

        // Track (full arc 180 to 0 degrees)
        for i in 0..100 {
            let angle = PI + (PI * i as f64 / 100.0);
            let x = (cx + radius * angle.cos()) * 2.0;
            let y = (cy - radius * angle.sin()) * 4.0;
            if x >= 0.0 && x < w_dots as f64 && y >= 0.0 && y < h_dots as f64 {
                canvas.set(x as usize, y as usize, th.panel);
            }
        }

        // Filled arc
        let end_angle = PI + (PI * self.value as f64);
        for i in 0..=(self.value * 100.0) as usize {
            let angle = PI + (PI * i as f64 / 100.0);
            if angle > end_angle {
                break;
            }
            let x = (cx + radius * angle.cos()) * 2.0;
            let y = (cy - radius * angle.sin()) * 4.0;
            if x >= 0.0 && x < w_dots as f64 && y >= 0.0 && y < h_dots as f64 {
                canvas.set(x as usize, y as usize, th.primary);
            }
        }

        canvas.render(area, buf, th.background);

        // Value text
        let pct = format!("{:.0}%", self.value * 100.0);
        put_centered(
            buf,
            Rect {
                y: area.y + area.height.saturating_sub(3),
                height: 1,
                ..area
            },
            &pct,
            bold(st(th.text, th.background)),
        );
        if let Some(ref l) = self.label {
            put_centered(
                buf,
                Rect {
                    y: area.bottom().saturating_sub(2),
                    height: 1,
                    ..area
                },
                l,
                st(th.text_muted, th.background),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn led_meter_lights_cells_by_value_and_keeps_label_percent_suffix() {
        // "Used: 73% ■■■…  665 GiB" - label(6) + pct(5) + bar + suffix(8)
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        Meter::new()
            .value(0.5)
            .label("Used:")
            .show_percent(true)
            .suffix("665 GiB")
            .style(MeterStyle::Blocks)
            .render(buf.area, &mut buf);
        let row: String = (0..40).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(row.starts_with("Used:  50% ■"), "{row}");
        assert!(row.ends_with("665 GiB"), "{row}");
        let bar_w = 40 - 6 - 5 - 8;
        let lit = (0..bar_w)
            .filter(|i| buf[(11 + i, 0)].fg != buf[(11 + bar_w - 1, 0)].fg)
            .count();
        assert_eq!(
            lit,
            (bar_w as f32 * 0.5).round() as usize,
            "half the cells take the lit colour"
        );
    }

    #[test]
    fn field_graph_mirrors_from_the_top_edge() {
        let vals = [1.0, 1.0, 1.0, 1.0];
        let mut up = Buffer::empty(Rect::new(0, 0, 2, 2));
        SparkChart::new(&vals)
            .min(0.0)
            .max(1.0)
            .style(SparkStyle::Field)
            .render(up.area, &mut up);
        let mut down = Buffer::empty(Rect::new(0, 0, 2, 2));
        SparkChart::new(&vals)
            .min(0.0)
            .max(1.0)
            .style(SparkStyle::Field)
            .mirrored(true)
            .render(down.area, &mut down);
        // full value fills both rows either way; a zero value lights only the edge row
        assert_ne!(up[(0, 0)].symbol(), " ");
        assert_ne!(down[(0, 1)].symbol(), " ");
        let zero = [0.0, 0.0];
        let mut z_up = Buffer::empty(Rect::new(0, 0, 1, 2));
        SparkChart::new(&zero)
            .min(0.0)
            .max(1.0)
            .style(SparkStyle::Field)
            .render(z_up.area, &mut z_up);
        assert_eq!(z_up[(0, 0)].symbol(), " ");
        assert_ne!(z_up[(0, 1)].symbol(), " ");
        let mut z_down = Buffer::empty(Rect::new(0, 0, 1, 2));
        SparkChart::new(&zero)
            .min(0.0)
            .max(1.0)
            .style(SparkStyle::Field)
            .mirrored(true)
            .render(z_down.area, &mut z_down);
        assert_ne!(z_down[(0, 0)].symbol(), " ");
        assert_eq!(z_down[(0, 1)].symbol(), " ");
    }

    #[test]
    fn test_nice_bounds() {
        let (min, max, step) = nice_bounds(0.3, 8.7);
        assert!(min <= 0.3);
        assert!(max >= 8.7);
        assert!(step > 0.0);
        assert_eq!(nice_bounds(5.0, 5.0), (0.0, 1.0, 0.2)); // degenerate
    }

    #[test]
    fn test_braille_canvas() {
        let mut canvas = BrailleCanvas::new(4, 2);
        canvas.set(0, 0, Rgb(255, 0, 0));
        canvas.set(1, 1, Rgb(0, 255, 0));
        canvas.line(0, 0, 7, 7, Rgb(0, 0, 255));
        let mut buf = Buffer::empty(Rect {
            x: 0,
            y: 0,
            width: 4,
            height: 2,
        });
        canvas.render(buf.area, &mut buf, Rgb(0, 0, 0));

        // Cell (0,0) has plotted points → braille glyph
        let cell_00 = buf[(0, 0)].symbol();
        assert_ne!(cell_00, " ", "cell (0,0) has dots: {cell_00}");
        // Cell (2,0) is away from the diagonal → blank
        let cell_20 = buf[(2, 0)].symbol();
        assert_eq!(cell_20, " ", "cell (2,0) untouched: {cell_20}");
    }

    #[test]
    fn test_spark_scaling() {
        let vals = vec![1.0, 2.0, 3.0];
        let spark = SparkChart::new(&vals);
        let mut buf = Buffer::empty(Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 3,
        });
        spark.render(buf.area, &mut buf);

        // Bar height must rise with the value: vbar paints a full cell as bg, so counting
        // coloured cells per column gives min=0 < mid < max=3 for values 1, 2, 3.
        let th = theme::current();
        let filled = |x: u16| {
            (0..3)
                .filter(|&y| buf[(x, y)].bg != th.background.color())
                .count()
        };
        let (lo, mid, hi) = (filled(0), filled(1), filled(2));
        assert!(lo < mid && mid < hi, "heights must rise: {lo} {mid} {hi}");
        assert_eq!(hi, 3, "the maximum fills the column: {hi}");
        let empty: Vec<f64> = vec![];
        let spark = SparkChart::new(&empty).baseline(true);
        spark.render(buf.area, &mut buf);

        let nans = vec![f64::NAN, f64::NAN];
        let spark = SparkChart::new(&nans);
        spark.render(buf.area, &mut buf);
    }
    #[test]
    fn test_bar_narrow() {
        // Four groups of 2-cell bars need 12 cells; the area gives 5.
        let groups: Vec<BarGroup> = ["A", "B", "C", "D"]
            .iter()
            .map(|l| BarGroup {
                label: (*l).into(),
                values: vec![5.0],
            })
            .collect();
        let bar = BarGraph::new(&groups).bar_width(2);
        let mut buf = Buffer::empty(Rect {
            x: 0,
            y: 0,
            width: 5,
            height: 5,
        });
        bar.render(buf.area, &mut buf);

        // The contract at this size is: no panic, and the bars that fit are still painted.
        // Off-edge groups are clipped by `put`/`vbar`, so their absence is not observable here.
        let th = theme::current();
        let painted = (0..5)
            .flat_map(|x| (0..5).map(move |y| (x, y)))
            .filter(|&(x, y)| buf[(x, y)].bg != th.background.color())
            .count();
        assert!(painted > 0, "bars that fit are drawn: {painted} cells");
    }

    #[test]
    fn test_heatmap_gradient() {
        let vals = vec![vec![0.0, 0.5, 1.0], vec![f64::NAN, 0.2, 0.8]];
        let gradient = [Rgb(0, 0, 0), Rgb(255, 255, 255)];
        let hm = Heatmap::new(&vals)
            .gradient(&gradient)
            .null_color(Rgb(9, 9, 9))
            .cell_width(2);
        let mut buf = Buffer::empty(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 10,
        });
        hm.render(buf.area, &mut buf);
        // cells are painted (bg), min → first stop, max → last stop, NaN → null colour
        assert_eq!(buf[(0, 0)].bg, Rgb(0, 0, 0).color());
        assert_eq!(buf[(1, 0)].bg, Rgb(0, 0, 0).color());
        assert_eq!(buf[(4, 0)].bg, Rgb(255, 255, 255).color());
        assert_eq!(buf[(0, 1)].bg, Rgb(9, 9, 9).color());
        assert_eq!(buf[(4, 0)].symbol(), " ");
    }

    #[test]
    fn test_activity_grid_weeks() {
        // 364 days = 52 weeks
        let mut vals = vec![0u8; 364];
        vals[0] = 0; // level 0
        vals[7] = 4; // level 4 (max)
        let ag = ActivityGraph::new(&vals);
        let mut buf = Buffer::empty(Rect {
            x: 0,
            y: 0,
            width: 110,
            height: 10,
        });
        ag.render(buf.area, &mut buf);
        // 52 weeks * 2 cells/week = 104 cells wide (centered in 110)
        // start_x = (110 - 104) / 2 = 3
        // Grid starts at cy = area.y + 2 (after month labels)
        let start_x = 3;
        let grid_y = 2;

        // Day 0 (week 0, day 0) at (start_x, grid_y + 0)
        let day0_bg = buf[(start_x, grid_y)].bg;

        // Day 7 (week 1, day 0) at (start_x + 2, grid_y + 0)
        let day7_bg = buf[(start_x + 2, grid_y)].bg;

        // Different levels get different colors
        assert_ne!(
            day0_bg, day7_bg,
            "level 0 and level 4 have different colors: {:?} vs {:?}",
            day0_bg, day7_bg
        );

        // All 52 weeks are drawn: the last column is painted, the cell past it is not.
        let last_week_x = start_x + 51 * 2;
        let bg = theme::current().background.color();
        assert_ne!(buf[(last_week_x, grid_y)].bg, bg, "week 52 is painted");
        assert_eq!(buf[(last_week_x + 2, grid_y)].bg, bg, "no 53rd week");
    }
}
