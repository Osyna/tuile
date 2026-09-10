//! Controls: buttons, toggles, radios, segmented controls, sliders, steppers and ratings —
//! every variant and style side by side, all focusable and mouse-driven.

use tuiforge::draw::{fill, put, st};
use tuiforge::prelude::*;

use super::{Ctx, Page, card};

const BUTTONS: [(&str, Variant, ButtonStyle); 12] = [
    ("Default", Variant::Default, ButtonStyle::Default),
    ("Primary", Variant::Primary, ButtonStyle::Default),
    ("Success", Variant::Success, ButtonStyle::Default),
    ("Warning", Variant::Warning, ButtonStyle::Default),
    ("Error", Variant::Error, ButtonStyle::Default),
    ("Flat", Variant::Primary, ButtonStyle::Flat),
    ("Outline", Variant::Primary, ButtonStyle::Outline),
    ("Outline", Variant::Error, ButtonStyle::Outline),
    ("Ghost", Variant::Default, ButtonStyle::Ghost),
    ("Ghost", Variant::Accent, ButtonStyle::Ghost),
    ("Disabled", Variant::Primary, ButtonStyle::Default),
    ("Compact", Variant::Success, ButtonStyle::Default),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Button(usize),
    Icon,
    Wide,
    Check(usize),
    Switch(usize),
    Radio,
    RadioH,
    CheckList,
    Segmented,
    Volume,
    Brightness,
    Range,
    Stepper,
    Rating,
}

pub struct ControlsPage {
    focus: Focus<Id>,
    buttons: Vec<ButtonState>,
    icon_btn: ButtonState,
    wide_btn: ButtonState,
    checks: [CheckboxState; 4],
    switches: [SwitchState; 3],
    radio: RadioState,
    radio_h: RadioState,
    checklist: CheckListState,
    segmented: SegmentedState,
    volume: SliderState,
    brightness: SliderState,
    range: RangeState,
    stepper: StepperState,
    rating: RatingState,
    presses: usize,
}

impl Default for ControlsPage {
    fn default() -> Self {
        let mut order = vec![];
        order.extend((0..BUTTONS.len()).map(Id::Button));
        order.extend([Id::Icon, Id::Wide]);
        order.extend((0..4).map(Id::Check));
        order.extend((0..3).map(Id::Switch));
        order.extend([Id::Radio, Id::RadioH, Id::CheckList, Id::Segmented, Id::Volume, Id::Brightness, Id::Range, Id::Stepper, Id::Rating]);
        let mut checks = [CheckboxState::new(CheckState::Off), CheckboxState::new(CheckState::On), CheckboxState::new(CheckState::Indeterminate), CheckboxState::new(CheckState::On)];
        checks[2].set_tri_state(true);
        let mut checklist = CheckListState::new(8);
        checklist.checked[1] = true;
        checklist.checked[4] = true;
        ControlsPage {
            focus: Focus::new(order),
            buttons: vec![ButtonState::new(); BUTTONS.len()],
            icon_btn: ButtonState::new(),
            wide_btn: ButtonState::new(),
            checks,
            switches: [SwitchState::new(true), SwitchState::new(false), SwitchState::new(true)],
            radio: RadioState::new(Some(1)),
            radio_h: RadioState::new(Some(0)),
            checklist,
            segmented: SegmentedState::new(1),
            volume: SliderState::new(60.0, 0.0, 100.0, 1.0),
            brightness: SliderState::new(0.75, 0.0, 1.0, 0.05),
            range: RangeState::new(20.0, 80.0, 0.0, 100.0, 1.0),
            stepper: StepperState::new(4, 1, 12, 1),
            rating: RatingState::new(3),
            presses: 0,
        }
    }
}

