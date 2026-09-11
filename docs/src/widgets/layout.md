# Layout and chrome

Structural widgets: split panes, scroll views, scrollbars, panels, collapsible sections, and header/footer chrome.

![layout](../screenshots/layout.png)

## SplitPane

`SplitPane` splits an area into two resizable regions with a draggable divider. Horizontal splits stack vertically, vertical splits sit side by side.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;
use tuile::widgets::SplitSize;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = SplitState::new(SplitSize::Ratio(0.3), 10, 10);
let (left, right) = SplitPane::new()
    .direction(Direction::Horizontal)
    .render_split(area, buf, &mut state);
# }
# fn main() {}
```

`SplitState` owns `initial` (the size spec at creation), `pos` (current first-pane size in cells), `min_first`, `min_second`, and the cached layout. `render_split` returns `(first_rect, second_rect)`.

| Method | Effect |
|--------|--------|
| `direction(Direction)` | `Horizontal` (side by side) or `Vertical` (stacked) |
| `divider(SplitDivider)` | `Line` (default), `Thick`, `Dotted`, `Hidden` |

**Keys and mouse:** Drag the divider to resize. Double-click the divider to reset to `initial`. Call `state.resize_by(delta)` from your key handler to resize by `delta` cells. `state.collapse_first()`, `state.collapse_second()`, and `state.restore()` hide and show panes.

**When not to use:** Single fixed split with no interaction: use `ratatui::layout` directly. Three-way split: nest two `SplitPane` calls or use `layout::stack`.

## ScrollView

`ScrollView` renders content into an offscreen buffer larger than the viewport, then blits the visible window. Wheel, arrow keys, PageUp/PageDown, Home/End scroll. Optional smooth tweened scrolling.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;
use tuile::draw::put;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = ScrollViewState::new();
ScrollView::new()
    .content_size(200, 100)
    .smooth(true)
    .render_with(area, buf, &mut state, |cbuf, carea| {
        put(cbuf, 0, 0, "Content...", carea.width, Style::new());
    });
# }
# fn main() {}
```

`ScrollViewState` tracks `offset_x`, `offset_y`, `anim_y` (the vertical scroll tween), `content` (width, height), `viewport`, and `vbar`/`hbar` (scrollbar states). Call `state.scroll_to(x, y, now, dur)` or `state.scroll_by(dx, dy, now, dur)` to scroll programmatically. A zero duration snaps; a non-zero duration animates.

| Method | Effect |
|--------|--------|
| `content_size(w, h)` | Offscreen buffer dimensions in cells |
| `smooth(bool)` | Tween vertical scroll (default false) |
| `scrollbars(ScrollBars)` | `Auto` (default), `Always`, `Never` |

**When you need it:** Content larger than the viewport (a 200-line log in a 20-row area, a wide table). When you want smooth animated scrolling. When you need scrollbars.

**When you do not:** The content fits. You are already paginating in your data model (a TUI table with a virtual scroll). Rendering into a temporary buffer costs allocation; if you can clip with `ratatui::layout` instead, do.

## Scrollbar

`Scrollbar` is the reference implementation of the tuile widget contract. It uses eighth-block glyphs for the thumb ends (Textual style), responds to wheel and drag, and updates `state.offset`.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = ScrollbarState::new();
let (total_rows, visible_rows) = (200, 10);
Scrollbar::vertical(total_rows, visible_rows)
    .offset(40)
    .render(Rect::new(39, 0, 1, 10), buf, &mut state);
# }
# fn main() {}
```

`ScrollbarState` tracks `offset`, `content`, `viewport`, `axis`, and `drag` state. `state.handle_mouse(m)` moves `offset` on wheel or drag and returns `Outcome::Changed` when it moved.

| Method | Effect |
|--------|--------|
| `vertical(content, viewport)` | Vertical scrollbar (axis defaults `Vertical`) |
| `horizontal(content, viewport)` | Horizontal scrollbar |
| `offset(usize)` | Current scroll position |
| `hide_when_fits(bool)` | Default `true`: invisible when `content <= viewport` |

**The source shape:** `Scrollbar` is simple enough to show in full. The widget is a builder holding `content`, `viewport`, `offset`, `axis`, `hide_when_fits`, and a theme. `impl StatefulWidget for Scrollbar` paints the track and thumb. `ScrollbarState` implements `Interactive` and turns mouse events into offset changes. This pattern repeats across tuile: builder, render, state, interactive.

## Panel

`Panel` draws a bordered container with optional title, subtitle, padding, shadow, and background fill. Returns the inner content rect.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;
use tuile::draw::Border;

# fn demo(area: Rect, buf: &mut Buffer) {
let inner = Panel::new()
    .title("Settings")
    .border(Border::Round)
    .shadow(true)
    .render(area, buf);
# }
# fn main() {}
```

