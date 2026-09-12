# The AI harness

Widgets for building a coding agent or chat UI in a terminal. They render the state you supply and handle navigation, expansion, and mouse interaction. None of them talk to a model or carry provider code.

![ai](../screenshots/ai.png)

## Chat

Conversation display, streaming text, thinking indicators, approvals, and token gauges.

### ChatView

A scrollable chat log with bubbles or full-width messages. Assistant messages have a role-coloured vertical bar on the left.

```rust
# extern crate tuile;
# extern crate ratatui_core;
use tuile::prelude::*;
use tuile::widgets::ai::{ChatView, ChatState, ChatMessage, Role};

# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = ChatState::default();
state.push(ChatMessage::new(Role::User, "Hello!"));
state.push(ChatMessage::new(Role::Assistant, "Hi! How can I help?"));
ChatView::new().bubbles(true).render(area, buf, &mut state);
# }
# fn main() {}
```

The state owns `messages: Vec<ChatMessage>`, `scroll: usize`, and `follow: bool`. When follow is true, the view scrolls to the bottom after every push. Mouse clicks on thinking blocks toggle their collapsed flag. Keys: `j`/`k` or arrow keys scroll, `g`/`G` jump to top or bottom, `Space`/`Shift+Space` page, and clicking the "N new" pill when scrolled up re-follows.

| Method | Effect |
|--------|--------|
| `.bubbles(bool)` | Bubble layout (rounded inset messages) or full-width rows |
| `.bar(Edge)` | Thickness of the left role bar on assistant messages (thin by default) |
| `.compact(bool)` | Dense spacing between messages |
| `.show_time(bool)` | Show timestamp on each message |
| `.focused(bool)` | Focused state (brighter scrollbar) |
| `.hover(bool)` | Track mouse hover row in `state.hover_row` |
| `.max_width(u16)` | Maximum message width in bubbles mode |
| `.now(Instant)` | Clock for thinking spinners and elapsed timers |
| `.highlighter(Highlighter)` | Syntax highlighting for code blocks; ranges are grapheme offsets |

### ChatMessage

One message in the conversation. The `role` is User, Assistant, System, or Tool. Text is rendered with inline markdown: `**bold**`, `` `code` ``, and `#` headings. Bullets (`- ` or `* `) become `• `.

```rust
# extern crate tuile;
# use tuile::widgets::ai::{ChatMessage, Role, ChatBlock};
# fn demo() {
let msg = ChatMessage::new(Role::User, "Add retry logic")
    .author("irvin")
    .time("14:02");
# }
# fn main() {}
```

For richer content use `.block()` or `.with_blocks()` with `ChatBlock` variants. **When blocks are non-empty, the `text` field is ignored**: a message with both `text` and `blocks` renders only the blocks.

### ChatBlock

Content blocks inside a message.

| Variant | Renders as |
|---|---|
| `Text(String)` | Plain text with inline markdown: `**bold**`, `` `code` `` in a different colour, `#` and `##` as headings |
| `Code { lang, text }` | A fenced code block with an optional language label |
| `Thinking { text, secs, collapsed, streaming }` | An expandable thinking block with elapsed time. Clicking the header toggles `collapsed` in place |
| `ToolCall { name, summary, status, duration_ms }` | An inline tool-call badge. `status` is `Pending`, `Running`, `Done` or `Error`; the duration is optional |
| `Divider(String)` | A horizontal rule with a centred label, for timestamps or turn markers |
| `Facts(Vec<(String, String)>)` | Aligned key/value pairs rendered with the `KeyValueList` layout. Keys are column-aligned within each block |
| `Alert { level, title, text }` | An inline alert rendered with the `InlineAlert` look. `level` is a `Variant` (Default, Success, Warning, Error) |

```rust
# extern crate tuile;
# use tuile::widgets::ai::{ChatMessage, Role, ChatBlock, ToolStatus};
# fn demo() {
let msg = ChatMessage::new(Role::Assistant, "")
    .with_blocks(vec![
        ChatBlock::Text("Checking the source:".into()),
        ChatBlock::ToolCall {
            name: "read_file".into(),
            summary: "src/main.rs".into(),
            status: ToolStatus::Done,
            duration_ms: Some(42),
        },
        ChatBlock::Code {
            lang: Some("rust".into()),
            text: "fn main() { println!(\"ok\"); }".into(),
        },
    ]);
# }
# fn main() {}
```

