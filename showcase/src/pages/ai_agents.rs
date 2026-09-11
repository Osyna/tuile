//! AI Agents gallery.

use std::time::{Duration, Instant};

use tuile::draw::Border;
use tuile::layout::{pad, stack};
use tuile::prelude::*;

use super::{Ctx, Page, card};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Tree,
    Sessions,
    Models,
}

pub struct AiAgentsPage {
    focus: Focus<Id>,
    tree: AgentTreeState,
    sessions: SessionListState,
    models: ModelPickerState,
    cost: CostMeterState,
    scenario_start: Option<Instant>,
    compaction_trigger: Option<Instant>,
    throughput_data: Vec<f64>,
    throughput_shift: usize,
}

impl Default for AiAgentsPage {
    fn default() -> Self {
        let mut sessions = SessionListState::new();
        sessions.entries = vec![
            SessionEntry::new("Build user auth", "2h ago")
                .messages(42)
                .cost(1.20)
                .model("sonnet")
                .active(true),
            SessionEntry::new("Fix database migration", "5h ago")
                .messages(18)
                .cost(0.35)
                .model("haiku"),
            SessionEntry::new("Implement search", "1d ago")
                .messages(67)
                .cost(2.10)
                .model("opus"),
            SessionEntry::new("Refactor API", "2d ago")
                .messages(25)
                .cost(0.78)
                .model("sonnet"),
            SessionEntry::new("Add caching layer", "3d ago")
                .messages(31)
                .cost(0.91)
                .model("haiku"),
            SessionEntry::new("Performance tuning", "1w ago")
                .messages(89)
                .cost(3.45)
                .model("opus"),
        ];

        let mut models = ModelPickerState::new();
        models.models = vec![
            ModelInfo::new("claude-sonnet-4", "anthropic")
                .context(200000)
                .prices(3.0, 15.0)
                .caps(vec![
                    Capability::Vision,
                    Capability::Tools,
                    Capability::Reasoning,
                ]),
            ModelInfo::new("claude-opus-4", "anthropic")
                .context(200000)
                .prices(15.0, 75.0)
                .caps(vec![
                    Capability::Vision,
                    Capability::Tools,
                    Capability::Reasoning,
                ]),
            ModelInfo::new("claude-haiku-4", "anthropic")
                .context(200000)
                .prices(0.8, 4.0)
                .caps(vec![Capability::Tools, Capability::Fast]),
            ModelInfo::new("gpt-4o", "openai")
                .context(128000)
                .prices(5.0, 15.0)
                .caps(vec![Capability::Vision, Capability::Tools]),
            ModelInfo::new("gpt-4o-mini", "openai")
                .context(128000)
                .prices(0.15, 0.60)
                .caps(vec![Capability::Tools, Capability::Fast]),
        ];
        models.selected = 0;

        // Deterministic throughput wave
        let mut throughput_data = Vec::with_capacity(60);
        for i in 0..60 {
            let t = i as f64 * 0.1;
            let val = 42.0 + 18.0 * (t * 0.5).sin() + 8.0 * (t * 1.3).cos();
            throughput_data.push(val.max(10.0));
        }

        Self {
            focus: Focus::new([Id::Tree, Id::Sessions, Id::Models]),
            tree: AgentTreeState::new(),
            sessions,
            models,
            cost: CostMeterState::new(),
            scenario_start: None,
            compaction_trigger: None,
            throughput_data,
            throughput_shift: 0,
        }
    }
}

impl AiAgentsPage {
    fn scenario_time(&self, ctx: &Ctx) -> f32 {
        match self.scenario_start {
            Some(start) => ctx.now.saturating_duration_since(start).as_secs_f32(),
            None => 0.0,
        }
    }

