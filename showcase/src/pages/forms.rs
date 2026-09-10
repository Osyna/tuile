//! Showcase for form layouts and validation patterns.

use std::time::Instant;

use tuiforge::draw::{Edge, FieldShape, put, st};
use tuiforge::layout::pad;
use tuiforge::prelude::*;
use tuiforge::widgets::button::{Button, ButtonState, ButtonStyle};
use tuiforge::widgets::input::{Input, InputState};
use tuiforge::widgets::select::{Combobox, ComboboxState, Select, SelectState};
use tuiforge::widgets::steps::{Steps, StepsState};
use tuiforge::widgets::textarea::TextAreaState;
use tuiforge::widgets::toggle::{CheckState, Checkbox, CheckboxState, Switch, SwitchState};

use super::{Ctx, Page, card};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    // Sign-in
    SigninEmail,
    SigninPassword,
    SigninRemember,
    SigninSubmit,
    SigninForgot,
    // Profile
    ProfileFirstName,
    ProfileLastName,
    ProfileDisplayName,
    ProfileRole,
    ProfileTimezone,
    ProfileBio,
    ProfileNotify1,
    ProfileNotify2,
    ProfileSave,
    ProfileCancel,
    // Password
    PassCurrent,
    PassNew,
    PassConfirm,
    PassSubmit,
    // Wizard
    WizEmail,
    WizPassword,
    WizAddress,
    WizCity,
    WizZip,
    WizBack,
    WizNext,
    // Filter
    FilterSearch,
    FilterStatus,
    FilterMine,
    FilterApply,
    // Dense
    Dense1,
    Dense2,
    Dense3,
    Dense4,
    Dense5,
    Dense6,
}

pub struct FormsPage {
    focus: Focus<Id>,
    // Sign-in
    signin_email: InputState,
    signin_password: InputState,
    signin_remember: CheckboxState,
    signin_submit: ButtonState,
    signin_forgot: ButtonState,
    // Profile
    profile_first: InputState,
    profile_last: InputState,
    profile_display: InputState,
    profile_role: SelectState,
    profile_timezone: ComboboxState,
    profile_bio: TextAreaState,
    profile_notify1: SwitchState,
    profile_notify2: SwitchState,
    profile_save: ButtonState,
    profile_cancel: ButtonState,
    profile_dirty: bool,
    // Password
    pass_current: InputState,
    pass_new: InputState,
    pass_confirm: InputState,
    pass_submit: ButtonState,
    // Wizard
    wiz_step: usize,
    wiz_email: InputState,
    wiz_password: InputState,
    wiz_address: InputState,
    wiz_city: InputState,
    wiz_zip: InputState,
    wiz_back: ButtonState,
    wiz_next: ButtonState,
    wiz_steps_state: StepsState,
    // Filter
    filter_search: InputState,
    filter_status: SelectState,
    filter_mine: CheckboxState,
    filter_apply: ButtonState,
    // Dense
    dense_states: [InputState; 6],
    // Global
    field_shape: FieldShape,
    show_errors: bool,
    label_colon: bool,
}

