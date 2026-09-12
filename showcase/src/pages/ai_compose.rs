//! AI Composer gallery.

use std::time::Instant;
use tuile::prelude::*;

use super::{Ctx, Page, card};
use tuile::widgets::ai::{ComposerState, PromptComposer};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Composer,
    Question,
    Plan,
    Queue,
    Suggestions,
}

pub struct AiComposePage {
    focus: Focus<Id>,
    composer: ComposerState,
    slash_menu: SlashMenuState,
    mention_picker: MentionPickerState,
    attachments_state: AttachmentChipsState,
    mode: HarnessMode,
    mode_from: Option<(HarnessMode, Instant)>,
    question: QuestionCardState,
    question_multi: bool,
    question_armed: Option<Instant>,
    plan: PlanViewState,
    queue: MessageQueueState,
    suggestions: SuggestionsState,
    messages: Vec<QueuedMessage>,
    task_tick: f32,
}

impl Default for AiComposePage {
    fn default() -> Self {
        Self {
            focus: Focus::new([
                Id::Composer,
                Id::Question,
                Id::Plan,
                Id::Queue,
                Id::Suggestions,
            ]),
            composer: ComposerState::default(),
            slash_menu: SlashMenuState::default(),
            mention_picker: MentionPickerState::default(),
            attachments_state: AttachmentChipsState::default(),
            mode: HarnessMode::Auto,
            mode_from: None,
            question: QuestionCardState {
                selected: vec![false, false, false],
                ..Default::default()
            },
            question_multi: false,
            question_armed: None,
            plan: PlanViewState::default(),
            queue: MessageQueueState::default(),
            suggestions: SuggestionsState::default(),
            messages: Vec::new(),
            task_tick: 0.0,
        }
    }
}

impl AiComposePage {
    /// Slash / mention popups, drawn last so they sit above everything.
    fn draw_popups(&mut self, area: Rect, composer_area: Rect, buf: &mut Buffer, th: &Theme) {
        if self.slash_menu.open {
            let commands = [
                SlashCommand::new("help", "Show help").category("Info"),
                SlashCommand::new("clear", "Clear conversation").category("Edit"),
                SlashCommand::new("compact", "Compact output").category("View"),
                SlashCommand::new("model", "Change model").category("Config"),
                SlashCommand::new("plan", "Plan approach").category("Mode"),
                SlashCommand::new("review", "Review code").category("Action"),
                SlashCommand::new("commit", "Commit changes").category("Action"),
                SlashCommand::new("test", "Run tests").category("Action"),
                SlashCommand::new("diff", "Show diff").category("Info"),
                SlashCommand::new("undo", "Undo last change").category("Edit"),
                SlashCommand::new("cost", "Show session cost").category("Info"),
                SlashCommand::new("quit", "Exit session").category("System"),
            ];
            SlashMenu::new()
                .commands(&commands)
                .anchor(composer_area)
                .max_rows(12)
                .theme(th)
                .render(area, buf, &mut self.slash_menu);
        }

        if self.mention_picker.open {
            let items = [
                MentionItem::new("src/widgets/ai_compose.rs", MentionKind::File)
                    .detail("Widget implementations")
                    .recent(true),
                MentionItem::new("src/fetch.rs", MentionKind::File)
                    .detail("HTTP helper")
                    .recent(true),
                MentionItem::new("showcase/src/pages/ai_compose.rs", MentionKind::File)
                    .detail("Showcase page")
                    .recent(true),
                MentionItem::new("src/widgets/", MentionKind::Dir),
                MentionItem::new("SlashMenu::render", MentionKind::Symbol)
                    .detail("Popup rendering"),
                MentionItem::new("HarnessStatus", MentionKind::Symbol).detail("Status line widget"),
                MentionItem::new("https://docs.rs/ratatui", MentionKind::Url),
                MentionItem::new("Main", MentionKind::Agent).detail("Coordinator"),
                MentionItem::new("AiTools", MentionKind::Agent).detail("Tool widgets"),
                MentionItem::new("AiChat", MentionKind::Agent).detail("Chat widgets"),
            ];
            MentionPicker::new()
                .items(&items)
                .anchor(composer_area)
                .max_rows(10)
                .theme(th)
                .render(area, buf, &mut self.mention_picker);
        }
    }
}

