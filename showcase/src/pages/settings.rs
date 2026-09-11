//! Settings: a realistic preferences form assembled from library widgets in ~300 lines.
//! Selects, switches, sliders, steppers, radios, segmented controls, swatches, validated
//! inputs, dependent fields, a confirm dialog and toasts.

use tuiforge::draw::{fill, put, st};
use tuiforge::prelude::*;
use tuiforge::theme::BUILTIN;

use super::{Ctx, Page, card};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Theme,
    Density,
    Accent,
    LineNumbers,
    SidebarWidth,
    TabSize,
    WordWrap,
    Ligatures,
    Cursor,
    Notify,
    Sound,
    Desktop,
    Quiet,
    Name,
    Email,
    Plan,
    Save,
    Reset,
    Delete,
}

const LABEL_W: u16 = 16;
const ACCENTS: [Rgb; 6] = [
    Rgb::hex(0x0178D4),
    Rgb::hex(0xffa62b),
    Rgb::hex(0x4EBF71),
    Rgb::hex(0xba3c5b),
    Rgb::hex(0xc4a7e7),
    Rgb::hex(0x24837B),
];

pub struct SettingsPage {
    focus: Focus<Id>,
    theme_sel: SelectState,
    density: SegmentedState,
    accent: SwatchesState,
    line_numbers: SwitchState,
    sidebar_width: SliderState,
    tab_size: StepperState,
    word_wrap: CheckboxState,
    ligatures: SwitchState,
    cursor: RadioState,
    notify: SwitchState,
    sound: CheckboxState,
    desktop: CheckboxState,
    quiet: InputState,
    name: InputState,
    email: InputState,
    plan: SelectState,
    save: ButtonState,
    reset: ButtonState,
    delete: ButtonState,
    confirm: ModalState,
    dirty: bool,
}

fn email_ok(s: &str) -> Result<(), String> {
    let at = s.find('@').ok_or_else(|| "must contain @".to_string())?;
    if at == 0 || !s[at + 1..].contains('.') {
        return Err("use name@domain.tld".into());
    }
    Ok(())
}

fn time_ok(s: &str) -> Result<(), String> {
    let err = || "use HH:MM-HH:MM".to_string();
    let (a, b) = s.split_once('-').ok_or_else(err)?;
    for t in [a, b] {
        let (h, m) = t.trim().split_once(':').ok_or_else(err)?;
        let (h, m): (u8, u8) = (h.parse().map_err(|_| err())?, m.parse().map_err(|_| err())?);
        if h > 23 || m > 59 {
            return Err(err());
        }
    }
    Ok(())
}

impl Default for SettingsPage {
    fn default() -> Self {
        let mut p = SettingsPage {
            focus: Focus::new([
                Id::Theme,
                Id::Density,
                Id::Accent,
                Id::LineNumbers,
                Id::SidebarWidth,
                Id::TabSize,
                Id::WordWrap,
                Id::Ligatures,
                Id::Cursor,
                Id::Notify,
                Id::Sound,
                Id::Desktop,
                Id::Quiet,
                Id::Name,
                Id::Email,
                Id::Plan,
                Id::Save,
                Id::Reset,
                Id::Delete,
            ]),
            theme_sel: SelectState::new(&BUILTIN.iter().map(|t| t.name).collect::<Vec<_>>()),
            density: SegmentedState::new(1),
            accent: SwatchesState::new(),
            line_numbers: SwitchState::new(true),
            sidebar_width: SliderState::new(22.0, 16.0, 40.0, 1.0),
            tab_size: StepperState::new(4, 1, 8, 1),
            word_wrap: CheckboxState::new(CheckState::Off),
            ligatures: SwitchState::new(true),
            cursor: RadioState::new(Some(0)),
            notify: SwitchState::new(true),
            sound: CheckboxState::new(CheckState::On),
            desktop: CheckboxState::new(CheckState::Off),
            quiet: InputState::new(),
            name: InputState::new(),
            email: InputState::new(),
            plan: SelectState::new(&["Free", "Pro", "Team", "Enterprise"]),
            save: ButtonState::new(),
            reset: ButtonState::new(),
            delete: ButtonState::new(),
            confirm: ModalState::new(),
            dirty: false,
        };
        p.reset_values();
        p
    }
}