### Streaming

New text appears character by character. Call `state.begin_stream(message, full_text, now)` when the model starts a completion, then `state.stream_tick(now, cps)` each frame. The revealed prefix grows at `cps` characters per second and stops at word boundaries.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use std::time::Instant;
# use tuile::prelude::*;
# use tuile::widgets::ai::{ChatView, ChatState, ChatMessage, Role};
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
let mut state = ChatState::default();
let msg = ChatMessage::new(Role::Assistant, "");
state.begin_stream(msg, "The quick brown fox jumps over the lazy dog.".into(), now);
state.stream_tick(now, 80.0);
ChatView::new().now(now).render(area, buf, &mut state);
# }
# fn main() {}
```

### StreamText

Standalone streaming text widget with a cursor. Used for previews or isolated displays.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use std::time::Instant;
# use tuile::prelude::*;
# use tuile::widgets::ai::{StreamText, StreamCursor};
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
StreamText::new("The assistant is typing...")
    .now(now)
    .cps(60.0)
    .cursor(StreamCursor::Block)
    .render(area, buf);
# }
# fn main() {}
```

Cursor styles: `Block` (default `▌`), `Bar` (`▏`), `Underline`, or `None`.

### TypingIndicator

Three-dot travelling pulse animation.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use std::time::Instant;
# use tuile::prelude::*;
# use tuile::widgets::ai::TypingIndicator;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
TypingIndicator::new()
    .label("Assistant")
    .now(now)
    .render(area, buf);
# }
# fn main() {}
```

### Thinking

One-row thinking indicator with spinner, label, optional detail, and elapsed counter. The label has a sweeping shimmer band when `.shimmer(true)`.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use std::time::Instant;
# use tuile::prelude::*;
# use tuile::widgets::ai::Thinking;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
Thinking::new("Planning")
    .detail("reading 3 files")
    .started(now)
    .now(now)
    .elapsed_label(true)
    .shimmer(true)
    .render(area, buf);
# }
# fn main() {}
```

Methods: `.label()`, `.detail()`, `.spinner()` (from `tuile::widgets::spinner::spinners`), `.started(Instant)`, `.elapsed(f32)`, `.elapsed_label(bool)`, `.shimmer(bool)`, `.color(Rgb)`.

### Approval

Interactive approval dialog with three choices: Once, Always, Deny. Three visual styles: Card (default), Inline, Banner.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai::{Approval, ApprovalState, ApprovalStyle};
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = ApprovalState::default();
Approval::new("Allow write to README.md?")
    .command("write_file")
    .detail("The agent wants to update the installation instructions.")
    .style(ApprovalStyle::Card)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The state owns `focus: usize` and `choice: Option<ApprovalChoice>`. Keys: arrow keys or `1`/`2`/`3` to pick, `Enter` to confirm. Read `state.choice` after render to see if the user decided. Methods: `.command()`, `.detail()`, `.style()`, `.danger(bool)`.

### PromptComposer

Prompt input field with model pill, send/newline hints, attachments, and optional token count in a hint row below the text area.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai::{PromptComposer, ComposerState};
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = ComposerState::default();
PromptComposer::new()
    .model("claude-sonnet-4")
    .tokens(1234)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The state owns `editor: TextAreaState` and `submitted: Option<String>`. Call `state.take_submitted()` to drain the submitted text after the user presses Enter.

**Token count**: By default, no token figure is shown. Pass `.tokens(n)` for an exact count from the caller's tokenizer (drawn as `1234 tokens`) or `.estimate_tokens(chars_per_token)` for an estimate (drawn as `~1234 tokens` with tilde prefix). Showing an invented estimate next to a model that bills by tokens is a confident wrong number—only show a count when you have one.

Methods: `.model(String)`, `.placeholder(String)`, `.shape(FieldShape)`, `.attachments(&[String])`, `.focused(bool)`, `.tokens(u32)`, `.estimate_tokens(f32)`.