    fn build_scenario(&self, t: f32) -> AgentNode {
        // Main agent runs throughout
        let main_status = if t < 30.0 {
            AgentStatus::Running
        } else {
            AgentStatus::Done
        };

        let main_task = if t < 5.0 {
            "Planning architecture..."
        } else if t < 15.0 {
            "Coordinating subagents..."
        } else if t < 25.0 {
            "Integrating results..."
        } else {
            "Final review and cleanup"
        };

        let mut main = AgentNode::new("Main", "opus", main_status)
            .task(main_task)
            .tokens(12000 + (t * 500.0) as u32)
            .elapsed(Duration::from_secs_f32(t));

        // Scout: spawns at 2s, finishes at 10s
        if t >= 2.0 {
            let scout_status = if t >= 10.0 {
                AgentStatus::Done
            } else {
                AgentStatus::Running
            };
            let scout_task = if t < 6.0 {
                "Scanning codebase for patterns..."
            } else {
                "Analyzing 47 modules"
            };
            let scout = AgentNode::new("Scout", "sonnet", scout_status)
                .task(scout_task)
                .tokens(3200 + ((t - 2.0) * 200.0) as u32)
                .elapsed(Duration::from_secs_f32((t - 2.0).max(0.0)));
            main = main.child(scout);
        }

        // Builder: spawns at 3s, waits 12-15s, done at 22s
        if t >= 3.0 {
            let builder_status = if t >= 22.0 {
                AgentStatus::Done
            } else if (12.0..15.0).contains(&t) {
                AgentStatus::Waiting
            } else {
                AgentStatus::Running
            };
            let builder_task = if t < 8.0 {
                "Generating components..."
            } else if t < 12.0 {
                "Building widget tree..."
            } else if t < 15.0 {
                "Waiting for Scout results..."
            } else {
                "Finalizing implementation..."
            };
            let builder = AgentNode::new("Builder", "sonnet", builder_status)
                .task(builder_task)
                .tokens(8400 + ((t - 3.0) * 350.0) as u32)
                .elapsed(Duration::from_secs_f32((t - 3.0).max(0.0)));
            main = main.child(builder);
        }

        // Reviewer: spawns at 4s, fails at 18s
        if t >= 4.0 {
            let reviewer_status = if t >= 18.0 {
                AgentStatus::Failed
            } else {
                AgentStatus::Running
            };
            let reviewer_task = if t < 10.0 {
                "Checking code quality..."
            } else if t < 18.0 {
                "Running security audit..."
            } else {
                "Error: security check failed on auth module"
            };
            let reviewer = AgentNode::new("Reviewer", "haiku", reviewer_status)
                .task(reviewer_task)
                .tokens(2100 + ((t - 4.0) * 120.0) as u32)
                .elapsed(Duration::from_secs_f32((t - 4.0).max(0.0)));
            main = main.child(reviewer);
        }

        main
    }

    fn build_lanes(&self, t: f32) -> Vec<Lane> {
        let mut lanes = Vec::new();

        let main_lane = Lane::new("Main").span(LaneSpan::new(
            0.0,
            if t < 30.0 { None } else { Some(30.0) },
            if t < 30.0 {
                AgentStatus::Running
            } else {
                AgentStatus::Done
            },
            "opus",
        ));
        lanes.push(main_lane);

        if t >= 2.0 {
            let scout_lane = Lane::new("Scout").span(LaneSpan::new(
                2.0,
                Some(t.min(10.0)),
                if t >= 10.0 {
                    AgentStatus::Done
                } else {
                    AgentStatus::Running
                },
                "scanning",
            ));
            lanes.push(scout_lane);
        }

        if t >= 3.0 {
            let mut builder = Lane::new("Builder");

            if t > 3.0 {
                builder = builder.span(LaneSpan::new(
                    3.0,
                    Some(t.min(12.0)),
                    AgentStatus::Running,
                    "building",
                ));
            }

            if t > 12.0 {
                builder = builder.span(LaneSpan::new(
                    12.0,
                    Some(t.min(15.0)),
                    AgentStatus::Waiting,
                    "waiting",
                ));
            }

            if t > 15.0 {
                builder = builder.span(LaneSpan::new(
                    15.0,
                    Some(t.min(22.0)),
                    AgentStatus::Running,
                    "finalizing",
                ));
            }

            lanes.push(builder);
        }

        if t >= 4.0 {
            let reviewer_lane = Lane::new("Reviewer").span(LaneSpan::new(
                4.0,
                Some(t.min(18.0)),
                if t >= 18.0 {
                    AgentStatus::Failed
                } else {
                    AgentStatus::Running
                },
                "auditing",
            ));
            lanes.push(reviewer_lane);
        }

        lanes
    }

