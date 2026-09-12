//! Feedback: progress bars, every spinner, loading states, toasts (with corners), callouts,
//! dialogs and tooltips, all live.

use tuile::draw::{fill, put, st};
use tuile::prelude::*;
use tuile::widgets::{CalloutBorder, SkeletonShape, ToastCorner, spinners};

use super::{Ctx, Page, card};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Info,
    Success,
    Warning,
    Error,
    Corner,
    Confirm,
    Alert,
    Prompt,
    Sheet,
    Steps,
}

const CORNERS: [ToastCorner; 4] = [
    ToastCorner::TopRight,
    ToastCorner::BottomRight,
    ToastCorner::TopLeft,
    ToastCorner::BottomLeft,
];

pub struct FeedbackPage {
    focus: Focus<Id>,
    animated: ProgressState,
    indeterminate: ProgressState,
    complete: ProgressState,
    threshold: ProgressState,
    compact: ProgressState,
    next_cycle: Option<Instant>,
    step: usize,
    toaster: Toaster,
    corner: SegmentedState,
    buttons: [ButtonState; 8],
    modal: ModalState,
    modal_kind: usize,
    picker: ListViewState,
    callouts: [CalloutState; 4],
    tooltip: TooltipState,
    spinner_hits: Vec<(Rect, &'static str)>,
    hover_spinner: Option<usize>,
}

impl Default for FeedbackPage {
    fn default() -> Self {
        let mut complete = ProgressState::new();
        complete.set(1.0, Instant::now(), Duration::ZERO);
        let mut threshold = ProgressState::new();
        threshold.set(0.82, Instant::now(), Duration::ZERO);
        let mut compact = ProgressState::new();
        compact.set(0.35, Instant::now(), Duration::ZERO);
        FeedbackPage {
            focus: Focus::new([
                Id::Info,
                Id::Success,
                Id::Warning,
                Id::Error,
                Id::Corner,
                Id::Confirm,
                Id::Alert,
                Id::Prompt,
                Id::Sheet,
                Id::Steps,
            ]),
            animated: ProgressState::new(),
            indeterminate: ProgressState::new(),
            complete,
            threshold,
            compact,
            next_cycle: None,
            step: 2,
            toaster: Toaster::new().max_visible(3),
            corner: SegmentedState::new(1),
            buttons: Default::default(),
            modal: ModalState::new(),
            modal_kind: 0,
            picker: ListViewState::new(),
            callouts: Default::default(),
            tooltip: TooltipState::new(),
            spinner_hits: Vec::new(),
            hover_spinner: None,
        }
    }
}

impl FeedbackPage {
    fn modal_builder(&self, th: &Theme, now: Instant) -> Modal {
        let m = match self.modal_kind {
            0 => Modal::confirm(
                "Discard changes?",
                "You have unsaved edits in 3 files. Discarding cannot be undone.",
            )
            .buttons(&[
                ("Keep editing", Variant::Default),
                ("Discard", Variant::Error),
            ]),
            1 => Modal::alert(
                "Update available",
                "tuile 0.2.0 adds a DataGrid and virtual lists. Restart to apply.",
            )
            .icon("↑"),
            2 => Modal::prompt("Rename branch", "New name for `feature/toasts`:"),
            3 => Modal::new("Sheet")
                .body("A bottom sheet slides up from the edge and keeps the page context visible.")
                .buttons(&[("Close", Variant::Primary)])
                .kind(ModalKind::Sheet),
            // a card the caller fills: the modal draws frame and title, we draw the picker
            _ => Modal::card("Pick a model").width(44).height(12),
        };
        m.theme(th).now(now)
    }

