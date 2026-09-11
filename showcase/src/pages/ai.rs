//! AI: one harness turn replayed - thinking, markdown, inline tool calls, streaming, approvals.

use std::time::{Duration, Instant};
use tuiforge::prelude::*;
use tuiforge::widgets::ai::{
    Approval, ApprovalChoice, ApprovalState, ApprovalStyle, ChatBlock, ChatMessage, ChatState, ChatView,
    ComposerState, ContextGauge, PromptComposer, Role, StreamCursor, StreamText, Thinking, TokenHeat,
    TokenUsage, ToolStatus, TypingIndicator,
};

use super::{Ctx, Page, card};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Chat,
    Composer,
    Approval,
}

/// What the replay does at a given second of the script.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Ask,
    Typing,
    ThinkStart,
    ThinkDone,
    AnswerStart,
    Tool(usize, ToolStatus),
    Code,
    AskApproval,
    Final,
}

/// (seconds into the turn, step). Approval pauses the clock until answered.
const SCRIPT: &[(f32, Step)] = &[
    (0.3, Step::Ask),
    (0.8, Step::Typing),
    (1.8, Step::ThinkStart),
    (5.0, Step::ThinkDone),
    (5.4, Step::AnswerStart),
    (9.0, Step::Tool(0, ToolStatus::Pending)),
    (9.4, Step::Tool(0, ToolStatus::Running)),
    (10.2, Step::Tool(0, ToolStatus::Done)),
    (10.3, Step::Tool(1, ToolStatus::Pending)),
    (10.6, Step::Tool(1, ToolStatus::Running)),
    (12.0, Step::Tool(1, ToolStatus::Done)),
    (12.1, Step::Tool(2, ToolStatus::Pending)),
    (12.4, Step::Tool(2, ToolStatus::Running)),
    (13.2, Step::Tool(2, ToolStatus::Error)),
    (13.8, Step::Code),
    (15.0, Step::AskApproval),
    (15.5, Step::Final),
];

const THOUGHT: &str = "The fetch helper has no retry path. Exponential backoff with a jitter cap is the \
standard choice; three attempts keeps the worst case under two seconds. I should also run the \
existing tests to make sure nothing regresses.";

const ANSWER: &str = "## Plan\n- add **exponential backoff** to `fetch_with_retry`\n- cap attempts at **3**, \
delays 100ms → 200ms → 400ms\n- run `cargo test` and report\n\nStarting with the helper:";

const CODE: &str = "async fn fetch_with_retry(url: &str, max: u32) -> Result<Response> {\n    let mut delay = \
Duration::from_millis(100);\n    for attempt in 0..max {\n        match reqwest::get(url).await {\n            Ok(r) => \
return Ok(r),\n            Err(e) if attempt + 1 < max => {\n                sleep(delay).await;\n                delay *= 2;\n            }\n            Err(e) => return Err(e),\n        }\n    }\n    unreachable!()\n}";

const FINAL: &str = "Tests pass: **124 ok**, the retry path is covered by `fetch::tests::retry_on_failure`. \
The `write_file` step was denied, so `README.md` is unchanged - say the word and I'll add the usage note.";

const TOOLS: [(&str, &str); 3] =
    [("read_file", "src/fetch.rs"), ("bash", "cargo test --workspace"), ("write_file", "README.md · permission denied")];

pub struct AiPage {
    focus: Focus<Id>,
    chat: ChatState,
    composer: ComposerState,
    inline_approval: ApprovalState,
    card_approval: ApprovalState,
    banner_approval: ApprovalState,
    compact: bool,
    /// Page clock origin; `None` before the first draw.
    started: Option<Instant>,
    /// Time subtracted from the clock while the approval waits (and the last pause start).
    paused: f32,
    pause_start: Option<Instant>,
    next_step: usize,
    typing: bool,
    approval_open: bool,
    /// Which right-column approval the keys go to (0 card, 1 banner).
    approval_focus: usize,
    /// Result shown on the resolved approvals until this instant, then they re-arm.
    resolved: [Option<(Instant, ApprovalChoice)>; 2],
    think_started: Option<Instant>,
}

