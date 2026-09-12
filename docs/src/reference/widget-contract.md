# tuile widget contract

Every widget in `tuile/src/widgets/` follows these rules so that apps can compose them
without reading their source. Read `tuile/src/{core,theme,anim,draw,layout}.rs` and
`tuile/src/widgets/scrollbar.rs` (the reference implementation) before writing one.

## Shape

```rust,ignore
/// Builder: cheap value, consumed by `render`. Holds only configuration for THIS frame.
pub struct Switch { on: Option<bool>, focused: bool, enabled: bool, now: Option<Instant>, theme: Option<Theme>, /* … */ }

/// State: lives in the app between frames. Owns the value, hover/press tracking, tweens,
/// scroll offsets, cursor positions. Implements `Interactive`.
#[derive(Debug, Default, Clone)]
pub struct SwitchState { pub on: bool, pub hit: HitBox, anim: Tween, /* … */ }

impl Switch {
    pub fn new() -> Self;                       // or `new(required_args)`
    pub fn theme(self, th: &Theme) -> Self;     // ALWAYS present; default `theme::current()`
    pub fn focused(self, v: bool) -> Self;      // focus is owned by the app
    pub fn enabled(self, v: bool) -> Self;      // disabled look + ignore events
    pub fn now(self, now: Instant) -> Self;     // ONLY if the widget animates; never call Instant::now()
    /* configuration setters, one per option, returning Self */
}

impl StatefulWidget for Switch { type State = SwitchState; fn render(self, area, buf, state) }
// Stateless widgets (Label, Rule, Badge, Digits, charts without interaction) implement `Widget` instead.

impl SwitchState {
    pub fn new(/* initial value */) -> Self;
    pub fn animating(&self, now: Instant) -> bool;  // ONLY if the widget animates
    /* value accessors / mutators: `toggle()`, `set(v)`, `value()` … */
}
impl Interactive for SwitchState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome;    // the app calls this only when focused
    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome;  // the app forwards EVERY mouse event
}
```

Rules:

1. **Render sets geometry.** `render` must store its `area` (and any sub-rects used for hit
   testing) in the state via `HitBox::set_area` / plain `Rect` fields. Mouse handling uses only
   those cached rects. Never recompute layout in `handle_mouse`.
2. **Outcome semantics.** `Ignored` = not mine. `Consumed` = redraw (hover/press/scroll/cursor
   move). `Changed` = the value the widget owns changed. `Submitted` = the value was *committed*
   (Enter in a field, a row activated, a dialog answered). An editing widget reports `Changed` per
   keystroke and `Submitted` once, so a screen that persists on commit does not persist on the
   first character typed. `is_changed()` is true for both, `is_submitted()` only for the commit;
   `|` keeps the strongest outcome, so `Submitted` sorts above `Changed`. Every widget states in
   its own docs what `Changed` means for it, and a widget that commits exposes the drain accessor
   (`take_submitted()` / `take_activated()`) next to it.
3. **Hover** comes from `HitBox` in the state; **focus** and **enabled** come from the builder.
   Build `Look { focused, hover: state.hit.hover, enabled }` and style from it
   (`th.focus_bg()`, `th.border` vs `th.border_blurred`, `th.hover_bg`, `th.text_disabled`).
4. **Animation.** Time comes in through `.now(instant)` on the builder and `state.now(instant)`
   (or the `now` argument) for event handlers that start tweens. Store `Tween`s in the state;
   expose `animating(now)`. Respect `.duration(Duration)` / `Duration::ZERO` = instant.
   Looping animations (spinners, shimmers, marquees) take their phase from
   `anim::since(now)` (seconds since the process-wide `anim::EPOCH`) and offer `.elapsed(f32)`
   as the explicit, testable override; never ask the app for a start instant.
5. **Theme.** `let th = self.theme.unwrap_or_else(theme::current);` inside `render` (`Theme` is
   `Copy`; builders store `Option<Theme>` and `.theme(&Theme)` does `Some(*th)`).
   Use semantic roles (`th.surface`, `th.panel`, `th.primary`, `th.text_muted`, `th.cursor_bg`,
   `Variant` → `th.variant(v)` / `th.text_variant(v)`). Never hard-code colours.
   Text fields (`Input`, `TextArea`, `Select`, `Combobox`, `MultiSelect`, `PromptComposer`) frame
   themselves through `draw::FieldShape` (`.shape(..)`, default `Tall(Edge::Full)`); accent bars
   use `draw::Edge` (`Hair`/`Thin`/`Half`/`Full`). Never hard-code a side-bar glyph in a widget.
