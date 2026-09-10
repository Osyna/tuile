//! Content widgets showcase: text, markdown, log, calendar, color, steps.

use std::time::Instant;

use tuiforge::draw::{Border, fill, put, st};

use tuiforge::prelude::*;
use tuiforge::layout::stack;
use tuiforge::runtime::local_ymd;
use tuiforge::anim::elapsed;

use crate::pages::{Ctx, Page};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Id {
    Markdown,
    Log,
    Calendar,
    DatePicker,
    ColorPicker,
    Swatches,
    Steps,
}

pub struct ContentPage {
    focus: Focus<Id>,
    markdown: MarkdownState,
    log: LogViewState,
    calendar: CalendarState,
    datepicker: DatePickerState,
    colorpicker: ColorPickerState,
    swatches: SwatchesState,
    steps: StepsState,
    log_timer: Option<Instant>,
}

impl ContentPage {
    fn new() -> Self {
        let mut page = Self {
            focus: Focus::new([Id::Markdown, Id::Log, Id::Calendar, Id::DatePicker, Id::ColorPicker, Id::Swatches, Id::Steps]),
            markdown: MarkdownState::default(),
            log: LogViewState::default(),
            calendar: CalendarState::default(),
            datepicker: DatePickerState::default(),
            colorpicker: ColorPickerState::default(),
            swatches: SwatchesState::default(),
            steps: StepsState::default(),
            log_timer: None,
        };

        // Pre-populate log
        page.log.push(LogLevel::Info, "Application started");
        page.log.push(LogLevel::Success, "Configuration loaded successfully");
        page.log.push(LogLevel::Debug, "Database connection established");
        page.log.push(LogLevel::Warn, "Cache size approaching limit");
        page.log.push(LogLevel::Error, "Failed to fetch remote data");
        page.log.set_level_column(true);
        page.log.max_lines = Some(100);

        // Calendar marks
        let today = local_ymd();
        page.calendar.set_today(today.0, today.1, today.2);

        // Swatches
        page.swatches.selected = Some(0);

        page
    }
}