### ContextGauge

Token usage bar with prompt, completion, limit, percentage, and optional cost.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai::{ContextGauge, TokenUsage};
# fn demo(area: Rect, buf: &mut Buffer) {
let usage = TokenUsage { prompt: 12400, completion: 3200, limit: 200000 };
ContextGauge::new(usage)
    .cost_usd(0.0156)
    .render(area, buf);
# }
# fn main() {}
```

The bar is a `Meter` underneath, so `.style(MeterStyle::Line)` and `.gradient(&[Rgb, ...])` work. Default style is `MeterStyle::Block`. Methods: `.compact(bool)`, `.label(String)`.

### TokenHeat

Token probability heatmap: each token is coloured by its log probability. Higher probability is cooler (blue), lower is warmer (red).

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai::TokenHeat;
# fn demo(area: Rect, buf: &mut Buffer) {
let tokens = [
    ("The", 0.98),
    ("quick", 0.85),
    ("brown", 0.42),
];
TokenHeat::new(&tokens)
    .legend(true)
    .render(area, buf);
# }
# fn main() {}
```

![ai-tools](../screenshots/ai-tools.png)

## Tools

Tool execution timeline, shell and code blocks, edit previews, change sets, and JSON trees.

### ToolTimeline

Hierarchical tool execution log with live elapsed time, nested steps, and expandable output. Each step has a name, summary, status (Pending, Running, Done, Error), optional duration, and output lines. Depth controls nesting.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use std::time::Instant;
# use tuile::prelude::*;
# use tuile::widgets::ai::{ToolStatus};
# use tuile::widgets::ai_tools::{ToolTimeline, ToolTimelineState, ToolStep};
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
let mut state = ToolTimelineState::default();
state.steps.push(
    ToolStep::new("read")
        .summary("Read src/main.rs")
        .status(ToolStatus::Done)
);
state.steps.push(
    ToolStep::new("grep")
        .summary("Search pattern")
        .status(ToolStatus::Running)
        .depth(1)
);
ToolTimeline::new()
    .now(now)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The state owns `steps: Vec<ToolStep>`, `cursor: usize`, `scroll: usize`. Keys: `j`/`k` navigate, `Enter` toggles output expansion, `Space`/`Shift+Space` page. Each `ToolStep` has `.name()`, `.summary()`, `.status()`, `.started(Instant)`, `.duration(Duration)`, `.output(Vec<String>)`, `.depth(u8)`, `.expanded(bool)`.

### ShellBlock

Shell command display with streaming output, working directory, exit code, and elapsed time. Output lines are `(bool, String)` where the bool is true for stdout, false for stderr.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use std::time::Duration;
# use tuile::prelude::*;
# use tuile::widgets::ai_tools::ShellBlock;
# fn demo(area: Rect, buf: &mut Buffer) {
let output = vec![
    (true, "Compiling tuile v0.1.0".into()),
    (true, "Finished dev [unoptimized] in 2.3s".into()),
];
ShellBlock::new()
    .command("cargo build")
    .cwd(Some("/home/user/project"))
    .output(&output)
    .exit_code(Some(0))
    .duration(Some(Duration::from_secs(2)))
    .render(area, buf);
# }
# fn main() {}
```

Methods: `.running(bool)`, `.collapsed(bool)`, `.max_rows(u16)`, `.lps(f32)` (lines per second for streaming animation).

### CodeBlock

Code display with language label, file path, line numbers, and optional syntax highlighting.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_tools::CodeBlock;
# fn demo(area: Rect, buf: &mut Buffer) {
let code = "fn main() {\n    println!(\"Hello\");\n}";
CodeBlock::new()
    .lang("rust")
    .path("src/main.rs")
    .text(code)
    .line_numbers(true)
    .start_line(1)
    .render(area, buf);
# }
# fn main() {}
```

Methods: `.wrap(bool)`, `.caret(bool)` (blinking caret on the last line for live editing previews), `.highlighter(Highlighter)` (syntax highlighting function; `Highlighter` is a type alias for `fn(&str) -> Vec<(usize, usize, Style)>` where ranges are half-open grapheme-cluster offsets).

### EditPreview