6. **Draw through `crate::draw`.** `fill`, `put`, `put_centered`, `Border::*.draw/draw_titled`,
   `hbar`, `blend_area`, `truncate`, `wrap`. They clip to the buffer; you never index `buf[(x, y)]`
   without first checking `buf.area.contains(..)` (or use the helpers).
7. **Never panic on size, and never vanish silently.** Any `area` (including 0×0 and 1×1) must
   render without panicking. Degrade first: hide parts, truncate text, drop a subtitle row. When a
   widget genuinely cannot draw, it implements `core::MinSize` and guards with
   `if draw::refuse(buf, area, self.min_size(), th.text_disabled) { return; }`, which paints a dim
   `⋯`. A private `if area.height < 2 { return; }` is forbidden: the state still accepts every key
   while nothing paints, so the symptom a consumer sees is "my keystrokes are not arriving", not
   "my widget is too small". `min_size()` reads `self`, because style and `FieldShape` change the
   answer, and it is what layout should allocate from: `Constraint::Length(w.min_size().1)`. The
   guard keeps whatever geometry reset the old early return did — a stale `HitBox` clicks a widget
   that is no longer on screen. Per-family numbers are tabled below.
8. **Keyboard.** Follow Textual bindings: `Enter`/`Space` activate, arrows move, `Home`/`End`,
   `PageUp`/`PageDown`, `Esc` closes overlays, `Tab` is *not* handled by widgets (the app's
   `Focus` ring does it) except inside multi-field widgets that document it.
   **A widget that can coexist with a focused text field must let the caller narrow what it
   claims.** A persistent nav bar wired ahead of the focused screen — the obvious wiring — used to
   swallow Enter, Space, every digit and caret movement app-wide, and because letters still arrived
   it read as a submit bug rather than a routing bug. Chrome widgets therefore take a key-group
   selector (`TabBar`/`TabBarState::keys(TabKeys::ARROWS)`, default `TabKeys::ALL` = today's
   behaviour) and return `Ignored` for anything outside it. List every key group you claim in the
   widget's docs.
9. **Names.** `PascalCase` builder, `<Builder>State` state, enums for options
   (`TabStyle::Underline`). Do not reuse ratatui widget names (`Tabs`, `Table`, `List`,
   `Sparkline`, `Gauge`, `Chart`, `BarChart`, `Scrollbar`, `Paragraph`, `Block`, `ListItem`,
   `Row`, `Cell`). Use `TabBar`, `DataTable`, `ListView`, `SparkChart`, `Meter`, `LineGraph`,
   `BarGraph`, `TableColumn`, `TableRow`, `TreeNode`, `MenuItem`, `ListEntry`…
10. **Docs.** Every `pub` item gets a `///` doc line. The module starts with `//!` explaining what
    it renders and a 3 to 6 line usage example.
11. **Tests.** One `#[cfg(test)] mod tests` per module with behaviour tests that render into a
    `Buffer::empty(Rect)` and/or drive `handle_key`/`handle_mouse` and assert on state/cells.
    No snapshot dumps. Test the tricky logic (cursor math, scroll clamping, sort, wrap).
12. **Overlays.** Widgets that pop something over other content (Select dropdown, Menu, Tooltip)
    render their base in `render` and expose `render_overlay(&self/state, buf, bounds)` (or take a
    `&mut Overlay<'a>`), positioned with `layout::popup_below`.
13. **No new dependencies.** `ratatui`, `unicode-width`, `unicode-segmentation` only.
14. **Performance.** No per-frame heap churn beyond small `String`s; no `Instant::now()` inside
    render; precompute glyph tables as `const`.
15. **Solid = background paint, lines = box drawing or edge eighths.** Paint filled areas with
    `" "` + `bg` (`fill`, `hbar`, `█` entries in `Border::glyphs`), never with `█`/`▐`/`▌`
    foreground glyphs: fonts leave seams between block glyphs, some fonts overshoot the cell
    vertically, and terminals with a minimum-contrast setting recolour any glyph whose fg is
    close to its bg (this turned every pill end and `tall` border into a stray bar). Accent
    markers and side rails use box drawing (`┃`, `│`), which tiles pixel-exact everywhere; the
    Textual-style `Tall`/`Panel`/`Wide` borders pair painted bars with a thin `▔`/`▁`/`▏`/`▕`
    line so the corners always meet. Half-blocks only for partial cells in animated bars.
    Wide/ambiguous glyphs (`★`, emoji) get two cells.
