//! AI Tools gallery: tool timeline, shell/code blocks, edit previews, change sets, JSON trees, retry notices.

use std::time::{Duration, Instant};
use tuile::layout::columns;
use tuile::prelude::*;
use tuile::widgets::ai::DiffLine;

use super::{Ctx, Page, card};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Timeline,
    EditPreview,
    ChangeSet,
    JsonTree,
}

pub struct AiToolsPage {
    focus: Focus<Id>,
    timeline: ToolTimelineState,
    timeline_started: Option<Instant>,
    edit_preview: EditPreviewState,
    edit_started: Option<Instant>,
    changeset: ChangeSetState,
    json: JsonTreeState,
    shell_failed: bool,
    shell_collapsed: bool,
    shell_elapsed: f32,
    retry_started: Option<Instant>,
}

impl Default for AiToolsPage {
    fn default() -> Self {
        let mut timeline = ToolTimelineState::default();
        timeline.steps = vec![
            ToolStep::new("read")
                .summary("Read file src/main.rs")
                .status(ToolStatus::Pending),
            ToolStep::new("grep")
                .summary("Search for pattern")
                .status(ToolStatus::Pending)
                .depth(1),
            ToolStep::new("read")
                .summary("Read file src/main.rs")
                .status(ToolStatus::Pending),
            ToolStep::new("write")
                .summary("Write updated file")
                .status(ToolStatus::Pending),
            ToolStep::new("bash")
                .summary("cargo build")
                .status(ToolStatus::Pending),
            ToolStep::new("test")
                .summary("Run test suite")
                .status(ToolStatus::Pending),
            ToolStep::new("debug")
                .summary("Attach debugger")
                .status(ToolStatus::Pending),
            ToolStep::new("eval")
                .summary("Execute Python code")
                .status(ToolStatus::Pending),
            ToolStep::new("task")
                .summary("Spawn subagent")
                .status(ToolStatus::Pending)
                .depth(1),
        ];

        let changeset = ChangeSetState {
            files: vec![
                FileChange::new("src/main.rs", ChangeKind::Modified)
                    .added(12)
                    .removed(3),
                FileChange::new("src/lib.rs", ChangeKind::Added)
                    .added(45)
                    .removed(0),
                FileChange::new("tests/integration.rs", ChangeKind::Modified)
                    .added(8)
                    .removed(5),
                FileChange::new("README.md", ChangeKind::Modified)
                    .added(2)
                    .removed(1),
                FileChange::new("src/old_module.rs", ChangeKind::Deleted)
                    .added(0)
                    .removed(120),
                FileChange::new("Cargo.toml", ChangeKind::Modified)
                    .added(3)
                    .removed(0),
            ],
            ..Default::default()
        };

        let mut json = JsonTreeState::default();
        let json_text = r#"{
            "tool": "read",
            "args": {"path": "src/main.rs"},
            "result": {
                "content": "fn main() { ... }",
                "lines": 42,
                "success": true,
                "meta": {
                    "modified": "2024-01-15",
                    "size": 1024
                }
            },
            "tokens": {
                "input": 150,
                "output": 800,
                "cached": 1200
            }
        }"#;
        if let Ok(parsed) = Json::parse(json_text) {
            json.set(parsed);
            // Expand top level
            json.expanded.insert(vec![]);
            json.expanded.insert(vec![3]); // "result" object
        }

        Self {
            focus: Focus::new([Id::Timeline, Id::EditPreview, Id::ChangeSet, Id::JsonTree]),
            timeline,
            timeline_started: None,
            edit_preview: EditPreviewState::default(),
            edit_started: None,
            changeset,
            json,
            shell_failed: false,
            shell_collapsed: false,
            shell_elapsed: 0.0,
            retry_started: None,
        }
    }
}