impl Page for AiComposePage {
    fn title(&self) -> &'static str {
        "AI Composer"
    }

    fn subtitle(&self) -> &'static str {
        "Composer, slash commands, mentions, attachments, mode, status line, questions, plan, queue"
    }

    fn icon(&self) -> &'static str {
        "❯"
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        // below 90x30 only the composer block and the status line are shown
        let reduced = area.width < 90 || area.height < 30;
        if area.height < 9 || area.width < 30 {
            return;
        }
        let th = &ctx.theme;

        // bottom-up layout: status row + composer block + suggestions
        let status_y = area.bottom() - 1;
        let composer_h = 5;
        let attachments_h = 1;
        let suggestions_h = 1;
        let composer_block_h = composer_h + attachments_h + suggestions_h + 1; // +1 for spacing

        let composer_block_y = status_y.saturating_sub(composer_block_h);
        let top_area_h = composer_block_y.saturating_sub(area.y);

        // status line
        let queued = self.messages.len() as u32;
        let elapsed_secs = (ctx.elapsed() * 10.0) as u64;
        let busy = ((ctx.elapsed() * 0.5).floor() as u64).is_multiple_of(2);
        let tokens = (ctx.elapsed() * 100.0) as u32 + 1200;
        let cost = 0.12 + ctx.elapsed() * 0.01;
        let context_pct = 0.31 + (ctx.elapsed() * 0.01).min(0.3);

        HarnessStatus::new()
            .mode(self.mode)
            .model("claude-sonnet-4")
            .branch("main")
            .dirty(true)
            .context_pct(context_pct)
            .cost_text(format!("£{:.2}", cost * 0.79)) // Show cost override in GBP
            .tokens(tokens)
            .elapsed(std::time::Duration::from_secs(elapsed_secs))
            .busy(busy)
            .queued(queued)
            .now(ctx.now)
            .theme(th)
            .render(
                Rect {
                    x: area.x,
                    y: status_y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );

        // composer block
        let suggestions_y = composer_block_y;
        let attachments_y = suggestions_y + suggestions_h;
        let composer_y = attachments_y + attachments_h;

        // suggestions row with mode badge
        let suggestions_area = Rect {
            x: area.x,
            y: suggestions_y,
            width: area.width,
            height: suggestions_h,
        };
        fill(buf, suggestions_area, th.background);

        let badge = ModeBadge::new()
            .mode(self.mode)
            .hint("m")
            .from(self.mode_from)
            .now(ctx.now)
            .theme(th);
        let badge_w = badge.width();
        badge.render(
            Rect {
                x: area.x,
                y: suggestions_y,
                width: badge_w,
                height: 1,
            },
            buf,
        );

        let suggestion_items = [
            "Run tests",
            "Explain the diff",
            "Commit",
            "Write docs",
            "Open PR",
        ];
        Suggestions::new()
            .items(&suggestion_items)
            .theme(th)
            .render(
                Rect {
                    x: area.x + badge_w + 2,
                    y: suggestions_y,
                    width: area.width.saturating_sub(badge_w + 2),
                    height: suggestions_h,
                },
                buf,
                &mut self.suggestions,
            );

        let attachments = [
            Attachment::new("shot.png", AttachmentKind::Image).size("1.2MB"),
            Attachment::new("README.md", AttachmentKind::File),
            Attachment::new("snippet", AttachmentKind::Snippet).size("45 lines"),
        ];
        AttachmentChips::new()
            .attachments(&attachments)
            .focused(self.focus.is(Id::Composer))
            .theme(th)
            .render(
                Rect {
                    x: area.x,
                    y: attachments_y,
                    width: area.width,
                    height: attachments_h,
                },
                buf,
                &mut self.attachments_state,
            );

        let composer_area = Rect {
            x: area.x,
            y: composer_y,
            width: area.width,
            height: composer_h,
        };
        PromptComposer::new()
            .model("claude-sonnet-4")
            .focused(self.focus.is(Id::Composer))
            .now(ctx.now)
            .theme(th)
            .render(composer_area, buf, &mut self.composer);

        if reduced {
            self.draw_popups(area, composer_area, buf, th);
            return;
        }

        // top area: left (question) + right (plan above queue)
        let top_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: top_area_h,
        };

        let left_w = top_area.width / 2;
        let right_w = top_area.width - left_w - 2;

        // left: question card (sized to content) + mentions catalog below
        let options = [
            QuestionOption::new(
                "Commit the changes",
                "Stage and commit with a generated message",
            ),
            QuestionOption::new("Show diff first", "Review changes before committing"),
            QuestionOption::new("Skip this file", "Exclude from commit and continue"),
        ];

        let question_rows = 1 + // question line
                           (self.question_multi as usize) + // extra for multi hint
                           options.len() * 2 + // each option: label + description
                           3; // borders + footer
        let question_h = (question_rows as u16).min(top_area.height.saturating_sub(10));
        let catalog_h = top_area.height.saturating_sub(question_h + 1);

        let question_area = Rect {
            x: top_area.x,
            y: top_area.y,
            width: left_w,
            height: question_h,
        };
        let question_inner = card(buf, question_area, th, "Question");
        QuestionCard::new()
            .question("Ready to commit ai_compose.rs?")
            .options(&options[..])
            .multi(self.question_multi)
            .recommended(Some(1))
            .focused(self.focus.is(Id::Question))
            .theme(th)
            .render(question_inner, buf, &mut self.question);

        // mentions catalog below
        let catalog_area = Rect {
            x: top_area.x,
            y: question_area.bottom() + 1,
            width: left_w,
            height: catalog_h,
        };
        // the picker's own frame is the box; a title row above it names the demo
        put(
            buf,
            catalog_area.x + 1,
            catalog_area.y,
            "Mention picker (static)",
            catalog_area.width.saturating_sub(2),
            st(th.text, th.background).add_modifier(Modifier::BOLD),
        );
        let catalog_inner = Rect {
            y: catalog_area.y + 1,
            height: catalog_area.height.saturating_sub(1),
            ..catalog_area
        };

        // show static mention picker as a catalog
        let catalog_items = [
            MentionItem::new("ai_compose.rs", MentionKind::File)
                .detail("This module")
                .recent(true),
            MentionItem::new("ai_compose page", MentionKind::File)
                .detail("Showcase")
                .recent(true),
            MentionItem::new("WIDGET_CONTRACT", MentionKind::File).detail("Rules"),
            MentionItem::new("src/widgets/", MentionKind::Dir),
            MentionItem::new("SlashMenu", MentionKind::Symbol),
            MentionItem::new("HarnessStatus", MentionKind::Symbol),
            MentionItem::new("docs.rs/ratatui", MentionKind::Url),
            MentionItem::new("Main", MentionKind::Agent),
        ];

        let mut catalog_state = MentionPickerState::default();
        catalog_state.open = true;
        // a zero-height anchor at the top of the screen → placed below, clamped into the box
        MentionPicker::new()
            .items(&catalog_items)
            .anchor(Rect {
                x: catalog_inner.x,
                y: 0,
                width: catalog_inner.width,
                height: 0,
            })
            .width(catalog_inner.width)
            .max_rows(catalog_inner.height.saturating_sub(2))
            .theme(th)
            .render(catalog_inner, buf, &mut catalog_state);

        // right: plan above queue
        let right_x = top_area.x + left_w + 2;
        let plan_h = (top_area.height * 3) / 5;
        let queue_h = top_area.height.saturating_sub(plan_h + 1);

        let plan_area = Rect {
            x: right_x,
            y: top_area.y,
            width: right_w,
            height: plan_h,
        };
        let plan_inner = card(buf, plan_area, th, "Plan");

        // animate tasks progressing
        let elapsed = ctx.elapsed();
        let tick_dur = 2.0;
        let current_tick = (elapsed / tick_dur).floor();
        if current_tick > self.task_tick {
            self.task_tick = current_tick;
        }

        let task_idx = (current_tick as usize) % 9;
        let mut tasks = [
            TaskState::Pending,
            TaskState::Pending,
            TaskState::Pending,
            TaskState::Pending,
            TaskState::Pending,
            TaskState::Pending,
            TaskState::Pending,
            TaskState::Blocked,
            TaskState::Dropped,
        ];
        tasks[..task_idx.min(6)].fill(TaskState::Done);
        if task_idx == 6 {
            tasks[6] = TaskState::InProgress;
        } else if task_idx > 6 {
            tasks[6] = TaskState::Done;
        }

        let phases = [
            PlanPhase::new("Foundation")
                .task("Read contract", tasks[0])
                .task("Core types", tasks[1])
                .task("Theme integration", tasks[2]),
            PlanPhase::new("Widgets")
                .task("Slash menu", tasks[3])
                .task("Mention picker", tasks[4])
                .task("Question card", tasks[5]),
            PlanPhase::new("Polish")
                .task("Screenshot verification", tasks[6])
                .task("Edge cases", tasks[7])
                .task("Performance", tasks[8]),
        ];

        PlanView::new()
            .title("Build tuile")
            .phases(&phases)
            .now(ctx.now)
            .theme(th)
            .render(plan_inner, buf, &mut self.plan);

        let queue_area = Rect {
            x: right_x,
            y: plan_area.bottom() + 1,
            width: right_w,
            height: queue_h,
        };
        let queue_inner = card(buf, queue_area, th, "Queue");

        MessageQueue::new()
            .messages(&self.messages)
            .theme(th)
            .render(queue_inner, buf, &mut self.queue);

        self.draw_popups(area, composer_area, buf, th);
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        use tuile::crossterm::event::{Event as CEvent, KeyCode};

        // popups take keys first; printable keys/Backspace also land in the composer so the
        // field shows the partial token being typed. Only the trailing token is rewritten.
        fn set_token(composer: &mut ComposerState, token: &str) {
            let line = composer.editor.lines.first().cloned().unwrap_or_default();
            let start = line.rfind(char::is_whitespace).map_or(0, |i| i + 1);
            let text = format!("{}{token}", &line[..start]);
            composer.editor.cursor = (0, text.len());
            composer.editor.lines = vec![text];
        }
        if self.slash_menu.open {
            if let CEvent::Key(k) = ev {
                let out = self.slash_menu.handle_key(*k);
                if !out.is_ignored() {
                    if matches!(k.code, KeyCode::Char(_) | KeyCode::Backspace) {
                        set_token(&mut self.composer, &format!("/{}", self.slash_menu.query));
                    }
                    if let Some(cmd) = self.slash_menu.take_selected() {
                        set_token(&mut self.composer, &format!("/{cmd} "));
                        ctx.notify(format!("Command: /{cmd}"), Variant::Default);
                    }
                    return out;
                }
            } else if let CEvent::Mouse(m) = ev {
                return self.slash_menu.handle_mouse(*m);
            }
        }

        if self.mention_picker.open {
            if let CEvent::Key(k) = ev {
                let out = self.mention_picker.handle_key(*k);
                if !out.is_ignored() {
                    if matches!(k.code, KeyCode::Char(_) | KeyCode::Backspace) {
                        set_token(
                            &mut self.composer,
                            &format!("@{}", self.mention_picker.query),
                        );
                    }
                    if let Some(label) = self.mention_picker.take_selected() {
                        set_token(&mut self.composer, &format!("@{label} "));
                        ctx.notify(format!("Mention: @{label}"), Variant::Default);
                    }
                    return out;
                }
            } else if let CEvent::Mouse(m) = ev {
                return self.mention_picker.handle_mouse(*m);
            }
        }

        // key events
        if let CEvent::Key(k) = ev {
            if !is_press(k) {
                return Outcome::Ignored;
            }

            // page bindings (never steal letters from the composer)
            let typing = self.focus.is(Id::Composer);
            if k.code == KeyCode::Char('m') && !typing {
                self.mode = match self.mode {
                    HarnessMode::Plan => HarnessMode::Act,
                    HarnessMode::Act => HarnessMode::Ask,
                    HarnessMode::Ask => HarnessMode::Auto,
                    HarnessMode::Auto => HarnessMode::Plan,
                };
                self.mode_from = Some((self.mode, ctx.now));
                return Outcome::Consumed;
            }

            if k.code == KeyCode::Char('q') && !typing {
                self.question_multi = !self.question_multi;
                self.question.selected = vec![false; 3];
                return Outcome::Consumed;
            }

            if k.code == KeyCode::Tab || k.code == KeyCode::BackTab {
                if k.code == KeyCode::Tab {
                    self.focus.next();
                } else {
                    self.focus.prev();
                }
                return Outcome::Consumed;
            }

            // route to focused widget
            if self.focus.is(Id::Composer) {
                let out = self.composer.handle_key(*k);
                if let Some(text) = self.composer.take_submitted() {
                    let when = format!("+{}s", self.messages.len() + 1);
                    self.messages.push(QueuedMessage::new(text, when));
                    return Outcome::Changed;
                }

                // a `/` at the very start or an `@` token anywhere opens a popup whose query is
                // the current token (the text after the trigger char)
                let text = self.composer.editor.lines.join("\n");
                let token_start = text.rfind(char::is_whitespace).map_or(0, |i| i + 1);
                let token = &text[token_start..];
                if token_start == 0 && text.starts_with('/') {
                    self.slash_menu.open = true;
                    self.slash_menu.query = text[1..].to_string();
                } else if let Some(q) = token.strip_prefix('@') {
                    self.mention_picker.open = true;
                    self.mention_picker.query = q.to_string();
                }

                return out;
            } else if self.focus.is(Id::Question) {
                let out = self.question.handle_key(*k);
                if let Some(answer) = self.question.take_answer() {
                    ctx.notify(format!("Answered: {:?}", answer), Variant::Success);
                    self.question_armed = Some(ctx.now);
                    return Outcome::Changed;
                }
                if self.question.take_cancelled() {
                    ctx.notify("Cancelled", Variant::Default);
                    self.question_armed = Some(ctx.now);
                    return Outcome::Changed;
                }
                return out;
            } else if self.focus.is(Id::Plan) {
                return self.plan.handle_key(*k);
            } else if self.focus.is(Id::Queue) {
                let out = self.queue.handle_key(*k);
                if let Some(idx) = self.queue.take_removed() {
                    if idx < self.messages.len() {
                        self.messages.remove(idx);
                        ctx.notify("Removed from queue", Variant::Default);
                    }
                    return Outcome::Changed;
                }
                return out;
            } else if self.focus.is(Id::Suggestions) {
                let out = self.suggestions.handle_key(*k);
                if let Some(idx) = self.suggestions.take_activated() {
                    let items = [
                        "Run tests",
                        "Explain the diff",
                        "Commit",
                        "Write docs",
                        "Open PR",
                    ];
                    if let Some(text) = items.get(idx) {
                        self.composer.editor.lines = vec![text.to_string()];
                        self.composer.editor.cursor = (0, text.len());
                        ctx.notify(format!("Suggestion: {}", text), Variant::Default);
                    }
                    return Outcome::Changed;
                }
                return out;
            }
        }

        // mouse events
        if let CEvent::Mouse(m) = ev {
            // check question
            let out = self.question.handle_mouse(*m);
            if out.is_consumed() || out.is_changed() {
                self.focus.set(Id::Question);
                if let Some(answer) = self.question.take_answer() {
                    ctx.notify(format!("Answered: {:?}", answer), Variant::Success);
                    self.question_armed = Some(ctx.now);
                    return Outcome::Changed;
                }
                return out;
            }

            // check plan
            let out = self.plan.handle_mouse(*m);
            if out.is_consumed() || out.is_changed() {
                self.focus.set(Id::Plan);
                return out;
            }

            // check queue
            let out = self.queue.handle_mouse(*m);
            if out.is_consumed() || out.is_changed() {
                self.focus.set(Id::Queue);
                if let Some(idx) = self.queue.take_removed() {
                    if idx < self.messages.len() {
                        self.messages.remove(idx);
                        ctx.notify("Removed from queue", Variant::Default);
                    }
                    return Outcome::Changed;
                }
                return out;
            }

            // check suggestions
            let out = self.suggestions.handle_mouse(*m);
            if out.is_consumed() || out.is_changed() {
                self.focus.set(Id::Suggestions);
                if let Some(idx) = self.suggestions.take_activated() {
                    let items = [
                        "Run tests",
                        "Explain the diff",
                        "Commit",
                        "Write docs",
                        "Open PR",
                    ];
                    if let Some(text) = items.get(idx) {
                        self.composer.editor.lines = vec![text.to_string()];
                        self.composer.editor.cursor = (0, text.len());
                        ctx.notify(format!("Suggestion: {}", text), Variant::Default);
                    }
                    return Outcome::Changed;
                }
                return out;
            }

            // check attachments
            let out = self.attachments_state.handle_mouse(*m);
            if out.is_consumed() || out.is_changed() {
                self.focus.set(Id::Composer);
                return out;
            }

            // check composer
            let out = self.composer.handle_mouse(*m);
            if out.is_consumed() || out.is_changed() {
                self.focus.set(Id::Composer);
                if let Some(text) = self.composer.take_submitted() {
                    let when = format!("+{}s", self.messages.len() + 1);
                    self.messages.push(QueuedMessage::new(text, when));
                    return Outcome::Changed;
                }
                return out;
            }
        }

        Outcome::Ignored
    }

    fn animating(&self, now: Instant) -> bool {
        // mode badge transition
        if let Some((_mode, start)) = self.mode_from
            && now.saturating_duration_since(start).as_secs_f32() < 0.25
        {
            return true;
        }

        // question re-arm
        if let Some(armed) = self.question_armed
            && now.saturating_duration_since(armed).as_secs_f32() < 1.5
        {
            return true;
        }

        // spinners in status and plan
        true
    }

    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("Tab", "Focus"),
            ("/ @", "Commands / mentions"),
            ("m", "Mode"),
            ("q", "Multi-select"),
            ("Enter", "Send / answer"),
        ]
    }
}
