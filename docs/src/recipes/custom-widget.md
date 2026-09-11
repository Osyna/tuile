# Writing your own widget

A widget that follows the same rules as the built-in ones looks native, works with the theme
and the focus ring, and can be dropped into a page next to library widgets without special
cases. The rules are in [the widget contract](../reference/widget-contract.md);
this recipe walks one widget from nothing to done.

We build a `Pager`: a row of dots showing which page of N you are on, clickable, with arrow
key support.

## 1. The two types

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

pub struct Pager {
    pages: usize,
    focused: bool,
    enabled: bool,
    theme: Option<Theme>,
}

#[derive(Clone, Debug, Default)]
pub struct PagerState {
    pub current: usize,
    pub hits: Vec<Rect>,
}
# fn main() {}
```

The builder holds configuration and an optional theme. The state holds the value and the
rectangles the last render produced. `PagerState` derives `Clone + Debug + Default` so a
caller can store it, snapshot it in a test, and construct it with `Default::default()`.

## 2. The builder

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# pub struct Pager { pages: usize, focused: bool, enabled: bool, theme: Option<Theme> }
impl Pager {
    pub fn new(pages: usize) -> Self {
        Pager { pages, focused: false, enabled: true, theme: None }
    }
    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }
    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }
    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}
# fn main() {}
```

Take the theme by reference and copy it. `Theme` is `Copy`, and every built-in widget uses
this exact signature, so a caller can swap your widget for a library one without changing the
call.

## 3. Render

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# pub struct Pager { pages: usize, focused: bool, enabled: bool, theme: Option<Theme> }
# #[derive(Clone, Debug, Default)] pub struct PagerState { pub current: usize, pub hits: Vec<Rect> }
impl Pager {
    pub fn render(self, area: Rect, buf: &mut Buffer, state: &mut PagerState) {
        let th = self.theme.unwrap_or_else(theme::current);
        state.hits.clear();
        if area.height == 0 || self.pages == 0 {
            return;                       // too small to draw: draw nothing, never panic
        }

        let fg = if !self.enabled {
            th.text_disabled
        } else if self.focused {
            th.text
        } else {
            th.text_muted
        };

        let y = area.y;
        for i in 0..self.pages {
            let x = area.x + i as u16 * 2;
            if x >= area.right() {
                break;                    // only what fits gets drawn, and gets a hit rect
            }
            let glyph = if i == state.current { "●" } else { "○" };
            let style = if i == state.current { st(th.primary, th.background) } else { st(fg, th.background) };
            put(buf, x, y, glyph, 1, style);
            state.hits.push(Rect::new(x, y, 1, 1));
        }
    }
}
# fn main() {}
```

Four rules are visible here.

The theme falls back to `theme::current()` when the caller did not pass one, so the widget
works with no configuration.

Hit rectangles are cleared and rebuilt every render, and only for what was actually drawn. A
dot scrolled off the edge has no rectangle, so it cannot be clicked.

The small-area check returns instead of panicking. Every widget in the library is fuzzed from
60x16 upward, and "draws nothing" is a valid answer at any size.

`put` clips to the buffer. Writing through `buf[(x, y)]` does not, and a one-cell overrun
panics.

## 4. Handle events

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# #[derive(Clone, Debug, Default)] pub struct PagerState { pub current: usize, pub hits: Vec<Rect> }
impl Interactive for PagerState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Left if self.current > 0 => {
                self.current -= 1;
                Outcome::Changed
            }
            KeyCode::Right if self.current + 1 < self.hits.len() => {
                self.current += 1;
                Outcome::Changed
            }
            KeyCode::Home if self.current != 0 => {
                self.current = 0;
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        if !is_left_down(&m) {
            return Outcome::Ignored;
        }
        for (i, r) in self.hits.iter().enumerate() {
            if mouse_in(*r, &m) {
                if self.current == i {
                    return Outcome::Consumed;      // clicked the current page: nothing changed
                }
                self.current = i;
                return Outcome::Changed;
            }
        }
        Outcome::Ignored
    }
}
# fn main() {}
```

Implementing `Interactive` gives you `handle(&Event)` for free, so callers can forward raw
events with one call.

Returning the right `Outcome` matters more than it looks. `Ignored` for a key you did not
use lets the app offer it elsewhere, which is how `Tab` reaches the focus ring instead of
being eaten. `Consumed` for a click that changed nothing still tells the app to redraw
(the hover moved). `Changed` is reserved for a real value change.

## 5. Test the behaviour, not the wiring

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# pub struct Pager { pages: usize, focused: bool, enabled: bool, theme: Option<Theme> }
# impl Pager {
#     pub fn new(pages: usize) -> Self { Pager { pages, focused: false, enabled: true, theme: None } }
#     pub fn render(self, area: Rect, buf: &mut Buffer, state: &mut PagerState) {
#         state.hits.clear();
#         for i in 0..self.pages {
#             let x = area.x + i as u16 * 2;
#             if x >= area.right() { break; }
#             put(buf, x, area.y, if i == state.current { "●" } else { "○" }, 1, Style::default());
#             state.hits.push(Rect::new(x, area.y, 1, 1));
#         }
#     }
# }
# #[derive(Clone, Debug, Default)] pub struct PagerState { pub current: usize, pub hits: Vec<Rect> }
# fn main() {
// A click on the third dot selects page 2.
let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
let mut state = PagerState::default();
Pager::new(5).render(buf.area, &mut buf, &mut state);

let third = state.hits[2];
assert_eq!(third.x, 4);

// Rendering into a one-cell area draws what fits and nothing more.
let mut tiny = Buffer::empty(Rect::new(0, 0, 1, 1));
let mut st2 = PagerState::default();
Pager::new(5).render(tiny.area, &mut tiny, &mut st2);
assert_eq!(st2.hits.len(), 1);
# }
```

Assert what a consumer observes: which cell was painted, how many hit rectangles exist, what
the state says after an event. A test that asserts a field was copied from the builder tests
nothing.

The no-panic sweep is worth writing once per widget, because "never panics at any size" is a
real contract here:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# pub struct Pager;
# impl Pager { pub fn new(_: usize) -> Self { Pager } pub fn render(self, _: Rect, _: &mut Buffer, _: &mut u8) {} }
# fn main() {
for w in [0u16, 1, 3, 40, 200] {
    for h in [0u16, 1, 2, 50] {
        let mut buf = Buffer::empty(Rect::new(0, 0, w.max(1), h.max(1)));
        let mut state = 0u8;
        Pager::new(8).render(Rect::new(0, 0, w, h), &mut buf, &mut state);
    }
}
# }
```

## 6. Animation, if you need it

Take the frame instant through the builder and store the tween in the state:

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use std::time::Duration;
# #[derive(Clone, Debug)] pub struct PagerState { pub current: usize, pub slide: Tween }
impl PagerState {
    pub fn go(&mut self, page: usize, now: Instant) {
        self.current = page;
        self.slide.go(page as f32, now, Duration::from_millis(150));
    }
    pub fn animating(&self, now: Instant) -> bool {
        self.slide.active(now)
    }
}
# fn main() {}
```

Expose `animating(now)` so the app can or it into `App::animating`. Never call
`Instant::now()` inside `render`: it makes output non-deterministic and breaks both tests and
screenshots. The [animation](../concepts/animation.md) chapter explains the two clocks.

## The reference implementation

`tuile/src/widgets/scrollbar.rs` is the widget the contract was written from. It is small,
handles keyboard, wheel and drag, hides itself when the content fits, and has the doc comment
and tests the contract asks for. Read it before writing anything non-trivial.