Interactive edit preview with animated reveal and decision buttons (Accept, Reject, Edit). The lines are supplied as `Vec<DiffLine>` where each line has a `kind` (Add, Del, Ctx, Hunk) and `text`.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use std::time::Instant;
# use tuile::prelude::*;
# use tuile::widgets::ai::{DiffLine, DiffKind};
# use tuile::widgets::ai_tools::{EditPreview, EditPreviewState};
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
let mut state = EditPreviewState::default();
let lines = vec![
    DiffLine { kind: DiffKind::Ctx, text: "fn main() {".into() },
    DiffLine { kind: DiffKind::Del, text: "-   println!(\"old\");".into() },
    DiffLine { kind: DiffKind::Add, text: "+   println!(\"new\");".into() },
    DiffLine { kind: DiffKind::Ctx, text: "}".into() },
];
EditPreview::new()
    .path("src/main.rs")
    .lines(&lines)
    .started(now)
    .now(now)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The state owns `decision: Option<EditDecision>`. Keys: `a` Accept, `r` Reject, `e` Edit. The diff reveals line by line over 0.8 seconds. Methods: `.reveal_duration(f32)`.

### ChangeSet

File change summary with stat bars. Each `FileChange` has a path, kind (Added, Modified, Deleted, Renamed), and added/removed line counts.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_tools::{ChangeSet, ChangeSetState, FileChange, ChangeKind};
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = ChangeSetState::default();
state.files.push(
    FileChange::new("src/main.rs", ChangeKind::Modified)
        .added(12)
        .removed(3)
);
state.files.push(
    FileChange::new("src/lib.rs", ChangeKind::Added)
        .added(45)
);
ChangeSet::new().render(area, buf, &mut state);
# }
# fn main() {}
```

Keys: `j`/`k` navigate, `Space`/`Shift+Space` page. The state owns `files: Vec<FileChange>`, `cursor: usize`, `scroll: usize`.

### JsonTree

Collapsible JSON tree viewer. Parse JSON text with `Json::parse(text)` then render it.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_tools::{JsonTree, JsonTreeState, Json};
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = JsonTreeState::default();
let json = Json::parse(r#"{"status": "ok", "count": 42}"#).unwrap();
state.root = Some(json);
JsonTree::new().render(area, buf, &mut state);
# }
# fn main() {}
```

Keys: `j`/`k` navigate, `Enter` toggles expansion, `h` collapses, `l` expands. The state owns the tree and expansion map.

### RetryNotice

Retry countdown banner shown during exponential backoff.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use std::time::{Instant, Duration};
# use tuile::prelude::*;
# use tuile::widgets::ai_tools::RetryNotice;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
RetryNotice::new()
    .reason("Rate limit exceeded")
    .attempt(2, 3)
    .deadline(now + Duration::from_secs(5))
    .now(now)
    .render(area, buf);
# }
# fn main() {}
```

Methods: `.compact(bool)`.

![ai-agents](../screenshots/ai-agents.png)

## Agents

Agent trees, gantt lanes, token and cost meters, context maps, session lists, and model pickers.

### AgentTree

Hierarchical agent tree with expansion, cursor, task labels, status, tokens, and elapsed time. Each `AgentNode` has a name, model, status (Idle, Running, Waiting, Done, Failed, Parked), task description, token count, elapsed duration, and children.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use std::time::{Instant, Duration};
# use tuile::prelude::*;
# use tuile::widgets::ai_agents::{AgentTree, AgentTreeState, AgentNode, AgentStatus};
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
let mut state = AgentTreeState::default();
let mut root = AgentNode::new("Main", "opus", AgentStatus::Running)
    .task("Refactor auth module")
    .tokens(12400)
    .elapsed(Duration::from_secs(45));
root.children.push(
    AgentNode::new("Scout", "haiku", AgentStatus::Done)
        .task("Find usages")
        .tokens(3200)
        .elapsed(Duration::from_secs(8))
);
state.root = Some(root);
AgentTree::new()
    .now(now)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

Keys: `j`/`k` navigate, `Enter` toggles expansion, `h` collapses, `l` expands. The state owns `root: Option<AgentNode>`, `cursor: usize`, `scroll: usize`, and expansion map.

### AgentLanes

Gantt chart showing agent activity over time. Each `Lane` has a name and spans; each `LaneSpan` has start, optional end, status, and label.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_agents::{AgentLanes, Lane, LaneSpan, AgentStatus};
# fn demo(area: Rect, buf: &mut Buffer) {
let lanes = vec![
    Lane::new("Main").span(LaneSpan::new(0.0, Some(12.0), AgentStatus::Running, "task")),
    Lane::new("Scout").span(LaneSpan::new(2.0, Some(8.0), AgentStatus::Done, "search")),
];
AgentLanes::new(&lanes)
    .render(area, buf);
# }
# fn main() {}
```