impl Page for ControlsPage {
    fn title(&self) -> &'static str {
        "Controls"
    }
    fn subtitle(&self) -> &'static str {
        "Buttons, toggles, radios, segmented, sliders, steppers, rating"
    }
    fn icon(&self) -> &'static str {
        "◉"
    }
    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[("Tab", "Focus"), ("Space/Enter", "Activate"), ("←→", "Adjust"), ("drag", "Sliders")]
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        let now = ctx.now;
        let dur = ctx.dur(200);
        let f = |id: Id, focus: &Focus<Id>| focus.is(id);
        let inner = pad(area, 1, 0);
        let [buttons_a, toggles_a, sliders_a] = Layout::vertical([Constraint::Length(13), Constraint::Length(15), Constraint::Fill(1)]).areas(inner);

        // ── Buttons: variants × styles ──
        let b = pad(card(buf, buttons_a, &th, &format!("Buttons  ·  {} presses", self.presses)), 1, 0);
        let bw: u16 = 14;
        let per_row = ((b.width + 1) / (bw + 1)).max(1) as usize;
        for (i, (label, variant, style)) in BUTTONS.iter().enumerate() {
            let (r, c) = (i / per_row, i % per_row);
            let rect = Rect { x: b.x + c as u16 * (bw + 1), y: b.y + r as u16 * 4, width: bw, height: 3 };
            if rect.bottom() > b.bottom() {
                break;
            }
            let compact = *label == "Compact";
            let rect = if compact { Rect { y: rect.y + 1, height: 1, ..rect } } else { rect };
            Button::new(*label)
                .variant(*variant)
                .style(*style)
                .compact(compact)
                .enabled(*label != "Disabled")
                .min_width(bw)
                .focused(f(Id::Button(i), &self.focus))
                .now(now)
                .theme(&th)
                .render(rect, buf, &mut self.buttons[i]);
        }
        let rows_used = BUTTONS.len().div_ceil(per_row) as u16;
        let y = b.y + rows_used * 4;
        if y + 3 <= b.bottom() {
            let icon_r = Rect { x: b.x, y, width: bw + 4, height: 3 };
            Button::new("Save").icon("✔").variant(Variant::Primary).min_width(bw + 4).focused(f(Id::Icon, &self.focus)).now(now).theme(&th).render(icon_r, buf, &mut self.icon_btn);
            let wide_r = Rect { x: icon_r.right() + 1, y, width: b.right().saturating_sub(icon_r.right() + 1), height: 3 };
            Button::new("Full-width button (flat)").style(ButtonStyle::Flat).full_width(true).focused(f(Id::Wide, &self.focus)).now(now).theme(&th).render(wide_r, buf, &mut self.wide_btn);
        }

        // ── Toggles ──
        let t = pad(card(buf, toggles_a, &th, "Toggles"), 1, 0);
        let [c1, c2, c3, c4] = Layout::horizontal([Constraint::Length(30), Constraint::Length(30), Constraint::Length(24), Constraint::Fill(1)]).areas(t);
        // checkboxes + switches
        let labels = ["Checkbox", "Checked", "Indeterminate (tri-state)", "Disabled"];
        for (i, label) in labels.iter().enumerate() {
            let r = Rect { y: c1.y + i as u16, height: 1, ..c1 };
            Checkbox::new(*label).tri_state(i == 2).enabled(i != 3).focused(f(Id::Check(i), &self.focus)).theme(&th).render(r, buf, &mut self.checks[i]);
        }
        let sw_labels = ["Dark mode", "Telemetry", "compact"];
        for (i, label) in sw_labels.iter().enumerate() {
            let compact = i == 2;
            let r = if compact {
                Rect { x: c1.x, y: c1.y + 5 + 3 * 2, width: c1.width, height: 1 }
            } else {
                Rect { x: c1.x, y: c1.y + 5 + i as u16 * 3, width: c1.width, height: 3 }
            };
            if r.bottom() <= t.bottom() {
                Switch::new().label(*label).compact(compact).duration(dur).focused(f(Id::Switch(i), &self.focus)).now(now).theme(&th).render(r, buf, &mut self.switches[i]);
            }
        }
        // radios
        RadioGroup::new(vec!["Small".into(), "Medium".into(), "Large".into(), "Extra large".into()])
            .bordered(true)
            .title("Size")
            .focused(f(Id::Radio, &self.focus))
            .theme(&th)
            .render(Rect { height: 6.min(c2.height), ..c2 }, buf, &mut self.radio);
        let rh = Rect { y: c2.y + 7, height: 1, width: c2.width, x: c2.x };
        if rh.bottom() <= t.bottom() {
            put(buf, rh.x, rh.y - 1, "Colour", 10, st(th.text_muted, th.background));
            RadioGroup::new(vec!["Red".into(), "Green".into(), "Blue".into()]).horizontal(true).focused(f(Id::RadioH, &self.focus)).theme(&th).render(rh, buf, &mut self.radio_h);
        }
        let seg = Rect { y: c2.y + 10, height: 1, width: c2.width, x: c2.x };
        if seg.bottom() <= t.bottom() {
            put(buf, seg.x, seg.y - 1, "Segmented", 10, st(th.text_muted, th.background));
            Segmented::new(vec!["Day".into(), "Week".into(), "Month".into()]).focused(f(Id::Segmented, &self.focus)).theme(&th).render(seg, buf, &mut self.segmented);
        }
        // checklist with scrollbar
        put(buf, c3.x, c3.y, "CheckList", c3.width, st(th.text_muted, th.background));
        let cl = Rect { y: c3.y + 1, height: c3.height.saturating_sub(1).min(7), width: c3.width.saturating_sub(2), x: c3.x };
        CheckList::new(["Autosave", "Format on save", "Line numbers", "Minimap", "Word wrap", "Ligatures", "Bracket pairs", "Sticky scroll"].iter().map(|s| s.to_string()).collect())
            .focused(f(Id::CheckList, &self.focus))
            .now(now)
            .theme(&th)
            .render(cl, buf, &mut self.checklist);
        // state summary
        let checked: Vec<&str> = ["Autosave", "Format on save", "Line numbers", "Minimap", "Word wrap", "Ligatures", "Bracket pairs", "Sticky scroll"]
            .iter()
            .zip(&self.checklist.checked)
            .filter(|(_, on)| **on)
            .map(|(s, _)| *s)
            .collect();
        let summary = [
            format!("checks: {:?}", self.checks.iter().map(|c| c.value).collect::<Vec<_>>()),
            format!("switches: {} {} {}", self.switches[0].on, self.switches[1].on, self.switches[2].on),
            format!("size: {:?}  colour: {:?}", self.radio.selected, self.radio_h.selected),
            format!("segment: {}", ["Day", "Week", "Month"][self.segmented.selected.min(2)]),
            format!("checklist: {}", checked.join(", ")),
        ];
        if c4.width > 10 {
            put(buf, c4.x, c4.y, "state", c4.width, st(th.text_muted, th.background));
            for (i, line) in summary.iter().enumerate() {
                if c4.y + 1 + (i as u16) < c4.bottom() {
                    put(buf, c4.x, c4.y + 1 + i as u16, &truncate(line, c4.width as usize), c4.width, st(th.text, th.background));
                }
            }
        }

        // ── Sliders & steppers ──
        let s = pad(card(buf, sliders_a, &th, "Sliders, steppers & rating"), 1, 0);
        let [s1, s2] = Layout::horizontal([Constraint::Percentage(50), Constraint::Fill(1)]).areas(s);
        let row = |a: Rect, i: u16, h: u16| Rect { y: a.y + i, height: h, width: a.width.saturating_sub(2), ..a };
        Slider::new().label("Volume").show_value(true).format(|v| format!("{v:.0}")).ticks(true).focused(f(Id::Volume, &self.focus)).duration(ctx.dur(150)).now(now).theme(&th).render(row(s1, 0, 3), buf, &mut self.volume);
        Slider::new()
            .label("Brightness")
            .show_value(true)
            .format(|v| format!("{:.0}%", v * 100.0))
            .variant(Variant::Warning)
            .focused(f(Id::Brightness, &self.focus))
            .duration(ctx.dur(150))
            .now(now)
            .theme(&th)
            .render(row(s1, 4, 3), buf, &mut self.brightness);
        RangeSlider::new().label("Price range").variant(Variant::Success).focused(f(Id::Range, &self.focus)).now(now).theme(&th).render(row(s2, 0, 3), buf, &mut self.range);
        let sr = row(s2, 4, 1);
        if sr.bottom() <= s.bottom() {
            put(buf, sr.x, sr.y, "Tab size", 10, st(th.text_muted, th.background));
            Stepper::new().focused(f(Id::Stepper, &self.focus)).theme(&th).render(Rect { x: sr.x + 11, width: 20, ..sr }, buf, &mut self.stepper);
            put(buf, sr.x + 34, sr.y, "Rating", 8, st(th.text_muted, th.background));
            Rating::new().max(5).focused(f(Id::Rating, &self.focus)).theme(&th).render(Rect { x: sr.x + 42, width: 12, ..sr }, buf, &mut self.rating);
        }
        let sv = row(s2, 6, 1);
        if sv.bottom() <= s.bottom() {
            let line = format!(
                "volume {:.0} · brightness {:.0}% · range {:.0}–{:.0} · tab {} · rating {}/5",
                self.volume.value,
                self.brightness.value * 100.0,
                self.range.lo,
                self.range.hi,
                self.stepper.value,
                self.rating.value
            );
            fill(buf, sv, th.background);
            put(buf, sv.x, sv.y, &truncate(&line, sv.width as usize), sv.width, st(th.text_muted, th.background));
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        match ev {
            Event::Key(k) => {
                if self.focus.handle_key(*k).is_consumed() {
                    return Outcome::Consumed;
                }
                
                match self.focus.current() {
                    Some(Id::Button(i)) => {
                        let o = self.buttons[i].handle_key(*k);
                        if o.is_changed() {
                            self.presses += 1;
                            ctx.notify(format!("{} button pressed", BUTTONS[i].0), BUTTONS[i].1);
                        }
                        o
                    }
                    Some(Id::Icon) => {
                        let o = self.icon_btn.handle_key(*k);
                        if o.is_changed() {
                            self.presses += 1;
                            ctx.notify("Saved", Variant::Success);
                        }
                        o
                    }
                    Some(Id::Wide) => {
                        let o = self.wide_btn.handle_key(*k);
                        if o.is_changed() {
                            self.presses += 1;
                        }
                        o
                    }
                    Some(Id::Check(i)) if i != 3 => self.checks[i].handle_key(*k),
                    Some(Id::Switch(i)) => self.switches[i].handle_key(*k),
                    Some(Id::Radio) => self.radio.handle_key(*k),
                    Some(Id::RadioH) => self.radio_h.handle_key(*k),
                    Some(Id::CheckList) => self.checklist.handle_key(*k),
                    Some(Id::Segmented) => self.segmented.handle_key(*k),
                    Some(Id::Volume) => self.volume.handle_key(*k),
                    Some(Id::Brightness) => self.brightness.handle_key(*k),
                    Some(Id::Range) => self.range.handle_key(*k),
                    Some(Id::Stepper) => self.stepper.handle_key(*k),
                    Some(Id::Rating) => self.rating.handle_key(*k),
                    _ => Outcome::Ignored,
                }
            }
            Event::Mouse(m) => {
                let m = *m;
                let mut out = Outcome::Ignored;
                for (i, (button, button_spec)) in self.buttons.iter_mut().zip(BUTTONS.iter()).enumerate() {
                    let o = button.handle_mouse(m);
                    if o.is_changed() {
                        self.presses += 1;
                        self.focus.set(Id::Button(i));
                        ctx.notify(format!("{} button pressed", button_spec.0), button_spec.1);
                    }
                    out |= o;
                }
                let o = self.icon_btn.handle_mouse(m);
                if o.is_changed() {
                    self.presses += 1;
                    self.focus.set(Id::Icon);
                    ctx.notify("Saved", Variant::Success);
                }
                out |= o;
                let o = self.wide_btn.handle_mouse(m);
                if o.is_changed() {
                    self.presses += 1;
                    self.focus.set(Id::Wide);
                }
                out |= o;
                macro_rules! route {
                    ($state:expr, $id:expr) => {{
                        let o = $state.handle_mouse(m);
                        if o.is_consumed() && is_left_down(&m) {
                            self.focus.set($id);
                        }
                        out |= o;
                    }};
                }
                for i in 0..3 {
                    route!(self.checks[i], Id::Check(i));
                }
                for i in 0..3 {
                    route!(self.switches[i], Id::Switch(i));
                }
                route!(self.radio, Id::Radio);
                route!(self.radio_h, Id::RadioH);
                route!(self.checklist, Id::CheckList);
                route!(self.segmented, Id::Segmented);
                route!(self.volume, Id::Volume);
                route!(self.brightness, Id::Brightness);
                route!(self.range, Id::Range);
                route!(self.stepper, Id::Stepper);
                route!(self.rating, Id::Rating);
                out
            }
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self, now: Instant) -> bool {
        self.switches.iter().any(|s| s.animating(now))
            || self.volume.animating(now)
            || self.brightness.animating(now)
            || self.range.animating(now)
            || self.buttons.iter().any(|b| b.animating(now))
            || self.icon_btn.animating(now)
            || self.wide_btn.animating(now)
    }
}
