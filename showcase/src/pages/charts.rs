//! Chart widgets: line, bar, scatter graphs with live animation.

use std::time::Instant;

use tuiforge::draw::bold;
use tuiforge::prelude::*;

use super::{Ctx, Page};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Id {
    Spark1,
    Spark2,
    Spark3,
    BarChart,
    LineChart,
    Heatmap,
    Meter1,
    Meter2,
    Meter3,
}

pub struct ChartsPage {
    focus: Focus<Id>,
    // Live data
    spark_data: Vec<f64>,
    bar_data: Vec<BarGroup>,
    line_marker: f64,
    paused: bool,
    spark_style: SparkStyle,
    bar_horizontal: bool,
    last_update: Option<Instant>,
}

impl Default for ChartsPage {
    fn default() -> Self {
        Self {
            focus: Focus::new([
                Id::Spark1,
                Id::Spark2,
                Id::Spark3,
                Id::BarChart,
                Id::LineChart,
                Id::Heatmap,
                Id::Meter1,
                Id::Meter2,
                Id::Meter3,
            ]),
            spark_data: vec![20.0, 35.0, 50.0, 45.0, 60.0, 55.0, 70.0, 65.0],
            bar_data: vec![
                BarGroup {
                    label: "Q1".into(),
                    values: vec![120.0, 80.0, 95.0],
                },
                BarGroup {
                    label: "Q2".into(),
                    values: vec![150.0, 110.0, 130.0],
                },
                BarGroup {
                    label: "Q3".into(),
                    values: vec![180.0, 140.0, 160.0],
                },
                BarGroup {
                    label: "Q4".into(),
                    values: vec![200.0, 170.0, 190.0],
                },
            ],
            line_marker: 0.0,
            paused: false,
            spark_style: SparkStyle::Bars,
            bar_horizontal: false,
            last_update: None,
        }
    }
}

impl ChartsPage {
    fn update_live_data(&mut self, now: Instant) {
        if self.paused {
            return;
        }
        // Throttle updates to ~10 fps
        if let Some(last) = self.last_update
            && now.duration_since(last).as_millis() < 100
        {
            return;
        }
        self.last_update = Some(now);

        // Deterministic pseudo-random walk seeded by elapsed time
        let seed = now.elapsed().as_millis() as u64;
        let val = ((seed * 1103515245 + 12345) % 100) as f64;
        self.spark_data.push(val);
        if self.spark_data.len() > 50 {
            self.spark_data.remove(0);
        }

        // Animate line marker
        let elapsed = (seed as f64 / 1000.0) % 10.0;
        self.line_marker = elapsed;
    }
}

