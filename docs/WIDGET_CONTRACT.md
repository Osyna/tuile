# tuiforge widget contract

Every widget in `tuiforge/src/widgets/` follows these rules so that apps can compose them
without reading their source. Read `tuiforge/src/{core,theme,anim,draw,layout}.rs` and
`tuiforge/src/widgets/scrollbar.rs` (the reference implementation) before writing one.

## Shape

```rust
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
   move). `Changed` = the value the widget owns changed. Apps react on `is_changed()`.
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
7. **Never panic on size.** Any `area` (including 0×0 and 1×1) must render without panicking.
   Degrade: hide parts, truncate text, return early.
8. **Keyboard.** Follow Textual bindings: `Enter`/`Space` activate, arrows move, `Home`/`End`,
   `PageUp`/`PageDown`, `Esc` closes overlays, `Tab` is *not* handled by widgets (the app's
   `Focus` ring does it) except inside multi-field widgets that document it.
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
    candidate with `unicode_width::UnicodeWidthStr::width` **and** a `tools/shot.py` screenshot
    before using it.

## Showcase page contract (`showcase/src/pages/<name>.rs`)

```rust
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
  `python tools/shot.py -s 60x16 --text -- ./target/debug/showcase --page <title>`.