impl Default for AiPage {
    fn default() -> Self {
        Self {
            focus: Focus::new([Id::Chat, Id::Composer, Id::Approval]),
            chat: ChatState::new(),
            composer: ComposerState::default(),
            inline_approval: ApprovalState::default(),
            card_approval: ApprovalState::default(),
            banner_approval: ApprovalState::default(),
            compact: false,
            started: None,
            paused: 0.0,
            pause_start: None,
            next_step: 0,
            typing: false,
            approval_open: false,
            approval_focus: 0,
            resolved: [None, None],
            think_started: None,
        }
    }
}

impl AiPage {
    /// Seconds into the turn, excluding time spent waiting for the approval.
    fn clock(&self, now: Instant) -> f32 {
        let Some(started) = self.started else { return 0.0 };
        let paused = self.paused
            + self.pause_start.map_or(0.0, |p| now.saturating_duration_since(p).as_secs_f32());
        now.saturating_duration_since(started).as_secs_f32() - paused
    }

    fn restart(&mut self, now: Instant) {
        self.chat = ChatState::new();
        self.started = Some(now);
        self.paused = 0.0;
        self.pause_start = None;
        self.next_step = 0;
        self.typing = false;
        self.approval_open = false;
        self.inline_approval = ApprovalState::default();
        self.think_started = None;
    }

    fn apply(&mut self, step: Step, now: Instant) {
        match step {
            Step::Ask => {
                self.chat.push(ChatMessage::new(Role::System, "").block(ChatBlock::Divider("14:02".into())));
                self.chat.push(
                    ChatMessage::new(Role::User, "Add retries to the fetch helper and run the tests")
                        .author("irvin")
                        .time("14:02"),
                );
            }
            Step::Typing => self.typing = true,
            Step::ThinkStart => {
                self.typing = false;
                self.think_started = Some(now);
                let msg = ChatMessage::new(Role::Assistant, "").author("Claude").time("14:02").block(
                    ChatBlock::Thinking { text: String::new(), secs: 0.0, collapsed: false, streaming: true },
                );
                self.chat.begin_stream(msg, THOUGHT.to_string(), now);
            }
            Step::ThinkDone => {
                // finish the thought stream whatever it revealed so far, then fold it
                self.chat.stream_tick(now + Duration::from_secs(3600), 60.0);
                let secs = self.think_started.map_or(3.1, |t| now.saturating_duration_since(t).as_secs_f32());
                if let Some(ChatBlock::Thinking { secs: s, collapsed, .. }) =
                    self.chat.messages.last_mut().and_then(|m| m.blocks.last_mut())
                {
                    *s = secs;
                    *collapsed = true;
                }
            }
            Step::AnswerStart => {
                // stream the markdown answer into a new Text block of the same message
                if let Some(m) = self.chat.messages.last_mut() {
                    m.blocks.push(ChatBlock::Text(String::new()));
                    m.streaming = true;
                }
                self.chat_restream(ANSWER, now);
            }
            Step::Tool(i, status) => {
                let (name, summary) = TOOLS[i];
                let msg = self.chat.messages.last_mut().expect("assistant message exists");
                let existing = msg.blocks.iter_mut().find_map(|b| match b {
                    ChatBlock::ToolCall { name: n, status: s, duration_ms, .. } if n == name => Some((s, duration_ms)),
                    _ => None,
                });
                match existing {
                    Some((s, dur)) => {
                        *s = status;
                        if matches!(status, ToolStatus::Done | ToolStatus::Error) {
                            *dur = Some([820, 1_340, 12][i]);
                        }
                    }
                    None => msg.blocks.push(ChatBlock::ToolCall {
                        name: name.into(),
                        summary: summary.into(),
                        status,
                        duration_ms: None,
                    }),
                }
            }
            Step::Code => {
                if let Some(m) = self.chat.messages.last_mut() {
                    m.blocks.push(ChatBlock::Code { lang: Some("rust".into()), text: CODE.into() });
                }
            }
            Step::AskApproval => {
                self.approval_open = true;
                self.inline_approval = ApprovalState::default();
                self.pause_start = Some(now);
            }
            Step::Final => {
                let msg = ChatMessage::new(Role::Assistant, "").author("Claude").time("14:03").block(ChatBlock::Text(String::new()));
                self.chat.begin_stream(msg, FINAL.to_string(), now);
            }
        }
    }