16. **Icons are width-1, non-emoji, and present in common monospace fonts.** Terminals size
    glyphs with their own tables: anything with the Unicode *Emoji* property (`▶ ◀ ▪ ▫ ⚠ ♥ ⚡ ☰
    ⚙ ✦ ℹ …`) is two cells for utf8proc-based terminals (tmux, foot, kitty) while `unicode-width`
    says one, so every cell after it on that row shifts and the whole row skews. Use glyphs that
    are 1 cell everywhere and exist in JetBrains Mono / DejaVu:
    `◈ ◉ ◔ ◌ ◆ ◇ ● ○ ■ □ ◧ ◫ ◊ ⊞ ⊡ ⊙ ⊛ ⊕ ⊗ ⌘ ⌂ ≡ ▸ ▹ ◂ ◃ ▴ ▾ ▲ ▼ • ◦ ✓ ✗ ✶ ¶ ⋮ ⋯ ❯`. Check a
    candidate with `unicode_width::UnicodeWidthStr::width` **and** a `cargo xtask shot` screenshot
    before using it.
17. **Syntax highlighting.** `Highlighter` (from `core`) returns styled ranges as `(start, end, Style)` where `start..end` are **grapheme-cluster offsets** (as walked by `line.graphemes(true)`), not byte or char indices. Use `draw::put_highlighted` to apply them. Ranges must be ascending and non-overlapping; the helper ignores malformed ranges rather than panicking. Text outside all ranges keeps the base style.
18. **Which side owns a setting.** The **state** owns what must survive frames: scroll offsets,
    cursor positions, hit rects, tween clocks, queues, and anything an event handler reads before
    the next render (a key-claim selector; a toast queue's corner, because the slide animation
    interpolates against it). The **builder** owns per-frame look. A setting that has to live on
    the state is still mirrored on the builder, writing through in `render`, so the first guess
    compiles: `ToastStack::new().corner(..)` and `Toaster::corner` both work. Exceptions — settings
    that are state-only with no builder mirror — are listed under "State-owned settings" below.

## Minimum sizes

`min_size()` on the builder is authoritative — it reads the configuration, so a style or a
`FieldShape` changes the answer. Allocate from it (`Constraint::Length(w.min_size().1)`) instead of
guessing. Below it, `render` paints a dim `⋯` and returns; it never draws nothing.

`chrome` below is `FieldShape::vertical_chrome()`: 2 for the default `Tall(Edge::Full)`, `Rule`,
`Round` and `Band`; 0 for `Bar`, `Bars`, `Prompt` and `None`. `.compact(true)` on `Input`,
`TextArea` and `Select` selects the zero-chrome shape, which is how a field fits in one row.

