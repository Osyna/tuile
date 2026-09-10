//! AI / LLM: chat, streaming, thinking, tool calls, context gauge, approvals, diffs.

use std::time::Duration;
use tuiforge::prelude::*;

use super::{Ctx, Page, card};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Chat,
    Composer,
    Approval,
}

pub struct AiPage {
    focus: Focus<Id>,
    chat: ChatState,
    composer: ComposerState,
    approval: ApprovalState,
    /// When the current canned reply started streaming; `None` once complete.
    stream_started: Option<Instant>,
    /// The seeded conversation streams its last reply on the first draw.
    stream_pending: bool,
    /// After a choice the approval card shows the result until this instant, then re-arms.
    approval_delay: Option<(Instant, ApprovalChoice)>,
    canned_reply: &'static str,
}

impl Default for AiPage {
    fn default() -> Self {
        let mut chat = ChatState::new();
        chat.push(ChatMessage::new(Role::System, "You are a helpful coding assistant."));
        chat.push(ChatMessage::new(Role::User, "Can you add retries to the fetch helper?").time("14:02"));
        chat.push(ChatMessage::new(Role::Assistant, "I'll add exponential backoff retries to the fetch helper:\n\n```rust\nasync fn fetch_with_retry(url: &str, max_retries: u32) -> Result<Response> {\n    let mut delay = Duration::from_millis(100);\n    for attempt in 0..max_retries {\n        match reqwest::get(url).await {\n            Ok(r) => return Ok(r),\n            Err(e) if attempt < max_retries - 1 => {\n                sleep(delay).await;\n                delay *= 2;\n            }\n            Err(e) => return Err(e),\n        }\n    }\n}\n```\n\nThis implements exponential backoff with configurable retries.").time("14:02"));
        chat.push(ChatMessage::new(Role::Tool, "cargo test\n   Compiling fetch v0.1.0\n    Finished test [unoptimized + debuginfo] target(s) in 2.34s\n\ntest fetch::tests::retry_on_failure ... ok\n\ntest result: ok. 124 passed; 0 failed").author("bash"));

        AiPage {
            focus: Focus::new([Id::Composer, Id::Chat, Id::Approval]),
            chat,
            composer: ComposerState::default(),
            approval: ApprovalState::default(),
            stream_started: None,
            stream_pending: true,
            approval_delay: None,
            canned_reply: "Perfect - the tests pass. The retry logic is in place with exponential backoff: each retry doubles the delay, starting from 100 ms, so transient failures get time to clear without hammering the server.",
        }
    }
}

impl AiPage {
    fn start_stream(&mut self, now: Instant) {
        self.chat.push(ChatMessage::new(Role::Assistant, "").streaming(true));
        self.stream_started = Some(now);
        self.stream_pending = false;
    }

    /// Reveal the canned reply at 40 chars/s into the last (streaming) message.
    fn tick_stream(&mut self, now: Instant) {
        if self.stream_pending {
            self.start_stream(now);
        }
        let Some(started) = self.stream_started else { return };
        let shown = revealed(self.canned_reply, elapsed(started, now), 40.0);
        if let Some(last) = self.chat.messages.last_mut() {
            last.text.clear();
            last.text.push_str(shown);
        }
        if shown.len() == self.canned_reply.len() {
            self.chat.finish_stream();
            self.stream_started = None;
        }
    }

    fn resolve_approval(&mut self, ctx: &mut Ctx) -> bool {
        let Some(choice) = self.approval.take_choice() else { return false };
        let (msg, v) = match choice {
            ApprovalChoice::Once => ("Approved once", Variant::Success),
            ApprovalChoice::Always => ("Always allowed", Variant::Success),
            ApprovalChoice::Deny => ("Denied", Variant::Error),
        };
        ctx.notify(msg, v);
        self.approval_delay = Some((ctx.now + Duration::from_millis(1500), choice));
        self.approval.focus = 0;
        true
    }
}