impl SettingsPage {
    fn reset_values(&mut self) {
        self.theme_sel
            .set_selected(BUILTIN.iter().position(|t| t.name == theme::current().name));
        self.density.selected = 1;
        self.accent.selected = Some(0);
        self.quiet.set_value("22:00-07:00");
        self.name.set_value("Ada Lovelace");
        self.email.set_value("ada@example.com");
        self.plan.set_selected(Some(1));
        self.dirty = false;
    }

    fn summary(&self) -> String {
        format!(
            "theme={} density={} tab={} wrap={} notify={} plan={}",
            self.theme_sel.selected_label().unwrap_or("-"),
            ["compact", "normal", "spacious"][self.density.selected.min(2)],
            self.tab_size.value,
            self.word_wrap.value == CheckState::On,
            self.notify.on,
            self.plan.selected_label().unwrap_or("-"),
        )
    }

    /// Label + control rects for one form row inside `col` at offset `y`.
    fn row(col: Rect, y: &mut u16, h: u16, control_w: u16) -> (Rect, Rect) {
        let label = Rect {
            x: col.x,
            y: *y,
            width: LABEL_W.min(col.width),
            height: 1,
        };
        let cx = col.x + LABEL_W;
        let control = Rect {
            x: cx,
            y: *y,
            width: control_w.min(col.right().saturating_sub(cx)),
            height: h,
        };
        *y += h + 1;
        (label, control)
    }

    fn press(state: &mut ButtonState, k: KeyEvent, id: Id, pressed: &mut Option<Id>) -> Outcome {
        let o = state.handle_key(k);
        if o.is_changed() {
            *pressed = Some(id);
        }
        o
    }

    fn label(buf: &mut Buffer, r: Rect, text: &str, th: &Theme, h: u16, enabled: bool) {
        let y = if h >= 3 { r.y + 1 } else { r.y };
        let fg = if enabled { th.text } else { th.text_disabled };
        put(buf, r.x, y, text, r.width, st(fg, th.background));
    }
}

