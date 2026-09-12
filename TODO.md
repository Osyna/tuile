# Work order: make the widgets fit cases they don't fit yet

Written after porting a real consumer onto tuile 0.2 — the tanuki-harness TUI (seven screens,
a composer, most of the `ai*` families, ~6k lines of Rust). Everything below is a gap that
consumer hit, with the file and line that caused it. No speculation: if an item is here, it cost
a debug cycle or forced a hand-rolled replacement for something the library should own.

Read `docs/src/reference/widget-contract.md` first. Per the README, each change ships with a
showcase page entry and tests that assert what a *consumer observes*, and the gate is
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, `cargo xtask layers`.

The four fixes already on this branch (`HarnessStatus`, `SessionEntry`, `ModelInfo`,
`ModelPicker`) are the template for items 3 and 10: a widget must be able to say *unknown*, and
the words and units belong to the caller. Read that diff before starting.

---

## 1. A widget that is one row too small draws nothing, silently

Every bordered widget has a guard like `if area.height < 2 { return; }` — `ai_agents.rs:1161`,
`1451`, `1591`, `1691`; `digits.rs:197` needs 3; `textarea.rs:723` needs
`1 + FieldShape::vertical_chrome()`, and `FieldShape::default()` is `Tall(Edge::Full)`
(`draw.rs:511`) which costs 2 rows (`draw.rs:519`). `TabBar::render` returns `Rect::ZERO` under
2 rows.

No panic, no log, no return value. The consumer symptom is not "my widget is too small", it is
**"my keystrokes are not arriving"**: the state accepts every key and the widget paints nothing,
not even its placeholder. That cost the better part of a session twice — once on a `TextArea`
given a 1-row interior inside a hand-rolled `Panel` (double chrome), once on section tabs at
height 2.

Nothing in `widget-contract.md` mentions minimum sizes.

Change: put the minimum in the type, not in a private guard. Either `fn min_size(&self) -> (u16, u16)`
on the builder (so layout code can allocate from it, and `Constraint::Length(w.min_size().1)`
becomes the idiom) or a `Refused { needs: (u16, u16) }` return from `render`. Whichever you pick,
document a per-family table in the contract, and give the field widgets a real 1-row mode
(`FieldShape::None` already costs 0 chrome — make it reachable as "compact" on `Input`,
`TextArea`, `Select` without the caller computing chrome arithmetic).

Acceptance: a test per widget family that renders at its stated minimum and asserts the buffer
is non-empty, plus one at minimum-minus-one asserting the refusal is *observable*; a squeezed
column on the showcase layout page; the table in the contract.

## 2. Chrome widgets claim keys a host cannot get back

`TabBarState::handle_key` (`tabs.rs:220-268`) claims Enter, Space, Left, Right, Home, End and
every digit 1-9. Route events through a chrome tab bar before the focused screen — the obvious
wiring for a persistent nav bar — and the app loses Enter, spaces, digits and caret movement
everywhere. Letters still work, so it reads as a submit bug, not a routing bug. The fix in the
consumer was to feed the bar `matches!(ev, Event::Mouse(_))` only and rebind the keyboard paths
by hand.

Change: let the caller narrow what an interactive widget claims — `.keys(TabKeys::ARROWS)`,
or a general `Interactive::claims(&KeyEvent) -> bool` that a host can consult before delegating.
Document the rule for every widget that may coexist with a text field.

Acceptance: a test that a narrowed `TabBar` returns `Outcome::Ignored` for Enter and `'3'` while
still switching on Left/Right; a note in the contract next to `handle_key`.

## 3. `Outcome` cannot distinguish edited from committed

`Outcome` is `Ignored | Consumed | Changed` (`core.rs:17`). `Input` returns `Changed` on *every*
keystroke; `Select` returns `Changed` exactly once, on pick. A settings screen that treats
`Changed` as "write this value" therefore writes the config on the first character typed. The
real submit signal is `InputState::take_submitted()`, which is discoverable only by reading the
source, and the equivalent accessor is not uniform across editing widgets.

Change: add `Outcome::Submitted` (order it above `Changed` so the existing `BitOr` max still
composes) or, if the enum must stay three-valued, give every editing widget the same
`take_submitted()`/`take_activated()` pair and state per widget in the contract what `Changed`
means for it.

Acceptance: one test per interactive widget pinning edit vs commit; a table in the contract.

## 4. There is no modal story, only the parts

`layout::center` (`layout.rs:8`), `popup_below` (`layout.rs:139`) and `Overlay` (`layout.rs:165`)
all exist and are exported, and `SelectState::render_overlay` (`select.rs:487`) shows the
intended shape — but nothing composes them into "a card over the current screen". The consumer
hand-rolled centering three times (all three copies were byte-identical to `layout::center`,
found and deleted only while writing this file), plus a `draw::fill` backdrop, plus its own
z-order rule, plus key-priority so the modal outranks the focused text field.

`ListView` has no `render_overlay`, so a picker-in-a-card is entirely caller-built.

Change: a `Modal`/`Dialog` widget — backdrop (optional dim), centered card, title, a body the
caller renders, Esc handling — or, at minimum, `ListView::render_overlay` mirroring `Select`'s
plus `docs/src/recipes/modal.md` showing the layer order and the "modal owns the keyboard"
rule. The discoverability *is* the bug: three helpers nobody finds is worse than one widget.