impl AiToolsPage {
    fn advance_timeline(&mut self, now: Instant) {
        let started = *self.timeline_started.get_or_insert(now);
        let elapsed = (now - started).as_secs_f32();

        // Advance steps every 1.5s
        let step_idx = (elapsed / 1.5) as usize;

        if step_idx >= self.timeline.steps.len() {
            // All done, wait 3s then restart
            if elapsed > self.timeline.steps.len() as f32 * 1.5 + 3.0 {
                self.timeline_started = Some(now);
                for step in &mut self.timeline.steps {
                    step.status = ToolStatus::Pending;
                    step.started = None;
                    step.duration = None;
                }
            }
            return;
        }

        for (i, step) in self.timeline.steps.iter_mut().enumerate() {
            if i < step_idx {
                // Completed
                if step.status != ToolStatus::Done && step.status != ToolStatus::Error {
                    step.status = if i == 6 {
                        ToolStatus::Error
                    } else {
                        ToolStatus::Done
                    }; // debug step fails
                    step.duration = Some(Duration::from_millis(((i % 3 + 1) * 400) as u64));
                }
            } else if i == step_idx {
                // Currently running
                if step.status == ToolStatus::Pending {
                    step.status = ToolStatus::Running;
                    step.started = Some(now);
                }
            }
        }
    }

    fn restart_timeline(&mut self, now: Instant) {
        self.timeline_started = Some(now);
        for step in &mut self.timeline.steps {
            step.status = ToolStatus::Pending;
            step.started = None;
            step.duration = None;
        }
    }
}