    fn open_modal(&mut self, kind: usize, ctx: &Ctx) {
        self.modal_kind = kind;
        self.modal.open(ctx.now);
    }
}

impl Page for FeedbackPage {
    fn title(&self) -> &'static str {
        "Feedback"
    }
    fn subtitle(&self) -> &'static str {
        "Progress, spinners, toasts, callouts, dialogs, tooltips"
    }
    fn icon(&self) -> &'static str {
        "◇"
    }
    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("Tab", "Focus"),
            ("Enter", "Press"),
            ("1-4", "Toast"),
            ("m", "Dialog"),
            ("l", "Card + picker"),
            ("Esc", "Dismiss"),
        ]
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        let now = ctx.now;
        // drive the demo
        if self.next_cycle.is_none_or(|t| now >= t) {
            let target = if self.animated.value(now) > 0.85 {
                0.05
            } else {
                (self.animated.value(now) + 0.3 + (ctx.elapsed() * 7.0).sin().abs() * 0.4).min(1.0)
            };
            self.animated.set(target, now, ctx.dur(1800));
            self.next_cycle = Some(now + Duration::from_millis(2500));
        }
        self.toaster.tick(now);
        self.tooltip.update_visibility(now, 400);

        let inner = pad(area, 1, 0);
        let [top, mid, bottom] = Layout::vertical([
            Constraint::Length(12),
            Constraint::Length(7),
            Constraint::Fill(1),
        ])
        .areas(inner);
        let [prog_a, spin_a] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Fill(1)]).areas(top);

        // ── progress ──
        let p = pad(card(buf, prog_a, &th, "Progress"), 1, 0);
        let rows: Vec<Rect> = (0..5)
            .map(|i| Rect {
                y: p.y + i * 2,
                height: 1,
                ..p
            })
            .filter(|r| r.bottom() <= p.bottom())
            .collect();
        let labels = [
            "animated",
            "indeterminate",
            "complete",
            "threshold",
            "compact",
        ];
        for (i, r) in rows.iter().enumerate() {
            put(
                buf,
                r.x,
                r.y,
                labels[i],
                13,
                st(th.text_muted, th.background),
            );
            let bar = Rect {
                x: r.x + 14,
                width: r.width.saturating_sub(14),
                ..*r
            };
            match i {
                0 => ProgressBar::new()
                    .show_eta(true)
                    .now(now)
                    .theme(&th)
                    .render(bar, buf, &mut self.animated),
                1 => ProgressBar::new()
                    .indeterminate()
                    .now(now)
                    .theme(&th)
                    .render(bar, buf, &mut self.indeterminate),
                2 => ProgressBar::new()
                    .variant(Variant::Success)
                    .now(now)
                    .theme(&th)
                    .render(bar, buf, &mut self.complete),
                3 => {
                    let v = self.threshold.value(now);
                    let variant = if v > 0.9 {
                        Variant::Error
                    } else if v > 0.7 {
                        Variant::Warning
                    } else {
                        Variant::Primary
                    };
                    ProgressBar::new()
                        .variant(variant)
                        .show_eta(false)
                        .now(now)
                        .theme(&th)
                        .render(bar, buf, &mut self.threshold)
                }
                _ => ProgressBar::new()
                    .compact(true)
                    .show_eta(false)
                    .show_percentage(false)
                    .now(now)
                    .theme(&th)
                    .render(bar, buf, &mut self.compact),
            }
        }
        if p.height > 10 {
            let y = p.y + 10;
            put(buf, p.x, y, "steps", 13, st(th.text_muted, th.background));
            let focused = self.focus.is(Id::Steps);
            let sp = Rect {
                x: p.x + 14,
                y,
                width: 12,
                height: 1,
            };
            StepProgress::new(6)
                .current(self.step)
                .theme(&th)
                .render(sp, buf);
            let hint = format!(
                "  {}/6  ←→ {}",
                self.step,
                if focused { "(focused)" } else { "" }
            );
            put(
                buf,
                sp.right(),
                y,
                &hint,
                p.width.saturating_sub(26),
                st(
                    if focused { th.text } else { th.text_disabled },
                    th.background,
                ),
            );
        }

        // ── spinners (a sample; the Spinners page has the whole catalog) ──
        let s = pad(
            card(
                buf,
                spin_a,
                &th,
                "Spinners (hover for code · all 102 on the Spinners page)",
            ),
            1,
            0,
        );
        self.spinner_hits.clear();
        let cols_n = if s.width >= 60 { 3 } else { 2 };
        let cw = s.width / cols_n as u16;
        for (i, def) in SAMPLE.iter().enumerate() {
            let (r, c) = (i / cols_n, i % cols_n);
            let rect = Rect {
                x: s.x + c as u16 * cw,
                y: s.y + r as u16,
                width: cw.saturating_sub(1),
                height: 1,
            };
            if rect.bottom() > s.bottom() {
                break;
            }
            if self.hover_spinner == Some(i) {
                fill(buf, rect, th.hover_bg);
            }
            Spinner::new(def)
                .label(def.name)
                .now(now)
                .theme(&th)
                .render(rect, buf);
            self.spinner_hits.push((rect, def.name));
        }

        // ── loading states ──
        let l = pad(card(buf, mid, &th, "Loading states"), 1, 0);
        let [l1, l2, l3] = Layout::horizontal([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Fill(1),
        ])
        .areas(l);
        put(
            buf,
            l1.x,
            l1.y,
            "LoadingIndicator",
            l1.width,
            st(th.text_muted, th.background),
        );
        LoadingIndicator::new()
            .elapsed(ctx.elapsed())
            .theme(&th)
            .render(
                Rect {
                    y: l1.y + 1,
                    height: 1,
                    ..l1
                },
                buf,
            );
        put(
            buf,
            l1.x,
            l1.y + 3,
            "Blinker",
            l1.width,
            st(th.text_muted, th.background),
        );
        Blinker::new().elapsed(ctx.elapsed()).theme(&th).render(
            Rect {
                y: l1.y + 3,
                x: l1.x + 9,
                width: 2,
                height: 1,
            },
            buf,
        );
        put(
            buf,
            l2.x,
            l2.y,
            "Skeleton (card)",
            l2.width,
            st(th.text_muted, th.background),
        );
        Skeleton::new()
            .shape(SkeletonShape::Card)
            .elapsed(ctx.elapsed())
            .theme(&th)
            .render(
                Rect {
                    y: l2.y + 1,
                    height: l2.height.saturating_sub(1),
                    width: l2.width.saturating_sub(2),
                    ..l2
                },
                buf,
            );
        put(
            buf,
            l3.x,
            l3.y,
            "Marquee",
            l3.width,
            st(th.text_muted, th.background),
        );
        Marquee::new("tuile ships 70+ widgets  •  press 1-4 for toasts  •  m for a dialog  •  ")
            .speed(12.0)
            .elapsed(ctx.elapsed())
            .theme(&th)
            .render(
                Rect {
                    y: l3.y + 1,
                    height: 1,
                    ..l3
                },
                buf,
            );
        Skeleton::new()
            .shape(SkeletonShape::Text)
            .lines(&[l3.width.saturating_sub(4), l3.width / 2])
            .elapsed(ctx.elapsed())
            .theme(&th)
            .render(
                Rect {
                    y: l3.y + 3,
                    height: 2,
                    ..l3
                },
                buf,
            );

        // ── toasts & dialogs | callouts ──
        let [tc, cc] =
            Layout::horizontal([Constraint::Percentage(55), Constraint::Fill(1)]).areas(bottom);
        let t = pad(card(buf, tc, &th, "Toasts & dialogs"), 1, 0);
        let bw = ((t.width.saturating_sub(3)) / 4).clamp(8, 14);
        let mk = |i: u16| Rect {
            x: t.x + i * (bw + 1),
            y: t.y,
            width: bw,
            height: 3,
        };
        let specs = [
            ("Info", Variant::Default, Id::Info),
            ("Success", Variant::Success, Id::Success),
            ("Warning", Variant::Warning, Id::Warning),
            ("Error", Variant::Error, Id::Error),
        ];
        for (i, (label, v, id)) in specs.iter().enumerate() {
            Button::new(*label)
                .variant(*v)
                .min_width(bw)
                .focused(self.focus.is(*id))
                .now(now)
                .theme(&th)
                .render(mk(i as u16), buf, &mut self.buttons[i]);
        }
        let cy = t.y + 4;
        put(buf, t.x, cy, "corner", 7, st(th.text_muted, th.background));
        let seg = Rect {
            x: t.x + 8,
            y: cy,
            width: t.width.saturating_sub(8),
            height: 1,
        };
        Segmented::new(vec![
            "↗ TR".into(),
            "↘ BR".into(),
            "↖ TL".into(),
            "↙ BL".into(),
        ])
        .focused(self.focus.is(Id::Corner))
        .theme(&th)
        .render(seg, buf, &mut self.corner);
        let by = cy + 2;
        let dspecs = [
            ("Confirm", Id::Confirm),
            ("Alert", Id::Alert),
            ("Prompt", Id::Prompt),
            ("Sheet", Id::Sheet),
        ];
        for (i, (label, id)) in dspecs.iter().enumerate() {
            let r = Rect {
                y: by,
                ..mk(i as u16)
            };
            if r.bottom() <= t.bottom() {
                Button::new(*label)
                    .style(ButtonStyle::Outline)
                    .min_width(bw)
                    .focused(self.focus.is(*id))
                    .now(now)
                    .theme(&th)
                    .render(r, buf, &mut self.buttons[4 + i]);
            }
        }
        // page-local toast stack lives inside this card so corners are visible
        self.toaster.corner = CORNERS[self.corner.selected.min(3)];
        ToastStack::new().theme(&th).now(now).render(
            Rect {
                y: t.y + 4,
                height: t.height.saturating_sub(5),
                ..t
            },
            buf,
            &mut self.toaster,
        );
        {
            let hint = "toasts appear in this box; click one to dismiss, Esc clears";
            put(
                buf,
                t.x,
                t.bottom().saturating_sub(1),
                hint,
                t.width,
                st(th.text_disabled, th.background),
            );
        }

        let c = pad(
            card(buf, cc, &th, "Callouts (click the first to dismiss)"),
            1,
            0,
        );
        let cspecs = [
            (
                "Heads up",
                "Callouts are inline, non-blocking notes.",
                Variant::Primary,
                CalloutBorder::LeftBar,
                true,
            ),
            (
                "Deployed",
                "Build 4821 is live in all regions.",
                Variant::Success,
                CalloutBorder::Round,
                false,
            ),
            (
                "Quota",
                "You have used 92% of your monthly minutes.",
                Variant::Warning,
                CalloutBorder::LeftBar,
                false,
            ),
            (
                "Failed",
                "Payment declined: card expired.",
                Variant::Error,
                CalloutBorder::Round,
                false,
            ),
        ];
        let mut y = c.y;
        for (i, (title, msg, v, border, dismiss)) in cspecs.iter().enumerate() {
            let h = 4;
            if y + h > c.bottom() {
                break;
            }
            let r = Rect {
                x: c.x,
                y,
                width: c.width,
                height: h,
            };
            Callout::new(*title, *msg)
                .variant(*v)
                .border_style(*border)
                .dismissible(*dismiss)
                .theme(&th)
                .render(r, buf, &mut self.callouts[i]);
            if !self.callouts[i].dismissed {
                y += h;
            }
        }

        // overlays: dialog, then tooltip
        if self.modal.is_open() {
            let m = self.modal_builder(&th, now);
            m.render(area, buf, &mut self.modal);
            if self.modal_kind == 4 {
                // the card published its interior; the picker paints into it, above the screen
                ListView::new(vec![
                    ListEntry::new("claude-opus-4.5").detail("200k ctx"),
                    ListEntry::new("claude-sonnet-4.5").detail("200k ctx"),
                    ListEntry::new("gpt-5.1").detail("128k ctx"),
                    ListEntry::new("gemini-3-pro").detail("1M ctx"),
                ])
                .title("models")
                .focused(true)
                .theme(&th)
                .render(self.modal.body, buf, &mut self.picker);
            }
        }
        if let Some(i) = self.hover_spinner
            && let Some((rect, name)) = self.spinner_hits.get(i)
        {
            self.tooltip.track(true, *rect, now);
            Tooltip::new(&format!(
                "Spinner::new(&spinners::{}).label(..).now(now)",
                super::spinners::const_name(name)
            ))
            .max_width(40)
            .theme(&th)
            .render_overlay(buf, area, &mut self.tooltip, now);
        } else {
            self.tooltip.track(false, Rect::default(), now);
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        // the modal outranks everything underneath it, including a focused field
        if self.modal.is_open() {
            // inside a card, the body owns the keyboard; Esc still reaches the modal
            if self.modal_kind == 4 && self.picker.handle(ev).is_submitted() {
                let i = self.picker.take_activated().unwrap_or(0);
                let name = [
                    "claude-opus-4.5",
                    "claude-sonnet-4.5",
                    "gpt-5.1",
                    "gemini-3-pro",
                ][i.min(3)];
                self.modal.close();
                ctx.notify(format!("Picked {name}"), Variant::Success);
                return Outcome::Consumed;
            }
            self.modal.handle(ev);
            if let Some(r) = self.modal.take_result() {
                let text = self.modal.input_text.clone();
                self.modal.close();
                let msg = match (self.modal_kind, r) {
                    (0, 1) => "Changes discarded".to_string(),
                    (0, _) => "Kept editing".to_string(),
                    (2, 1) => format!("Renamed to `{text}`"),
                    (2, _) => "Rename cancelled".to_string(),
                    (4, _) => "Picker dismissed".to_string(),
                    _ => "Dialog closed".to_string(),
                };
                ctx.notify(msg, Variant::Primary);
            }
            return Outcome::Consumed;
        }
        let toast = |t: &mut Toaster, i: usize| match i {
            0 => t.push(
                Toast::new("Info", "Nightly build finished in 4m 12s.")
                    .timeout(Duration::from_secs(4))
                    .progress(true),
            ),
            1 => t.success("Settings saved to ~/.config/app.toml"),
            2 => t.push(
                Toast::new("Warning", "Disk usage at 91% on /var")
                    .variant(Variant::Warning)
                    .timeout(Duration::from_secs(6))
                    .progress(true),
            ),
            _ => t.push(
                Toast::new("Error", "Connection to api.example.com timed out")
                    .variant(Variant::Error)
                    .no_timeout(),
            ),
        };
        match ev {
            Event::Key(k) => {
                if self.focus.handle_key(*k).is_consumed() {
                    return Outcome::Consumed;
                }
                match k.code {
                    KeyCode::Char(c @ '1'..='4') => {
                        toast(&mut self.toaster, c as usize - '1' as usize);
                        return Outcome::Changed;
                    }
                    KeyCode::Char('m') => {
                        self.open_modal(0, ctx);
                        return Outcome::Changed;
                    }
                    KeyCode::Char('l') => {
                        self.open_modal(4, ctx);
                        return Outcome::Changed;
                    }
                    KeyCode::Esc => {
                        self.toaster.dismiss_all(ctx.now);
                        return Outcome::Consumed;
                    }
                    _ => {}
                }
                match self.focus.current() {
                    Some(Id::Info | Id::Success | Id::Warning | Id::Error) => {
                        let i = self.focus.index();
                        let o = self.buttons[i].handle_key(*k);
                        if o.is_changed() {
                            toast(&mut self.toaster, i);
                        }
                        o
                    }
                    Some(Id::Corner) => self.corner.handle_key(*k),
                    Some(Id::Confirm | Id::Alert | Id::Prompt | Id::Sheet) => {
                        let i = self.focus.index() - 5;
                        let o = self.buttons[4 + i].handle_key(*k);
                        if o.is_changed() {
                            self.open_modal(i, ctx);
                        }
                        o
                    }
                    Some(Id::Steps) => match k.code {
                        KeyCode::Left => {
                            self.step = self.step.saturating_sub(1);
                            Outcome::Changed
                        }
                        KeyCode::Right => {
                            self.step = (self.step + 1).min(6);
                            Outcome::Changed
                        }
                        _ => Outcome::Ignored,
                    },
                    None => Outcome::Ignored,
                }
            }
            Event::Mouse(m) => {
                let mut out = self.toaster.handle_mouse(*m);
                for i in 0..4 {
                    let o = self.buttons[i].handle_mouse(*m);
                    if o.is_changed() {
                        toast(&mut self.toaster, i);
                        self.focus.set_index(i);
                    }
                    out |= o;
                }
                for i in 0..4 {
                    let o = self.buttons[4 + i].handle_mouse(*m);
                    if o.is_changed() {
                        self.open_modal(i, ctx);
                        self.focus.set_index(5 + i);
                    }
                    out |= o;
                }
                let o = self.corner.handle_mouse(*m);
                if o.is_changed() {
                    self.focus.set(Id::Corner);
                }
                out |= o;
                for c in &mut self.callouts {
                    out |= c.handle_mouse(*m);
                }
                let pos = mouse_pos(m);
                let over = self.spinner_hits.iter().position(|(r, _)| r.contains(pos));
                if over != self.hover_spinner {
                    self.hover_spinner = over;
                    out |= Outcome::Consumed;
                }
                out
            }
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self, _now: Instant) -> bool {
        true
    }
}

/// A cross-section of the catalog: classics, wide ones, emoji, and tuile originals.
const SAMPLE: &[&SpinnerDef] = &[
    &spinners::DOTS,
    &spinners::LINE,
    &spinners::ARC,
    &spinners::BOUNCING_BAR,
    &spinners::MOON,
    &spinners::CLOCK,
    &spinners::MATERIAL,
    &spinners::SPARKLE,
    &spinners::RING,
    &spinners::WAVE,
    &spinners::DNA,
    &spinners::SCANNER,
    &spinners::EQUALIZER,
    &spinners::SHIMMER,
    &spinners::MATRIX,
    &spinners::BRAILLE_WAVE,
    &spinners::AESTHETIC,
    &spinners::TOGGLE,
];