`Panel` has no state. It is a `Widget`-like builder that returns a `Rect` instead of implementing `Widget`.

| Method | Effect |
|--------|--------|
| `title(&str)` | Top-left title |
| `title_align(Alignment)` | Where the title sits |
| `title_right(&str)` | A second title on the right of the top edge |
| `subtitle(&str)` | Bottom-left subtitle |
| `footer(&[(&str, &str)])` | Key and description pairs along the bottom edge |
| `border(Border)` | Any `Border` style: `Round`, `Thick`, `Double`, `Heavy`, `Tall`, `Panel`, `Wide`, `Inner`, `Outer`, `Dashed`, `Solid`, `Ascii`, `Hkey`, `Vkey`, `Blank`, `None` |
| `border_color(Rgb)` | Override the border colour |
| `background(PanelBg)` | `Transparent`, `Surface`, `Panel`, `Boost` |
| `padding(u16)` | Inner padding |
| `shadow(bool)` | Drop shadow |
| `badge(&str)` | A badge in the title row |
| `variant(Variant)` | Title colour |
| `focused(bool)` | Use the focused border colour |

`Panel::card()` presets a surface background, a round border and a shadow.
`Panel::section()` presets no border, for a title row with a rule instead of a box.

## Placeholder

`Placeholder` is Textual's placeholder block: cycles colours, shows dimensions and a label. Use it to sketch layouts.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
Placeholder::new()
    .index(0)
    .render(area, buf, "Sidebar");
# }
# fn main() {}
```

`index` picks a colour from a 22-colour palette. `variant(PlaceholderVariant)` overrides the display: `Default` (show name), `Size` (show WxH), `Text` (show custom text). The name is passed to `render()`, not a builder method.

## Collapsible and Accordion

`Collapsible` is a header row that expands to reveal content. Click the header or call `state.toggle()` to open and close. The height animates.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;
use tuile::draw::put;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = CollapsibleState::new(false);
let used = Collapsible::new()
    .title("Details")
    .render_with(area, buf, &mut state, 10, |inner, buf| {
        put(buf, inner.x, inner.y, "Content", inner.width, Style::new());
    });
# }
# fn main() {}
```

`CollapsibleState::new(open)` sets the initial state. `state.open` is public. `state.animating(now)` is true while the height tween is active.

| Method | Effect |
|--------|--------|
| `title(&str)` | Header text |
| `subtitle(&str)` | Secondary header text |
| `header_style(CollapsibleHeader)` | `Plain`, `Panel` (bordered), `Underline` |
| `markers(closed, open)` | The two glyphs shown before the title |
| `border(Border)` | Frame around the whole widget |
| `duration(Duration)` | Expand and collapse time |
| `focused(bool)` | Focus styling |

`render_with` takes `content_height` (the fully expanded height) and a closure that draws into the content area.

**Accordion** stacks multiple collapsibles. Set `exclusive(true)` to enforce single-open mode.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;
use tuile::draw::put;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = AccordionState::new(3, true);
Accordion::new()
    .titles(&["General", "Privacy", "Advanced"])
    .render_with(area, buf, &mut state, &[5, 5, 5], |idx, inner, buf| {
        put(buf, inner.x, inner.y, &format!("Section {}", idx), inner.width, Style::new());
    });
# }
# fn main() {}
```

`AccordionState::new(count, exclusive)` creates `count` collapsible sections. `state.states` is public (a `Vec<CollapsibleState>`). The render closure receives `(index, area, buf)`. Pass `heights` as a slice of content heights per section.

## AppHeader

Fixed-height application header with icon, title, subtitle, clock, and clickable action labels.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = AppHeaderState::default();
AppHeader::new()
    .title("Settings")
    .subtitle("Preferences")
    .icon("⊛")
    .clock(true)
    .actions(&["Help", "Quit"])
    .render(area, buf, &mut state);
# if let Some(idx) = state.take_action() {
#     // handle action click
# }
# if state.take_icon_click() {
#     // handle icon click
# }
# }
# fn main() {}
```

`AppHeaderState` tracks hover and pressed actions. `state.take_action()` returns `Some(index)` when an action is clicked. `state.take_icon_click()` returns true when the icon is clicked and clears the flag.