impl FormsPage {
    fn new() -> Self {
        let signin_email = InputState::with_value("user@example.com");
        let signin_password = InputState::with_value("password123");

        let profile_role = SelectState::new(&["Admin", "Editor", "Viewer", "Guest"]);
        let mut profile_timezone = ComboboxState::new(&[
            "UTC",
            "America/New_York",
            "America/Los_Angeles",
            "Europe/London",
            "Asia/Tokyo",
        ]);
        profile_timezone.input.set_value("UTC");

        let filter_status = SelectState::new(&["All", "Active", "Pending", "Closed"]);

        let mut s = Self {
            focus: Focus::new([
                Id::SigninEmail,
                Id::SigninPassword,
                Id::SigninRemember,
                Id::SigninSubmit,
                Id::SigninForgot,
                Id::ProfileFirstName,
                Id::ProfileLastName,
                Id::ProfileDisplayName,
                Id::ProfileRole,
                Id::ProfileTimezone,
                Id::ProfileNotify1,
                Id::ProfileNotify2,
                Id::ProfileSave,
                Id::ProfileCancel,
                Id::PassCurrent,
                Id::PassNew,
                Id::PassConfirm,
                Id::PassSubmit,
                Id::WizEmail,
                Id::WizPassword,
                Id::WizAddress,
                Id::WizCity,
                Id::WizZip,
                Id::WizBack,
                Id::WizNext,
                Id::FilterSearch,
                Id::FilterStatus,
                Id::FilterMine,
                Id::FilterApply,
                Id::Dense1,
                Id::Dense2,
                Id::Dense3,
                Id::Dense4,
                Id::Dense5,
                Id::Dense6,
            ]),
            signin_email,
            signin_password,
            signin_remember: CheckboxState::new(CheckState::Off),
            signin_submit: ButtonState::default(),
            signin_forgot: ButtonState::default(),
            profile_first: InputState::with_value("Jane"),
            profile_last: InputState::with_value("Doe"),
            profile_display: InputState::with_value("jdoe"),
            profile_role,
            profile_timezone,
            profile_bio: TextAreaState::with_text("Developer and designer."),
            profile_notify1: SwitchState::new(true),
            profile_notify2: SwitchState::new(false),
            profile_save: ButtonState::default(),
            profile_cancel: ButtonState::default(),
            profile_dirty: true,
            pass_current: InputState::new(),
            pass_new: InputState::with_value("NewPass123!"),
            pass_confirm: InputState::with_value("NewPass123!"),
            pass_submit: ButtonState::default(),
            wiz_step: 0,
            wiz_email: InputState::with_value("user@example.com"),
            wiz_password: InputState::new(),
            wiz_address: InputState::new(),
            wiz_city: InputState::new(),
            wiz_zip: InputState::new(),
            wiz_back: ButtonState::default(),
            wiz_next: ButtonState::default(),
            wiz_steps_state: StepsState::default(),
            filter_search: InputState::new(),
            filter_status,
            filter_mine: CheckboxState::new(CheckState::Off),
            filter_apply: ButtonState::default(),
            dense_states: [
                InputState::with_value("Value 1"),
                InputState::with_value("Value 2"),
                InputState::with_value("Value 3"),
                InputState::with_value("Value 4"),
                InputState::with_value("Value 5"),
                InputState::with_value("Value 6"),
            ],
            field_shape: FieldShape::Tall(Edge::Full),
            show_errors: false,
            label_colon: true,
        };
        s.profile_role.set_selected(Some(0));
        s.filter_status.set_selected(Some(0));
        s
    }
}