impl Default for ContentPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Page for ContentPage {
    fn title(&self) -> &'static str {
        "Content"
    }

    fn subtitle(&self) -> &'static str {
        "Text, markdown, logs, calendars, colors and progress"
    }

    fn icon(&self) -> &'static str {
        "¶"
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        if area.width < 80 || area.height < 20 {
            fill(buf, area, ctx.theme.surface);
            put(buf, area.x + 2, area.y + 2, "Content page requires at least 80×20", 50, st(ctx.theme.text_muted, ctx.theme.surface));
            return;
        }

        fill(buf, area, ctx.theme.background);

        // Two columns
        let split_x = area.width / 2;
        let left = Rect { x: area.x, y: area.y, width: split_x.saturating_sub(1), height: area.height };
        let right = Rect { x: area.x + split_x, y: area.y, width: area.width.saturating_sub(split_x), height: area.height };

        self.draw_left(left, buf, ctx);
        self.draw_right(right, buf, ctx);

        // Overlay (datepicker calendar)
        if self.datepicker.open {
            DatePicker::new().focused(self.focus.is(Id::DatePicker)).theme(&ctx.theme).render_overlay(&mut self.datepicker, buf, area);
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        // Tab cycles focus
        if let Event::Key(k) = ev
            && is_press(k) && k.code == KeyCode::Tab {
                if k.modifiers.contains(KeyModifiers::SHIFT) {
                    self.focus.prev();
                } else {
                    self.focus.next();
                }
                return Outcome::Consumed;
            }

        // Route to focused widget
        let mut out = Outcome::Ignored;
        match self.focus.current() {
            Some(Id::Markdown) => {
                if let Event::Key(k) = ev {
                    out = self.markdown.handle_key(*k);
                } else if let Event::Mouse(m) = ev {
                    out = self.markdown.handle_mouse(*m);
                }
            }
            Some(Id::Log) => {
                if let Event::Key(k) = ev {
                    out = self.log.handle_key(*k);
                } else if let Event::Mouse(m) = ev {
                    out = self.log.handle_mouse(*m);
                }
            }
            Some(Id::Calendar) => {
                if let Event::Key(k) = ev {
                    out = self.calendar.handle_key(*k);
                    if out.is_changed()
                        && let Some((y, m, d)) = self.calendar.selected {
                            ctx.notify(format!("Selected {}-{:02}-{:02}", y, m, d), Variant::Success);
                        }
                } else if let Event::Mouse(m) = ev {
                    out = self.calendar.handle_mouse(*m);
                }
            }
            Some(Id::DatePicker) => {
                if let Event::Key(k) = ev {
                    out = self.datepicker.handle_key(*k);
                    if out.is_changed()
                        && let Some((y, m, d)) = self.datepicker.selected() {
                            ctx.notify(format!("DatePicker: {}-{:02}-{:02}", y, m, d), Variant::Primary);
                        }
                }
            }
            Some(Id::ColorPicker) => {
                if let Event::Key(k) = ev {
                    out = self.colorpicker.handle_key(*k);
                    if out.is_changed() {
                        let rgb = self.colorpicker.value();
                        ctx.notify(format!("Color: #{:02X}{:02X}{:02X}", rgb.0, rgb.1, rgb.2), Variant::Accent);
                    }
                } else if let Event::Mouse(m) = ev {
                    out = self.colorpicker.handle_mouse(*m);
                }
            }
            Some(Id::Swatches) => {
                if let Event::Key(k) = ev {
                    out = self.swatches.handle_key(*k);
                    if out.is_changed()
                        && let Some(idx) = self.swatches.selected {
                            ctx.notify(format!("Swatch {}", idx), Variant::Secondary);
                        }
                } else if let Event::Mouse(m) = ev {
                    out = self.swatches.handle_mouse(*m);
                }
            }
            Some(Id::Steps) => {
                if let Event::Key(k) = ev {
                    out = self.steps.handle_key(*k);
                } else if let Event::Mouse(m) = ev {
                    out = self.steps.handle_mouse(*m);
                    if let Some(idx) = self.steps.take_clicked() {
                        ctx.notify(format!("Step {} clicked", idx + 1), Variant::Success);
                    }
                }
            }
            None => {}
        }

        // Forward all mouse events to all widgets (for hover, scrollbars, etc.)
        if let Event::Mouse(m) = ev {
            self.markdown.handle_mouse(*m);
            self.log.handle_mouse(*m);
            self.calendar.handle_mouse(*m);
            // the picker opens on click from anywhere, so it takes focus and reports here
            let dp = self.datepicker.handle_mouse(*m);
            if dp != Outcome::Ignored {
                if mouse_in(self.datepicker.hit.area, m) {
                    self.focus.set(Id::DatePicker);
                }
                if dp.is_changed()
                    && let Some((y, m, d)) = self.datepicker.selected() {
                        ctx.notify(format!("DatePicker: {}-{:02}-{:02}", y, m, d), Variant::Primary);
                    }
                out |= dp;
            }
            self.colorpicker.handle_mouse(*m);
            self.swatches.handle_mouse(*m);
            self.steps.handle_mouse(*m);
        }

        out
    }

    fn animating(&self, now: Instant) -> bool {
        // animate log entries
        if let Some(t) = self.log_timer {
            elapsed(t, now) > 0.7
        } else {
            false
        }
    }

    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[("Tab", "Focus"), ("f", "Log follow"), ("t", "Today")]
    }
}

impl ContentPage {
    fn draw_left(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let sections = stack(area, &[20, 8, 1, 4], 1);

        // Markdown section
        if let Some(md_area) = sections.first() {
            self.draw_markdown(*md_area, buf, ctx);
        }

        // Text widgets gallery
        if let Some(text_area) = sections.get(1) {
            self.draw_text_gallery(*text_area, buf, ctx);
        }

        // Rule
        if let Some(rule_area) = sections.get(2) {
            Rule::horizontal().style(RuleStyle::Solid).title("Divider").theme(&ctx.theme).render(*rule_area, buf);
        }

        // Stat cards
        if let Some(stats_area) = sections.get(3) {
            self.draw_stats(*stats_area, buf, ctx);
        }
    }

    fn draw_right(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let sections = stack(area, &[10, 10, 12, 4, 6], 0);

        // Log
        if let Some(log_area) = sections.first() {
            self.draw_log(*log_area, buf, ctx);
        }

        // Calendar & DatePicker
        if let Some(cal_area) = sections.get(1) {
            self.draw_calendar_section(*cal_area, buf, ctx);
        }

        // Color widgets
        if let Some(color_area) = sections.get(2) {
            self.draw_color_section(*color_area, buf, ctx);
        }

        // Steps horizontal
        if let Some(steps_area) = sections.get(3) {
            let inner = Border::Round.draw_titled_with(
                buf, *steps_area, ctx.theme.border_blurred, ctx.theme.background, "Steps", Alignment::Left,
                st(ctx.theme.text, ctx.theme.background).add_modifier(Modifier::BOLD)
            );
            Steps::new(&["Init", "Build", "Test", "Deploy"])
                .active(2)
                .theme(&ctx.theme)
                .render(inner, buf, &mut self.steps);
        }

        // Timeline
        if let Some(timeline_area) = sections.get(4) {
            self.draw_timeline(*timeline_area, buf, ctx);
        }
    }