Methods: `.compact(bool)`.

### TokenMeter

Stacked bar showing token breakdown: input, output, cache read, cache write. Includes a legend.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_agents::{TokenMeter, TokenBreakdown};
# fn demo(area: Rect, buf: &mut Buffer) {
let breakdown = TokenBreakdown {
    input: 12000,
    output: 3200,
    cache_read: 8000,
    cache_write: 2000,
};
TokenMeter::new(breakdown).render(area, buf);
# }
# fn main() {}
```

### CostMeter

Budget bar with current spend, total budget, and spend rate. The spend is passed as a direct value, not through state.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_agents::{CostMeter, CostMeterState};
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = CostMeterState::default();
CostMeter::new()
    .spent(0.42)
    .budget(10.0)
    .rate_per_min(0.05)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

Methods: `.compact(bool)`.

### ContextMap

Horizontal stacked bar showing context window segments by source. Each `ContextSegment` has a label, token count, and optional colour.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_agents::{ContextMap, ContextSegment};
# fn demo(area: Rect, buf: &mut Buffer) {
let segments = vec![
    ContextSegment::new("system", 1200),
    ContextSegment::new("history", 8400),
    ContextSegment::new("tools", 2400),
];
ContextMap::new(&segments, 200000)
    .render(area, buf);
# }
# fn main() {}
```

### CompactionBanner

Context compaction result banner showing before/after percentages, saved tokens, and optional summary.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use std::time::Instant;
# use tuile::prelude::*;
# use tuile::widgets::ai_agents::CompactionBanner;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
CompactionBanner::new(85.0, 42.0, 86000)
    .summary("Removed 3 old turns")
    .started(now)
    .now(now)
    .render(area, buf);
# }
# fn main() {}
```

### TurnStats

KPI cells showing input tokens, output tokens, cache hit rate, tool calls, duration, and cost. Each stat is optional.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use std::time::Duration;
# use tuile::prelude::*;
# use tuile::widgets::ai_agents::TurnStats;
# fn demo(area: Rect, buf: &mut Buffer) {
TurnStats::new()
    .input(12400)
    .output(3200)
    .cache_hit(0.68)
    .tool_calls(5)
    .duration(Duration::from_secs(8))
    .cost(0.042)
    .render(area, buf);
# }
# fn main() {}
```

### RateGraph

Sparkline-style throughput rate graph.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_agents::RateGraph;
# fn demo(area: Rect, buf: &mut Buffer) {
let values = vec![12.0, 18.0, 15.0, 22.0, 19.0];
RateGraph::new(&values)
    .label("tok/s")
    .max(30.0)
    .render(area, buf);
# }
# fn main() {}
```

### SessionList

Scrollable session list with cursor, time, and a detail row of facts. Each `SessionEntry` has a title, a when string, an active flag, and `facts: Vec<String>`.

Facts are strings, not fixed fields, because a session's vocabulary belongs to the harness: one counts messages and dollars, another counts steps and tokens on a branch. `.messages(u32)`, `.cost(f32)` and `.model(&str)` are conveniences that push `"42 msgs"`, `"$1.20"` and the model name; `.fact(impl Into<String>)` pushes anything else. They render in call order, separated by `·`.

A fact that was never reported is simply never pushed, so a local model with no cost shows `7 msgs · qwen3` rather than `$0.0000`. Never call `.cost(0.0)` to mean "unknown".

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_agents::{SessionList, SessionListState, SessionEntry};
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = SessionListState::default();
state.entries.push(
    SessionEntry::new("Refactor auth", "2h ago")
        .messages(24)
        .cost(0.42)
        .model("opus")
        .active(true)
);
SessionList::new().render(area, buf, &mut state);
# }
# fn main() {}
```