impl Page for AiToolsPage {
    fn title(&self) -> &'static str {
        "AI Tools"
    }

    fn subtitle(&self) -> &'static str {
        "Tool timeline, shell blocks, code blocks, edit previews, change sets, JSON trees, retries"
    }

    fn icon(&self) -> &'static str {
        "⊛"
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = &ctx.theme.clone();

        // Advance timeline animation
        self.advance_timeline(ctx.now);

        // Advance shell stream
        self.shell_elapsed += 0.016; // ~60fps

        // Reduced layout below 90x30
        if area.width < 90 || area.height < 30 {
            let cards = [("Tool timeline", 8u16), ("Edit preview", 0u16)];
            let rects =
                tuile::layout::stack(area, &cards.iter().map(|c| c.1).collect::<Vec<_>>(), 1);

            if !rects.is_empty() {
                let inner = card(buf, rects[0], th, cards[0].0);
                ToolTimeline::new()
                    .theme(th)
                    .focused(self.focus.is(Id::Timeline))
                    .now(ctx.now)
                    .render(inner, buf, &mut self.timeline);
            }

            // Edit preview
            if rects.len() > 1 {
                let inner = card(buf, rects[1], th, cards[1].0);
                let diff_lines = vec![
                    DiffLine {
                        kind: tuile::widgets::ai::DiffKind::Ctx,
                        text: "fn main() {".into(),
                    },
                    DiffLine {
                        kind: tuile::widgets::ai::DiffKind::Del,
                        text: "    println!(\"Hello\");".into(),
                    },
                    DiffLine {
                        kind: tuile::widgets::ai::DiffKind::Add,
                        text: "    println!(\"Hello, world!\");".into(),
                    },
                    DiffLine {
                        kind: tuile::widgets::ai::DiffKind::Ctx,
                        text: "}".into(),
                    },
                ];
                EditPreview::new()
                    .theme(th)
                    .path("src/main.rs")
                    .lines(&diff_lines)
                    .started(self.edit_started.unwrap_or(ctx.now))
                    .now(ctx.now)
                    .lps(24.0)
                    .render(inner, buf, &mut self.edit_preview);
            }
            return;
        }

        // Two columns at 130x42
        let cols = columns(area, 2, 2);
        let left = cols[0];
        let right = cols[1];

        // Left column
        let left_cards = [("Tool timeline", 18u16), ("Shell", 0u16)];
        let left_rects =
            tuile::layout::stack(left, &left_cards.iter().map(|c| c.1).collect::<Vec<_>>(), 1);

        // Timeline card
        if !left_rects.is_empty() {
            let inner = card(buf, left_rects[0], th, left_cards[0].0);
            ToolTimeline::new()
                .theme(th)
                .focused(self.focus.is(Id::Timeline))
                .now(ctx.now)
                .max_output_rows(4)
                .render(inner, buf, &mut self.timeline);
        }

        // Shell card
        if left_rects.len() > 1 {
            let inner = card(buf, left_rects[1], th, left_cards[1].0);
            let output = if self.shell_failed {
                vec![
                    (false, "   Compiling tuile v0.1.0".into()),
                    (true, "error[E0308]: mismatched types".into()),
                    (true, " --> src/main.rs:42:10".into()),
                    (false, "   |".into()),
                    (true, "42 |     let x: u32 = \"hello\";".into()),
                    (
                        true,
                        "   |                  ^^^^^^^ expected `u32`, found `&str`".into(),
                    ),
                    (false, "".into()),
                    (true, "error: could not compile `tuile`".into()),
                ]
            } else {
                vec![
                    (false, "   Compiling tuile v0.1.0".into()),
                    (
                        false,
                        "    Finished `test` profile [optimized] in 2.3s".into(),
                    ),
                    (false, "     Running unittests src/lib.rs".into()),
                    (false, "".into()),
                    (false, "running 8 tests".into()),
                    (false, "test core::tests::hit_box ... ok".into()),
                    (false, "test core::tests::focus_ring ... ok".into()),
                    (false, "test widgets::button::tests::press ... ok".into()),
                    (false, "".into()),
                    (false, "test result: ok. 8 passed; 0 failed".into()),
                ]
            };

            ShellBlock::new()
                .theme(th)
                .command(if self.shell_failed {
                    "cargo build"
                } else {
                    "cargo test"
                })
                .cwd(Some("/workspace"))
                .output(&output[..])
                .exit_code(Some(if self.shell_failed { 101 } else { 0 }))
                .running(false)
                .collapsed(self.shell_collapsed)
                .elapsed(self.shell_elapsed)
                .lps(8.0)
                .max_rows(6)
                .render(inner, buf);
        }

        // Right column - use Layout::vertical for proper budgeting
        use tuile::ratatui_core::layout::{Constraint, Layout};
        let right_rects = Layout::vertical([
            Constraint::Length(10), // Edit preview
            Constraint::Length(10), // Change set
            Constraint::Fill(1),    // JSON
            Constraint::Length(8),  // Code
            Constraint::Length(1),  // Retry
        ])
        .split(right);

        // Edit preview card
        if !right_rects.is_empty() {
            let inner = card(buf, right_rects[0], th, "Edit preview");
            let diff_lines = vec![
                DiffLine {
                    kind: tuile::widgets::ai::DiffKind::Ctx,
                    text: "pub fn execute(&self) -> Result<(), Error> {".into(),
                },
                DiffLine {
                    kind: tuile::widgets::ai::DiffKind::Del,
                    text: "    let config = Config::default();".into(),
                },
                DiffLine {
                    kind: tuile::widgets::ai::DiffKind::Add,
                    text: "    let config = self.load_config()?;".into(),
                },
                DiffLine {
                    kind: tuile::widgets::ai::DiffKind::Ctx,
                    text: "    config.validate()?;".into(),
                },
                DiffLine {
                    kind: tuile::widgets::ai::DiffKind::Del,
                    text: "    self.run()".into(),
                },
                DiffLine {
                    kind: tuile::widgets::ai::DiffKind::Add,
                    text: "    self.run(&config)".into(),
                },
                DiffLine {
                    kind: tuile::widgets::ai::DiffKind::Ctx,
                    text: "}".into(),
                },
            ];

            let started = *self.edit_started.get_or_insert(ctx.now);
            EditPreview::new()
                .theme(th)
                .path("src/executor.rs")
                .lines(&diff_lines)
                .started(started)
                .now(ctx.now)
                .lps(24.0)
                .accept_text("[y] Apply")
                .reject_text("[n] Skip")
                .render(inner, buf, &mut self.edit_preview);

            if let Some(decision) = self.edit_preview.take_decision() {
                ctx.notify(
                    format!("Edit {:?}", decision),
                    tuile::theme::Variant::Default,
                );
                // Reset animation
                self.edit_started = Some(
                    ctx.now
                        .checked_add(Duration::from_secs_f32(1.5))
                        .unwrap_or(ctx.now),
                );
            }
        }

        // Change set card
        if right_rects.len() > 1 {
            let inner = card(buf, right_rects[1], th, "Change set");
            ChangeSet::new()
                .theme(th)
                .footer(true)
                .render(inner, buf, &mut self.changeset);

            if self.changeset.take_activated().is_some() {
                ctx.notify("File activated", tuile::theme::Variant::Default);
            }
        }

        // JSON tree card
        if right_rects.len() > 2 {
            let inner = card(buf, right_rects[2], th, "Tool result (JSON)");
            JsonTree::new()
                .theme(th)
                .focused(self.focus.is(Id::JsonTree))
                .render(inner, buf, &mut self.json);
        }

        // Code card
        if right_rects.len() > 3 {
            let inner = card(buf, right_rects[3], th, "Code");
            let code = r#"fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file>", args[0]);
        return;
    }
    match process(&args[1]) {
        Ok(_) => println!("Success!"),
        Err(e) => eprintln!("Error: {}", e),
    }
}"#;
            CodeBlock::new()
                .theme(th)
                .lang("rust")
                .path("examples/demo.rs")
                .text(code)
                .line_numbers(true)
                .start_line(1)
                .caret(false)
                .render(inner, buf);
        }

        // Retry notice
        if right_rects.len() > 4 {
            let retry_start = *self.retry_started.get_or_insert(ctx.now);
            let deadline = retry_start
                .checked_add(Duration::from_secs(6))
                .unwrap_or(ctx.now);

            // Reset every 7s
            if (ctx.now - retry_start).as_secs_f32() > 7.0 {
                self.retry_started = Some(ctx.now);
            }

            RetryNotice::new()
                .theme(th)
                .reason("Rate limited (429)")
                .attempt(2, 5)
                .deadline(deadline)
                .now(ctx.now)
                .compact(true)
                .retrying_text("reconnecting…")
                .render(right_rects[4], buf);
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        let mut out = Outcome::Ignored;

        match ev {
            Event::Key(k) if tuile::core::is_press(k) => {
                match k.code {
                    KeyCode::Char('r') => {
                        self.restart_timeline(ctx.now);
                        return Outcome::Consumed;
                    }
                    KeyCode::Char('x') => {
                        self.shell_failed = !self.shell_failed;
                        self.shell_elapsed = 0.0;
                        return Outcome::Consumed;
                    }
                    KeyCode::Enter
                        if !self.focus.is(Id::Timeline) && !self.focus.is(Id::JsonTree) =>
                    {
                        // Shell block toggle collapse
                        self.shell_collapsed = !self.shell_collapsed;
                        return Outcome::Consumed;
                    }
                    _ => {}
                }

                // Route to focused widget
                if self.focus.is(Id::Timeline) {
                    out |= self.timeline.handle_key(*k);
                } else if self.focus.is(Id::EditPreview) {
                    out |= self.edit_preview.handle_key(*k);
                } else if self.focus.is(Id::ChangeSet) {
                    out |= self.changeset.handle_key(*k);
                } else if self.focus.is(Id::JsonTree) {
                    out |= self.json.handle_key(*k);
                }
            }
            Event::Mouse(m) => {
                // every widget checks its own cached rects, so all four see every event
                out |= self.timeline.handle_mouse(*m);
                out |= self.edit_preview.handle_mouse(*m);
                out |= self.changeset.handle_mouse(*m);
                out |= self.json.handle_mouse(*m);
            }
            _ => {}
        }

        out
    }

    fn animating(&self, now: Instant) -> bool {
        // Timeline animation
        if self.timeline.animating(now) {
            return true;
        }

        if let Some(started) = self.timeline_started {
            let elapsed = (now - started).as_secs_f32();
            if elapsed < self.timeline.steps.len() as f32 * 1.5 + 3.0 {
                return true;
            }
        }

        // Edit preview reveal animation
        if let Some(started) = self.edit_started {
            let elapsed = (now - started).as_secs_f32();
            if elapsed < 1.0 {
                return true;
            }
        }

        // Retry countdown
        if let Some(started) = self.retry_started {
            let elapsed = (now - started).as_secs_f32();
            if elapsed < 7.0 {
                return true;
            }
        }

        // Shell stream
        true
    }

    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("Tab", "Focus"),
            ("↑↓ Enter", "Navigate / toggle"),
            ("a/r/e", "Edit decision"),
            ("x", "Fail shell"),
            ("r", "Replay"),
        ]
    }
}
