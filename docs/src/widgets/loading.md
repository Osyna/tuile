# Loading

Indeterminate loaders, skeleton placeholders, and dimming overlays for async work.

![loading](../screenshots/loading.png)

## Loader

An animated progress indicator in one of 20 styles. Every loader takes `.now(Instant)` for real-time animation or `.elapsed(secs)` for deterministic headless rendering. Single-row styles are vertically centered in their area; multi-row scenes fill every row you give them.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
Loader::new(LoaderStyle::Scanner).now(now).render(area, buf);
# }
# fn main() {}
```

### Bars

Animated progress bars that use one row, centered vertically.

| Style | Appearance | label | color | color2 | speed |
|-------|-----------|-------|-------|--------|-------|
| `Scanner` | Bright cell with fading tail sweeping back and forth | no | yes | no | yes |
| `Comet` | Tail traveling one way and wrapping | no | yes | no | yes |
| `Sweep` | Material indeterminate: eased segment sliding across | no | yes | no | yes |
| `FillDrain` | Bar fills from left, then drains from left | no | yes | no | yes |
| `Pulse` | Whole bar breathes | no | yes | no | yes |
| `Stripes` | Barber-pole stripes marching right | no | yes | yes | yes |
| `Rainbow` | Hue-cycling rainbow flowing along the bar | no | no | no | yes |
| `Snake` | Three dots chasing along a dotted track | no | yes | yes | yes |
| `Chase` | LED chase: one lit segment with dim trail | no | yes | no | yes |
| `Blocks` | `▰▰▰▱▱▱` filling and emptying | no | yes | no | yes |
| `Wave` | Traveling sine wave of eighth-blocks | no | yes | yes | yes |
| `Bounce` | Ball bouncing between brackets | no | yes | no | yes |
| `Ping` | Expanding rings `(((●)))` | no | yes | no | yes |
| `Heartbeat` | Scrolling ECG trace with glowing spike | no | yes | yes | yes |

All bar styles ignore `label` (the text is drawn separately to the left of the loader area). `color` defaults to `$primary`, `color2` defaults to `$accent`. When given one row, they render normally. When given more than one row, the animation is vertically centered.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
Loader::new(LoaderStyle::Sweep).color(Rgb(100, 150, 255)).speed(1.5).now(now).render(area, buf);
# }
# fn main() {}
```

### Text

The `label` is the animated content. These styles use one row.

| Style | Appearance | label | color | color2 | speed |
|-------|-----------|-------|-------|--------|-------|
| `Ellipsis` | `label`, `label.`, `label..`, `label...` | yes | yes | no | yes |
| `Shimmer` | Label with sweeping highlight band | yes | yes | yes | yes |
| `Typewriter` | Label typed out with cursor, held, cleared, repeated | yes | yes | no | yes |

For textual styles, the `label` is required (the default is an empty string, which renders nothing). `color` is the text color, `color2` is the highlight color (Shimmer only). When given more than one row, the text is vertically centered.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
Loader::new(LoaderStyle::Typewriter).label("Indexing workspace").now(now).render(area, buf);
# }
# fn main() {}
```

### Scenes

Full-area animations that use every row you give them.

| Style | Appearance | label | color | color2 | speed |
|-------|-----------|-------|-------|--------|-------|
| `Equalizer` | Bouncing spectrum bars | no | yes | yes | yes |
| `Rain` | Matrix rain | no | yes | no | yes |
| `Radar` | Radar sweep over a dot field with blips | no | yes | yes | yes |

All scene styles ignore `label`. `color` is the main color, `color2` is the accent color. These styles need at least 3 rows to be recognizable. When given only one row, they attempt to render but the result is a degraded horizontal slice of the animation (Rain becomes a flickering line, Radar becomes a scanning line, Equalizer becomes a flat bar).

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
Loader::new(LoaderStyle::Radar).color(Rgb(0, 200, 100)).color2(Rgb(255, 200, 0)).now(now).render(area, buf);
# }
# fn main() {}
```

### Builder options

| Method | Effect |
|--------|--------|
| `label(s)` | Text left of the bar; for textual styles this is the animated text |
| `color(rgb)` | Main color (default `$primary`) |
| `color2(rgb)` | Secondary color for two-tone styles (default `$accent`) |
| `speed(f)` | Playback rate; 1.0 is the designed speed |
| `now(Instant)` | Animation phase from app clock |
| `elapsed(f32)` | Explicit elapsed seconds for deterministic rendering |
| `theme(&Theme)` | Override theme |

## Skeleton

Painted placeholder shapes with a diagonal shimmer band. Use them to reserve space while content loads.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
Skeleton::new().shape(SkeletonShape::Card).now(now).render(area, buf);
# }
# fn main() {}
```

### Shapes

| Shape | Layout |
|-------|--------|
| `Text` | One bar per entry in `.lines([widths])` (cells), a blank row between when there's room |
| `Card` | Hero image block on top, then a title and two body lines |
| `Avatar` | Avatar block with a name and handle beside it |
| `List` | Stacked avatar rows: a list waiting for its items |
| `Table` | Header cells and rows of column cells (2-4 columns depending on width) |
| `Chart` | Bars of varying height along a baseline |

The default shape is `Text` with line widths `[60, 45, 50]` cells. For `Text`, pass custom widths with `.lines(&[80, 70, 85])`. Other shapes ignore `.lines()` and lay out based on the area size.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
Skeleton::new().shape(SkeletonShape::List).now(now).render(area, buf);
# }
# fn main() {}
```

The shimmer band sweeps diagonally across the skeleton. All shapes respect `.now(Instant)` or `.elapsed(secs)` for animation phase.

## LoadingOverlay

Dim an area and center a `Loader` with an optional message over it. Draw this last, over the content it covers.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
LoadingOverlay::new(Loader::new(LoaderStyle::Sweep)).message("Fetching rows").now(now).render(area, buf);
# }
# fn main() {}
```

The overlay dims the existing buffer content by blending each cell's foreground and background toward the theme background color. The dimming amount is controlled with `.dim(f)` where 0.0 leaves content untouched and 1.0 hides it completely (default 0.65).

The loader is centered in the area. For single-row loader styles, the `.width(cells)` option controls the loader width (default 24). Multi-row loaders occupy a fixed centered region.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
LoadingOverlay::new(Loader::new(LoaderStyle::Scanner).color(Rgb(100, 200, 255)))
    .message("Syncing")
    .dim(0.75)
    .width(32)
    .now(now)
    .render(area, buf);
# }
# fn main() {}
```

The message appears below the loader in muted text. When the area is too small (less than 5 rows or 20 columns), the overlay renders the dimming only, skipping the loader and message.

| Method | Effect |
|--------|--------|
| `message(s)` | Text below the loader |
| `dim(f)` | Fade strength: 0.0 (untouched) to 1.0 (hidden); default 0.65 |
| `width(u16)` | Loader width for single-row styles; default 24 |
| `now(Instant)` | Animation phase from app clock |
| `elapsed(f32)` | Explicit elapsed seconds |
| `theme(&Theme)` | Override theme |