Keys: `j`/`k` navigate, `Enter` activates the selected session (read `state.activated`).

### ModelPicker

Interactive model picker with capability badges, context window, and pricing. Each `ModelInfo` has an id, provider, capabilities (Vision, Tools, Reasoning, Fast), and two optional facts: `context: Option<u32>` and `prices: Option<(f32, f32)>` (per million input/output tokens).

Both are published per model, so a managed or local model that publishes neither simply takes less width in the row. Only call `.context()` and `.prices()` with numbers a provider actually stated.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_agents::{ModelPicker, ModelPickerState, ModelInfo, Capability};
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = ModelPickerState::default();
state.models.push(
    ModelInfo::new("claude-opus-4", "anthropic")
        .context(200000)
        .prices(15.0, 75.0)
        .caps(vec![Capability::Vision, Capability::Tools])
);
ModelPicker::new().render(area, buf, &mut state);
# }
# fn main() {}
```

Keys: `j`/`k` navigate, `Enter` selects (read `state.selected`).

### ElapsedTimer

Elapsed time display with a pulsing dot when running.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use std::time::{Instant, Duration};
# use tuile::prelude::*;
# use tuile::widgets::ai_agents::ElapsedTimer;
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
ElapsedTimer::new()
    .since(now - Duration::from_secs(83))
    .now(now)
    .running(true)
    .render(area, buf);
# }
# fn main() {}
```

### Formatting helpers

`fmt_duration(d)` formats a duration as `850ms`, `1.2s`, `12.4s`, `1m 03s`, or `2h 05m`. `fmt_usd(v)` formats dollars as `$0.0042`, `$0.42`, or `$12.30`.

![ai-composer](../screenshots/ai-composer.png)

## Composer

Slash commands, mentions, attachments, mode badges, status lines, question cards, plan views, message queues, and suggestion chips.

### SlashMenu

Popup slash command menu with fuzzy matching. Each `SlashCommand` has a name, description, optional args hint, and optional category.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_compose::{SlashMenu, SlashMenuState, SlashCommand};
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = SlashMenuState::default();
state.query = "hel".into();
let commands = vec![
    SlashCommand::new("help", "Show help"),
    SlashCommand::new("clear", "Clear conversation"),
];
SlashMenu::new()
    .commands(&commands)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The state owns `query: String`, `cursor: usize`, and `selected: Option<String>`. Keys: `j`/`k` navigate, `Enter` selects. The widget writes `ranked` match indices and `names` each render; read `selected` when the user picks one. Methods: `.max_rows(usize)`.

### MentionPicker

Popup mention picker for files, directories, symbols, URLs, and agents. Each `MentionItem` has a label, kind, optional detail line, and recent flag.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_compose::{MentionPicker, MentionPickerState, MentionItem, MentionKind};
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = MentionPickerState::default();
state.query = "main".into();
let items = vec![
    MentionItem::new("src/main.rs", MentionKind::File).recent(true),
    MentionItem::new("main()", MentionKind::Symbol).detail("function"),
];
MentionPicker::new()
    .items(&items)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

Keys and state structure match `SlashMenu`. Methods: `.max_rows(usize)`.

### AttachmentChips

Horizontal row of removable attachment chips with hover and focus. Each `Attachment` has a name, kind (Image, File, Snippet, Url), and optional size label.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_compose::{AttachmentChips, AttachmentChipsState, Attachment, AttachmentKind};
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = AttachmentChipsState::default();
let chips = vec![
    Attachment::new("screenshot.png", AttachmentKind::Image).size("42 KB"),
    Attachment::new("config.toml", AttachmentKind::File),
];
AttachmentChips::new()
    .attachments(&chips)
    .focused(true)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The state owns `hover: Option<usize>` and `removed: Option<usize>`. Mouse hover updates `hover`; clicking the `×` sets `removed`. Keys: left/right to focus, `Backspace` to remove.

### ModeBadge