| Family | Widget | Minimum `(w, h)` |
| --- | --- | --- |
| Text entry | `Input` | `(7, 1 + chrome)` |
| | `TextArea` | `(4, 1 + chrome)` |
| | `Select`, `Combobox`, `MultiSelect` | `(8, 1 + chrome)`, compact `(4, 1)` |
| | `Slider`, `RangeSlider` | `(10, 1 + chrome)` |
| | `Stepper` | `(15, 1)` for a one-digit value; a wider value refuses visibly |
| | `Rating` | `(2 × max, 1)` |
| | `PromptComposer` | `(8, 2 + chrome)` |
| Controls | `Button`, `Checkbox`, `RadioGroup`, `Segmented` | `(3, 1)` |
| | `Switch` | style-dependent |
| | `CheckList` | `(5, 3)` |
| | `OptionList` | `(8, 1)` |
| Navigation | `TabBar` | `(8, 2)` `Underline`, `(8, 3)` `Boxed`, else `(8, 1)` |
| | `TabbedContent` | the bar's minimum + 1 content row, + 2 each way when bordered |
| | `MenuBar`, `Breadcrumbs`, `Paginator`, `KeyFooter`, `Collapsible` | `(8, 1)` |
| | `AppHeader` | `(8, 1)`, tall `(8, 3)` |
| | `Steps` | `(8, 1)` horizontal, one row per label vertical |
| | `Timeline` | `(10, 1)` |
| | `Accordion` | `(8, titles)` |
| | `ScrollView` | `(1, 1)` |
| | `SplitPane` | `(3, 1)` |
| | `Panel` | `1 + chrome` both ways |
| Data | `DataTable` | content `(2, header + 1)` plus `2 × padding` both ways: `(4, 4)` by default |
| | `TreeView` | `(3, 3)` bordered (the default), `(1, 1)` without |
| | `KeyValueList` | `(2, 1)` |
| | `Catalog` | columns + header rows |
| | `Digits` | 3 cells per digit × `(_, 3)` |
| | `BigMenu`, `BigText` | one character in the configured font, plus that style's chrome |
| | `LineGraph` | `(10, 5)` |
| | `ActivityGraph` | `(10, 8)` |
| | `Meter` | `(4, 1)` |
| | `RadialGauge` | `(12, 6)` |
| | `LogView`, `Markdown` | `(5, 2)` |
| | `ColorPicker` | `(20, 10)` |
| | `GradientBar` | `(5, 1)` |
| | `ThemePalette` | `(15, 2)` |
| Content | `Badge`, `KeyValueList` | `(2, 1)` |
| | `Pill`, `KeyCap`, `Loader` | `(3, 1)` |
| | `StatusLine` | `(4, 1)` |
| | `StatCard` | `(6, 2)` |
| | `Skeleton` | `(3, 1)` for `Chart` (it degrades to one bar), else `(3, 2)` |
| AI | `StreamText` | `(1, 1)` |
| | `TokenHeat` | `(2, 1)` |
| | `Thinking`, `DiffView`, `ModeBadge` | `(4, 1)` |
| | `ContextGauge` | `(4, 1)` compact, `(4, 2)` full |
| | `ToolCall` | `(4, 3)` |
| | `ChatView` | `(8, 2)` |
| | `CostMeter`, `EditPreview` | `(8, 2)` |
| | `HarnessStatus` | `(10, 1)` |
| | `RateGraph` | `(10, 2)` |
| | `RetryNotice` | `(10, 1)` compact, `(10, 3)` card |
| | `AgentLanes` | name column + a 10-cell track, 2 rows |
| | `Approval` | `(12, 1)` inline, `(12, 2)` banner, `(12, 5)` card |
| | `TurnStats` | `(12, 2)` |
| | `CompactionBanner` | `(20, 2)` |

Degrading beats refusing where a widget can still say something true: `Skeleton`'s chart shape
draws a single bar under 2 rows rather than a marker, and reports `1` as its minimum height.

## State-owned settings

Settings that live on the state because an event handler reads them between renders. Both of
these are mirrored on the builder, so either side works:

| State field | Builder mirror | Why the state owns it |
| --- | --- | --- |
| `Toaster::corner` | `ToastStack::corner` | the queue slides toasts toward it between frames |
| `TabBarState::keys` | `TabBar::keys` | `handle_key` consults it before the next render |

## Showcase page contract (`showcase/src/pages/<name>.rs`)

```rust,ignore
#[derive(Default)] pub struct XxxPage { /* widget states, Focus<Id> */ }
impl Page for XxxPage {
    fn title(&self) -> &'static str;              // fixed, matches pages/mod.rs registry
    fn subtitle(&self) -> &'static str;           // one line
    fn icon(&self) -> &'static str;               // single narrow glyph (avoid emoji/wide)
    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx);
    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome;
    fn animating(&self, now: Instant) -> bool;
    fn bindings(&self) -> &'static [(&'static str, &'static str)];
}
```

* The page gets the content rect; paint your own panels/cards inside it (`ctx.theme.background`
  is already filled). Use `Border::Round`/`Panel` cards with titles per widget family; group by
  rows with `layout::stack`/`columns`.
* Keyboard: keep a `Focus<Id>` ring; route keys to the focused widget; return `Outcome::Ignored`
  for keys you do not handle so the shell can use `[`, `]`, `q`. Mouse: forward to *every* widget
  state (each one checks its own rect); on `Hit::Press`/`Changed` from a widget, move focus to it.
* Show every option of each widget: variants, disabled, focused, sizes, styles. Make it
  interactive: clicking/typing must visibly change something. Use `ctx.notify(msg, Variant)` for
  actions (button pressed, row selected).
* Use `ctx.now` for `.now(..)`, `ctx.dur(ms)` for tween durations (respects reduce-motion), and
  `ctx.elapsed()` for looping animations.
* Must render without panic from 60×16 up to 250×70. Verify with
  `cargo xtask shot -s 60x16 --text -- ./target/debug/showcase --page <title>`.