    fn draw_markdown(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let md_src = r#"# Markdown Demo

This is a **CommonMark** subset renderer. It supports:

- **Bold** and *italic* text
- Inline `code` with background
- [Links](https://example.com) (URL dropped)
- ~~Strikethrough~~ text

## Lists and Tasks

1. First item
2. Second item
3. Third item

- [ ] Todo item
- [x] Completed task

> Blockquotes with a left bar
> and italic dim text.

### Code Blocks

```rust
fn main() {
    println!("Hello, world!");
}
```

| Feature | Status |
| ------- | ------ |
| Headings | ✓ |
| Lists | ✓ |
| Tables | ✓ |

---
"#;

        let _focused = self.focus.is(Id::Markdown);

        let inner = Border::Round.draw_titled_with(
            buf, area, ctx.theme.border_blurred, ctx.theme.background, "Markdown", Alignment::Left,
            st(ctx.theme.text, ctx.theme.background).add_modifier(Modifier::BOLD)
        );

        Markdown::new(md_src).theme(&ctx.theme).render(inner, buf, &mut self.markdown);
    }
    fn draw_text_gallery(&self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let inner = Border::Round.draw_titled_with(
            buf, area, ctx.theme.border_blurred, ctx.theme.background, "Text Widgets", Alignment::Left,
            st(ctx.theme.text, ctx.theme.background).add_modifier(Modifier::BOLD)
        );

        let rows = stack(inner, &[1, 1, 1, 1, 1, 1, 1], 0);

        if let Some(r) = rows.first() {
            Label::new("Primary label").variant(Variant::Primary).bold(true).theme(&ctx.theme).render(*r, buf);
        }
        if let Some(r) = rows.get(1) {
            let mut x = r.x;
            Badge::new("NEW").variant(Variant::Accent).theme(&ctx.theme).render(Rect { x, y: r.y, width: 5, height: 1 }, buf);
            x += 6;
            Badge::new("BETA").variant(Variant::Warning).style(BadgeStyle::Outline).theme(&ctx.theme).render(Rect { x, y: r.y, width: 6, height: 1 }, buf);
            x += 7;
            Pill::new("v2.0").variant(Variant::Success).theme(&ctx.theme).render(Rect { x, y: r.y, width: 6, height: 1 }, buf);
        }
        if let Some(r) = rows.get(2) {
            let mut x = r.x;
            KeyCap::new("⌘K").theme(&ctx.theme).render(Rect { x, y: r.y, width: 4, height: 1 }, buf);
            x += 5;
            KeyCap::new("Esc").theme(&ctx.theme).render(Rect { x, y: r.y, width: 5, height: 1 }, buf);
        }
        if let Some(r) = rows.get(3) {
            Link::new("Documentation").url("https://docs.rs").show_url(false).theme(&ctx.theme).render(*r, buf, &mut LinkState::default());
        }
        if let Some(r) = rows.get(4) {
            MarkupLabel::new("[b]Bold[/b] [i]italic[/] [accent]accent[/] [#FF5733]custom[/]").theme(&ctx.theme).render(*r, buf);
        }
    }

    fn draw_stats(&self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let cols = tuiforge::layout::columns(area, 3, 1);

        if let Some(c) = cols.first() {
            StatCard::new("1.2k", "Users")
                .delta(12.5, true)
                .trend(&[10.0, 12.0, 11.0, 15.0, 14.0, 18.0, 20.0])
                .variant(Variant::Success)
                .icon("👤")
                .bordered(true)
                .theme(&ctx.theme)
                .render(*c, buf);
        }
        if let Some(c) = cols.get(1) {
            StatCard::new("89%", "Uptime")
                .delta(2.1, false)
                .variant(Variant::Warning)
                .icon("⚡")
                .bordered(true)
                .theme(&ctx.theme)
                .render(*c, buf);
        }
        if let Some(c) = cols.get(2) {
            StatCard::new("42ms", "Latency")
                .trend(&[50.0, 45.0, 40.0, 42.0, 38.0, 42.0])
                .variant(Variant::Primary)
                .icon("⏱")
                .bordered(true)
                .theme(&ctx.theme)
                .render(*c, buf);
        }
    }

    fn draw_log(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        // Auto-add log entry every ~700ms
        if self.log_timer.is_none() {
            self.log_timer = Some(ctx.now);
        }
        if let Some(t) = self.log_timer
            && elapsed(t, ctx.now) > 0.7 {
                let levels = [LogLevel::Info, LogLevel::Debug, LogLevel::Warn, LogLevel::Success, LogLevel::Error];
                let messages = [
                    "Processing batch job",
                    "Cache hit ratio: 92%",
                    "Memory usage high",
                    "Backup completed",
                    "Connection timeout",
                ];
                let idx = (self.log.len() % levels.len()).min(messages.len() - 1);
                self.log.push(levels[idx], messages[idx]);
                self.log_timer = Some(ctx.now);
            }

        let _focused = self.focus.is(Id::Log);
        LogView::new()
            .border(Border::Round)
            .title("Live Log")
            .level_column(true)
            .max_lines(100)
            .theme(&ctx.theme)
            .render(area, buf, &mut self.log);
    }

    fn draw_calendar_section(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let cols = tuiforge::layout::columns(area, 2, 1);

        // Calendar
        if let Some(cal_area) = cols.first() {
            let inner = Border::Round.draw_titled_with(
                buf, *cal_area, ctx.theme.border_blurred, ctx.theme.background, "Calendar", Alignment::Left,
                st(ctx.theme.text, ctx.theme.background).add_modifier(Modifier::BOLD)
            );
            let focused = self.focus.is(Id::Calendar);
            let marks = [(ctx.theme.primary, Variant::Primary),
                (ctx.theme.warning, Variant::Warning),
                (ctx.theme.success, Variant::Success)];
            let today = local_ymd();
            let month_marks: Vec<(i32, u32, u32, Variant)> = (1..=5)
                .map(|d| {
                    let v = marks[d % marks.len()].1;
                    (today.0, today.1, ((d * 6) as u32).min(28), v)
                })
                .collect();

            Calendar::new()
                .focused(focused)
                .marks(&month_marks)
                .theme(&ctx.theme)
                .render(inner, buf, &mut self.calendar);
        }

        // DatePicker
        if let Some(dp_area) = cols.get(1) {
            let inner = Border::Round.draw_titled_with(
                buf, *dp_area, ctx.theme.border_blurred, ctx.theme.background, "DatePicker", Alignment::Left,
                st(ctx.theme.text, ctx.theme.background).add_modifier(Modifier::BOLD)
            );
            let field_area = Rect { x: inner.x, y: inner.y + 1, width: inner.width, height: 3 };
            let focused = self.focus.is(Id::DatePicker);
            DatePicker::new()
                .focused(focused)
                .theme(&ctx.theme)
                .render(field_area, buf, &mut self.datepicker);
        }
    }

    fn draw_color_section(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let inner = Border::Round.draw_titled_with(
            buf, area, ctx.theme.border_blurred, ctx.theme.background, "Color Widgets", Alignment::Left,
            st(ctx.theme.text, ctx.theme.background).add_modifier(Modifier::BOLD)
        );

        let rows = stack(inner, &[10, 2, 2], 0);

        // ColorPicker
        if let Some(picker_area) = rows.first() {
            let _focused = self.focus.is(Id::ColorPicker);
            ColorPicker::new().theme(&ctx.theme).render(*picker_area, buf, &mut self.colorpicker);
        }

        // Swatches
        if let Some(sw_area) = rows.get(1) {
            let colors = vec![
                ctx.theme.primary,
                ctx.theme.secondary,
                ctx.theme.accent,
                ctx.theme.success,
                ctx.theme.warning,
                ctx.theme.error,
            ];
            let labels = ["Pri", "Sec", "Acc", "OK", "Warn", "Err"];
            Swatches::new(&colors).labels(&labels).cell_width(5).theme(&ctx.theme).render(*sw_area, buf, &mut self.swatches);
        }

        // GradientBar
        if let Some(grad_area) = rows.get(2) {
            let stops = vec![ctx.theme.primary, ctx.theme.accent, ctx.theme.warning];
            GradientBar::new(&stops).labels("Min", "Max").marker(0.6).theme(&ctx.theme).render(*grad_area, buf);
        }
    }

    fn draw_timeline(&self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let inner = Border::Round.draw_titled_with(
            buf, area, ctx.theme.border_blurred, ctx.theme.background, "Timeline", Alignment::Left,
            st(ctx.theme.text, ctx.theme.background).add_modifier(Modifier::BOLD)
        );

        let entries = vec![
            TimelineEntry {
                time: "10:00".to_string(),
                title: "Project started".to_string(),
                description: Some("Initial commit".to_string()),
                variant: Variant::Success,
            },
            TimelineEntry {
                time: "12:30".to_string(),
                title: "Build completed".to_string(),
                description: None,
                variant: Variant::Primary,
            },
            TimelineEntry {
                time: "14:15".to_string(),
                title: "Tests failed".to_string(),
                description: Some("2 tests failing".to_string()),
                variant: Variant::Error,
            },
        ];

        Timeline::new(entries).theme(&ctx.theme).render(inner, buf);
    }
}
