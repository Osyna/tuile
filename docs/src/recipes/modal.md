# Modal dialogs and overlays

A modal is a card over the current screen that owns the keyboard until closed. This recipe shows how to layer them correctly and use `Modal::card` for custom content.

## The layer order

Modal dialogs render in four layers, from back to front:

1. **Screen** — your page, buttons, fields, lists
2. **Backdrop** — dimmed overlay covering the screen  
3. **Card** — centered card with border, title, and body
4. **Overlay queue** — dropdowns and pickers rendered *over* the card

## Keyboard priority

A modal outranks the focused field underneath. The rule:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# struct Page { modal: ModalState, field: InputState }
# impl Page {
fn handle_key(&mut self, ev: KeyEvent) -> Outcome {
    if self.modal.is_open() {
        return self.modal.handle_key(ev);
    }
    self.field.handle_key(ev)
}
# }
# fn main() {}
```

The modal consumes events first; the screen only sees them when the modal is closed.

## A card the caller fills

`Modal::card(title)` gives you a card with a title and a body rect you fill. The body rect is published on `ModalState::body` after `render`:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# struct Page { modal: ModalState, picker: ListViewState }
# impl Page {
fn draw(&mut self, area: Rect, buf: &mut Buffer, now: Instant) {
    // … your page content draws first, so the card lands on top of it

    Modal::card("Pick an item")
        .width(50)
        .height(12)
        .now(now)
        .render(area, buf, &mut self.modal);

    // then fill the rect the card published
    if self.modal.is_open() {
        ListView::new(vec![
            ListEntry::new("Option A"),
            ListEntry::new("Option B"),
            ListEntry::new("Option C"),
        ])
        .title("options")
        .focused(true)
        .render(self.modal.body, buf, &mut self.picker);
    }
}
# }
# fn main() {}
```

The modal renders its backdrop, card, and title, then sets `state.body` to the rect you can draw into. Render your content into that rect — a list, a form, a data table, whatever the modal holds.

## Overlays over the modal

A dropdown inside a card must paint after the card, or the card covers it. `SelectState` and
`ListViewState` expose `render_overlay` for exactly that: draw the field into the card body, then
drain the overlay at the end of the frame.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# struct ModalForm { modal: ModalState, select: SelectState }
# impl ModalForm {
fn draw(&mut self, area: Rect, buf: &mut Buffer, now: Instant) {
    Modal::card("Settings")
        .width(60)
        .height(10)
        .now(now)
        .render(area, buf, &mut self.modal);

    if self.modal.is_open() {
        let th = theme::current();
        Select::new()
            .placeholder("Theme")
            .focused(true)
            .theme(&th)
            .render(self.modal.body, buf, &mut self.select);

        // last, so it lands over the card instead of under it
        self.select.render_overlay(buf, area, &th, 5);
    }
}
# }
# fn main() {}
```

With more than one deferred layer, collect them in a [`layout::Overlay`] and call `overlay.draw(buf)`
once at the end of the frame; the queue keeps the z-order explicit instead of implicit in call order.

## Opening and closing

```rust
# extern crate tuile;
# use tuile::prelude::*;
# use std::time::Instant;
# struct State { modal: ModalState }
# impl State {
fn open_modal(&mut self, now: Instant) {
    self.modal.open(now);
}

fn handle_key(&mut self, ev: KeyEvent) -> Outcome {
    if self.modal.is_open() {
        let outcome = self.modal.handle_key(ev);
        // Esc closes the modal if configured (default: true)
        if !self.modal.is_open() {
            // Modal closed, react to it
        }
        return outcome;
    }
    // Route to screen widgets
    Outcome::Ignored
}
# }
# fn main() {}
```

`Modal::card` closes on Esc by default. To keep it open, use `.close_on_escape(false)`. To close it programmatically, call `state.close()`.

## Example: ListView picker in a modal

```no_run
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

struct PickerModal {
    modal: ModalState,
    list: ListViewState,
    items: Vec<ListEntry>,
}

impl PickerModal {
    fn new() -> Self {
        Self {
            modal: ModalState::new(),
            list: ListViewState::new(),
            items: vec![
                ListEntry::new("Alice"),
                ListEntry::new("Bob"),
                ListEntry::new("Charlie"),
            ],
        }
    }
    
    fn draw(&mut self, area: Rect, buf: &mut Buffer, now: Instant) {
        // the busy screen underneath
        let th = theme::current();
        fill(buf, area, th.background);
        put(
            buf,
            2,
            1,
            "the screen below the modal",
            40,
            st(th.text, th.background),
        );

        Modal::card("Pick a name")
            .width(40)
            .height(10)
            .now(now)
            .render(area, buf, &mut self.modal);

        if self.modal.is_open() {
            ListView::new(self.items.clone())
                .title("names")
                .focused(true)
                .render(self.modal.body, buf, &mut self.list);
        }
    }

    fn handle_key(&mut self, ev: KeyEvent) -> Outcome {
        if !self.modal.is_open() {
            return Outcome::Ignored;
        }
        // the body owns navigation and picking; Esc still reaches the modal
        let picked = self.list.handle_key(ev);
        if picked.is_submitted() && self.list.take_activated().is_some() {
            self.modal.close();
            return Outcome::Submitted;
        }
        picked | self.modal.handle_key(ev)
    }
}
# fn main() {}
```

The modal owns the keyboard, the list gets events first (for navigation), then the modal (for Esc). When the user picks an item or presses Esc, the modal closes and the screen below regains control.