    fn get_context_segments(&self, t: f32) -> Vec<ContextSegment> {
        let compacted = if let Some(trigger) = self.compaction_trigger {
            trigger.elapsed().as_secs_f32() > 6.0
        } else {
            false
        };

        if compacted {
            let system = (4000.0 + t * 50.0) as u32;
            let tools = (6000.0 + t * 100.0) as u32;
            let files = (12000.0 + t * 200.0) as u32;
            let history = (9000.0 + t * 150.0) as u32;

            vec![
                ContextSegment::new("system", system),
                ContextSegment::new("tools", tools),
                ContextSegment::new("files", files),
                ContextSegment::new("history", history),
            ]
        } else {
            let pct = if t < 26.0 {
                (t / 26.0 * 78.0).min(78.0)
            } else {
                78.0
            };

            let total = (200000.0 * pct / 100.0) as u32;
            let system = (total as f32 * 0.05) as u32;
            let tools = (total as f32 * 0.15) as u32;
            let files = (total as f32 * 0.40) as u32;
            let history = (total as f32 * 0.40) as u32;

            vec![
                ContextSegment::new("system", system),
                ContextSegment::new("tools", tools),
                ContextSegment::new("files", files),
                ContextSegment::new("history", history),
            ]
        }
    }

    fn get_token_breakdown(&self, t: f32) -> TokenBreakdown {
        TokenBreakdown {
            input: (12400.0 + t * 300.0) as u32,
            output: (3100.0 + t * 150.0) as u32,
            cache_read: (9800.0 + t * 400.0) as u32,
            cache_write: (1200.0 + t * 50.0) as u32,
        }
    }
}

