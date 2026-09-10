//! The smallest useful tuiforge app: a switch and an input with focus, mouse and animation.
//!
//! ```sh
//! cargo run -p tuiforge --example minimal
//! ```

use tuiforge::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Id {
    Dark,
    Name,
    Save,
}

struct Demo {
    dark: SwitchState,
    name: InputState,
    save: ButtonState,
    focus: Focus<Id>,
    toasts: Toaster,
}

impl App for Demo {
    fn draw(&mut self, frame: &mut Frame, now: Instant) {
        let area = frame.area();
        let buf = frame.buffer_mut();
        let th = theme::current();
        fill(buf, area, th.background);
        let card = center(area, 46, 13);
        let inner = Border::Round.draw_titled(
            buf,
            card,
            th.border_blurred,
            th.background,
            "tuiforge",
            Alignment::Left,
        );
        let [a, b, c] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .areas(pad(inner, 1, 0));

        Switch::new()
            .label("Dark mode")
            .focused(self.focus.is(Id::Dark))
            .now(now)
            .render(a, buf, &mut self.dark);
        Input::new()
            .placeholder("Your name")
            .focused(self.focus.is(Id::Name))
            .now(now)
            .render(b, buf, &mut self.name);
        Button::new("Save")
            .variant(Variant::Primary)
            .focused(self.focus.is(Id::Save))
            .now(now)
            .render(Rect { width: 16, ..c }, buf, &mut self.save);
        put(
            buf,
            c.x + 18,
            c.y + 1,
            "Tab: focus · Enter: save · ^c: quit",
            c.width.saturating_sub(18),
            st(th.text_muted, th.background),
        );

        ToastStack::new()
            .now(now)
            .render(area, buf, &mut self.toasts);
    }

    fn update(&mut self, now: Instant) {
        self.toasts.tick(now);
    }

    fn event(&mut self, ev: Event, _now: Instant) -> Flow {
        if let Event::Key(k) = &ev {
            if ctrl(k, 'c') {
                return Flow::Quit;
            }
            if self.focus.handle_key(*k).is_consumed() {
                return Flow::Continue;
            }
        }
        let out = match (&ev, self.focus.current()) {
            (Event::Key(_), Some(Id::Dark)) => self.dark.handle(&ev),
            (Event::Key(_), Some(Id::Name)) => self.name.handle(&ev),
            (Event::Key(_), Some(Id::Save)) => self.save.handle(&ev),
            // every widget sees every mouse event and checks its own rect
            (Event::Mouse(_), _) => {
                let mut o = self.dark.handle(&ev);
                if o.is_changed() {
                    self.focus.set(Id::Dark);
                }
                let n = self.name.handle(&ev);
                if n.is_consumed() {
                    self.focus.set(Id::Name);
                }
                let s = self.save.handle(&ev);
                if s.is_changed() {
                    self.focus.set(Id::Save);
                }
                o |= n | s;
                o
            }
            _ => Outcome::Ignored,
        };
        if out.is_changed() {
            if self.focus.is(Id::Dark) {
                theme::set_by_name(if self.dark.on {
                    "textual-dark"
                } else {
                    "textual-light"
                });
            }
            if self.focus.is(Id::Save) {
                self.toasts.success(format!("Saved {:?}", self.name.value));
            }
        }
        Flow::Continue
    }

    fn animating(&self, now: Instant) -> bool {
        self.dark.animating(now) || self.save.animating(now) || self.toasts.animating(now)
    }
}

fn main() -> std::io::Result<()> {
    theme::set_by_name("textual-dark");
    run(&mut Demo {
        dark: SwitchState::new(true),
        name: InputState::new(),
        save: ButtonState::new(),
        focus: Focus::new([Id::Dark, Id::Name, Id::Save]),
        toasts: Toaster::new(),
    })
}