impl Page for SettingsPage {
    fn title(&self) -> &'static str {
        "Settings"
    }
    fn subtitle(&self) -> &'static str {
        "A complete preferences form from library widgets"
    }
    fn icon(&self) -> &'static str {
        "⊕"
    }
    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("Tab", "Next field"),
            ("Enter/Space", "Toggle / open"),
            ("←→", "Adjust"),
        ]
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        let now = ctx.now;
        let f = |id: Id, focus: &Focus<Id>| focus.is(id);
        let inner = pad(area, 1, 0);
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Fill(1)]).areas(inner);

        // ── Appearance ──
        let [app_a, editor_a] =
            Layout::vertical([Constraint::Length(20), Constraint::Fill(1)]).areas(left);
        let a = pad(card(buf, app_a, &th, "Appearance"), 1, 0);
        let mut y = a.y;
        let (l, c) = Self::row(a, &mut y, 3, 30);
        Self::label(buf, l, "Theme", &th, 3, true);
        Select::new()
            .focused(f(Id::Theme, &self.focus))
            .now(now)
            .theme(&th)
            .render(c, buf, &mut self.theme_sel);
        let (l, c) = Self::row(a, &mut y, 1, 40);
        Self::label(buf, l, "Density", &th, 1, true);
        Segmented::new(vec!["Compact".into(), "Normal".into(), "Spacious".into()])
            .focused(f(Id::Density, &self.focus))
            .theme(&th)
            .render(c, buf, &mut self.density);
        let (l, c) = Self::row(a, &mut y, 1, 40);
        Self::label(buf, l, "Accent", &th, 1, true);
        Swatches::new(&ACCENTS)
            .focused(f(Id::Accent, &self.focus))
            .theme(&th)
            .render(c, buf, &mut self.accent);
        let (l, c) = Self::row(a, &mut y, 3, 16);
        Self::label(buf, l, "Line numbers", &th, 3, true);
        Switch::new()
            .focused(f(Id::LineNumbers, &self.focus))
            .now(now)
            .theme(&th)
            .render(c, buf, &mut self.line_numbers);
        let (l, c) = Self::row(a, &mut y, 3, 34);
        Self::label(buf, l, "Sidebar width", &th, 3, true);
        Slider::new()
            .show_value(true)
            .format(|v| format!("{v:.0} cols"))
            .focused(f(Id::SidebarWidth, &self.focus))
            .now(now)
            .theme(&th)
            .render(c, buf, &mut self.sidebar_width);

        // ── Editor ──
        let e = pad(card(buf, editor_a, &th, "Editor"), 1, 0);
        let mut y = e.y;
        let (l, c) = Self::row(e, &mut y, 1, 16);
        Self::label(buf, l, "Tab size", &th, 1, true);
        Stepper::new()
            .focused(f(Id::TabSize, &self.focus))
            .theme(&th)
            .render(c, buf, &mut self.tab_size);
        let (l, c) = Self::row(e, &mut y, 1, 30);
        Self::label(buf, l, "Word wrap", &th, 1, true);
        Checkbox::new("Soft-wrap long lines")
            .focused(f(Id::WordWrap, &self.focus))
            .theme(&th)
            .render(c, buf, &mut self.word_wrap);
        let (l, c) = Self::row(e, &mut y, 3, 16);
        Self::label(buf, l, "Ligatures", &th, 3, true);
        Switch::new()
            .focused(f(Id::Ligatures, &self.focus))
            .now(now)
            .theme(&th)
            .render(c, buf, &mut self.ligatures);
        let (l, c) = Self::row(e, &mut y, 1, 40);
        Self::label(buf, l, "Cursor", &th, 1, true);
        RadioGroup::new(vec!["Block".into(), "Beam".into(), "Underline".into()])
            .horizontal(true)
            .focused(f(Id::Cursor, &self.focus))
            .theme(&th)
            .render(c, buf, &mut self.cursor);

        // ── Notifications ──
        let [notif_a, acct_a, actions_a] = Layout::vertical([
            Constraint::Length(12),
            Constraint::Length(14),
            Constraint::Fill(1),
        ])
        .areas(right);
        let n = pad(card(buf, notif_a, &th, "Notifications"), 1, 0);
        let enabled = self.notify.on;
        let mut y = n.y;
        let (l, c) = Self::row(n, &mut y, 3, 16);
        Self::label(buf, l, "Enabled", &th, 3, true);
        Switch::new()
            .focused(f(Id::Notify, &self.focus))
            .now(now)
            .theme(&th)
            .render(c, buf, &mut self.notify);
        let (l, c) = Self::row(n, &mut y, 1, 30);
        Self::label(buf, l, "Sound", &th, 1, enabled);
        Checkbox::new("Play a sound")
            .enabled(enabled)
            .focused(f(Id::Sound, &self.focus))
            .theme(&th)
            .render(c, buf, &mut self.sound);
        let (l, c) = Self::row(n, &mut y, 1, 30);
        Self::label(buf, l, "Desktop", &th, 1, enabled);
        Checkbox::new("Desktop banners")
            .enabled(enabled)
            .focused(f(Id::Desktop, &self.focus))
            .theme(&th)
            .render(c, buf, &mut self.desktop);
        let (l, c) = Self::row(n, &mut y, 3, 24);
        Self::label(buf, l, "Quiet hours", &th, 3, enabled);
        Input::new()
            .validator(time_ok)
            .enabled(enabled)
            .focused(f(Id::Quiet, &self.focus))
            .now(now)
            .theme(&th)
            .render(c, buf, &mut self.quiet);

        // ── Account ──
        let ac = pad(card(buf, acct_a, &th, "Account"), 1, 0);
        let mut y = ac.y;
        let (l, c) = Self::row(ac, &mut y, 3, 34);
        Self::label(buf, l, "Name", &th, 3, true);
        Input::new()
            .placeholder("Full name")
            .max_len(40)
            .focused(f(Id::Name, &self.focus))
            .now(now)
            .theme(&th)
            .render(c, buf, &mut self.name);
        let (l, c) = Self::row(ac, &mut y, 3, 34);
        Self::label(buf, l, "Email", &th, 3, true);
        Input::new()
            .placeholder("you@example.com")
            .validator(email_ok)
            .focused(f(Id::Email, &self.focus))
            .now(now)
            .theme(&th)
            .render(c, buf, &mut self.email);
        let (l, c) = Self::row(ac, &mut y, 3, 24);
        Self::label(buf, l, "Plan", &th, 3, true);
        Select::new()
            .focused(f(Id::Plan, &self.focus))
            .now(now)
            .theme(&th)
            .render(c, buf, &mut self.plan);

        // ── Actions ──
        let act = pad(
            card(
                buf,
                actions_a,
                &th,
                if self.dirty {
                    "Actions • unsaved changes"
                } else {
                    "Actions"
                },
            ),
            1,
            0,
        );
        if act.height >= 3 {
            let bw = 14u16;
            let b1 = Rect {
                x: act.x,
                y: act.y,
                width: bw,
                height: 3,
            };
            let b2 = Rect {
                x: b1.right() + 1,
                y: act.y,
                width: bw,
                height: 3,
            };
            let b3 = Rect {
                x: act.right().saturating_sub(18),
                y: act.y,
                width: 18,
                height: 3,
            };
            Button::new("Save")
                .variant(Variant::Primary)
                .min_width(bw)
                .focused(f(Id::Save, &self.focus))
                .now(now)
                .theme(&th)
                .render(b1, buf, &mut self.save);
            Button::new("Reset")
                .min_width(bw)
                .focused(f(Id::Reset, &self.focus))
                .now(now)
                .theme(&th)
                .render(b2, buf, &mut self.reset);
            if b3.x > b2.right() {
                Button::new("Delete account")
                    .variant(Variant::Error)
                    .min_width(18)
                    .focused(f(Id::Delete, &self.focus))
                    .now(now)
                    .theme(&th)
                    .render(b3, buf, &mut self.delete);
            }
            if act.height >= 5 {
                let s = Rect {
                    x: act.x,
                    y: act.y + 4,
                    width: act.width,
                    height: 1,
                };
                fill(buf, s, th.background);
                put(
                    buf,
                    s.x,
                    s.y,
                    &truncate(&self.summary(), s.width as usize),
                    s.width,
                    st(th.text_muted, th.background),
                );
            }
        }

        // overlays last: dropdowns above everything, then the dialog
        self.theme_sel.render_overlay(buf, area, &th, 8);
        self.plan.render_overlay(buf, area, &th, 8);
        if self.confirm.open {
            Modal::confirm(
                "Delete account?",
                "This removes Ada Lovelace and every workspace she owns. This cannot be undone.",
            )
            .buttons(&[("Cancel", Variant::Default), ("Delete", Variant::Error)])
            .now(now)
            .theme(&th)
            .render(area, buf, &mut self.confirm);
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        if self.confirm.open {
            self.confirm.handle(ev);
            if let Some(r) = self.confirm.take_result() {
                self.confirm.close();
                if r == 1 {
                    ctx.notify("Account deleted (not really)", Variant::Error);
                } else {
                    ctx.notify("Kept your account", Variant::Default);
                }
            }
            return Outcome::Consumed;
        }
        let mut pressed: Option<Id> = None;
        let out = match ev {
            Event::Key(k) => {
                // dropdowns capture keys while open
                let dropdown_open = self.theme_sel.open || self.plan.open;
                if !dropdown_open && self.focus.handle_key(*k).is_consumed() {
                    return Outcome::Consumed;
                }
                match self.focus.current() {
                    Some(Id::Theme) => self.theme_sel.handle_key(*k),
                    Some(Id::Density) => self.density.handle_key(*k),
                    Some(Id::Accent) => self.accent.handle_key(*k),
                    Some(Id::LineNumbers) => self.line_numbers.handle_key(*k),
                    Some(Id::SidebarWidth) => self.sidebar_width.handle_key(*k),
                    Some(Id::TabSize) => self.tab_size.handle_key(*k),
                    Some(Id::WordWrap) => self.word_wrap.handle_key(*k),
                    Some(Id::Ligatures) => self.ligatures.handle_key(*k),
                    Some(Id::Cursor) => self.cursor.handle_key(*k),
                    Some(Id::Notify) => self.notify.handle_key(*k),
                    Some(Id::Sound) if self.notify.on => self.sound.handle_key(*k),
                    Some(Id::Desktop) if self.notify.on => self.desktop.handle_key(*k),
                    Some(Id::Quiet) if self.notify.on => self.quiet.handle_key(*k),
                    Some(Id::Name) => self.name.handle_key(*k),
                    Some(Id::Email) => self.email.handle_key(*k),
                    Some(Id::Plan) => self.plan.handle_key(*k),
                    Some(Id::Save) => Self::press(&mut self.save, *k, Id::Save, &mut pressed),
                    Some(Id::Reset) => Self::press(&mut self.reset, *k, Id::Reset, &mut pressed),
                    Some(Id::Delete) => Self::press(&mut self.delete, *k, Id::Delete, &mut pressed),
                    _ => Outcome::Ignored,
                }
            }
            Event::Mouse(m) => {
                let m = *m;
                let mut out = Outcome::Ignored;
                macro_rules! route {
                    ($state:expr, $id:expr) => {{
                        let o = $state.handle_mouse(m);
                        if o.is_consumed() && is_left_down(&m) {
                            self.focus.set($id);
                        }
                        out |= o;
                    }};
                }
                // open dropdowns get first pick, and a click elsewhere closes them
                if self.theme_sel.open || self.plan.open {
                    out |= self.theme_sel.handle_mouse(m);
                    out |= self.plan.handle_mouse(m);
                    if is_left_down(&m) && out.is_ignored() {
                        self.theme_sel.close();
                        self.plan.close();
                        out = Outcome::Consumed;
                    }
                } else {
                    route!(self.theme_sel, Id::Theme);
                    route!(self.plan, Id::Plan);
                }
                route!(self.density, Id::Density);
                route!(self.accent, Id::Accent);
                route!(self.line_numbers, Id::LineNumbers);
                route!(self.sidebar_width, Id::SidebarWidth);
                route!(self.tab_size, Id::TabSize);
                route!(self.word_wrap, Id::WordWrap);
                route!(self.ligatures, Id::Ligatures);
                route!(self.cursor, Id::Cursor);
                route!(self.notify, Id::Notify);
                if self.notify.on {
                    route!(self.sound, Id::Sound);
                    route!(self.desktop, Id::Desktop);
                    route!(self.quiet, Id::Quiet);
                }
                route!(self.name, Id::Name);
                route!(self.email, Id::Email);
                for (state, id) in [
                    (&mut self.save, Id::Save),
                    (&mut self.reset, Id::Reset),
                    (&mut self.delete, Id::Delete),
                ] {
                    let o = state.handle_mouse(m);
                    if o.is_changed() {
                        pressed = Some(id);
                        self.focus.set(id);
                    }
                    out |= o;
                }
                out
            }
            _ => Outcome::Ignored,
        };
        // side effects of value changes
        if out.is_changed() {
            self.dirty = true;
            if let Some(i) = self.theme_sel.selected()
                && BUILTIN[i].name != ctx.theme.name
            {
                let th = Theme::resolve(&BUILTIN[i], None);
                theme::set(th);
                ctx.theme = th;
            }
            match pressed {
                Some(Id::Save) => {
                    ctx.notify(format!("Saved: {}", self.summary()), Variant::Success);
                    self.dirty = false;
                }
                Some(Id::Reset) => {
                    self.reset_values();
                    ctx.notify("Settings reset to defaults", Variant::Default);
                }
                Some(Id::Delete) => self.confirm.open(ctx.now),
                _ => {}
            }
        }
        out
    }

    fn animating(&self, now: Instant) -> bool {
        [&self.line_numbers, &self.ligatures, &self.notify]
            .iter()
            .any(|s| s.animating(now))
            || self.sidebar_width.animating(now)
            || self.confirm.open
            || [&self.save, &self.reset, &self.delete]
                .iter()
                .any(|b| b.animating(now))
    }
}
