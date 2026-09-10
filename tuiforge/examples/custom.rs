//! Customizing tuiforge: your own palette, your own spinner frames, and the AI widgets wired to
//! a fake streaming model. Everything here is public API.
//!
//! ```sh
//! cargo run -p tuiforge --example custom
//! ```

use tuiforge::prelude::*;

/// A palette: name, dark?, primary; every other role is derived (override any with the setters).
const BRAND: ThemeSpec = ThemeSpec::new("brand", true, Rgb(124, 58, 237));

/// A spinner is just frames + interval; this one lives in your crate, not ours.
const ORBIT: SpinnerDef = SpinnerDef::new("orbit", 120, &["●∙∙", "∙●∙", "∙∙●", "∙●∙"]);

const REPLY: &str = "Sure - I'll switch the cache to an LRU with a 1 000 entry cap and add a hit/miss counter so we can see whether it earns its keep.";

struct Demo {
    chat: ChatState,
    composer: ComposerState,
    started: Instant,
    stream_from: Option<Instant>,
}

impl App for Demo {
    fn draw(&mut self, frame: &mut Frame, now: Instant) {
        let area = frame.area();
        let buf = frame.buffer_mut();
        let th = theme::current();
        fill(buf, area, th.background);

        // stream the canned reply into the last message
        if let Some(t0) = self.stream_from {
            let shown = revealed(REPLY, elapsed(t0, now), 45.0);
            if let Some(last) = self.chat.messages.last_mut() {
                last.text.clear();
                last.text.push_str(shown);
            }
            if shown.len() == REPLY.len() {
                self.chat.finish_stream();
                self.stream_from = None;
            }
        }

        let [chat, status, composer] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(5),
        ])
        .areas(pad(area, 1, 0));
        ChatView::new()
            .show_time(true)
            .now(now)
            .render(chat, buf, &mut self.chat);

        // status line: a custom spinner while streaming, the context gauge always
        let [left, right] =
            Layout::horizontal([Constraint::Length(28), Constraint::Fill(1)]).areas(status);
        if self.stream_from.is_some() {
            Thinking::new("Generating")
                .spinner(&ORBIT)
                .started(self.started)
                .now(now)
                .render(left, buf);
        } else {
            Spinner::new(&spinners::SPARKLE)
                .label("idle - Enter to send")
                .now(now)
                .render(left, buf);
        }
        let used = self
            .chat
            .messages
            .iter()
            .map(|m| m.text.len() as u32 / 4)
            .sum();
        ContextGauge::new(TokenUsage {
            prompt: used,
            completion: 0,
            limit: 8_000,
        })
        .compact(true)
        .render(right, buf);

        PromptComposer::new()
            .model("brand-1")
            .focused(true)
            .now(now)
            .render(composer, buf, &mut self.composer);
    }

    fn event(&mut self, ev: Event, now: Instant) -> Flow {
        if let Event::Key(k) = &ev
            && (ctrl(k, 'c') || k.code == KeyCode::Esc)
        {
            return Flow::Quit;
        }
        self.chat.handle(&ev);
        self.composer.handle(&ev);
        if let Some(text) = self.composer.take_submitted() {
            self.chat.push(ChatMessage::new(Role::User, text));
            self.chat
                .push(ChatMessage::new(Role::Assistant, "").streaming(true));
            self.chat.scroll_to_end();
            self.started = now;
            self.stream_from = Some(now);
        }
        Flow::Continue
    }

    fn animating(&self, _now: Instant) -> bool {
        true // spinner always visible
    }
}

fn main() -> std::io::Result<()> {
    theme::set(Theme::resolve(&BRAND, None));
    let mut chat = ChatState::new();
    chat.push(ChatMessage::new(
        Role::System,
        "brand-1 · custom theme · custom spinner",
    ));
    chat.push(ChatMessage::new(
        Role::Assistant,
        "Ask me anything. Replies are canned, the widgets are not.",
    ));
    run(&mut Demo {
        chat,
        composer: ComposerState::default(),
        started: Instant::now(),
        stream_from: None,
    })
}
