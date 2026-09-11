# Content

Widgets for displaying text, logs, dates, colors, and rich formatted content.

![content](../screenshots/content.png)

## Text widgets

### Label

Plain text with variant tinting, alignment, and wrapping.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
Label::new("Status OK").variant(Variant::Success).bold(true).render(area, buf);
# }
# fn main() {}
```

| Method | Effect |
|--------|--------|
| `variant(Variant)` | Tint with theme color |
| `muted(bool)`, `disabled(bool)` | Muted or disabled appearance |
| `bold(bool)`, `italic(bool)`, `underline(bool)` | Text modifiers |
| `align(Alignment)` | Left, Center, or Right (default Left) |
| `wrap(bool)` | Wrap to area width (default false) |
| `bg(Rgb)` | Background override |
| `ellipsis(bool)` | Truncate with … (default true) |

`Label::height_for(width)` returns wrapped height.

### Rule

Horizontal or vertical separator with optional title.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
Rule::horizontal().title("Settings").render(area, buf);
# }
# fn main() {}
```

`RuleStyle`: Solid, Heavy, Double, Dashed, Dotted, Ascii, Blank. Methods: `style(RuleStyle)`, `title(&str)`, `title_align(Alignment)`, `color(Rgb)`.

### Badge, Pill, KeyCap

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
Badge::new("v1.2").variant(Variant::Accent).render(area, buf);
# }
# fn main() {}
```

`Badge` has `variant(Variant)`, `style(BadgeStyle)` (Filled, Outline, Soft), `icon(&str)`. `Badge::width(text)` returns rendered width.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
Pill::new("Live").variant(Variant::Success).render(area, buf);
# }
# fn main() {}
```

`Pill` is a rounded badge using block glyphs.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
KeyCap::new("Ctrl").render(area, buf);
# }
# fn main() {}
```

`KeyCap` renders keyboard keys with inverted panel style.

### Link

Underlined clickable link with visited state.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = LinkState::default();
Link::new("Documentation").url("https://example.com").render(area, buf, &mut state);
# }
# fn main() {}
```

`LinkState` fields: `url`, `visited`, `hit`, `clicked`. Call `state.take_clicked()` to consume the event. Mouse and Enter activate.

### StatCard

Dashboard card with large value, label, delta, and sparkline.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;

# fn demo(area: Rect, buf: &mut Buffer) {
StatCard::new("1,245", "Active Users")
    .delta(12.5, true)
    .trend(&[10.0, 12.0, 11.5, 13.0])
    .variant(Variant::Primary)
    .render(area, buf);
