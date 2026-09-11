# Overlays

A dropdown list, a context menu, a tooltip and a modal all have the same problem: they belong
to a widget somewhere in the middle of your layout, but they must paint over everything drawn
after it. A terminal buffer has no z-index. The only ordering is the order you write cells.

tuile solves this two ways, and which one you need depends on who owns the overlay.

## The simple case: draw it last

If your app owns the overlay, render it after everything else. Toasts, modals and the command
palette work this way:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# struct Page { table: DataTableState, toasts: Toaster, modal: ModalState }
# impl Page {
fn draw(&mut self, area: Rect, buf: &mut Buffer, now: Instant) {
    let cols = vec![TableColumn::new("Host"), TableColumn::new("Status")];
    let rows = vec![TableRow::from(vec!["web-01", "up"])];
    DataTable::new(cols, rows).render(area, buf, &mut self.table);

    // after the page content, so they paint on top
    Modal::confirm("Delete host?", "This cannot be undone.").render(area, buf, &mut self.modal);
    ToastStack::new().now(now).render(area, buf, &mut self.toasts);
}
# }
# fn main() {}
```

Ordering the draw calls is the whole mechanism. The overlay widgets take the full area and
place themselves inside it.

## The harder case: a widget owns its own popup

A `Select` renders a field wherever your layout put it, and its dropdown must appear next to
that field and over whatever follows. The widget cannot draw the list immediately, because
the rest of your layout has not been drawn yet.

`Overlay` is a queue of deferred draw closures for this:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# fn demo(area: Rect, buf: &mut Buffer) {
let mut overlay = Overlay::new();

overlay.push(|buf: &mut Buffer| {
    // runs after everything else has been drawn
    fill(buf, Rect::new(4, 6, 20, 5), Rgb::hex(0x1e1e2e));
});

// ... draw the rest of the page ...

overlay.draw(buf);      // drain the queue last
# let _ = area;
# }
# fn main() {}
```

`push` takes any `FnOnce(&mut Buffer)`, `is_empty()` asks whether anything is queued, and
`draw(self)` consumes the queue in push order. The closures borrow for the lifetime of the
overlay, so they can capture the widget state and the theme without copying.

## Placing a popup

`popup_below` does the geometry every dropdown needs: prefer below the anchor, flip above
when there is no room, and slide horizontally to stay inside the bounds.

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# fn main() {
let screen = Rect::new(0, 0, 80, 24);
let field = Rect::new(10, 20, 20, 3);

let list = popup_below(field, 20, 8, screen);
assert!(list.bottom() <= screen.bottom());   // flipped above, it did not overflow
# }
```

It never returns a rectangle outside `bounds`, so a dropdown at the bottom of the screen
opens upward and one at the right edge shifts left.

## Events go the other way

Drawing is back to front. Event handling is front to back. The widget on top must see the
click first, or the thing underneath will react to a click it never received visually:

```rust
# extern crate tuile;
# extern crate ratatui;
# use tuile::prelude::*;
# struct Page { modal: ModalState, menu: ContextMenuState, table: DataTableState }
# impl Page {
fn event(&mut self, ev: &Event) -> Outcome {
    // topmost first, and stop as soon as one consumes
    let out = self.modal.handle(ev);
    if out.is_consumed() {
        return out;
    }
    let out = self.menu.handle(ev);
    if out.is_consumed() {
        return out;
    }
    self.table.handle(ev)
}
# }
# fn main() {}
```

This is the most common source of "why did my table scroll while a dialog was open". The
broadcast pattern from the [events](events.md) chapter is correct only for widgets that do
not overlap.

A modal that is closed consumes nothing, so the early return costs one comparison per event
when no dialog is open.

## Escape and click-outside

Users expect `Esc` to close the topmost overlay and a click outside it to dismiss it. Widgets
that own a popup handle both internally: `Select` closes its dropdown on `Esc` and on a click
that lands outside the list. For app-owned overlays you write the rule, and the order above
is what makes it work: the modal sees `Esc` first and consumes it, so the table never treats
it as a filter reset.

When several overlays are open at once, `Esc` should close one layer per press. Handling them
in topmost-first order with an early return gives that for free.

## Cost

An overlay closure is one boxed allocation per popup per frame, drained immediately. A screen
with a dropdown open allocates once more than the same screen with it closed. The queue is
empty in the common case and `is_empty()` is what widgets check before doing any work.