impl Page for AiPage {
    fn title(&self) -> &'static str {
        "AI"
    }

    fn subtitle(&self) -> &'static str {
        "Chat, streaming, thinking, tool calls, context gauge, approvals, diffs"
    }

    fn icon(&self) -> &'static str {
        "✦"
    }

    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[("Enter", "Send"), ("⇧Enter", "Newline"), ("y/a/n", "Approve"), ("r", "Replay"), ("Tab", "Focus")]
    }

    fn animating(&self, _now: Instant) -> bool {
        true // always animating due to spinners/streaming
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        let now = ctx.now;
        self.tick_stream(now);
        if self.approval_delay.is_some_and(|(t, _)| now >= t) {
            self.approval_delay = None;
        }

        // left: chat over composer; right: cards while they fit
        let wide = area.width >= 96;
        let [left, right] = if wide {
            Layout::horizontal([Constraint::Percentage(56), Constraint::Fill(1)]).areas(area)
        } else {
            [area, Rect::default()]
        };
        let composer_h = 5.min(left.height / 2);
        let [chat_area, composer_area] = Layout::vertical([Constraint::Fill(1), Constraint::Length(composer_h)]).areas(pad(left, 1, 0));
        ChatView::new().show_time(true).focused(self.focus.is(Id::Chat)).now(now).theme(&th).render(chat_area, buf, &mut self.chat);
        PromptComposer::new()
            .model("claude-sonnet-4")
            .attachments(&["fetch.rs"])
            .focused(self.focus.is(Id::Composer))
            .now(now)
            .theme(&th)
            .render(composer_area, buf, &mut self.composer);
        if right.width < 30 {
            return;
        }

        let mut y = right.y;
        let slot = |h: u16, y: &mut u16| -> Option<Rect> {
            (*y + h <= right.bottom()).then(|| {
                let r = Rect { x: right.x, y: *y, width: right.width, height: h };
                *y += h;
                r
            })
        };

        if let Some(r) = slot(4, &mut y) {
            let c = pad(card(buf, r, &th, "Thinking"), 1, 0);
            Thinking::new("Thinking").now(now).theme(&th).render(Rect { height: 1, ..c }, buf);
            Thinking::new("Reading files")
                .spinner(&spinners::SPARKLE)
                .detail("analyzing 3 modules")
                .started(ctx.started)
                .now(now)
                .theme(&th)
                .render(Rect { y: c.y + 1, height: 1, ..c }, buf);
        }

        if let Some(r) = slot(5, &mut y) {
            let c = pad(card(buf, r, &th, "Context"), 1, 0);
            ContextGauge::new(TokenUsage { prompt: 12_400, completion: 3_200, limit: 200_000 })
                .label("context")
                .cost_usd(0.0123)
                .theme(&th)
                .render(Rect { height: 2, ..c }, buf);
            put(buf, c.x, c.y + 2, "near the limit", 14, st(th.text_muted, th.background));
            ContextGauge::new(TokenUsage { prompt: 180_000, completion: 12_000, limit: 200_000 })
                .compact(true)
                .theme(&th)
                .render(Rect { x: c.x + 15, y: c.y + 2, width: c.width.saturating_sub(15), height: 1 }, buf);
        }

        let calls = [
            ToolCall::new("read_file").args(&[("path", "src/fetch.rs")]).status(ToolStatus::Running).now(now).theme(&th),
            ToolCall::new("bash")
                .args(&[("cmd", "cargo test")])
                .status(ToolStatus::Done)
                .duration_ms(340)
                .output("    Finished test in 2.34s\ntest result: ok. 124 passed")
                .max_output_lines(2)
                .theme(&th),
            ToolCall::new("write_file").args(&[("path", "README.md")]).status(ToolStatus::Failed).duration_ms(12).output("Permission denied").theme(&th),
        ];
        let calls_h: u16 = calls.iter().map(|c| c.height(right.width - 4)).sum::<u16>() + 2;
        if let Some(r) = slot(calls_h, &mut y) {
            let c = pad(card(buf, r, &th, "Tool calls"), 1, 0);
            let mut ty = c.y;
            for call in calls {
                let h = call.height(c.width);
                call.render(Rect { y: ty, height: h, ..c }, buf);
                ty += h;
            }
        }

        let approval = Approval::new("Allow bash to run `cargo test`?")
            .detail("The assistant wants to verify the change by running the test suite.")
            .focused(self.focus.is(Id::Approval))
            .theme(&th);
        if let Some(r) = slot(approval.height(right.width - 4) + 2, &mut y) {
            let c = pad(card(buf, r, &th, "Approval"), 1, 0);
            match self.approval_delay {
                Some((_, choice)) => {
                    let (text, color) = match choice {
                        ApprovalChoice::Once => ("✓ allowed once - running…", th.success),
                        ApprovalChoice::Always => ("✓ always allowed for bash", th.success),
                        ApprovalChoice::Deny => ("✗ denied", th.error),
                    };
                    put(buf, c.x + 1, c.y + 1, text, c.width.saturating_sub(2), st(color, th.background).add_modifier(Modifier::BOLD));
                }
                None => approval.render(c, buf, &mut self.approval),
            }
        }

        // what is left: the diff first (needs 8 rows), the token heat map with the remainder
        let rest = right.bottom().saturating_sub(y);
        let heat_h = 5;
        let diff_h = if rest >= 8 + heat_h { rest - heat_h } else if rest >= 8 { rest } else { 0 };
        if let Some(r) = slot(diff_h, &mut y).filter(|r| r.height >= 8) {
            let diff_lines = DiffView::parse(DIFF);
            let (adds, dels) = DiffView::stats(&diff_lines);
            let c = pad(card(buf, r, &th, &format!("Proposed edit  +{adds} −{dels}")), 1, 0);
            DiffView::new(&diff_lines).file("src/lib.rs").line_numbers(true).theme(&th).render(c, buf);
        }
        if let Some(r) = slot(heat_h, &mut y) {
            let c = pad(card(buf, r, &th, "Token confidence"), 1, 0);
            TokenHeat::new(&[
                ("The", 0.95),
                (" retry", 0.72),
                (" logic", 0.88),
                (" is", 0.91),
                (" now", 0.65),
                (" in", 0.89),
                (" place", 0.73),
                (" with", 0.86),
                (" exponential", 0.42),
                (" backoff", 0.38),
                (".", 0.94),
            ])
            .legend(true)
            .theme(&th)
            .render(c, buf);
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        match ev {
            Event::Key(k) => {
                if self.focus.handle_key(*k).is_consumed() {
                    return Outcome::Consumed;
                }
                match self.focus.current() {
                    Some(Id::Composer) => {
                        let out = self.composer.handle_key(*k);
                        if let Some(text) = self.composer.take_submitted() {
                            self.chat.push(ChatMessage::new(Role::User, text));
                            self.chat.scroll_to_end();
                            self.start_stream(ctx.now);
                            ctx.notify("Sent", Variant::Primary);
                            return Outcome::Changed;
                        }
                        out
                    }
                    Some(Id::Approval) => {
                        let out = self.approval.handle_key(*k);
                        if self.resolve_approval(ctx) {
                            return Outcome::Changed;
                        }
                        out
                    }
                    _ => {
                        if is_press(k) && k.code == KeyCode::Char('r') {
                            self.start_stream(ctx.now);
                            return Outcome::Changed;
                        }
                        self.chat.handle_key(*k)
                    }
                }
            }
            Event::Mouse(m) => {
                let mut out = self.chat.handle_mouse(*m) | self.composer.handle_mouse(*m);
                if self.approval_delay.is_none() {
                    out |= self.approval.handle_mouse(*m);
                    if self.resolve_approval(ctx) {
                        out = Outcome::Changed;
                    }
                }
                out
            }
            _ => Outcome::Ignored,
        }
    }
}

const DIFF: &str = "@@ -12,3 +12,8 @@\n async fn fetch(url: &str) -> Result<Response> {\n-    reqwest::get(url).await\n+    fetch_with_retry(url, 3).await\n+}\n+\n+async fn fetch_with_retry(url: &str, max: u32) -> Result<Response> {\n+    // retry logic here\n+    todo!()\n }";