Acceptance: a showcase page with a modal over a busy screen; a test that the card's backdrop
covers what was underneath.

## 5. `ChatBlock` is closed, so half the library can't appear in a chat

`ChatBlock` is `Text | Code | Thinking | ToolCall | Divider` (`ai.rs:74`). `InlineAlert`,
`KeyValueList` and `DataTable` are standalone widgets that cannot be blocks, so a consumer
showing a note or a table of facts inside a transcript renders them as hand-aligned text and
loses every behaviour those widgets have. (A `ChatMessage`'s `text` is also ignored once it
carries blocks, which silently swallowed note bodies until it was found — worth a doc line.)

Change: add `ChatBlock::Facts(Vec<(String, String)>)` and `ChatBlock::Alert { level, title, text }`
reusing the `KeyValueList`/`InlineAlert` renderers, or make the enum extensible with a
height-reporting custom variant. Prefer the two concrete variants unless a third consumer asks
for more.

Acceptance: a `ChatView` test asserting a facts block aligns its keys across rows;
`docs/src/widgets/ai.md` updated.

## 6. Builder/state split is inconsistent, and the compiler is the only documentation

`ToastStack::new().corner(…)` does not exist: the corner lives on the *state*
(`Toaster::corner`, `toast.rs:241`), while every other widget takes its look on the per-frame
builder. Nothing says which side owns what, so the first guess compiles for every widget except
this one.

Change: state owns what must survive frames (scroll, cursor, hit boxes, animation clocks);
the builder owns per-frame look. Mirror the misplaced setters on the builder or move them, and
write the rule down in the contract with the exception list.

Acceptance: contract section plus either a mirrored `ToastStack::corner` or a moved one.

## 7. The AI pickers bake one schema

`ModelPicker`, `SessionList` and `AgentTree` each hardcode a set of columns for one domain. A
consumer with a different catalogue to show — a skills library, MCP servers, saved records —
must either fake its rows into a model picker (we filled `·` into unused columns) or fall back
to `ListView` and lose the column alignment.

Change: build them on `DataTable`'s column machinery, or add one generic `Catalog` (columns,
marks, filter, cursor) and keep `ModelPicker`/`SessionList` as thin presets over it. This is
the single biggest "fits more cases" win in the list.

Acceptance: a showcase page rendering two unrelated catalogues through the same widget; the
existing `ModelPicker` tests still pass unchanged.

## 8. The library never asks what the terminal can do

No `COLORTERM`, `TERM` or `NO_COLOR` probe anywhere in `tuile/src`, and the glyphs are literals:
`■`, `●`, `○`, `✓`, `▎`, half-blocks in the sparkline and big-text fonts. A consumer on a
256-colour terminal, under `NO_COLOR`, or on a font without box drawing gets whatever the
palette produced. The harness hand-rolled a capability layer (truecolor from `COLORTERM`,
256-colour fallback, attributes-only under `NO_COLOR`) and an ASCII path for its logo, and the
agent shell that runs the tests exports `NO_COLOR=1`, which makes every colour assertion read
"default" unless the harness strips it — a trap for anyone testing tuile in CI.

Change: a `Caps { color: ColorDepth, unicode: UnicodeLevel }` probed once (env first, overridable),
`theme::current()` resolving against it, and a glyph table with an ASCII fallback row. Name the
type something other than `Caps` — `ai_agents.rs:2123` already uses `caps` for model capabilities.

Acceptance: a test that renders one widget at each colour depth and asserts distinct output,
one under `NO_COLOR` asserting attributes-only; a doc page on capabilities.

## 9. Widgets that invent numbers

`ai.rs:2454` prints `~{} tokens` from `chars.div_ceil(4)`. The widget cannot know the caller's
tokenizer, and a compose box that reports a token count next to a model that bills by tokens is
a confident wrong number — the same class of bug as the `$0.00` and `0%` fixed on this branch.

Change: `tokens: Option<u32>` supplied by the caller; draw nothing when absent. Keep the
estimate only behind an explicit `.estimate_tokens(chars_per_token: f32)` opt-in if anyone wants
it.

Acceptance: a test that the compose editor shows no token figure unless one was given.

## 10. Vocabulary, units and currency are the widget's, not the caller's

`ai_compose.rs:1402` formats a percentage, `1412` formats `${:.2}`; `ai_agents.rs:37-60`
hardcodes `$` in `fmt_cost` and `ms`/`s`/`m` in `fmt_ms`; `ai_agents.rs:1340` and `1384` print
`"{}% used"` and `"free {}%"`; `ai_agents.rs:1600` prints `cache {}%`. A harness that counts
*steps* could not use `SessionEntry` because it said `msgs` — that one is fixed, and
`SessionEntry::messages` (`ai_agents.rs:1756`) is now a convenience over a raw `fact()`, which is
the pattern to copy: keep the ergonomic preset, add the escape hatch.

Change: sweep the `ai*` families for (a) user-visible English, (b) `$` as the only currency,
(c) units the caller might not be using; give each a caller-supplied override with the current
behaviour as the default. Not a rename — an addition, so no consumer breaks.

Acceptance: one test showing a non-USD, non-English label riding through; no signature removed.

---

## Sequencing

1 and 3 first — they change the contract, and every later item inherits from them. 4 and 7 are
the two that make the library fit new cases rather than merely fit better. 2, 6, 8 are
correctness-of-API. 5, 9, 10 are honesty sweeps and can go in any order.