impl Default for FormsPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Page for FormsPage {
    fn title(&self) -> &'static str {
        "Forms"
    }

    fn subtitle(&self) -> &'static str {
        "Form layouts, validation, and field grouping"
    }

    fn icon(&self) -> &'static str {
        "⊡"
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        ctx.area = area;
        let th = ctx.theme;
        let now = ctx.now;

        // small terminals: the sign-in form only
        if area.width < 90 || area.height < 30 {
            self.draw_signin(area, buf, &th, now);
            self.profile_role.render_overlay(buf, area, &th, 6);
            return;
        }

        let rows = Layout::vertical([Constraint::Fill(1); 2]).split(area);
        let top = Layout::horizontal([Constraint::Fill(1); 3]).split(rows[0]);
        let bottom = Layout::horizontal([Constraint::Fill(1); 3]).split(rows[1]);
        self.draw_signin(top[0], buf, &th, now);
        self.draw_profile(top[1], buf, &th, now);
        self.draw_password(top[2], buf, &th, now);
        self.draw_wizard(bottom[0], buf, &th, now);
        self.draw_filter(bottom[1], buf, &th, now);
        self.draw_dense(bottom[2], buf, &th, now);

        // dropdowns last, over everything
        self.profile_role.render_overlay(buf, area, &th, 6);
        self.profile_timezone.render_overlay(buf, area, &th, 6);
        self.filter_status.render_overlay(buf, area, &th, 4);
    }
    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        // Close dropdowns on outside click
        if let Event::Mouse(m) = ev
            && is_left_down(m)
        {
            if !mouse_in(self.profile_role.hit.area, m)
                && !mouse_in(self.profile_role.dropdown_area, m)
            {
                self.profile_role.close();
            }
            if !mouse_in(self.profile_timezone.hit.area, m)
                && !mouse_in(self.profile_timezone.dropdown_area, m)
            {
                self.profile_timezone.close();
            }
            if !mouse_in(self.filter_status.hit.area, m)
                && !mouse_in(self.filter_status.dropdown_area, m)
            {
                self.filter_status.close();
            }
        }

        // Page-level keys
        if let Event::Key(k) = ev
            && is_press(k)
        {
            match k.code {
                KeyCode::Tab if !k.modifiers.contains(KeyModifiers::SHIFT) => {
                    self.focus.next();
                    return Outcome::Consumed;
                }
                KeyCode::BackTab | KeyCode::Tab if k.modifiers.contains(KeyModifiers::SHIFT) => {
                    self.focus.prev();
                    return Outcome::Consumed;
                }
                KeyCode::Char('f') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.field_shape = match self.field_shape {
                        FieldShape::Tall(_) => FieldShape::Round,
                        FieldShape::Round => FieldShape::Band,
                        FieldShape::Band => FieldShape::Rule,
                        FieldShape::Rule => FieldShape::Bars(Edge::Thin),
                        _ => FieldShape::Tall(Edge::Full),
                    };
                    return Outcome::Consumed;
                }
                KeyCode::Char('e') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.show_errors = !self.show_errors;
                    return Outcome::Consumed;
                }
                KeyCode::Char('l') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.label_colon = !self.label_colon;
                    return Outcome::Consumed;
                }
                _ => {}
            }
        }

        // Forward to focused widget
        if let Event::Key(k) = ev {
            let out = match self.focus.current() {
                Some(Id::SigninEmail) => self.signin_email.handle_key(*k),
                Some(Id::SigninPassword) => self.signin_password.handle_key(*k),
                Some(Id::SigninRemember) => self.signin_remember.handle_key(*k),
                Some(Id::SigninSubmit) => {
                    let o = self.signin_submit.handle_key(*k);
                    if o.is_changed() {
                        let email_err = email(self.signin_email.value());
                        if email_err.is_some() || self.signin_password.value().is_empty() {
                            ctx.notify(
                                format!("{} errors found", if email_err.is_some() { 2 } else { 1 }),
                                Variant::Error,
                            );
                        } else {
                            ctx.notify("Signed in successfully", Variant::Success);
                        }
                    }
                    o
                }
                Some(Id::SigninForgot) => {
                    let o = self.signin_forgot.handle_key(*k);
                    if o.is_changed() {
                        ctx.notify("Password reset link sent", Variant::Primary);
                    }
                    o
                }
                Some(Id::ProfileFirstName) => {
                    let o = self.profile_first.handle_key(*k);
                    if o.is_changed() {
                        self.profile_dirty = true;
                    }
                    o
                }
                Some(Id::ProfileLastName) => {
                    let o = self.profile_last.handle_key(*k);
                    if o.is_changed() {
                        self.profile_dirty = true;
                    }
                    o
                }
                Some(Id::ProfileDisplayName) => {
                    let o = self.profile_display.handle_key(*k);
                    if o.is_changed() {
                        self.profile_dirty = true;
                    }
                    o
                }
                Some(Id::ProfileRole) => self.profile_role.handle_key(*k),
                Some(Id::ProfileTimezone) => self.profile_timezone.handle_key(*k),
                Some(Id::ProfileBio) => {
                    let o = self.profile_bio.handle_key(*k);
                    if o.is_changed() {
                        self.profile_dirty = true;
                    }
                    o
                }
                Some(Id::ProfileNotify1) => self.profile_notify1.handle_key(*k),
                Some(Id::ProfileNotify2) => self.profile_notify2.handle_key(*k),
                Some(Id::ProfileSave) => {
                    let o = self.profile_save.handle_key(*k);
                    if o.is_changed() {
                        self.profile_dirty = false;
                        ctx.notify("Profile saved", Variant::Success);
                    }
                    o
                }
                Some(Id::ProfileCancel) => {
                    let o = self.profile_cancel.handle_key(*k);
                    if o.is_changed() {
                        self.profile_dirty = false;
                        ctx.notify("Changes discarded", Variant::Default);
                    }
                    o
                }
                Some(Id::PassCurrent) => self.pass_current.handle_key(*k),
                Some(Id::PassNew) => self.pass_new.handle_key(*k),
                Some(Id::PassConfirm) => self.pass_confirm.handle_key(*k),
                Some(Id::PassSubmit) => {
                    let o = self.pass_submit.handle_key(*k);
                    if o.is_changed() {
                        let confirm_err = matches(self.pass_new.value())(self.pass_confirm.value());
                        if confirm_err.is_some() {
                            ctx.notify("Passwords do not match", Variant::Error);
                        } else {
                            ctx.notify("Password updated", Variant::Success);
                        }
                    }
                    o
                }
                Some(Id::WizEmail) => self.wiz_email.handle_key(*k),
                Some(Id::WizPassword) => self.wiz_password.handle_key(*k),
                Some(Id::WizAddress) => self.wiz_address.handle_key(*k),
                Some(Id::WizCity) => self.wiz_city.handle_key(*k),
                Some(Id::WizZip) => self.wiz_zip.handle_key(*k),
                Some(Id::WizBack) => {
                    let o = self.wiz_back.handle_key(*k);
                    if o.is_changed() && self.wiz_step > 0 {
                        self.wiz_step -= 1;
                    }
                    o
                }
                Some(Id::WizNext) => {
                    let o = self.wiz_next.handle_key(*k);
                    if o.is_changed() && self.wiz_step < 2 {
                        self.wiz_step += 1;
                    }
                    o
                }
                Some(Id::FilterSearch) => self.filter_search.handle_key(*k),
                Some(Id::FilterStatus) => self.filter_status.handle_key(*k),
                Some(Id::FilterMine) => self.filter_mine.handle_key(*k),
                Some(Id::FilterApply) => {
                    let o = self.filter_apply.handle_key(*k);
                    if o.is_changed() {
                        ctx.notify("Filters applied", Variant::Success);
                    }
                    o
                }
                Some(Id::Dense1) => self.dense_states[0].handle_key(*k),
                Some(Id::Dense2) => self.dense_states[1].handle_key(*k),
                Some(Id::Dense3) => self.dense_states[2].handle_key(*k),
                Some(Id::Dense4) => self.dense_states[3].handle_key(*k),
                Some(Id::Dense5) => self.dense_states[4].handle_key(*k),
                Some(Id::Dense6) => self.dense_states[5].handle_key(*k),
                None => Outcome::Ignored,
            };
            if out.is_changed() || out.is_consumed() {
                return out;
            }
        }

        // Mouse forwarding
        if let Event::Mouse(m) = ev {
            let mut result = Outcome::Ignored;

            macro_rules! handle_mouse {
                ($state:expr, $id:expr) => {
                    let out = $state.handle_mouse(*m);
                    if matches!(out, Outcome::Changed | Outcome::Consumed)
                        && matches!($state.hit.mouse(m), Hit::Press)
                    {
                        self.focus.set($id);
                    }
                    result |= out;
                };
            }

            handle_mouse!(self.signin_email, Id::SigninEmail);
            handle_mouse!(self.signin_password, Id::SigninPassword);
            handle_mouse!(self.signin_remember, Id::SigninRemember);
            handle_mouse!(self.signin_submit, Id::SigninSubmit);
            handle_mouse!(self.signin_forgot, Id::SigninForgot);
            handle_mouse!(self.profile_first, Id::ProfileFirstName);
            handle_mouse!(self.profile_last, Id::ProfileLastName);
            handle_mouse!(self.profile_display, Id::ProfileDisplayName);
            handle_mouse!(self.profile_role, Id::ProfileRole);
            handle_mouse!(self.profile_timezone, Id::ProfileTimezone);
            handle_mouse!(self.profile_bio, Id::ProfileBio);
            handle_mouse!(self.profile_notify1, Id::ProfileNotify1);
            handle_mouse!(self.profile_notify2, Id::ProfileNotify2);
            handle_mouse!(self.profile_save, Id::ProfileSave);
            handle_mouse!(self.profile_cancel, Id::ProfileCancel);
            handle_mouse!(self.pass_current, Id::PassCurrent);
            handle_mouse!(self.pass_new, Id::PassNew);
            handle_mouse!(self.pass_confirm, Id::PassConfirm);
            handle_mouse!(self.pass_submit, Id::PassSubmit);
            handle_mouse!(self.wiz_email, Id::WizEmail);
            handle_mouse!(self.wiz_password, Id::WizPassword);
            handle_mouse!(self.wiz_address, Id::WizAddress);
            handle_mouse!(self.wiz_city, Id::WizCity);
            handle_mouse!(self.wiz_zip, Id::WizZip);
            handle_mouse!(self.wiz_back, Id::WizBack);
            handle_mouse!(self.wiz_next, Id::WizNext);
            handle_mouse!(self.filter_search, Id::FilterSearch);
            handle_mouse!(self.filter_status, Id::FilterStatus);
            handle_mouse!(self.filter_mine, Id::FilterMine);
            handle_mouse!(self.filter_apply, Id::FilterApply);

            for i in 0..6 {
                let id = match i {
                    0 => Id::Dense1,
                    1 => Id::Dense2,
                    2 => Id::Dense3,
                    3 => Id::Dense4,
                    4 => Id::Dense5,
                    5 => Id::Dense6,
                    _ => unreachable!(),
                };
                handle_mouse!(self.dense_states[i], id);
            }

            if result.is_changed() || result.is_consumed() {
                return result;
            }
        }

        Outcome::Ignored
    }

    fn animating(&self, now: Instant) -> bool {
        self.signin_email.animating(now)
            || self.signin_password.animating(now)
            || self.profile_first.animating(now)
            || self.pass_new.animating(now)
    }

    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("Tab", "Next field"),
            ("⇧Tab", "Previous"),
            ("^f", "Field shape"),
            ("^e", "Errors"),
            ("^l", "Label colon"),
        ]
    }
}