    /// Re-point the stream at the last message without pushing a new one.
    fn chat_restream(&mut self, full: &str, now: Instant) {
        let msg = self.chat.messages.pop().expect("message to restream");
        self.chat.begin_stream(msg, full.to_string(), now);
    }

    fn tick(&mut self, now: Instant) {
        if self.started.is_none() {
            self.restart(now);
        }
        let t = self.clock(now);
        while let Some(&(at, step)) = SCRIPT.get(self.next_step) {
            if t < at || self.approval_open {
                break;
            }
            self.apply(step, now);
            self.next_step += 1;
        }
        self.chat.stream_tick(now, 60.0);
        // loop the replay a few seconds after the final answer lands
        if self.next_step >= SCRIPT.len() && !self.chat.messages.last().is_some_and(|m| m.streaming) && t > 32.0 {
            self.restart(now);
        }
    }

    fn resolve_inline(&mut self, ctx: &mut Ctx) {
        if let Some(choice) = self.inline_approval.take_choice() {
            self.approval_open = false;
            if let Some(p) = self.pause_start.take() {
                self.paused += ctx.now.saturating_duration_since(p).as_secs_f32();
            }
            let (msg, v) = match choice {
                ApprovalChoice::Once => ("bash allowed once", Variant::Success),
                ApprovalChoice::Always => ("bash always allowed for this session", Variant::Primary),
                ApprovalChoice::Deny => ("bash denied", Variant::Error),
            };
            ctx.notify(msg, v);
            if self.focus.is(Id::Approval) {
                self.focus.set(Id::Chat);
            }
        }
    }
}