# }
# fn main() {}
```

Methods: `delta(f64, bool)`, `trend(&[f64])`, `variant(Variant)`, `icon(&str)`, `bordered(bool)`. `StatCard::min_height()` returns 3.

## Markup

Rich console markup with Rich-style `[tag]` syntax. `Markup::parse(input, theme)` returns ratatui `Text`. Escape `[` as `[[`.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::text::Markup;

# fn demo(buf: &mut Buffer) {
let th = Theme::default();
let text = Markup::parse("[b]Bold[/b] and [success]green[/]", &th);
# }
# fn main() {}
```

### Supported tags

| Tag | Effect |
|-----|--------|
| `[b]`, `[i]`, `[u]`, `[s]` | Bold, italic, underline, strikethrough |
| `[dim]`, `[reverse]` | Dim, reverse video |
| `[primary]`, `[secondary]`, `[accent]` | Theme colors |
| `[success]`, `[warning]`, `[error]`, `[muted]` | Semantic theme colors |
| `[#rrggbb]` | RGB hex foreground (e.g. `[#ff5500]`) |
| `[on #rrggbb]` | RGB hex background |
| `[/]` or `[/tag]` | Close tag |

Tags nest and stack. Unknown tags render as literal text.

## Markdown

CommonMark subset renderer with scrolling.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::markdown::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = MarkdownState::default();
let src = "# Heading\n\nThis is **bold** and *italic*.";
Markdown::new(src).render(area, buf, &mut state);
# }
# fn main() {}
```

`MarkdownState` handles scrolling. Arrow keys and Page Up/Down navigate.

### Supported syntax

**Block elements:**

| Syntax | Rendered as |
|--------|-------------|
| `# Heading` to `#### Heading` | Headings (1-2 underlined) |
| ` ```lang ` fenced blocks | Code with language label |
| `---`, `***`, `___` | Horizontal rule |
| `> Quote` | Blockquote with vertical bar |
| `- item` or `* item` | Unordered list (nested: two-space indent) |
| `1. item` | Ordered list |
| `- [ ]` or `- [x]` | Task list (checked/unchecked) |
| `\| header \|` with `\| --- \|` | Tables |

**Inline elements:**

| Syntax | Effect |
|--------|--------|
| `**bold**` | Bold |
| `*italic*` | Italic |
| ` `code` ` | Inline code with background |
| `~~strike~~` | Strikethrough |
| `[text](url)` | Link (URL not shown, text underlined) |

## LogView

Scrolling log viewer with level badges, filtering, and auto-follow.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::log::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = LogViewState::new();
state.push(LogLevel::Info, "Server started on port 8080");
state.push(LogLevel::Warn, "High memory usage detected");
LogView::new().timestamps(true).render(area, buf, &mut state);
# }
# fn main() {}
```

`LogViewState` fields: `lines`, `follow` (default true), `scroll`, `filter`, `filter_mode` (Highlight or Only), `max_lines`.

Call `state.push(level, text)` to add entries. Levels: `Trace`, `Debug`, `Info`, `Warn`, `Error`, `Success`.

Keys: Up/Down scroll one line, Page Up/Down by viewport, Home/End toggle follow.

| Method | Effect |
|--------|--------|
| `timestamps(bool)` | Show HH:MM:SS column (default false) |
| `level_column(bool)` | Show level badges (default true) |
| `wrap(bool)` | Wrap long lines (default false) |
| `max_lines(usize)` | Ring buffer size |
| `border(Border)`, `title(&str)` | Border and title |

## Calendar and DatePicker

### Calendar

Month grid with navigation, selection, and marks.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::calendar::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = CalendarState::default();
Calendar::new().show_week_numbers(true).render(area, buf, &mut state);
# }
# fn main() {}
```

`CalendarState` fields: `year`, `month`, `cursor_day`, `selected` (Option<(i32, u32, u32)>), `today`. Arrow keys move cursor, Enter selects, mouse clicks select.

| Method | Effect |
|--------|--------|
| `week_start(Weekday)` | Sunday or Monday (default Sunday) |
| `show_adjacent(bool)` | Days from adjacent months (default true) |
| `show_week_numbers(bool)` | ISO week column (default false) |
| `min(y, m, d)`, `max(y, m, d)` | Restrict range |
| `disabled(fn)` | Custom disable predicate |
| `marks(&[(y, m, d, Variant)])` | Highlight dates |

### DatePicker

Date field with popup calendar.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::calendar::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = DatePickerState::default();
DatePicker::new().focused(true).render(area, buf, &mut state);
# }
# fn main() {}
```

`DatePickerState` fields: `cal`, `open`, `hit`. Space toggles popup, Escape closes. Selected date is in `state.cal.selected`. Call `render_overlay(&self, state, buf, bounds)` after page render for the popup.

## Color widgets

### Swatches

Row of selectable color chips with optional labels.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::color::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = SwatchesState::default();
let colors = vec![Rgb(255, 0, 0), Rgb(0, 255, 0), Rgb(0, 0, 255)];
let labels = ["Red", "Green", "Blue"];
Swatches::new(&colors).labels(&labels).render(area, buf, &mut state);
# }
# fn main() {}
```

`SwatchesState` fields: `cursor`, `hit`. Arrow keys navigate, Enter selects. Selected color is `colors[state.cursor]`.

Methods: `labels(&[&str])`, `cell_width(u16)` (default 4), `focused(bool)`, `enabled(bool)`, `bg(Rgb)`.

### ColorPicker

HSL picker with hue strip and saturation/lightness grid.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::color::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = ColorPickerState::default();
ColorPicker::new().render(area, buf, &mut state);
# }
# fn main() {}
```

`ColorPickerState` fields: `hue` (0.0..360.0), `sat`, `light` (0.0..1.0), `mode`, `hit`. Arrow keys adjust. `state.rgb()` returns current `Rgb`.

### GradientBar

Horizontal gradient with labels and marker.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::color::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let stops = vec![Rgb(0, 0, 255), Rgb(255, 0, 0)];
GradientBar::new(&stops).labels("Min", "Max").marker(0.5).render(area, buf);
# }
# fn main() {}
```

Gradient interpolates evenly across stops. Methods: `labels(&str, &str)`, `marker(f32)` (0.0..1.0).

### ThemePalette

Renders every role of a theme as labeled swatches.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::color::*;

# fn demo(area: Rect, buf: &mut Buffer) {
ThemePalette::new().render(area, buf);
# }
# fn main() {}
```

## Steps and Timeline

### Steps

Horizontal or vertical step indicator.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::steps::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = StepsState::default();
Steps::new(&["Start", "Build", "Deploy"]).active(1).render(area, buf, &mut state);
# }
# fn main() {}
```

Methods: `statuses(&[StepStatus])`, `active(usize)`, `numbered(bool)`, `vertical(bool)`, `compact(bool)`.

`StepStatus`: Done, Active, Pending, Error, Skipped.

### Timeline

Vertical timeline with time labels.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::steps::*;

# fn demo(area: Rect, buf: &mut Buffer) {
let entries = vec![
    TimelineEntry {
        time: "10:00".into(),
        title: "Build started".into(),
        description: None,
        variant: Variant::Primary,
    },
];
Timeline::new(entries).render(area, buf);
# }
# fn main() {}
```

Methods: `compact(bool)`, `reverse(bool)`. Each `TimelineEntry` has `time`, `title`, `description` (`Option<String>`), `variant`.
