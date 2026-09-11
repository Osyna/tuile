# tuile

Widget library and design system for [ratatui](https://ratatui.rs): 108 widget types, a
colour system that expands ten colours into thirty semantic roles, tweened animation, mouse
handling with cached hit rectangles, and overlays.

[Documentation](https://osyna.github.io/tuile/) ·
[Repository](https://github.com/Osyna/tuile) ·
[Widget index](https://osyna.github.io/tuile/reference/widget-index.html)

```toml
[dependencies]
tuile = "0.1"
```

```rust
use tuile::prelude::*;

struct Demo { dark: SwitchState, name: InputState, focus: Focus<Id> }
#[derive(Clone, Copy, PartialEq)] enum Id { Dark, Name }

impl App for Demo {
    fn draw(&mut self, frame: &mut Frame, now: Instant) {
        let [a, b] = Layout::vertical([Constraint::Length(3); 2]).areas(center(frame.area(), 40, 7));
        let buf = frame.buffer_mut();
        Switch::new().label("Dark mode").focused(self.focus.is(Id::Dark)).now(now)
            .render(a, buf, &mut self.dark);
        Input::new().placeholder("Your name").focused(self.focus.is(Id::Name)).now(now)
            .render(b, buf, &mut self.name);
    }

    fn event(&mut self, ev: Event, _now: Instant) -> Flow {
        if let Event::Key(k) = &ev {
            if ctrl(k, 'c') { return Flow::Quit; }
            if self.focus.handle_key(*k).is_consumed() { return Flow::Continue; }
        }
        match self.focus.current() {
            Some(Id::Dark) => { self.dark.handle(&ev); }
            Some(Id::Name) => { self.name.handle(&ev); }
            None => {}
        }
        Flow::Continue
    }

    fn animating(&self, now: Instant) -> bool { self.dark.animating(now) }
}

fn main() -> std::io::Result<()> {
    theme::set_by_name("nord");
    run(&mut Demo {
        dark: SwitchState::new(true),
        name: InputState::new(),
        focus: Focus::new([Id::Dark, Id::Name]),
    })
}
```

## What it adds over plain ratatui

Focus rings, hover and press tracking, scroll offsets, dropdowns and dialogs that paint on
top, twelve colour palettes, and animation that redraws only while something moves. Every
widget is a builder plus a state: `Widget::new().options().render(area, buf, &mut state)`,
and `state.handle(&event)` returns `Ignored`, `Consumed` or `Changed`.

ratatui and crossterm are re-exported (`tuile::ratatui`, `tuile::crossterm`), so an app does
not pin its own versions.

## Families

Controls, text entry, navigation, tables, nine chart types, feedback, notifications, loaders,
layout and chrome, big text and menus, content, and a set of components for building an AI
chat or coding-agent interface.

The gallery app in the repository has a page per family:

```
git clone https://github.com/Osyna/tuile && cd tuile
cargo run --release -p showcase
```

## Status

Pre-1.0. `#![forbid(unsafe_code)]`, 260 tests, MSRV 1.88 (edition 2024). The widget contract
is stable; individual builder methods can still be renamed between 0.x releases.

Not affiliated with Textualize. The design language is [Textual](https://textual.textualize.io)'s,
the code is not.

MIT.