| Method | Effect |
|--------|--------|
| `title(&str)` | Main title (left, after icon) |
| `subtitle(&str)` | Subtitle (left, below title if multi-row header) |
| `icon(&str)` | Left icon (clickable) |
| `clock(bool)` | Right-aligned `HH:MM:SS` clock |
| `actions(&[&str])` | Right-aligned clickable labels (left of clock) |

## KeyFooter

Horizontal footer that renders key bindings as `[key] description` pairs. Truncates by priority when narrow.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = KeyFooterState::default();
KeyFooter::new()
    .bindings(&[("q", "Quit"), ("^S", "Save"), ("?", "Help")])
    .render(area, buf, &mut state);
# if let Some(idx) = state.take_pressed() {
#     // handle binding click
# }
# }
# fn main() {}
```

`FooterBinding::new(key, desc)` creates a binding. Call `.priority(n)` to set sort order (lower renders first, higher survives truncation). Call `.disabled()` to grey out a binding.

`state.take_pressed()` returns `Some(index)` when a binding is clicked.

## StatusLine

One-row status strip of separated segments with optional right-aligned text. Each segment has an optional icon, text, and colour.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;
use tuile::widgets::{StatusSegment, StatusSep};
use tuile::theme::Rgb;

# fn demo(area: Rect, buf: &mut Buffer) {
StatusLine::new(&[
    StatusSegment::new("π"),
    StatusSegment::new("Opus 5").icon("◕").color(Rgb(255, 140, 0)),
    StatusSegment::new("/tmp").icon("⌂"),
    StatusSegment::new("4.0%/1M"),
])
.sep(StatusSep::Chevron)
.right("omp")
.render(area, buf);
# }
# fn main() {}
```

| Method | Effect |
|--------|--------|
| `sep(StatusSep)` | `Chevron` (` › `, default), `Dot`, `Pipe`, `Arrow`, `Space` |
| `right(&str)` | Right-aligned text (colour defaults `th.accent`) |
| `right_color(Rgb)` | Override right text colour |
| `bg(Rgb)` | Background strip colour (default `th.background`) |

`StatusLine` is stateless and implements `Widget`.

## Steps

Horizontal or vertical step indicator for process flows. Shows step labels, status (pending, active, complete, error), and connecting lines.

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = StepsState::default();
Steps::new(&["Start", "Build", "Deploy"])
    .active(1)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

| Method | Effect |
|--------|--------|
| `active(usize)` | Current step index (drawn as active) |
| `statuses(&[StepStatus])` | Per-step status: `Pending`, `Active`, `Done`, `Error`, `Skipped` |
| `direction(Direction)` | `Horizontal` (default) or `Vertical` |

`StepsState` is currently a placeholder (the widget has no interaction). `Steps` also provides a `Timeline` widget for vertical time-stamped entries.

## Layout helpers

The `tuile::layout` module provides small helpers on top of `ratatui::layout`.

**Centring:**

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::layout::{center, center_h};
use ratatui::layout::Rect;

# fn demo(area: Rect) {
let popup = center(area, 60, 20);
let banner = center_h(area, 80);
# }
# fn main() {}
```

`center(area, w, h)` returns a `w`×`h` rect centred in `area` (clamped to fit). `center_h(area, w)` centres horizontally only, full height.

**Padding:**

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::layout::{pad, pad_trbl};
use ratatui::layout::Rect;

# fn demo(area: Rect) {
let inner = pad(area, 2, 1);
let inner2 = pad_trbl(area, 1, 2, 1, 2);
# }
# fn main() {}
```

`pad(area, x, y)` shrinks by `x` cells left/right and `y` cells top/bottom. `pad_trbl(area, t, r, b, l)` takes individual sides (CSS order).

**Rows and columns:**

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::layout::{stack, columns};
use ratatui::layout::Rect;

# fn demo(area: Rect) {
let rows = stack(area, &[3, 1, 5], 1);
let cols = columns(area, 3, 2);
# }
# fn main() {}
```

`stack(area, heights, gap)` splits into fixed-height rows with `gap` between them; the last row takes the remainder. `columns(area, n, gap)` splits into `n` equal columns with `gap` between them.

**Popup placement:**

```rust
# extern crate tuile;
# extern crate ratatui;
use tuile::layout::popup_below;
use ratatui::layout::Rect;

# fn demo(anchor: Rect, bounds: Rect) {
let popup = popup_below(anchor, 40, 10, bounds);
# }
# fn main() {}
```

`popup_below(anchor, w, h, bounds)` places a `w`×`h` popup below `anchor`, flipping above when it would overflow `bounds`, and sliding horizontally to stay inside.
