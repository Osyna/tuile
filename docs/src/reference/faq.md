# FAQ

## Does tuile replace ratatui?

No. tuile is a widget library built on top of ratatui, not a replacement for it. It builds on `ratatui-core`, the crate upstream recommends for widget libraries, and re-exports it alongside `crossterm` so your `Cargo.toml` needs only `tuile`. Every widget renders into ratatui's `Buffer`, the same type the `ratatui` facade gives you. Your own `Rect` math, raw text placement, and custom drawing code keep working exactly as before.

## Do I need the `App` runtime?

No. The `run(&mut app)` runtime is a convenience: it sets up the terminal with mouse capture and panic-safe restore, then redraws at 60 fps only while `app.animating()` returns true. If you already have an event loop or want fine control over redrawing, skip the runtime and call `.render(area, buf, &mut state)` yourself from whatever loop you have. Every widget works in either mode.

## How do I test animated widgets without waiting?

Pass a fixed `Instant` offset with `.elapsed(secs)` instead of `.now(Instant::now())`. The animation helpers work from an epoch you control, so tests can freeze time or skip ahead without sleeping. For example, `.elapsed(0.5)` renders the widget at exactly 500 ms into its tween, letting you snapshot the halfway frame instantly.

## Why doesn't my widget react to mouse clicks?

Widget states cache their hit rectangles during `render`, and `handle_mouse` uses those rects to resolve whether a click lands inside. If the state has never rendered, or if the widget moved since the last render and you reused the old area, the cached rect is stale or zero and every click will miss. Always render before you handle mouse events.

## How do I use a different theme for one widget?

Pass `.theme(&custom_theme)` on the widget builder. Every widget falls back to `theme::current()`, but an explicit `.theme()` overrides it for just that one render. Build a `Theme` from a `ThemeSpec` with `Theme::resolve(&spec, None)`, or tweak one role on an existing theme by cloning and editing the struct directly.

## What is the minimum terminal size?

The showcase app enforces 60×16 and shows a message below that. Individual widgets do not panic at any size; they clip or collapse gracefully, but usability suffers under about 40 columns or 10 rows. If your app has a practical minimum, check `frame.area()` in `draw` and render a prompt when too small.

## Does tuile support Windows?

tuile itself is platform-agnostic. It depends on `crossterm`, which supports Windows, macOS, and Linux. The runtime uses `crossterm::terminal` for setup and teardown, and the event loop reads `crossterm::event::poll`. The local clock helpers (`local_hms`, `local_ymd`) shell out to `date +%z` on Unix to read the timezone offset; on Windows that call will fail and the offset defaults to zero (UTC). Widgets and rendering work everywhere crossterm works.

## How do I add my own widget?

The drawing and layout primitives are public and match the ones every built-in widget uses: `draw::{Border, put, fill, hbar, wrap, truncate, st}` and `layout::{stack, columns, center, popup_below}`. Read `docs/src/reference/widget-contract.md` for the checklist (builder plus state, `Interactive`, cached hit rects, no panics at any size) and look at `tuile/src/widgets/scrollbar.rs` as the reference implementation. The `custom` example shows a complete app with two custom widgets.

## Why are my solid-color buttons or fills showing seams between cells?

The library paints backgrounds with `cell.bg` and a space character, not foreground block glyphs like `█`. Real terminals with minimum-contrast settings brighten foreground glyphs when `fg` is close to `bg`, turning solid runs of `█` into visible bars, and fonts leave hairline seams between adjacent block characters. Background fills with `" "` render as true solid rectangles. If you are porting code that uses `▐` `▌` `█` for solids, replace them with background color on a space.

## What is the MSRV?

Rust 1.88. The library uses let-chains and inline-const patterns, which stabilized in 1.88. The `Cargo.toml` pins `rust-version = "1.88"` to enforce this at build time.

## Is tuile affiliated with Textualize?

No. The design language is deliberate (Textual's color system, widget appearance, and keyboard conventions), but the code is independent. tuile ports the parts of Textual that make building a TUI pleasant into plain Rust and ratatui, with no Python runtime, no CSS engine, and no messaging layer.

## How stable is the API before 1.0?

The API is pre-1.0, so breaking changes may land between minor versions. The builder and state pattern is stable, and the core helpers (`Outcome`, `Interactive`, `Focus`, `HitBox`) are unlikely to change shape. Widget options and field names may be renamed or reorganized as patterns settle. Pin a git rev in `Cargo.toml` if you need stability, or track the main branch for fixes and new widgets.

## Can I use async or tokio with tuile?

The `run` runtime is a synchronous loop over `crossterm::event::poll`. It does not use `async` or require an executor. If your app already runs on tokio, you can skip the runtime and drive the widgets yourself: spawn `crossterm::event::EventStream` on a task, send events to your app's state, call `.render()` and flush the terminal from the main task. The widgets themselves are plain structs with no async dependencies.

## Why is my text input showing the wrong characters?

`Input` and `TextArea` work in grapheme clusters (via `unicode-segmentation`), not bytes or chars. A single cursor position may correspond to multiple `char`s (for example an emoji with a skin-tone modifier). If you are indexing into `state.value` as bytes or chars, you will see misaligned insertions or mojibake. Use `state.value.graphemes(true).collect::<Vec<_>>()` or the provided cursor methods, which already handle grapheme boundaries correctly.