impl Page for AiPage {
    fn title(&self) -> &'static str {
        "AI"
    }

    fn subtitle(&self) -> &'static str {
        "Chat with thinking, markdown, inline tool calls, streaming, approvals"
    }

    fn icon(&self) -> &'static str {
        "✶"
    }

    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[("Tab", "Focus"), ("↑↓ G", "Scroll / follow"), ("y/a/n", "Approve"), ("t", "Compact"), ("r", "Replay"), ("Enter", "Send")]
    }

    fn animating(&self, _now: Instant) -> bool {
        true
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        let now = ctx.now;
        self.tick(now);
        for (i, slot) in self.resolved.iter_mut().enumerate() {
            if slot.is_some_and(|(until, _)| now >= until) {
                *slot = None;
                if i == 0 {
                    self.card_approval = ApprovalState::default();
                } else {
                    self.banner_approval = ApprovalState::default();
                }
            }
        }

        let reduced = area.width < 90 || area.height < 30;
        let (left, right) = if reduced {
            (area, Rect::ZERO)
        } else {
            let [l, r] = Layout::horizontal([Constraint::Percentage(62), Constraint::Fill(1)]).areas(area);
            (l, r)
        };

        // ── conversation + docked rows + composer ──
        let composer_h = 3;
        let [conv_area, composer_area] = Layout::vertical([Constraint::Fill(1), Constraint::Length(composer_h)]).areas(left);
        let inner = card(buf, conv_area, &th, "Conversation");
        let dock_h = if self.approval_open { 1 } else if self.typing { 1 } else { 0 };
        let [chat_area, dock] = Layout::vertical([Constraint::Fill(1), Constraint::Length(dock_h)]).areas(inner);
        ChatView::new()
            .bubbles(true)
            .show_time(true)
            .hover(true)
            .compact(self.compact)
            .focused(self.focus.is(Id::Chat))
            .now(now)
            .theme(&th)
            .render(chat_area, buf, &mut self.chat);
        if dock_h > 0 {
            if self.approval_open {
                Approval::new("Allow bash to run `cargo test --workspace`?")
                    .style(ApprovalStyle::Inline)
                    .focused(self.focus.is(Id::Approval))
                    .now(now)
                    .theme(&th)
                    .render(dock, buf, &mut self.inline_approval);
            } else {
                TypingIndicator::new().label("Claude is typing").now(now).theme(&th).render(dock, buf);
            }
        }
        PromptComposer::new()
            .model("claude-sonnet-4")
            .placeholder("Reply…")
            .focused(self.focus.is(Id::Composer))
            .now(now)
            .theme(&th)
            .render(composer_area, buf, &mut self.composer);

        if reduced {
            return;
        }

        // ── right column ──
        let danger = Approval::new("Allow bash to delete the build directory?")
            .detail("This removes every compiled artefact; the next build starts from scratch.")
            .command("rm -rf target")
            .danger(true);
        let card_h = danger.height(right.width.saturating_sub(2));
        let [approvals, cursors, context, thinking] = Layout::vertical([
            Constraint::Length(card_h + 2 + 1 + 2),
            Constraint::Length(7),
            Constraint::Length(8),
            Constraint::Fill(1),
        ])
        .areas(right);

        let inner = card(buf, approvals, &th, "Approval styles");
        let [card_a, _gap, banner_a] =
            Layout::vertical([Constraint::Length(card_h), Constraint::Length(1), Constraint::Length(2)]).areas(inner);
        let focus_right = !self.approval_open && self.focus.is(Id::Approval);
        match self.resolved[0] {
            Some((_, choice)) => {
                put(buf, card_a.x + 1, card_a.y + 1, &format!("rm -rf target → {choice:?}"), card_a.width.saturating_sub(2), st(th.text_muted, th.background));
            }
            None => danger
                .focused(focus_right && self.approval_focus == 0)
                .now(now)
                .theme(&th)
                .render(card_a, buf, &mut self.card_approval),
        }
        match self.resolved[1] {
            Some((_, choice)) => {
                put(buf, banner_a.x + 1, banner_a.y, &format!("git push → {choice:?}"), banner_a.width.saturating_sub(2), st(th.text_muted, th.background));
            }
            None => Approval::new("Push 3 commits to origin/main?")
                .command("git push origin main")
                .style(ApprovalStyle::Banner)
                .focused(focus_right && self.approval_focus == 1)
                .now(now)
                .theme(&th)
                .render(banner_a, buf, &mut self.banner_approval),
        }

        let inner = card(buf, cursors, &th, "Stream cursors");
        let phase = ctx.elapsed() % 6.0;
        let rows: [Rect; 4] = Layout::vertical([Constraint::Length(1); 4]).areas(inner);
        let demos: [(&str, StreamCursor, bool, bool); 4] = [
            ("block", StreamCursor::Block, false, false),
            ("bar", StreamCursor::Bar, false, false),
            ("underline + fade", StreamCursor::Underline, true, false),
            ("word mode", StreamCursor::None, false, true),
        ];
        let sample = "Streaming tokens arrive a few at a time; the cursor style is yours to pick.";
        for ((label, cursor, fade, words), row) in demos.iter().zip(rows.iter()) {
            let label_w = 17;
            put(buf, row.x, row.y, label, label_w, st(th.text_muted, th.background));
            let text_area = Rect { x: row.x + label_w, width: row.width.saturating_sub(label_w), ..*row };
            StreamText::new(sample)
                .elapsed(phase)
                .cps(if *words { 22.0 } else { 18.0 })
                .cursor(*cursor)
                .fade(*fade)
                .word_mode(*words)
                .theme(&th)
                .render(text_area, buf);
        }

        let inner = card(buf, context, &th, "Context");
        let [gauge, heat] = Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(inner);
        let used = 15_600 + (self.clock(now) * 900.0) as u32;
        ContextGauge::new(TokenUsage { prompt: used, completion: 3_100, limit: 200_000 })
            .compact(true)
            .cost_usd(0.0123 + self.clock(now) * 0.0004)
            .theme(&th)
            .render(gauge, buf);
        let tokens: [(&str, f32); 9] = [
            ("The", 0.98), (" retry", 0.61), (" logic", 0.93), (" is", 0.99), (" now", 0.72), (" in", 0.97), (" place", 0.55),
            (" with", 0.9), (" backoff.", 0.34),
        ];
        TokenHeat::new(&tokens).legend(true).theme(&th).render(heat, buf);

        let inner = card(buf, thinking, &th, "Thinking");
        let rows: [Rect; 2] = Layout::vertical([Constraint::Length(1); 2]).areas(inner);
        Thinking::new("Thinking").elapsed(self.clock(now) % 9.0).elapsed_label(true).now(now).theme(&th).render(rows[0], buf);
        Thinking::new("Reading files")
            .spinner(&tuiforge::widgets::spinners::LINE)
            .detail("3 modules")
            .elapsed(2.3 + (self.clock(now) % 4.0))
            .elapsed_label(true)
            .now(now)
            .theme(&th)
            .render(rows[1], buf);
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        match ev {
            Event::Key(k) if is_press(k) => {
                match k.code {
                    KeyCode::Tab => {
                        self.focus.next();
                        return Outcome::Changed;
                    }
                    KeyCode::BackTab => {
                        self.focus.prev();
                        return Outcome::Changed;
                    }
                    KeyCode::Char('r') if !self.focus.is(Id::Composer) => {
                        self.restart(ctx.now);
                        return Outcome::Changed;
                    }
                    KeyCode::Char('t') if !self.focus.is(Id::Composer) => {
                        self.compact = !self.compact;
                        return Outcome::Changed;
                    }
                    _ => {}
                }
                // the live inline approval always answers y/a/n while it is open
                if self.approval_open && !self.focus.is(Id::Composer) {
                    let out = self.inline_approval.handle_key(*k);
                    if out.is_changed() {
                        self.resolve_inline(ctx);
                        return Outcome::Changed;
                    }
                    if out.is_consumed() {
                        return out;
                    }
                }
                match self.focus.current() {
                    Some(Id::Chat) => self.chat.handle_key(*k),
                    Some(Id::Composer) => {
                        let out = self.composer.handle_key(*k);
                        if let Some(text) = self.composer.take_submitted() {
                            self.chat.push(ChatMessage::new(Role::User, text).author("irvin").time("14:04"));
                            ctx.notify("Sent", Variant::Primary);
                        }
                        out
                    }
                    Some(Id::Approval) => {
                        if matches!(k.code, KeyCode::Up | KeyCode::Down) {
                            self.approval_focus ^= 1;
                            return Outcome::Changed;
                        }
                        let (state, idx) = if self.approval_focus == 0 {
                            (&mut self.card_approval, 0)
                        } else {
                            (&mut self.banner_approval, 1)
                        };
                        let out = state.handle_key(*k);
                        if let Some(choice) = state.take_choice() {
                            self.resolved[idx] = Some((ctx.now + Duration::from_millis(1500), choice));
                            ctx.notify(format!("{choice:?}"), Variant::Default);
                        }
                        out
                    }
                    None => Outcome::Ignored,
                }
            }
            Event::Mouse(m) => {
                let mut out = self.chat.handle_mouse(*m);
                out |= self.composer.handle_mouse(*m);
                if self.approval_open {
                    out |= self.inline_approval.handle_mouse(*m);
                    self.resolve_inline(ctx);
                }
                out |= self.card_approval.handle_mouse(*m);
                if let Some(choice) = self.card_approval.take_choice() {
                    self.resolved[0] = Some((ctx.now + Duration::from_millis(1500), choice));
                }
                out |= self.banner_approval.handle_mouse(*m);
                if let Some(choice) = self.banner_approval.take_choice() {
                    self.resolved[1] = Some((ctx.now + Duration::from_millis(1500), choice));
                }
                out
            }
            _ => Outcome::Ignored,
        }
    }
}