impl FormsPage {
    /// Rows a framed field takes in the current shape.
    fn fh(&self) -> u16 {
        1 + self.field_shape.vertical_chrome()
    }

    fn err(&self, msg: &'static str) -> Option<&'static str> {
        self.show_errors.then_some(msg)
    }

    fn input(&self, id: Id) -> Input {
        Input::new()
            .shape(self.field_shape)
            .focused(self.focus.is(id))
    }

    /// Sign in · `Stacked`: label above, help/error below, checkbox and a button row.
    fn draw_signin(&mut self, area: Rect, buf: &mut Buffer, th: &Theme, now: Instant) {
        let inner = pad(card(buf, area, th, "Sign in · Stacked"), 1, 0);
        if inner.width < 20 || inner.height < 6 {
            return;
        }
        let fh = self.fh();
        let email_err = self
            .err("Invalid email address")
            .or_else(|| email(self.signin_email.value()).map(|_| "Invalid email address"));
        let fields = [
            FormField::new("Email")
                .required(true)
                .help("work address")
                .error(email_err)
                .height(fh),
            FormField::new("Password")
                .required(true)
                .error(self.err("Password is required"))
                .height(fh),
        ];
        let rects = Form::new()
            .style(FormStyle::Stacked)
            .gap(0)
            .fields(&fields)
            .theme(th)
            .render(inner, buf);
        if let Some(r) = rects.fields.first() {
            self.input(Id::SigninEmail)
                .placeholder("you@example.com")
                .now(now)
                .theme(th)
                .render(r.field, buf, &mut self.signin_email);
        }
        if let Some(r) = rects.fields.get(1) {
            self.input(Id::SigninPassword)
                .password(true)
                .now(now)
                .theme(th)
                .render(r.field, buf, &mut self.signin_password);
        }
        let Some(last) = rects.fields.last() else {
            return;
        };
        let y = last.help.bottom();
        if y < inner.bottom() {
            Checkbox::new("Remember me")
                .focused(self.focus.is(Id::SigninRemember))
                .theme(th)
                .render(
                    Rect::new(inner.x, y, inner.width, 1),
                    buf,
                    &mut self.signin_remember,
                );
        }
        let by = y + 2;
        let bh = inner.bottom().saturating_sub(by).min(3);
        if bh > 0 {
            let btns = FormActions::new(&["Sign in", "Forgot?"])
                .align(Alignment::Left)
                .theme(th)
                .render(Rect::new(inner.x, by, inner.width, bh), buf);
            if let Some(r) = btns.first() {
                Button::new("Sign in")
                    .variant(Variant::Primary)
                    .compact(bh < 3)
                    .focused(self.focus.is(Id::SigninSubmit))
                    .theme(th)
                    .render(*r, buf, &mut self.signin_submit);
            }
            if let Some(r) = btns.get(1) {
                Button::new("Forgot?")
                    .style(ButtonStyle::Ghost)
                    .compact(bh < 3)
                    .focused(self.focus.is(Id::SigninForgot))
                    .theme(th)
                    .render(*r, buf, &mut self.signin_forgot);
            }
        }
    }

    /// Profile · `Grid` (2 columns, `span(2)` row, right hint), switches, `FormActions` with a dirty hint.
    fn draw_profile(&mut self, area: Rect, buf: &mut Buffer, th: &Theme, now: Instant) {
        let inner = pad(card(buf, area, th, "Profile · Grid"), 1, 0);
        if inner.width < 30 || inner.height < 8 {
            return;
        }
        let fh = self.fh();
        let (counter, counter_fg) =
            char_counter(self.profile_display.value().chars().count(), 32, th);
        let _ = counter_fg;
        let fields = [
            FormField::new("First name").required(true).height(fh),
            FormField::new("Last name").required(true).height(fh),
            FormField::new("Display name")
                .span(2)
                .hint_right(counter)
                .error(self.err("Already taken"))
                .height(fh),
            FormField::new("Role").height(fh),
            FormField::new("Timezone").height(fh),
        ];
        let rects = Form::new()
            .style(FormStyle::Grid)
            .columns(2)
            .gap(2)
            .fields(&fields)
            .theme(th)
            .render(inner, buf);
        let f = |i: usize| rects.fields.get(i).map(|r| r.field);
        if let Some(r) = f(0) {
            self.input(Id::ProfileFirstName).now(now).theme(th).render(
                r,
                buf,
                &mut self.profile_first,
            );
        }
        if let Some(r) = f(1) {
            self.input(Id::ProfileLastName).now(now).theme(th).render(
                r,
                buf,
                &mut self.profile_last,
            );
        }
        if let Some(r) = f(2) {
            self.input(Id::ProfileDisplayName)
                .max_len(32)
                .now(now)
                .theme(th)
                .render(r, buf, &mut self.profile_display);
        }
        if let Some(r) = f(3) {
            Select::new()
                .shape(self.field_shape)
                .focused(self.focus.is(Id::ProfileRole))
                .now(now)
                .theme(th)
                .render(r, buf, &mut self.profile_role);
        }
        if let Some(r) = f(4) {
            Combobox::new()
                .shape(self.field_shape)
                .focused(self.focus.is(Id::ProfileTimezone))
                .now(now)
                .theme(th)
                .render(r, buf, &mut self.profile_timezone);
        }
        let Some(last) = rects.fields.last() else {
            return;
        };
        let y = last.help.bottom();
        if y < inner.bottom() {
            let half = inner.width / 2;
            Switch::new()
                .label("Digests")
                .compact(true)
                .focused(self.focus.is(Id::ProfileNotify1))
                .now(now)
                .theme(th)
                .render(
                    Rect::new(inner.x, y, half, 1),
                    buf,
                    &mut self.profile_notify1,
                );
            Switch::new()
                .label("Push")
                .compact(true)
                .focused(self.focus.is(Id::ProfileNotify2))
                .now(now)
                .theme(th)
                .render(
                    Rect::new(inner.x + half, y, inner.width - half, 1),
                    buf,
                    &mut self.profile_notify2,
                );
        }
        let by = y + 2;
        let bh = inner.bottom().saturating_sub(by).min(3);
        if bh > 0 {
            let btns = FormActions::new(&["Save", "Cancel"])
                .dirty(self.profile_dirty)
                .theme(th)
                .render(Rect::new(inner.x, by, inner.width, bh), buf);
            if let Some(r) = btns.first() {
                Button::new("Save")
                    .variant(Variant::Success)
                    .compact(bh < 3)
                    .focused(self.focus.is(Id::ProfileSave))
                    .theme(th)
                    .render(*r, buf, &mut self.profile_save);
            }
            if let Some(r) = btns.get(1) {
                Button::new("Cancel")
                    .compact(bh < 3)
                    .focused(self.focus.is(Id::ProfileCancel))
                    .theme(th)
                    .render(*r, buf, &mut self.profile_cancel);
            }
        }
    }

    /// Change password · `Floating`: labels ride the field's top edge; strength meter; match check.
    fn draw_password(&mut self, area: Rect, buf: &mut Buffer, th: &Theme, now: Instant) {
        let inner = pad(card(buf, area, th, "Change password · Floating"), 1, 0);
        if inner.width < 24 || inner.height < 8 {
            return;
        }
        // floating labels need a frame with a top edge
        let shape = match self.field_shape {
            FieldShape::Tall(_) | FieldShape::Round => self.field_shape,
            _ => FieldShape::Round,
        };
        let mismatch = self.pass_new.value() != self.pass_confirm.value();
        let fields = [
            FormField::new("Current password")
                .required(true)
                .error(self.err("Wrong password"))
                .height(3),
            FormField::new("New password")
                .required(true)
                .help(" ")
                .height(3),
            FormField::new("Confirm")
                .required(true)
                .error((mismatch || self.show_errors).then_some("Passwords do not match"))
                .height(3),
        ];
        let rects = Form::new()
            .style(FormStyle::Floating)
            .gap(0)
            .fields(&fields)
            .theme(th)
            .render(inner, buf);
        let ids = [Id::PassCurrent, Id::PassNew, Id::PassConfirm];
        for (i, (r, id)) in rects.fields.iter().zip(ids).enumerate() {
            let state = match i {
                0 => &mut self.pass_current,
                1 => &mut self.pass_new,
                _ => &mut self.pass_confirm,
            };
            Input::new()
                .password(true)
                .shape(shape)
                .focused(self.focus.is(id))
                .now(now)
                .theme(th)
                .render(r.field, buf, state);
            float_label(
                buf,
                r.field,
                &fields[i].label,
                fields[i].required,
                self.focus.is(id),
                th,
            );
        }
        if let Some(r) = rects.fields.get(1) {
            PasswordStrength::new(self.pass_new.value())
                .theme(th)
                .render(r.help, buf);
        }
        let Some(last) = rects.fields.last() else {
            return;
        };
        let by = last.help.bottom() + 1;
        let bh = inner.bottom().saturating_sub(by).min(3);
        if bh > 0 {
            let btns = FormActions::new(&["Update password"])
                .align(Alignment::Right)
                .theme(th)
                .render(Rect::new(inner.x, by, inner.width, bh), buf);
            if let Some(r) = btns.first() {
                Button::new("Update password")
                    .variant(Variant::Primary)
                    .compact(bh < 3)
                    .focused(self.focus.is(Id::PassSubmit))
                    .theme(th)
                    .render(*r, buf, &mut self.pass_submit);
            }
        }
    }

    /// Wizard: `Steps` header, one `Aligned` section per step, Back/Next, summary on the last step.
    fn draw_wizard(&mut self, area: Rect, buf: &mut Buffer, th: &Theme, now: Instant) {
        let inner = pad(card(buf, area, th, "Wizard · Aligned sections"), 1, 0);
        if inner.width < 30 || inner.height < 9 {
            return;
        }
        Steps::new(&["Account", "Address", "Review"])
            .active(self.wiz_step)
            .theme(th)
            .render(Rect { height: 3, ..inner }, buf, &mut self.wiz_steps_state);
        let bh = 3.min(inner.height.saturating_sub(4));
        let body = Rect {
            y: inner.y + 4,
            height: inner.height.saturating_sub(4 + bh + 1),
            ..inner
        };
        let fh = self.fh();
        let form = Form::new()
            .style(FormStyle::Aligned)
            .label_width(10)
            .label_colon(self.label_colon)
            .gap(0)
            .theme(th);
        match self.wiz_step {
            0 => {
                let fields = [
                    FormField::new("Email")
                        .required(true)
                        .error(self.err("Invalid email"))
                        .height(fh),
                    FormField::new("Password").required(true).height(fh),
                ];
                let rects = form.fields(&fields).render(body, buf);
                if let Some(r) = rects.fields.first() {
                    self.input(Id::WizEmail).now(now).theme(th).render(
                        r.field,
                        buf,
                        &mut self.wiz_email,
                    );
                }
                if let Some(r) = rects.fields.get(1) {
                    self.input(Id::WizPassword)
                        .password(true)
                        .now(now)
                        .theme(th)
                        .render(r.field, buf, &mut self.wiz_password);
                }
            }
            1 => {
                let fields = [
                    FormField::new("Street").required(true).height(fh),
                    FormField::new("City").required(true).height(fh),
                    FormField::new("ZIP").height(fh),
                ];
                let rects = form.fields(&fields).render(body, buf);
                let states = [&mut self.wiz_address, &mut self.wiz_city, &mut self.wiz_zip];
                let ids = [Id::WizAddress, Id::WizCity, Id::WizZip];
                for ((r, state), id) in rects.fields.iter().zip(states).zip(ids) {
                    Input::new()
                        .shape(self.field_shape)
                        .focused(self.focus.is(id))
                        .now(now)
                        .theme(th)
                        .render(r.field, buf, state);
                }
            }
            _ => {
                let rows = [
                    ("Email", self.wiz_email.value().to_string()),
                    ("Street", self.wiz_address.value().to_string()),
                    ("City", self.wiz_city.value().to_string()),
                    ("ZIP", self.wiz_zip.value().to_string()),
                ];
                for (i, (k, v)) in rows.iter().enumerate() {
                    let y = body.y + i as u16;
                    if y >= body.bottom() {
                        break;
                    }
                    put(
                        buf,
                        body.x,
                        y,
                        &format!("{k:>8}"),
                        8,
                        st(th.text_muted, th.background),
                    );
                    let v = if v.is_empty() { "-" } else { v.as_str() };
                    put(
                        buf,
                        body.x + 10,
                        y,
                        v,
                        body.width.saturating_sub(10),
                        st(th.text, th.background),
                    );
                }
            }
        }
        if bh > 0 {
            let by = inner.bottom() - bh;
            let next = if self.wiz_step == 2 { "Finish" } else { "Next" };
            let btns = FormActions::new(&["Back", next])
                .theme(th)
                .render(Rect::new(inner.x, by, inner.width, bh), buf);
            if let Some(r) = btns.first() {
                Button::new("Back")
                    .enabled(self.wiz_step > 0)
                    .compact(bh < 3)
                    .focused(self.focus.is(Id::WizBack))
                    .theme(th)
                    .render(*r, buf, &mut self.wiz_back);
            }
            if let Some(r) = btns.get(1) {
                Button::new(next)
                    .variant(Variant::Primary)
                    .compact(bh < 3)
                    .focused(self.focus.is(Id::WizNext))
                    .theme(th)
                    .render(*r, buf, &mut self.wiz_next);
            }
        }
    }

    /// Filter bar · `Inline` one-row fields, then a `Fieldset` (the `Cards` grouping) with `Compact` rows.
    fn draw_filter(&mut self, area: Rect, buf: &mut Buffer, th: &Theme, now: Instant) {
        let inner = pad(card(buf, area, th, "Filter bar · Inline + Fieldset"), 1, 0);
        if inner.width < 30 || inner.height < 4 {
            return;
        }
        // inline row: label + 1-row field pairs, the button at the end
        let row = Rect { height: 1, ..inner };
        let fields = [
            FormField::new("Search").height(1),
            FormField::new("Status").height(1),
        ];
        let rects = Form::new()
            .style(FormStyle::Inline)
            .gap(1)
            .fields(&fields)
            .theme(th)
            .render(row, buf);
        if let Some(r) = rects.fields.first() {
            Input::new()
                .shape(FieldShape::Bars(Edge::Thin))
                .placeholder("query")
                .focused(self.focus.is(Id::FilterSearch))
                .now(now)
                .theme(th)
                .render(r.field, buf, &mut self.filter_search);
        }
        if let Some(r) = rects.fields.get(1) {
            Select::new()
                .compact(true)
                .shape(FieldShape::Bars(Edge::Thin))
                .focused(self.focus.is(Id::FilterStatus))
                .now(now)
                .theme(th)
                .render(r.field, buf, &mut self.filter_status);
        }
        // second row: the toggle and the action button, right-aligned
        let y2 = inner.y + 2;
        if y2 < inner.bottom() {
            let apply_w = 9u16.min(inner.width / 3);
            let bx = inner.right().saturating_sub(apply_w);
            Checkbox::new("Only mine")
                .focused(self.focus.is(Id::FilterMine))
                .theme(th)
                .render(
                    Rect::new(inner.x, y2, bx.saturating_sub(inner.x + 1), 1),
                    buf,
                    &mut self.filter_mine,
                );
            Button::new("Apply")
                .variant(Variant::Primary)
                .compact(true)
                .focused(self.focus.is(Id::FilterApply))
                .theme(th)
                .render(Rect::new(bx, y2, apply_w, 1), buf, &mut self.filter_apply);
        }

        // a fieldset with compact rows below
        let fs_area = Rect {
            y: inner.y + 4,
            height: inner.height.saturating_sub(4),
            ..inner
        };
        if fs_area.height < 4 {
            return;
        }
        let fs = Fieldset::new("Saved filters")
            .description("Compact rows inside a bordered group")
            .theme(th)
            .render(fs_area, buf);
        let rows = [
            ("Owner", "me"),
            ("Label", "bug, p1"),
            ("Updated", "last 7 days"),
            ("Sort", "newest first"),
        ];
        for (i, (k, v)) in rows.iter().enumerate() {
            let y = fs.y + i as u16;
            if y >= fs.bottom() {
                break;
            }
            put(
                buf,
                fs.x,
                y,
                &format!("{k:>8}"),
                8,
                st(th.text_muted, th.background),
            );
            put(
                buf,
                fs.x + 10,
                y,
                v,
                fs.width.saturating_sub(10),
                st(th.text, th.background),
            );
        }
    }

    /// Dense settings · `Compact`: right-aligned labels, one-row bar fields; the validation summary.
    fn draw_dense(&mut self, area: Rect, buf: &mut Buffer, th: &Theme, now: Instant) {
        let inner = pad(card(buf, area, th, "Dense settings · Compact"), 1, 0);
        if inner.width < 30 || inner.height < 6 {
            return;
        }
        let labels = [
            "Hostname",
            "Port",
            "Username",
            "Timeout",
            "Retries",
            "Log level",
        ];
        let fields: Vec<FormField> = labels
            .iter()
            .enumerate()
            .map(|(i, l)| FormField::new(*l).required(i < 3).height(1))
            .collect();
        let form_area = Rect {
            height: inner.height.min(6),
            ..inner
        };
        let rects = Form::new()
            .style(FormStyle::Compact)
            .label_width(11)
            .label_colon(self.label_colon)
            .gap(0)
            .fields(&fields)
            .theme(th)
            .render(form_area, buf);
        let ids = [
            Id::Dense1,
            Id::Dense2,
            Id::Dense3,
            Id::Dense4,
            Id::Dense5,
            Id::Dense6,
        ];
        for (i, (r, id)) in rects.fields.iter().zip(ids).enumerate() {
            Input::new()
                .shape(FieldShape::Bars(Edge::Hair))
                .focused(self.focus.is(id))
                .now(now)
                .theme(th)
                .render(r.field, buf, &mut self.dense_states[i]);
        }
        if self.show_errors {
            let y = inner.y + 7;
            let h = inner.bottom().saturating_sub(y);
            if h >= 4 {
                let errors = [
                    ("Email", "invalid address"),
                    ("Password", "required"),
                    ("Confirm", "does not match"),
                    ("Display name", "already taken"),
                ];
                ValidationSummary::new(&errors)
                    .theme(th)
                    .render(Rect::new(inner.x, y, inner.width, h.min(7)), buf);
            }
        } else {
            let y = inner.y + 7;
            if y + 1 < inner.bottom() {
                put(
                    buf,
                    inner.x,
                    y,
                    "^e shows the validation summary here",
                    inner.width,
                    st(th.text_muted, th.background),
                );
            }
        }
    }
}