impl Page for AiAgentsPage {
    fn title(&self) -> &'static str {
        "AI Agents"
    }

    fn subtitle(&self) -> &'static str {
        "Agent tree and lanes, tokens, cost, context map, compaction, sessions, model picker"
    }

    fn icon(&self) -> &'static str {
        "◈"
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = &ctx.theme;

        if self.scenario_start.is_none() {
            self.scenario_start = Some(ctx.now);
        }

        let t = self.scenario_time(ctx);

        // Restart scenario at ~40s
        if t > 40.0 {
            self.scenario_start = Some(ctx.now);
            self.compaction_trigger = None;
            self.throughput_shift = 0;
        }

        self.tree.root = Some(self.build_scenario(t));
        // Auto-expand tree on first render
        if self.tree.expanded.is_empty() && self.tree.root.is_some() {
            self.tree.expanded.push(Vec::new()); // Expand root
        }
        let lanes = self.build_lanes(t);

        // Shift throughput data every 250ms
        let shift_interval = 0.25;
        let expected_shifts = (t / shift_interval) as usize;
        while self.throughput_shift < expected_shifts
            && self.throughput_shift < self.throughput_data.len()
        {
            self.throughput_shift += 1;
        }

        // Reduced layout for small screens
        if area.width < 90 || area.height < 30 {
            let rows = stack(area, &[0u16, 0u16], 1);

            if let Some(top) = rows.first() {
                let agents_area = card(buf, *top, th, "Agents");
                AgentTree::new()
                    .show_tasks(true)
                    .now(ctx.now)
                    .theme(th)
                    .render(agents_area, buf, &mut self.tree);
            }

            if let Some(bottom) = rows.get(1) {
                let tokens_area = card(buf, *bottom, th, "Tokens & Cost");
                let inner = pad(tokens_area, 1, 1);

                if inner.height >= 4 {
                    let rows = stack(inner, &[2u16, 2u16], 1);

                    if let Some(r) = rows.first() {
                        TokenMeter::new(self.get_token_breakdown(t))
                            .theme(th)
                            .render(*r, buf);
                    }

                    if let Some(r) = rows.get(1) {
                        CostMeter::new()
                            .spent(0.42 + t * 0.04)
                            .budget(5.0)
                            .rate_per_min(0.06)
                            .dur(ctx.dur(400))
                            .now(ctx.now)
                            .theme(th)
                            .render(*r, buf, &mut self.cost);
                    }
                }
            }

            return;
        }

        // Full layout: left column (tree + lanes), right column (stats + details)
        let cols = [Constraint::Percentage(46), Constraint::Percentage(54)];
        let [left, right] = Layout::horizontal(cols).areas(area);

        // Left column: tree sized to its rows (2 per node), lanes sized to their content, then a
        // live turn timer strip in whatever is left
        let tree_rows = self.tree.visible_len() as u16 * 2;
        let lanes_h = lanes.len() as u16 + 1 + 2; // rows + axis + frame
        let [tree_area, lanes_area, rest] = Layout::vertical([
            Constraint::Length((tree_rows + 2).min(left.height.saturating_sub(lanes_h + 3))),
            Constraint::Length(lanes_h),
            Constraint::Fill(1),
        ])
        .areas(left);

        let agents_inner = card(buf, tree_area, th, "Agents");
        AgentTree::new()
            .show_tasks(true)
            .focused(self.focus.is(Id::Tree))
            .now(ctx.now)
            .theme(th)
            .render(agents_inner, buf, &mut self.tree);

        let lanes_inner = card(buf, lanes_area, th, "Lanes");
        AgentLanes::new(&lanes)
            .clock(t)
            .window(30.0)
            .now(ctx.now)
            .theme(th)
            .render(lanes_inner, buf);
        if rest.height >= 3 {
            let inner = card(buf, rest, th, "Turn");
            let [timer_row, _] =
                Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(inner);
            ElapsedTimer::new()
                .since(self.scenario_start.unwrap_or(ctx.now))
                .now(ctx.now)
                .running(t < 30.0)
                .label(if t < 30.0 { "running" } else { "finished" })
                .theme(th)
                .render(timer_row, buf);
        }

        // Right column
        use ratatui::layout::{Constraint, Layout};

        let right_constraints = [
            Constraint::Length(3), // TurnStats
            Constraint::Length(7), // Tokens & Cost
            Constraint::Length(6), // Context
            Constraint::Length(7), // Throughput
            Constraint::Fill(1),   // Sessions + Models
        ];
        let right_rows: [Rect; 5] = Layout::vertical(right_constraints).areas(right);

        // Turn stats strip
        TurnStats::new()
            .input(12400)
            .output(3100)
            .cache_hit(0.71)
            .tool_calls(4)
            .duration(Duration::from_secs(133))
            .cost(0.12)
            .theme(th)
            .render(right_rows[0], buf);

        // Tokens & Cost
        {
            let inner = card(buf, right_rows[1], th, "Tokens & Cost");
            let token_rows = stack(inner, &[2u16, 2u16], 1);
            if let Some(r) = token_rows.first() {
                TokenMeter::new(self.get_token_breakdown(t))
                    .theme(th)
                    .render(*r, buf);
            }
            if let Some(r) = token_rows.get(1) {
                let cost = (0.42 + t * 0.04).min(1.80);
                CostMeter::new()
                    .spent(cost)
                    .budget(5.0)
                    .rate_per_min(0.06)
                    .dur(ctx.dur(400))
                    .now(ctx.now)
                    .theme(th)
                    .render(*r, buf, &mut self.cost);
            }
        }

        {
            let inner = card(buf, right_rows[2], th, "Context");

            if let Some(trigger) = self.compaction_trigger {
                if trigger.elapsed().as_secs_f32() < 6.0 {
                    CompactionBanner::new(78.0, 31.0, 94000)
                        .started(trigger)
                        .summary("Removed 47 old tool results, 12 stale file reads")
                        .now(ctx.now)
                        .theme(th)
                        .render(inner, buf);
                } else {
                    let segments = self.get_context_segments(t);
                    ContextMap::new(&segments, 200000)
                        .theme(th)
                        .render(inner, buf);
                }
            } else {
                let segments = self.get_context_segments(t);
                ContextMap::new(&segments, 200000)
                    .theme(th)
                    .render(inner, buf);
            }
        }

        {
            let inner = card(buf, right_rows[3], th, "Throughput");

            let visible_count = 40.min(self.throughput_data.len());
            let start = self.throughput_shift.saturating_sub(visible_count);
            let end = self.throughput_shift.max(visible_count);
            let visible: Vec<f64> = if start < self.throughput_data.len() {
                self.throughput_data[start..end.min(self.throughput_data.len())].to_vec()
            } else {
                vec![42.0]
            };

            RateGraph::new(&visible)
                .label("tok/s")
                .max(80.0)
                .theme(th)
                .render(inner, buf);
        }

        // Bottom row: Sessions + Models
        {
            let cols = [Constraint::Percentage(50), Constraint::Percentage(50)];
            let [sessions_area, models_area] = Layout::horizontal(cols).areas(right_rows[4]);

            {
                let inner = card(buf, sessions_area, th, "Sessions");
                let focused = self.focus.is(Id::Sessions);

                SessionList::new()
                    .two_line(true)
                    .theme(th)
                    .render(inner, buf, &mut self.sessions);

                if focused {
                    Border::Round.draw(buf, sessions_area, th.primary, th.background);
                }
            }

            {
                let inner = card(buf, models_area, th, "Models");
                let focused = self.focus.is(Id::Models);

                ModelPicker::new()
                    .theme(th)
                    .render(inner, buf, &mut self.models);

                if focused {
                    Border::Round.draw(buf, models_area, th.primary, th.background);
                }
            }
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        let _th = &ctx.theme;

        match ev {
            Event::Key(k) if is_press(k) => {
                match k.code {
                    KeyCode::Char('r') => {
                        // Restart scenario
                        self.scenario_start = Some(ctx.now);
                        self.compaction_trigger = None;
                        self.throughput_shift = 0;
                        return Outcome::Consumed;
                    }
                    KeyCode::Char('c') => {
                        // Trigger compaction
                        if self.compaction_trigger.is_none() {
                            self.compaction_trigger = Some(ctx.now);
                        }
                        return Outcome::Consumed;
                    }
                    KeyCode::Tab => {
                        self.focus.next();
                        return Outcome::Consumed;
                    }
                    KeyCode::BackTab => {
                        self.focus.prev();
                        return Outcome::Consumed;
                    }
                    _ => {}
                }

                // Route to focused widget
                let out = match self.focus.current() {
                    Some(Id::Tree) => self.tree.handle_key(*k),
                    Some(Id::Sessions) => self.sessions.handle_key(*k),
                    Some(Id::Models) => self.models.handle_key(*k),
                    None => Outcome::Ignored,
                };

                if let Some(idx) = self.sessions.take_activated()
                    && let Some(entry) = self.sessions.entries.get(idx)
                {
                    ctx.notify(format!("Opened session: {}", entry.title), Variant::Default);
                }

                if let Some(idx) = self.models.take_selected()
                    && let Some(model) = self.models.models.get(idx)
                {
                    ctx.notify(format!("Selected model: {}", model.id), Variant::Default);
                }

                return out;
            }
            Event::Mouse(m) => {
                let mut out = Outcome::Ignored;
                out |= self.tree.handle_mouse(*m);
                out |= self.sessions.handle_mouse(*m);
                out |= self.models.handle_mouse(*m);

                // Focus on click
                if self.tree.hit.hover && matches!(m.kind, MouseEventKind::Down(_)) {
                    self.focus.set(Id::Tree);
                    out |= Outcome::Consumed;
                }
                if self.sessions.hit.hover && matches!(m.kind, MouseEventKind::Down(_)) {
                    self.focus.set(Id::Sessions);
                    out |= Outcome::Consumed;
                }
                if self.models.hit.hover && matches!(m.kind, MouseEventKind::Down(_)) {
                    self.focus.set(Id::Models);
                    out |= Outcome::Consumed;
                }

                return out;
            }
            _ => {}
        }

        Outcome::Ignored
    }

    fn animating(&self, now: Instant) -> bool {
        self.cost.animating(now)
            || self.compaction_trigger.is_some()
            || self.scenario_start.is_some()
    }

    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("Tab", "Focus"),
            ("↑↓ Enter", "Navigate / select"),
            ("type", "Filter sessions"),
            ("c", "Compact context"),
            ("r", "Restart scenario"),
        ]
    }
}