Animated mode badge with colour transition. Modes are Plan, Act, Ask, Auto.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use std::time::Instant;
# use tuile::prelude::*;
# use tuile::widgets::ai_compose::{ModeBadge, HarnessMode};
# fn demo(area: Rect, buf: &mut Buffer, now: Instant) {
ModeBadge::new()
    .mode(HarnessMode::Act)
    .now(now)
    .render(area, buf);
# }
# fn main() {}
```

Methods: `.from(Option<(HarnessMode, Instant)>)` for the transition animation.

### HarnessStatus

Status line showing model, mode, and optional stats (tokens, cost).

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_compose::{HarnessStatus, HarnessMode};
# fn demo(area: Rect, buf: &mut Buffer) {
HarnessStatus::new()
    .model("opus")
    .mode(HarnessMode::Act)
    .tokens(15600)
    .cost(0.042)
    .render(area, buf);
# }
# fn main() {}
```

Methods: `.branch(String)`, `.dirty(bool)`, `.context_pct(f32)`, `.elapsed(Duration)`, `.busy(bool)`, `.queued(u32)`.

`.context_pct` takes a **fraction**, `0.0..=1.0`, and renders it as a percentage beside a small bar. `.context_pct` and `.cost` are both unset by default and draw nothing at all when unset, so a provider that manages its own context window shows no percentage rather than `0%`. `.cost(0.0)` is a reported zero and does render as `$0.00`.

### QuestionCard

Interactive card presenting a question with single or multi-select options. Each `QuestionOption` has a label and description.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_compose::{QuestionCard, QuestionCardState, QuestionOption};
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = QuestionCardState::default();
let options = vec![
    QuestionOption::new("Yes", "Proceed with the change"),
    QuestionOption::new("No", "Skip this step"),
];
QuestionCard::new()
    .question("Apply the migration?")
    .options(&options)
    .multi(false)
    .recommended(Some(0))
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The state owns `cursor: usize`, `selected: Vec<bool>`, `submitted: Option<Vec<usize>>`, and `cancelled: bool`. Keys: `j`/`k` navigate, `Space` toggles, `Enter` submits. Read `submitted` or `cancelled` after render.

### PlanView

Collapsible tree of plan phases and tasks with keyboard navigation. Each `PlanPhase` has a name, tasks, and collapsed flag; each `PlanTask` has text and state (Pending, InProgress, Done, Blocked, Dropped).

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_compose::{PlanView, PlanViewState, PlanPhase, PlanTask, TaskState};
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = PlanViewState::default();
let phases = vec![
    PlanPhase::new("Setup").task("Install deps", TaskState::Done),
    PlanPhase::new("Implementation").task("Write tests", TaskState::InProgress),
];
PlanView::new()
    .phases(&phases)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

Keys: `j`/`k` navigate, `Enter` toggles phase collapse. The state owns `cursor: usize`, `scroll: usize`, and `collapsed: Vec<bool>`.

### MessageQueue

Scrollable list of queued messages with delete controls. Each `QueuedMessage` has text and a when label.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_compose::{MessageQueue, MessageQueueState, QueuedMessage};
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = MessageQueueState::default();
let messages = vec![
    QueuedMessage::new("Fix the timeout", "2m ago"),
    QueuedMessage::new("Add tests", "5m ago"),
];
MessageQueue::new()
    .messages(&messages)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The state owns `cursor: usize` and `removed: Option<usize>`. Keys: `j`/`k` navigate, `d` or `Delete` removes the selected message.

### Suggestions

Row of clickable suggestion chips. The cursor moves with arrow keys and Enter picks the selected text.

```rust
# extern crate tuile;
# extern crate ratatui_core;
# use tuile::prelude::*;
# use tuile::widgets::ai_compose::{Suggestions, SuggestionsState};
# fn demo(area: Rect, buf: &mut Buffer) {
let mut state = SuggestionsState::default();
let items = vec!["Fix typo", "Add docs", "Run tests"];
Suggestions::new()
    .items(&items)
    .render(area, buf, &mut state);
# }
# fn main() {}
```

The state owns `cursor: usize` and `activated: Option<usize>`. Keys: `h`/`l` or left/right navigate, `Enter` activates.