impl Page for ChartsPage {
    fn title(&self) -> &'static str {
        "Charts"
    }

    fn subtitle(&self) -> &'static str {
        "Sparklines, graphs, heatmaps, gauges"
    }

    fn icon(&self) -> &'static str {
        "~"
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        self.update_live_data(ctx.now);
        ctx.area = area;
        let th = &ctx.theme;

        if area.height < 20 {
            put_centered(
                buf,
                area,
                "Too small (need ≥20 rows)",
                st(th.text_muted, th.background),
            );
            return;
        }

        // Layout: 4 rows of content
        let rows = stack(area, &[8, 13, 11, 8], 1);

        // Row 1: Live sparklines
        if let Some(&r1) = rows.first() {
            let inner = Border::Round.draw_titled_with(
                buf,
                r1,
                th.border_blurred,
                th.background,
                "Live Sparklines",
                Alignment::Left,
                bold(st(th.text, th.background)),
            );
            if inner.width > 10 && inner.height >= 4 {
                let cols = columns(inner, 3, 2);
                let gradient = [th.primary, th.accent];

                // Spark 1: Bars with gradient
                if let Some(&c) = cols.first() {
                    let spark_inner = Border::Panel.draw_titled_with(
                        buf,
                        c,
                        th.border_blurred,
                        th.background,
                        "Bars",
                        Alignment::Left,
                        bold(st(th.text, th.background)),
                    );
                    SparkChart::new(&self.spark_data)
                        .style(SparkStyle::Bars)
                        .gradient(&gradient)
                        .show_last_value(true)
                        .theme(th)
                        .render(spark_inner, buf);
                }

                // Spark 2: Line
                if let Some(&c) = cols.get(1) {
                    let spark_inner = Border::Panel.draw_titled_with(
                        buf,
                        c,
                        th.border_blurred,
                        th.background,
                        "Line",
                        Alignment::Left,
                        bold(st(th.text, th.background)),
                    );
                    SparkChart::new(&self.spark_data)
                        .style(SparkStyle::Line)
                        .color(th.secondary)
                        .show_last_value(true)
                        .theme(th)
                        .render(spark_inner, buf);
                }

                // Spark 3: Area
                if let Some(&c) = cols.get(2) {
                    let spark_inner = Border::Panel.draw_titled_with(
                        buf,
                        c,
                        th.border_blurred,
                        th.background,
                        "Area",
                        Alignment::Left,
                        bold(st(th.text, th.background)),
                    );
                    SparkChart::new(&self.spark_data)
                        .style(SparkStyle::Area)
                        .gradient(&gradient)
                        .theme(th)
                        .render(spark_inner, buf);
                }
            }
        }

        // Row 2: Bar graph + Line graph
        if let Some(&r2) = rows.get(1) {
            let [left, right] =
                Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(r2);

            // Bar graph
            if left.width > 20 {
                let bar_inner = Border::Round.draw_titled_with(
                    buf,
                    left,
                    th.border_blurred,
                    th.background,
                    "Revenue by Region",
                    Alignment::Left,
                    bold(st(th.text, th.background)),
                );
                BarGraph::new(&self.bar_data)
                    .series_names(&["APAC", "EMEA", "AMER"])
                    .horizontal(self.bar_horizontal)
                    .show_values(true)
                    .bar_width(3)
                    .gap(1)
                    .theme(th)
                    .render(bar_inner, buf);
            }

            // Line graph with animated marker
            if right.width > 20 {
                let line_inner = Border::Round.draw_titled_with(
                    buf,
                    right,
                    th.border_blurred,
                    th.background,
                    "Multi-Series",
                    Alignment::Left,
                    bold(st(th.text, th.background)),
                );
                let series = vec![
                    LineSeries {
                        name: "CPU".into(),
                        points: vec![
                            (0.0, 20.0),
                            (1.0, 35.0),
                            (2.0, 30.0),
                            (3.0, 50.0),
                            (4.0, 45.0),
                            (5.0, 65.0),
                            (6.0, 60.0),
                            (7.0, 80.0),
                            (8.0, 75.0),
                            (9.0, 85.0),
                            (10.0, 90.0),
                        ],
                        color: Some(th.primary),
                        style: LineStyle::Line,
                    },
                    LineSeries {
                        name: "Memory".into(),
                        points: vec![
                            (0.0, 10.0),
                            (1.0, 15.0),
                            (2.0, 25.0),
                            (3.0, 30.0),
                            (4.0, 35.0),
                            (5.0, 40.0),
                            (6.0, 50.0),
                            (7.0, 55.0),
                            (8.0, 60.0),
                            (9.0, 70.0),
                            (10.0, 75.0),
                        ],
                        color: Some(th.secondary),
                        style: LineStyle::Area,
                    },
                    LineSeries {
                        name: "Marker".into(),
                        points: vec![(self.line_marker, 0.0), (self.line_marker, 100.0)],
                        color: Some(th.accent),
                        style: LineStyle::Line,
                    },
                ];
                LineGraph::new(&series)
                    .grid(true)
                    .legend(LegendPos::TopLeft)
                    .theme(th)
                    .render(line_inner, buf);
            }
        }

        // Row 3: Heatmap + Activity + Scatter
        if let Some(&r3) = rows.get(2) {
            let cols = columns(r3, 3, 1);

            // Heatmap: weekday × hour
            if let Some(&c) = cols.first() {
                let hm_inner = Border::Round.draw_titled_with(
                    buf,
                    c,
                    th.border_blurred,
                    th.background,
                    "Usage Heat",
                    Alignment::Left,
                    bold(st(th.text, th.background)),
                );
                let data: Vec<Vec<f64>> = (0..7)
                    .map(|day| {
                        (0..24)
                            .map(|hour| {
                                // Simulate weekday/weekend + peak hours pattern
                                let base = if day < 5 { 30.0 } else { 15.0 };
                                let peak = if (9..18).contains(&hour) { 40.0 } else { 0.0 };
                                base + peak + ((day * hour) % 20) as f64
                            })
                            .collect()
                    })
                    .collect();
                Heatmap::new(&data)
                    .row_labels(&["M", "T", "W", "T", "F", "S", "S"])
                    .legend(false)
                    .theme(th)
                    .render(hm_inner, buf);
            }

            // Activity graph
            if let Some(&c) = cols.get(1) {
                let ag_inner = Border::Round.draw_titled_with(
                    buf,
                    c,
                    th.border_blurred,
                    th.background,
                    "52-Week Activity",
                    Alignment::Left,
                    bold(st(th.text, th.background)),
                );
                let mut vals = vec![0u8; 364];
                // Deterministic pattern with varied levels
                for (i, v) in vals.iter_mut().enumerate() {
                    *v = (((i * 17 + i / 7) % 37) / 9) as u8; // 0-4 range
                }
                ActivityGraph::new(&vals).theme(th).render(ag_inner, buf);
            }

            // Scatter plot
            if let Some(&c) = cols.get(2) {
                let sc_inner = Border::Round.draw_titled_with(
                    buf,
                    c,
                    th.border_blurred,
                    th.background,
                    "Scatter",
                    Alignment::Left,
                    bold(st(th.text, th.background)),
                );
                let mut points1 = Vec::new();
                let mut points2 = Vec::new();
                for i in 0..20 {
                    let x = i as f64 * 0.5;
                    let y1 = 10.0 + (i % 7) as f64 * 3.0;
                    let y2 = 30.0 + (i % 5) as f64 * 4.0;
                    points1.push((x, y1));
                    points2.push((x, y2));
                }
                let series = vec![
                    LineSeries {
                        name: "A".into(),
                        points: points1,
                        color: Some(th.primary),
                        style: LineStyle::Points,
                    },
                    LineSeries {
                        name: "B".into(),
                        points: points2,
                        color: Some(th.accent),
                        style: LineStyle::Points,
                    },
                ];
                ScatterPlot::new(&series).theme(th).render(sc_inner, buf);
            }
        }

        // Row 4: Meters
        if let Some(&r4) = rows.get(3) {
            let inner = Border::Round.draw_titled_with(
                buf,
                r4,
                th.border_blurred,
                th.background,
                "Gauges",
                Alignment::Left,
                bold(st(th.text, th.background)),
            );
            if inner.height >= 3 {
                let rows = stack(inner, &[1, 1, 1, 1, 1], 0);
                let heat = [th.success, th.warning, th.error];
                let cool = [th.primary.blend(th.background, 0.5), th.primary];
                let meters = [
                    Meter::new()
                        .value(0.65)
                        .label("CPU Load")
                        .show_percent(true)
                        .thresholds(&[
                            (0.0, Variant::Success),
                            (0.7, Variant::Warning),
                            (0.9, Variant::Error),
                        ])
                        .style(MeterStyle::Line),
                    Meter::new()
                        .value(0.82)
                        .label("Memory  ")
                        .show_percent(true)
                        .thresholds(&[(0.0, Variant::Primary), (0.8, Variant::Warning)])
                        .style(MeterStyle::Block),
                    Meter::new()
                        .value(0.45)
                        .label("Disk    ")
                        .show_percent(true)
                        .style(MeterStyle::Segments(20)),
                    Meter::new()
                        .value(0.73)
                        .label("Used    ")
                        .show_percent(true)
                        .suffix("665 GiB")
                        .style(MeterStyle::Blocks)
                        .gradient(&heat),
                    Meter::new()
                        .value(0.30)
                        .label("Core 3  ")
                        .show_percent(true)
                        .style(MeterStyle::Dots)
                        .gradient(&cool),
                ];
                for (m, r) in meters.into_iter().zip(rows) {
                    m.theme(th).render(r, buf);
                }
            }
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        match ev {
            Event::Key(k) if is_press(k) => match k.code {
                KeyCode::Char(' ') => {
                    self.paused = !self.paused;
                    ctx.notify(
                        if self.paused { "Paused" } else { "Playing" },
                        Variant::Primary,
                    );
                    Outcome::Changed
                }
                KeyCode::Char('1') => {
                    self.spark_style = SparkStyle::Bars;
                    ctx.notify("Spark: Bars", Variant::Primary);
                    Outcome::Changed
                }
                KeyCode::Char('2') => {
                    self.spark_style = SparkStyle::Line;
                    ctx.notify("Spark: Line", Variant::Primary);
                    Outcome::Changed
                }
                KeyCode::Char('3') => {
                    self.spark_style = SparkStyle::Area;
                    ctx.notify("Spark: Area", Variant::Primary);
                    Outcome::Changed
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    for g in &mut self.bar_data {
                        for v in &mut g.values {
                            *v *= 1.2;
                        }
                    }
                    ctx.notify("Bar data increased", Variant::Success);
                    Outcome::Changed
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    for g in &mut self.bar_data {
                        for v in &mut g.values {
                            *v *= 0.8;
                        }
                    }
                    ctx.notify("Bar data decreased", Variant::Warning);
                    Outcome::Changed
                }
                KeyCode::Char('h') => {
                    self.bar_horizontal = !self.bar_horizontal;
                    ctx.notify(
                        if self.bar_horizontal {
                            "Horizontal bars"
                        } else {
                            "Vertical bars"
                        },
                        Variant::Primary,
                    );
                    Outcome::Changed
                }
                KeyCode::Tab => {
                    self.focus.next();
                    Outcome::Consumed
                }
                _ => Outcome::Ignored,
            },
            Event::Mouse(_) => Outcome::Ignored,
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self, _now: Instant) -> bool {
        !self.paused
    }

    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("Space", "pause/play"),
            ("1-3", "spark style"),
            ("+/-", "bar data"),
            ("h", "horizontal"),
        ]
    }
}
