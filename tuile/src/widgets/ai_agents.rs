//! AI harness: agent trees and lanes, token/cost meters, context map, sessions, model picker.
//!
//! ```no_run
//! use tuile::prelude::*;
//! use tuile::widgets::ai_agents::*;
//! # let area = Rect::new(0, 0, 80, 24);
//! # let mut buf = Buffer::empty(area);
//! # let now = Instant::now();
//! let mut state = AgentTreeState::default();
//! state.root = Some(AgentNode::new("Main", "opus", AgentStatus::Running));
//! AgentTree::new().now(now).render(area, &mut buf, &mut state);
//! ```

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::widgets::{StatefulWidget, Widget};

use crate::anim::{self, Easing, Tween};
use crate::core::{Hit, HitBox, Interactive, MinSize, Outcome, is_press, wheel_delta};
use crate::draw::{self, Border, bold, fill, hbar, put, put_right, st, truncate};
use crate::fuzzy;
use crate::theme::{self, Rgb, Theme};
use crate::widgets::ai::fmt_tokens;
use crate::widgets::charts::{SparkChart, SparkStyle};
use crate::widgets::spinner::spinners;
use crate::widgets::{Scrollbar, ScrollbarState};

// Duration formatting

/// Format duration for UI: `850ms`, `1.2s`, `12.4s`, `1m 03s`, `2h 05m`.
pub fn fmt_duration(d: Duration) -> String {
    let ms = d.as_millis();
    let secs = d.as_secs();
    if ms < 1000 {
        format!("{ms}ms")
    } else if secs < 60 {
        format!("{:.1}s", d.as_secs_f32())
    } else if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        format!("{}m {:02}s", m, s)
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("{}h {:02}m", h, m)
    }
}

/// Format USD: `$0.0042`, `$0.42`, `$12.30`.
pub fn fmt_usd(v: f32) -> String {
    if v < 0.01 {
        format!("${:.4}", v)
    } else if v < 10.0 {
        format!("${:.2}", v)
    } else {
        format!("${:.0}", v)
    }
}

// ElapsedTimer

/// Elapsed time display with pulsing dot when running: `● 1m 23s`.
pub struct ElapsedTimer {
    since: Option<Instant>,
    now: Option<Instant>,
    running: bool,
    label: Option<String>,
    theme: Option<Theme>,
}

impl ElapsedTimer {
    pub fn new() -> Self {
        Self {
            since: None,
            now: None,
            running: false,
            label: None,
            theme: None,
        }
    }

    pub fn since(mut self, s: Instant) -> Self {
        self.since = Some(s);
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn running(mut self, r: bool) -> Self {
        self.running = r;
        self
    }

    pub fn label(mut self, l: &str) -> Self {
        self.label = Some(l.to_string());
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Default for ElapsedTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ElapsedTimer {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme.unwrap_or_else(theme::current);
        let elapsed = match (self.since, self.now) {
            (Some(s), Some(n)) => n.saturating_duration_since(s),
            _ => Duration::ZERO,
        };
        let dur_str = fmt_duration(elapsed);

        let dot_color = if self.running {
            let phase = anim::since(self.now.unwrap_or_else(Instant::now));
            let t = anim::pulse(phase, 1.6);
            th.primary.blend(th.text_muted, t)
        } else {
            th.text_muted
        };

        let mut x = area.x;
        put(buf, x, area.y, "●", 1, st(dot_color, th.background));
        x += 2;

        if let Some(lbl) = &self.label {
            put(
                buf,
                x,
                area.y,
                lbl,
                area.width.saturating_sub(x - area.x),
                st(th.text, th.background),
            );
            x += lbl.len() as u16 + 1;
        }

        put(
            buf,
            x,
            area.y,
            &dur_str,
            area.width.saturating_sub(x - area.x),
            st(th.text, th.background),
        );
    }
}

// AgentStatus

/// Agent execution status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AgentStatus {
    #[default]
    Idle,
    Running,
    Waiting,
    Done,
    Failed,
    Parked,
}

impl AgentStatus {
    pub fn glyph(self) -> &'static str {
        match self {
            AgentStatus::Idle => "○",
            AgentStatus::Running => "", // spinner
            AgentStatus::Waiting => "◔",
            AgentStatus::Done => "✓",
            AgentStatus::Failed => "✗",
            AgentStatus::Parked => "◌",
        }
    }

    pub fn color(self, th: &Theme) -> Rgb {
        match self {
            AgentStatus::Idle => th.text_muted,
            AgentStatus::Running => th.primary,
            AgentStatus::Waiting => th.warning,
            AgentStatus::Done => th.success,
            AgentStatus::Failed => th.error,
            AgentStatus::Parked => th.text_disabled,
        }
    }
}

// AgentNode

/// One node in the agent tree.
#[derive(Clone, Debug)]
pub struct AgentNode {
    pub name: String,
    pub model: String,
    pub status: AgentStatus,
    pub task: String,
    pub tokens: u32,
    pub elapsed: Duration,
    pub children: Vec<AgentNode>,
}

impl AgentNode {
    pub fn new(name: &str, model: &str, status: AgentStatus) -> Self {
        Self {
            name: name.to_string(),
            model: model.to_string(),
            status,
            task: String::new(),
            tokens: 0,
            elapsed: Duration::ZERO,
            children: Vec::new(),
        }
    }

    pub fn task(mut self, t: &str) -> Self {
        self.task = t.to_string();
        self
    }

    pub fn tokens(mut self, n: u32) -> Self {
        self.tokens = n;
        self
    }

    pub fn elapsed(mut self, d: Duration) -> Self {
        self.elapsed = d;
        self
    }

    pub fn child(mut self, c: AgentNode) -> Self {
        self.children.push(c);
        self
    }

    pub fn children(mut self, cs: Vec<AgentNode>) -> Self {
        self.children = cs;
        self
    }
}

// AgentTreeState

/// Flattened tree row for rendering.
#[derive(Clone, Debug)]
struct TreeRow {
    path: Vec<usize>,
    depth: usize,
    is_last: Vec<bool>,
}

/// State for agent tree: cursor, expansion, scroll.
#[derive(Clone, Debug, Default)]
pub struct AgentTreeState {
    pub root: Option<AgentNode>,
    pub cursor: usize,
    pub expanded: Vec<Vec<usize>>, // paths to expanded nodes
    pub offset: usize,
    pub hit: HitBox,
    scrollbar: ScrollbarState,
}

impl AgentTreeState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn animating(&self, _now: Instant) -> bool {
        false
    }

    /// Nodes currently visible (collapsed subtrees excluded); rows = this × 1 or 2 with tasks.
    pub fn visible_len(&self) -> usize {
        self.visible_rows().len()
    }

    fn visible_rows(&self) -> Vec<TreeRow> {
        let Some(ref root) = self.root else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        self.walk(root, &Vec::new(), 0, &Vec::new(), &mut rows);
        rows
    }

    fn walk(
        &self,
        node: &AgentNode,
        path: &[usize],
        depth: usize,
        is_last: &[bool],
        out: &mut Vec<TreeRow>,
    ) {
        out.push(TreeRow {
            path: path.to_vec(),
            depth,
            is_last: is_last.to_vec(),
        });

        let expanded = self.expanded.iter().any(|p| p == path);
        if expanded && !node.children.is_empty() {
            for (i, child) in node.children.iter().enumerate() {
                let mut child_path = path.to_vec();
                child_path.push(i);
                let mut child_is_last = is_last.to_vec();
                child_is_last.push(i + 1 == node.children.len());
                self.walk(child, &child_path, depth + 1, &child_is_last, out);
            }
        }
    }

    fn get_node(&self, path: &[usize]) -> Option<&AgentNode> {
        let mut node = self.root.as_ref()?;
        for &i in path {
            node = node.children.get(i)?;
        }
        Some(node)
    }

    fn is_expanded(&self, path: &[usize]) -> bool {
        self.expanded.iter().any(|p| p == path)
    }

    fn toggle_expanded(&mut self, path: &[usize]) {
        if let Some(pos) = self.expanded.iter().position(|p| p == path) {
            self.expanded.remove(pos);
        } else {
            self.expanded.push(path.to_vec());
        }
    }

    fn expand_all(&mut self) {
        self.expanded.clear();
        let Some(root) = &self.root else { return };
        let mut paths = Vec::new();
        Self::collect_paths_recursive(root, &Vec::new(), &mut paths);
        self.expanded = paths;
    }

    fn collect_paths_recursive(node: &AgentNode, path: &[usize], out: &mut Vec<Vec<usize>>) {
        if !node.children.is_empty() {
            out.push(path.to_vec());
            for (i, child) in node.children.iter().enumerate() {
                let mut child_path = path.to_vec();
                child_path.push(i);
                Self::collect_paths_recursive(child, &child_path, out);
            }
        }
    }
    fn collapse_all(&mut self) {
        self.expanded.clear();
    }

    pub fn take_activated(&mut self) -> Option<Vec<usize>> {
        None // would set a flag if needed
    }
}

impl Interactive for AgentTreeState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }
        let rows = self.visible_rows();
        if rows.is_empty() {
            return Outcome::Ignored;
        }

        match key.code {
            KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Outcome::Consumed
            }
            KeyCode::Down => {
                self.cursor = (self.cursor + 1).min(rows.len().saturating_sub(1));
                Outcome::Consumed
            }
            KeyCode::Left => {
                if self.cursor < rows.len() {
                    let path = &rows[self.cursor].path;
                    if self.is_expanded(path) {
                        self.toggle_expanded(path);
                    }
                }
                Outcome::Consumed
            }
            KeyCode::Right => {
                if self.cursor < rows.len() {
                    let path = &rows[self.cursor].path;
                    if let Some(node) = self.get_node(path)
                        && !node.children.is_empty()
                        && !self.is_expanded(path)
                    {
                        self.toggle_expanded(path);
                    }
                }
                Outcome::Consumed
            }
            KeyCode::Enter => {
                if self.cursor < rows.len() {
                    let path = &rows[self.cursor].path;
                    if let Some(node) = self.get_node(path)
                        && !node.children.is_empty()
                    {
                        self.toggle_expanded(path);
                    }
                }
                Outcome::Consumed
            }
            KeyCode::Char('e') => {
                self.expand_all();
                Outcome::Consumed
            }
            KeyCode::Char('c') => {
                self.collapse_all();
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let mut out = Outcome::Ignored;
        if let Some(d) = wheel_delta(&m)
            && self.hit.hover
        {
            let before = self.offset;
            self.offset = (self.offset as i32 - d * 3).max(0) as usize;
            out |= Outcome::changed_if(before != self.offset);
        }
        let hit_out = match self.hit.mouse(&m) {
            Hit::HoverChanged => Outcome::Consumed,
            Hit::None => Outcome::Ignored,
            _ => Outcome::Consumed,
        };
        out | self.scrollbar.handle_mouse(m) | hit_out
    }
}

// AgentTree

/// Agent tree widget with expansion, cursor, tasks.
pub struct AgentTree {
    show_tasks: bool,
    focused: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl AgentTree {
    pub fn new() -> Self {
        Self {
            show_tasks: false,
            focused: false,
            now: None,
            theme: None,
        }
    }

    pub fn show_tasks(mut self, v: bool) -> Self {
        self.show_tasks = v;
        self
    }

    /// Cursor row uses `th.cursor_bg` when focused, `th.cursor_blurred_bg` otherwise.
    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }

    /// Rows per agent: the task line doubles it.
    fn row_height(&self) -> usize {
        if self.show_tasks { 2 } else { 1 }
    }
}

impl MinSize for AgentTree {
    /// One agent row, plus its task line when tasks are shown.
    fn min_size(&self) -> (u16, u16) {
        (8, self.row_height() as u16)
    }
}

impl Default for AgentTree {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for AgentTree {
    type State = AgentTreeState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.unwrap_or_else(theme::current);
        state.hit.set_area(area);

        let rows = state.visible_rows();
        if rows.is_empty() {
            return;
        }
        if draw::refuse(buf, area, self.min_size(), th.text_disabled) {
            state.hit.set_area(Rect::default());
            return;
        }

        let row_height = self.row_height();
        let visible = (area.height as usize / row_height).max(1);

        if state.cursor >= rows.len() {
            state.cursor = rows.len().saturating_sub(1);
        }

        if state.cursor >= state.offset + visible {
            state.offset = state.cursor + 1 - visible;
        } else if state.cursor < state.offset {
            state.offset = state.cursor;
        }

        let scrollbar_area = Rect {
            x: area.right().saturating_sub(1),
            y: area.y,
            width: 1,
            height: area.height,
        };
        state.scrollbar.offset = state.offset * row_height;

        Scrollbar::vertical(rows.len() * row_height, area.height as usize)
            .offset(state.offset * row_height)
            .theme(&th)
            .render(scrollbar_area, buf, &mut state.scrollbar);

        let content_width = area.width.saturating_sub(1);
        let mut y = area.y;

        for (i, row) in rows.iter().enumerate().skip(state.offset) {
            if y >= area.bottom() {
                break;
            }

            let node = state.get_node(&row.path);
            let Some(node) = node else { continue };

            let is_cursor = i == state.cursor;
            let bg = if !is_cursor {
                th.background
            } else if self.focused {
                th.cursor_bg
            } else {
                th.cursor_blurred_bg
            };

            if row_height == 2 {
                fill(buf, Rect::new(area.x, y, content_width, 2), bg);
            } else {
                fill(buf, Rect::new(area.x, y, content_width, 1), bg);
            }

            let mut x = area.x;

            // Tree guides
            for (d, &last) in row.is_last.iter().enumerate() {
                if d > 0 {
                    let sym = if last && d == row.depth {
                        "└"
                    } else if d == row.depth {
                        "├"
                    } else if !last {
                        "│"
                    } else {
                        " "
                    };
                    put(buf, x, y, sym, 1, st(th.border, bg));
                    x += 2;
                }
            }

            // Expand/collapse marker
            if !node.children.is_empty() {
                let marker = if state.is_expanded(&row.path) {
                    "▾"
                } else {
                    "▸"
                };
                put(buf, x, y, marker, 1, st(th.text_muted, bg));
                x += 2;
            } else {
                x += 2;
            }

            // Status glyph/spinner
            if node.status == AgentStatus::Running {
                let phase = anim::since(self.now.unwrap_or_else(Instant::now));
                let frame = spinners::DOTS.frame(phase);
                put(buf, x, y, frame, 1, st(node.status.color(&th), bg));
            } else if node.status == AgentStatus::Waiting {
                let phase = anim::since(self.now.unwrap_or_else(Instant::now));
                let t = anim::pulse(phase, 1.6);
                let c = th.warning.blend(th.text_muted, t);
                put(buf, x, y, node.status.glyph(), 1, st(c, bg));
            } else {
                put(
                    buf,
                    x,
                    y,
                    node.status.glyph(),
                    1,
                    st(node.status.color(&th), bg),
                );
            }
            x += 2;

            // Name
            let name_width = 16.min(content_width.saturating_sub(x - area.x));
            put(buf, x, y, &node.name, name_width, bold(st(th.text, bg)));
            x += name_width.min(node.name.len() as u16) + 1;

            // Model chip
            let model_str = truncate(&node.model, 10);
            let chip_width = model_str.len() as u16 + 2;
            if x + chip_width <= area.x + content_width {
                fill(buf, Rect::new(x, y, chip_width, 1), th.surface);
                put(
                    buf,
                    x + 1,
                    y,
                    &model_str,
                    chip_width.saturating_sub(2),
                    st(th.text_muted, th.surface),
                );
                x += chip_width + 1;
            }

            // Right-aligned: tokens · duration
            let info = format!(
                "{} · {}",
                fmt_tokens(node.tokens),
                fmt_duration(node.elapsed)
            );
            let info_w = info.len() as u16;
            if x + info_w <= area.x + content_width {
                let rx = area.x + content_width - info_w;
                if rx > x {
                    put(buf, rx, y, &info, info_w, st(th.text_muted, bg));
                }
            }

            y += 1;

            // Task row
            if self.show_tasks && !node.task.is_empty() && y < area.bottom() {
                let task_x = area.x + (row.depth as u16 * 2) + 4;
                let task_width = content_width.saturating_sub(task_x - area.x);
                let task_str = truncate(&node.task, task_width as usize);
                put(buf, task_x, y, &task_str, task_width, st(th.text_muted, bg));
                y += 1;
            }
        }
    }
}

// AgentLanes

/// One span in a gantt lane.
#[derive(Clone, Debug)]
pub struct LaneSpan {
    pub start: f32,
    pub end: Option<f32>,
    pub status: AgentStatus,
    pub label: String,
}

impl LaneSpan {
    pub fn new(start: f32, end: Option<f32>, status: AgentStatus, label: &str) -> Self {
        Self {
            start,
            end,
            status,
            label: label.to_string(),
        }
    }
}

/// One lane in the gantt chart.
#[derive(Clone, Debug)]
pub struct Lane {
    pub name: String,
    pub spans: Vec<LaneSpan>,
}

impl Lane {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            spans: Vec::new(),
        }
    }

    pub fn span(mut self, s: LaneSpan) -> Self {
        self.spans.push(s);
        self
    }

    pub fn spans(mut self, ss: Vec<LaneSpan>) -> Self {
        self.spans = ss;
        self
    }
}

/// Gantt chart for agent activity lanes.
pub struct AgentLanes<'a> {
    lanes: &'a [Lane],
    clock: f32,
    window: f32,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl<'a> AgentLanes<'a> {
    pub fn new(lanes: &'a [Lane]) -> Self {
        Self {
            lanes,
            clock: 0.0,
            window: 30.0,
            now: None,
            theme: None,
        }
    }

    pub fn clock(mut self, c: f32) -> Self {
        self.clock = c;
        self
    }

    pub fn window(mut self, w: f32) -> Self {
        self.window = w;
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl MinSize for AgentLanes<'_> {
    /// The name column plus a 10-cell track, and 2 rows (ruler + one lane).
    fn min_size(&self) -> (u16, u16) {
        (self.name_width() + 10, 2)
    }
}

impl AgentLanes<'_> {
    /// Width reserved for lane names, capped so a long name cannot eat the track.
    fn name_width(&self) -> u16 {
        let max_name = self.lanes.iter().map(|l| l.name.len()).max().unwrap_or(0);
        (max_name.min(14) + 1) as u16
    }
}

impl Widget for AgentLanes<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.lanes.is_empty() {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        if draw::refuse(buf, area, self.min_size(), th.text_disabled) {
            return;
        }

        let name_width = self.name_width();
        let track_width = area.width - name_width;

        // Auto-scroll: keep clock at 80% position
        let scroll_threshold = self.window * 0.8;
        let view_start = if self.clock > scroll_threshold {
            self.clock - scroll_threshold
        } else {
            0.0
        };
        let view_end = view_start + self.window;

        // Lanes
        let lane_height = area.height.saturating_sub(1);
        let lanes_per_row = if lane_height > 0 {
            (lane_height as usize).min(self.lanes.len())
        } else {
            0
        };

        for (i, lane) in self.lanes.iter().take(lanes_per_row).enumerate() {
            let y = area.y + i as u16;

            // Alternating bg
            if i % 2 == 1 {
                fill(buf, Rect::new(area.x, y, area.width, 1), th.surface);
            }

            // Name
            let name_str = truncate(&lane.name, name_width.saturating_sub(1) as usize);
            put(
                buf,
                area.x,
                y,
                &name_str,
                name_width,
                st(
                    th.text,
                    if i % 2 == 1 {
                        th.surface
                    } else {
                        th.background
                    },
                ),
            );

            // Track
            let track_x = area.x + name_width;
            for span in &lane.spans {
                let span_end = span.end.unwrap_or(self.clock);
                if span_end < view_start || span.start > view_end {
                    continue;
                }

                let cell_start =
                    ((span.start - view_start) / self.window * track_width as f32).max(0.0) as u16;
                let cell_end = ((span_end - view_start) / self.window * track_width as f32)
                    .min(track_width as f32) as u16;
                let span_width = cell_end.saturating_sub(cell_start).max(1);

                let span_color = span.status.color(&th);
                let span_bg = if span.end.is_none() {
                    // Pulsing for open spans
                    let phase = anim::since(self.now.unwrap_or_else(Instant::now));
                    let t = anim::pulse(phase, 1.2);
                    span_color.blend(th.background, t * 0.3)
                } else {
                    span_color
                };

                fill(
                    buf,
                    Rect::new(track_x + cell_start, y, span_width, 1),
                    span_bg,
                );

                // Label if it fits
                if span_width > 2 && !span.label.is_empty() {
                    let label_str = truncate(&span.label, span_width.saturating_sub(2) as usize);
                    let label_color = span_bg.text_on(1.0);
                    put(
                        buf,
                        track_x + cell_start + 1,
                        y,
                        &label_str,
                        span_width.saturating_sub(2),
                        st(label_color, span_bg),
                    );
                }
            }
        }

        // Time axis
        let axis_y = area.bottom().saturating_sub(1);
        fill(buf, Rect::new(area.x, axis_y, area.width, 1), th.background);

        // Ticks every 5s
        let tick_interval = 5.0;
        let mut t = (view_start / tick_interval).floor() * tick_interval;
        while t <= view_end {
            if t >= view_start {
                let tick_x = ((t - view_start) / self.window * track_width as f32) as u16;
                let tick_label = format!("{}s", t as i32);
                put(
                    buf,
                    area.x + name_width + tick_x,
                    axis_y,
                    &tick_label,
                    track_width.saturating_sub(tick_x),
                    st(th.text_muted, th.background),
                );
            }
            t += tick_interval;
        }

        // Clock marker
        if self.clock >= view_start && self.clock <= view_end {
            let marker_x = area.x
                + name_width
                + ((self.clock - view_start) / self.window * track_width as f32) as u16;
            for i in 0..lane_height.min(lanes_per_row as u16) {
                put(
                    buf,
                    marker_x,
                    area.y + i,
                    "│",
                    1,
                    st(
                        th.accent,
                        if i % 2 == 1 {
                            th.surface
                        } else {
                            th.background
                        },
                    ),
                );
            }
        }
    }
}

// TokenBreakdown & TokenMeter

/// Token usage breakdown.
#[derive(Clone, Copy, Debug, Default)]
pub struct TokenBreakdown {
    pub input: u32,
    pub output: u32,
    pub cache_read: u32,
    pub cache_write: u32,
}

impl TokenBreakdown {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn total(&self) -> u32 {
        self.input + self.output + self.cache_read + self.cache_write
    }
}

/// Token usage meter with stacked bar and legend.
pub struct TokenMeter {
    breakdown: TokenBreakdown,
    compact: bool,
    theme: Option<Theme>,
}

impl TokenMeter {
    pub fn new(b: TokenBreakdown) -> Self {
        Self {
            breakdown: b,
            compact: false,
            theme: None,
        }
    }

    pub fn compact(mut self, c: bool) -> Self {
        self.compact = c;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for TokenMeter {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme.unwrap_or_else(theme::current);

        if area.height == 0 {
            return;
        }

        let b = self.breakdown;
        let total = b.total();

        if total == 0 {
            return;
        }

        // Bar row
        let bar_width = area.width;
        let segments = [
            (b.input, th.primary, "input"),
            (b.output, th.accent, "output"),
            (b.cache_read, th.success, "cache read"),
            (b.cache_write, th.secondary, "cache write"),
        ];

        // Allocate cells ensuring >=1 per non-zero segment
        let mut cells = [0u16; 4];
        let _remaining = bar_width;

        for (i, &(count, _, _)) in segments.iter().enumerate() {
            if count > 0 {
                let frac = count as f32 / total as f32;
                let c = (frac * bar_width as f32).round() as u16;
                cells[i] = c.max(1);
            }
        }

        // Adjust to exactly bar_width
        let sum: u16 = cells.iter().sum();
        if sum > bar_width {
            for _ in 0..(sum - bar_width) {
                if let Some((idx, _)) = cells.iter().enumerate().max_by_key(|&(_, c)| c)
                    && cells[idx] > 1
                {
                    cells[idx] -= 1;
                }
            }
        } else if sum < bar_width {
            for _ in 0..(bar_width - sum) {
                if let Some((idx, _)) = cells.iter().enumerate().max_by_key(|&(_, c)| c) {
                    cells[idx] += 1;
                }
            }
        }

        // Draw bar
        let mut x = area.x;
        for (i, &(count, color, _)) in segments.iter().enumerate() {
            if count > 0 {
                fill(buf, Rect::new(x, area.y, cells[i], 1), color);
                x += cells[i];
            }
        }

        if self.compact {
            // Just show total on the right
            let total_str = fmt_tokens(total);
            put_right(
                buf,
                Rect::new(area.x, area.y, area.width, 1),
                &total_str,
                st(th.text, th.background),
            );
        } else if area.height > 1 {
            // Legend row
            let mut leg_x = area.x;
            let leg_y = area.y + 1;

            for &(count, color, label) in &segments {
                if count > 0 {
                    let entry = format!("■ {} {}  ", label, fmt_tokens(count));
                    if leg_x + entry.len() as u16 <= area.right() {
                        put(buf, leg_x, leg_y, "■", 1, st(color, th.background));
                        put(
                            buf,
                            leg_x + 2,
                            leg_y,
                            &format!("{} {}", label, fmt_tokens(count)),
                            area.width,
                            st(th.text, th.background),
                        );
                        leg_x += entry.len() as u16;
                    }
                }
            }

            // Total on the right
            let total_str = format!("total {}", fmt_tokens(total));
            let total_x = area.right().saturating_sub(total_str.len() as u16);
            if total_x > leg_x {
                put(
                    buf,
                    total_x,
                    leg_y,
                    &total_str,
                    total_str.len() as u16,
                    st(th.text, th.background),
                );
            }
        }
    }
}

// CostMeter

/// Cost meter state with animated spending.
#[derive(Clone, Debug)]
pub struct CostMeterState {
    pub spent: Tween,
}

impl Default for CostMeterState {
    fn default() -> Self {
        Self {
            spent: Tween::new(0.0),
        }
    }
}

impl CostMeterState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.spent.active(now)
    }
}

/// Cost meter with budget bar and rate.
pub struct CostMeter {
    spent: Option<f32>,
    budget: Option<f32>,
    rate_per_min: Option<f32>,
    dur: Option<Duration>,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl CostMeter {
    pub fn new() -> Self {
        Self {
            spent: None,
            budget: None,
            rate_per_min: None,
            dur: None,
            now: None,
            theme: None,
        }
    }

    pub fn spent(mut self, s: f32) -> Self {
        self.spent = Some(s);
        self
    }

    pub fn budget(mut self, b: f32) -> Self {
        self.budget = Some(b);
        self
    }

    pub fn rate_per_min(mut self, r: f32) -> Self {
        self.rate_per_min = Some(r);
        self
    }

    pub fn dur(mut self, d: Duration) -> Self {
        self.dur = Some(d);
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Default for CostMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl MinSize for CostMeter {
    /// Minimum size: (8, 2).
    fn min_size(&self) -> (u16, u16) {
        (8, 2)
    }
}

impl StatefulWidget for CostMeter {
    type State = CostMeterState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.unwrap_or_else(theme::current);

        if draw::refuse(buf, area, self.min_size(), th.text_disabled) {
            return;
        }

        let spent = self.spent.unwrap_or(0.0);

        // Update tween
        if let Some(d) = self.dur {
            if let Some(n) = self.now
                && state.spent.target() != spent
            {
                state.spent.go_with(spent, n, d, Easing::OutCubic);
            }
        } else {
            state.spent = Tween::new(spent);
        }

        let current = state.spent.value(self.now.unwrap_or_else(Instant::now));
        if let Some(budget) = self.budget {
            let pct = if budget > 0.0 { current / budget } else { 0.0 };
            let color = if pct > 0.9 {
                th.error
            } else if pct > 0.7 {
                th.warning
            } else {
                th.success
            };

            hbar(
                buf,
                area.x,
                area.y,
                area.width,
                pct.min(1.0),
                color,
                th.panel,
            );
        }

        // Text row
        if area.height > 1 {
            let mut parts = Vec::new();

            parts.push(fmt_usd(current));

            if let Some(budget) = self.budget {
                parts.push(format!(" / {}", fmt_usd(budget)));
            }

            if let Some(rate) = self.rate_per_min {
                parts.push(format!(" · {}/min", fmt_usd(rate)));

                if let Some(budget) = self.budget
                    && rate > 0.0
                    && current < budget
                {
                    let remaining = budget - current;
                    let mins = remaining / rate;
                    parts.push(format!(" · ~{} min left", mins.round() as i32));
                }
            }

            let text = parts.join("");
            put(
                buf,
                area.x,
                area.y + 1,
                &text,
                area.width,
                st(th.text, th.background),
            );
        }
    }
}

// ContextMap

/// One segment in the context map.
#[derive(Clone, Debug)]
pub struct ContextSegment {
    pub label: String,
    pub tokens: u32,
    pub color: Option<Rgb>,
}

impl ContextSegment {
    pub fn new(label: &str, tokens: u32) -> Self {
        Self {
            label: label.to_string(),
            tokens,
            color: None,
        }
    }

    pub fn color(mut self, c: Rgb) -> Self {
        self.color = Some(c);
        self
    }
}

/// Context window usage map.
pub struct ContextMap<'a> {
    segments: &'a [ContextSegment],
    limit: u32,
    hover: Option<usize>,
    compact: bool,
    used_text: Option<String>,
    free_text: Option<String>,
    theme: Option<Theme>,
}

impl<'a> ContextMap<'a> {
    pub fn new(segments: &'a [ContextSegment], limit: u32) -> Self {
        Self {
            segments,
            limit,
            hover: None,
            compact: false,
            used_text: None,
            free_text: None,
            theme: None,
        }
    }

    pub fn hover(mut self, h: Option<usize>) -> Self {
        self.hover = h;
        self
    }

    pub fn compact(mut self, c: bool) -> Self {
        self.compact = c;
        self
    }

    /// Used text as the caller's own formatted string (e.g., `"42% benutzt"` for German).
    /// Overrides the default `"42% used"`.
    pub fn used_text(mut self, s: impl Into<String>) -> Self {
        self.used_text = Some(s.into());
        self
    }

    /// Free text as the caller's own formatted string (e.g., `"libre 58%"` for French).
    /// Overrides the default `"free 58%"`.
    pub fn free_text(mut self, s: impl Into<String>) -> Self {
        self.free_text = Some(s.into());
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Widget for ContextMap<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme.unwrap_or_else(theme::current);

        if area.height == 0 {
            return;
        }

        let palette = [th.secondary, th.accent, th.primary, th.success];

        let used: u32 = self.segments.iter().map(|s| s.tokens).sum();
        let free = self.limit.saturating_sub(used);

        // Bar
        let mut x = area.x;
        for (i, seg) in self.segments.iter().enumerate() {
            if seg.tokens == 0 {
                continue;
            }
            let frac = seg.tokens as f32 / self.limit as f32;
            let mut width = (frac * area.width as f32).round() as u16;
            width = width.max(1).min(area.width.saturating_sub(x - area.x));

            let mut color = seg.color.unwrap_or(palette[i % palette.len()]);
            if self.hover == Some(i) {
                color = color.blend(th.boost, 0.3);
            }

            fill(buf, Rect::new(x, area.y, width, 1), color);
            x += width;
        }

        // Free space
        if x < area.right() {
            fill(buf, Rect::new(x, area.y, area.right() - x, 1), th.surface);
        }

        if self.compact {
            let text = if let Some(t) = &self.used_text {
                t.clone()
            } else {
                let used_pct = if self.limit > 0 {
                    (used as f32 / self.limit as f32 * 100.0) as u32
                } else {
                    0
                };
                format!("{}% used", used_pct)
            };
            put_right(
                buf,
                Rect::new(area.x, area.y, area.width, 1),
                &text,
                st(th.text, th.background),
            );
        } else if area.height > 1 {
            // Legend
            let mut leg_x = area.x;
            let leg_y = area.y + 1;

            for (i, seg) in self.segments.iter().enumerate() {
                if seg.tokens == 0 {
                    continue;
                }
                let pct = if self.limit > 0 {
                    (seg.tokens as f32 / self.limit as f32 * 100.0) as u32
                } else {
                    0
                };
                let entry = format!("■ {} {}%  ", seg.label, pct);

                if leg_x + entry.len() as u16 <= area.right() {
                    let color = seg.color.unwrap_or(palette[i % palette.len()]);
                    put(buf, leg_x, leg_y, "■", 1, st(color, th.background));
                    put(
                        buf,
                        leg_x + 2,
                        leg_y,
                        &format!("{} {}%", seg.label, pct),
                        area.width,
                        st(th.text, th.background),
                    );
                    leg_x += entry.len() as u16;
                }
            }

            // Free
            let free_pct = if self.limit > 0 {
                (free as f32 / self.limit as f32 * 100.0) as u32
            } else {
                0
            };
            let free_entry = format!("free {}%", free_pct);
            let free_x = area.right().saturating_sub(free_entry.len() as u16);
            if free_x > leg_x {
                put(
                    buf,
                    free_x,
                    leg_y,
                    &free_entry,
                    free_entry.len() as u16,
                    st(th.text, th.background),
                );
            }
        }
    }
}

// CompactionBanner

/// Context compaction result banner.
pub struct CompactionBanner {
    from_pct: f32,
    to_pct: f32,
    saved_tokens: u32,
    started: Option<Instant>,
    summary: Option<String>,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl CompactionBanner {
    pub fn new(from: f32, to: f32, saved: u32) -> Self {
        Self {
            from_pct: from,
            to_pct: to,
            saved_tokens: saved,
            started: None,
            summary: None,
            now: None,
            theme: None,
        }
    }

    pub fn started(mut self, s: Instant) -> Self {
        self.started = Some(s);
        self
    }

    pub fn summary(mut self, s: &str) -> Self {
        self.summary = Some(s.to_string());
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl MinSize for CompactionBanner {
    /// Minimum size: (20, 2).
    fn min_size(&self) -> (u16, u16) {
        (20, 2)
    }
}

impl Widget for CompactionBanner {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme.unwrap_or_else(theme::current);

        if draw::refuse(buf, area, self.min_size(), th.text_disabled) {
            return;
        }

        let inner = Border::Round.inner(area);
        Border::Round.draw(buf, area, th.success, th.background);

        if inner.height == 0 {
            return;
        }

        // Title row
        let title = format!(
            "Context compacted  {}% → {}%  (saved {})",
            self.from_pct as u32,
            self.to_pct as u32,
            fmt_tokens(self.saved_tokens)
        );
        put(
            buf,
            inner.x,
            inner.y,
            &title,
            inner.width,
            bold(st(th.text, th.background)),
        );

        // Animated bar
        if inner.height > 1 {
            let elapsed = match (self.started, self.now) {
                (Some(s), Some(n)) => n.saturating_duration_since(s).as_secs_f32(),
                _ => 0.0,
            };
            let anim_dur = 1.2;
            let t = (elapsed / anim_dur).min(1.0);
            let eased = Easing::OutCubic.apply(t);

            let current_pct = self.from_pct + (self.to_pct - self.from_pct) * eased;
            let bar_color = th.warning.blend(th.success, eased);

            hbar(
                buf,
                inner.x,
                inner.y + 1,
                inner.width,
                current_pct / 100.0,
                bar_color,
                th.panel,
            );
        }

        // Summary
        if inner.height > 2
            && let Some(ref s) = self.summary
        {
            let summary_str = truncate(s, inner.width as usize);
            put(
                buf,
                inner.x,
                inner.y + 2,
                &summary_str,
                inner.width,
                st(th.text_muted, th.background),
            );
        }
    }
}

// TurnStats

/// Turn statistics KPI cells.
pub struct TurnStats {
    input: Option<u32>,
    output: Option<u32>,
    cache_hit: Option<f32>,
    cache_text: Option<String>,
    tool_calls: Option<u32>,
    duration: Option<Duration>,
    cost: Option<f32>,
    theme: Option<Theme>,
}

impl TurnStats {
    pub fn new() -> Self {
        Self {
            input: None,
            output: None,
            cache_hit: None,
            cache_text: None,
            tool_calls: None,
            duration: None,
            cost: None,
            theme: None,
        }
    }

    pub fn input(mut self, n: u32) -> Self {
        self.input = Some(n);
        self
    }

    pub fn output(mut self, n: u32) -> Self {
        self.output = Some(n);
        self
    }

    pub fn cache_hit(mut self, f: f32) -> Self {
        self.cache_hit = Some(f);
        self
    }

    /// Cache hit text as the caller's own formatted string (e.g., `"cache 42%"` or `"42% im Cache"`).
    /// Overrides [`Self::cache_hit`], which formats `"42%"`.
    pub fn cache_text(mut self, s: impl Into<String>) -> Self {
        self.cache_text = Some(s.into());
        self
    }

    pub fn tool_calls(mut self, n: u32) -> Self {
        self.tool_calls = Some(n);
        self
    }

    pub fn duration(mut self, d: Duration) -> Self {
        self.duration = Some(d);
        self
    }

    pub fn cost(mut self, c: f32) -> Self {
        self.cost = Some(c);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Default for TurnStats {
    fn default() -> Self {
        Self::new()
    }
}

impl MinSize for TurnStats {
    /// Minimum size: (12, 2).
    fn min_size(&self) -> (u16, u16) {
        (12, 2)
    }
}

impl Widget for TurnStats {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme.unwrap_or_else(theme::current);

        if draw::refuse(buf, area, self.min_size(), th.text_disabled) {
            return;
        }

        let cells = [
            ("input", self.input.map(fmt_tokens), None),
            ("output", self.output.map(fmt_tokens), None),
            (
                "cache",
                if let Some(t) = &self.cache_text {
                    Some(t.clone())
                } else {
                    self.cache_hit.map(|f| format!("{}%", (f * 100.0) as u32))
                },
                self.cache_hit
                    .map(|f| if f > 0.5 { th.success } else { th.warning }),
            ),
            ("tools", self.tool_calls.map(|n| n.to_string()), None),
            ("time", self.duration.map(fmt_duration), None),
            ("cost", self.cost.map(fmt_usd), None),
        ];

        let active: Vec<_> = cells.iter().filter(|(_, v, _)| v.is_some()).collect();
        if active.is_empty() {
            return;
        }

        let cell_width = 12u16;
        let cols = (area.width / cell_width).max(1) as usize;
        let to_show = active.len().min(cols);

        for (i, (label, value, color)) in active.iter().take(to_show).enumerate() {
            let x = area.x + (i as u16 * cell_width);
            if x >= area.right() {
                break;
            }

            let Some(val) = value else { continue };

            // Label row
            put(
                buf,
                x,
                area.y,
                label,
                cell_width,
                st(th.text_muted, th.background),
            );

            // Value row
            let val_color = color.unwrap_or(th.text);
            put(
                buf,
                x,
                area.y + 1,
                val,
                cell_width,
                bold(st(val_color, th.background)),
            );
        }
    }
}

// RateGraph

/// Throughput rate graph.
pub struct RateGraph<'a> {
    values: &'a [f64],
    label: Option<String>,
    max: Option<f64>,
    theme: Option<Theme>,
}

impl<'a> RateGraph<'a> {
    pub fn new(values: &'a [f64]) -> Self {
        Self {
            values,
            label: None,
            max: None,
            theme: None,
        }
    }

    pub fn label(mut self, l: &str) -> Self {
        self.label = Some(l.to_string());
        self
    }

    pub fn max(mut self, m: f64) -> Self {
        self.max = Some(m);
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl MinSize for RateGraph<'_> {
    /// Minimum size: (10, 2).
    fn min_size(&self) -> (u16, u16) {
        (10, 2)
    }
}

impl Widget for RateGraph<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.values.is_empty() {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        if draw::refuse(buf, area, self.min_size(), th.text_disabled) {
            return;
        }

        let current = self.values.last().copied().unwrap_or(0.0);
        let current_str = format!(
            "{} {}",
            current.round() as u32,
            self.label.as_deref().unwrap_or("")
        );

        put(
            buf,
            area.x,
            area.y,
            &current_str,
            area.width,
            bold(st(th.accent, th.background)),
        );

        if area.height > 1 {
            let chart_area = Rect::new(area.x, area.y + 1, area.width, area.height - 1);
            let mut chart = SparkChart::new(self.values)
                .style(SparkStyle::Line)
                .color(th.accent)
                .theme(&th);
            if let Some(m) = self.max {
                chart = chart.max(m);
            }
            chart.render(chart_area, buf);
        }
    }
}

// SessionList

/// One session entry: a title, a when string, and whatever facts the host wants on the detail
/// row. Facts are strings rather than fixed fields because a session's vocabulary belongs to
/// the harness — one counts messages and dollars, another counts steps and tokens on a branch.
/// A fact that was never reported is simply never pushed, so absence cannot be rendered as zero.
#[derive(Clone, Debug)]
pub struct SessionEntry {
    pub title: String,
    pub when: String,
    pub facts: Vec<String>,
    pub active: bool,
}

impl SessionEntry {
    pub fn new(title: &str, when: &str) -> Self {
        Self {
            title: title.to_string(),
            when: when.to_string(),
            facts: Vec::new(),
            active: false,
        }
    }

    /// Add a fact verbatim. Facts render in the order they were added, separated by `·`.
    pub fn fact(mut self, f: impl Into<String>) -> Self {
        self.facts.push(f.into());
        self
    }

    /// `N msgs`, for the common chat-shaped session.
    pub fn messages(self, m: u32) -> Self {
        self.fact(format!("{m} msgs"))
    }

    /// A reported cost. Do not call it when no cost was reported: that is not zero dollars.
    pub fn cost(self, c: f32) -> Self {
        self.fact(fmt_usd(c))
    }

    /// Cost as the caller's own text, for a non-USD currency or a different precision.
    /// Overrides [`Self::cost`], which formats `$1.23`.
    pub fn cost_text(self, s: impl Into<String>) -> Self {
        self.fact(s.into())
    }

    pub fn model(self, m: &str) -> Self {
        self.fact(m)
    }

    pub fn active(mut self, a: bool) -> Self {
        self.active = a;
        self
    }
}

/// Session list state: entries, cursor, scroll, hit boxes, and activation slot.
#[derive(Clone, Debug, Default)]
pub struct SessionListState {
    pub entries: Vec<SessionEntry>,
    pub filter: String,
    pub cursor: usize,
    pub offset: usize,
    pub hit: HitBox,
    activated: bool,
}

impl SessionListState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take_activated(&mut self) -> Option<usize> {
        if self.activated {
            self.activated = false;
            Some(self.cursor)
        } else {
            None
        }
    }

    fn filtered(&self) -> Vec<(usize, i32, Vec<usize>)> {
        if self.filter.is_empty() {
            self.entries
                .iter()
                .enumerate()
                .map(|(i, _)| (i, 0, Vec::new()))
                .collect()
        } else {
            fuzzy::rank(&self.filter, self.entries.iter().map(|e| e.title.as_str()))
        }
    }
}

impl Interactive for SessionListState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }

        match key.code {
            KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Outcome::Consumed
            }
            KeyCode::Down => {
                let max = self.filtered().len().saturating_sub(1);
                self.cursor = (self.cursor + 1).min(max);
                Outcome::Consumed
            }
            KeyCode::Enter => {
                self.activated = true;
                Outcome::Submitted
            }
            KeyCode::Backspace => {
                if !self.filter.is_empty() {
                    self.filter.pop();
                    self.cursor = 0;
                    Outcome::Consumed
                } else {
                    Outcome::Ignored
                }
            }
            KeyCode::Esc => {
                if !self.filter.is_empty() {
                    self.filter.clear();
                    self.cursor = 0;
                    Outcome::Consumed
                } else {
                    Outcome::Ignored
                }
            }
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.cursor = 0;
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        if let Some(d) = wheel_delta(&m)
            && self.hit.hover
        {
            let before = self.offset;
            self.offset = (self.offset as i32 - d * 3).max(0) as usize;
            return Outcome::changed_if(before != self.offset);
        }

        match self.hit.mouse(&m) {
            Hit::HoverChanged => Outcome::Consumed,
            Hit::None => Outcome::Ignored,
            _ => Outcome::Consumed,
        }
    }
}

/// Session list widget rendering entries with time, model, and a detail line.
pub struct SessionList {
    title: Option<String>,
    two_line: bool,
    theme: Option<Theme>,
}

impl SessionList {
    pub fn new() -> Self {
        Self {
            title: None,
            two_line: false,
            theme: None,
        }
    }

    pub fn title(mut self, t: &str) -> Self {
        self.title = Some(t.to_string());
        self
    }

    pub fn two_line(mut self, v: bool) -> Self {
        self.two_line = v;
        self
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Default for SessionList {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for SessionList {
    type State = SessionListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.unwrap_or_else(theme::current);
        state.hit.set_area(area);

        let mut y = area.y;

        // Title row
        if let Some(ref title) = self.title {
            let count = state.filtered().len();
            let header = format!("{} ({})", title, count);
            put(
                buf,
                area.x,
                y,
                &header,
                area.width,
                bold(st(th.text, th.background)),
            );
            y += 1;
        }

        let row_height = if self.two_line { 2 } else { 1 };
        let visible = (area.bottom().saturating_sub(y) as usize) / row_height;

        let filtered = state.filtered();
        if state.cursor >= filtered.len() {
            state.cursor = filtered.len().saturating_sub(1);
        }

        if state.cursor >= state.offset + visible {
            state.offset = state.cursor.saturating_sub(visible - 1);
        } else if state.cursor < state.offset {
            state.offset = state.cursor;
        }

        for (list_idx, &(entry_idx, _, ref positions)) in
            filtered.iter().enumerate().skip(state.offset)
        {
            if y >= area.bottom() {
                break;
            }

            let Some(entry) = state.entries.get(entry_idx) else {
                continue;
            };

            let is_cursor = list_idx == state.cursor;
            let bg = if is_cursor {
                th.cursor_bg
            } else {
                th.background
            };

            fill(buf, Rect::new(area.x, y, area.width, row_height as u16), bg);

            // Active dot or space
            let dot = if entry.active { "●" } else { " " };
            put(
                buf,
                area.x,
                y,
                dot,
                1,
                st(
                    if entry.active {
                        th.primary
                    } else {
                        th.text_muted
                    },
                    bg,
                ),
            );

            // Title with fuzzy highlights
            let title_x = area.x + 2;
            let when_w = entry.when.len() as u16 + 1;
            let title_w = area.width.saturating_sub(2 + when_w);

            if positions.is_empty() {
                let title_str = truncate(&entry.title, title_w as usize);
                put(buf, title_x, y, &title_str, title_w, bold(st(th.text, bg)));
            } else {
                // Fuzzy highlight
                for (x, (i, ch)) in
                    (title_x..title_x + title_w).zip(entry.title.chars().enumerate())
                {
                    let color = if positions.contains(&i) {
                        th.accent
                    } else {
                        th.text
                    };
                    put(
                        buf,
                        x,
                        y,
                        ch.encode_utf8(&mut [0; 4]),
                        1,
                        bold(st(color, bg)),
                    );
                }
            }

            // When (right-aligned)
            let when_x = area.right().saturating_sub(when_w);
            put(buf, when_x, y, &entry.when, when_w, st(th.text_muted, bg));

            y += 1;

            // Second line
            if self.two_line && y < area.bottom() {
                let detail = entry.facts.join(" · ");
                put(
                    buf,
                    title_x,
                    y,
                    &detail,
                    area.width.saturating_sub(2),
                    st(th.text_muted, bg),
                );
                y += 1;
            }
        }
    }
}

// ModelPicker

/// Model capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Capability {
    Vision,
    Tools,
    Reasoning,
    Fast,
}

impl Capability {
    pub fn glyph(self) -> &'static str {
        match self {
            Capability::Vision => "◉",
            Capability::Tools => "⊛",
            Capability::Reasoning => "◈",
            Capability::Fast => "▸",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Capability::Vision => "vision",
            Capability::Tools => "tools",
            Capability::Reasoning => "reasoning",
            Capability::Fast => "fast",
        }
    }

    pub fn color(self, th: &Theme) -> Rgb {
        match self {
            Capability::Vision => th.accent,
            Capability::Tools => th.primary,
            Capability::Reasoning => th.secondary,
            Capability::Fast => th.success,
        }
    }
}

/// Model metadata: id, name, provider, context window, capabilities, and pricing.
#[derive(Clone, Debug)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    /// `None` when the context window is not published.
    pub context: Option<u32>,
    /// Input and output price per million tokens. One `Option` for the pair because pricing is
    /// published as a pair or not at all; a local model has none.
    pub prices: Option<(f32, f32)>,
    pub caps: Vec<Capability>,
}

impl ModelInfo {
    pub fn new(id: &str, provider: &str) -> Self {
        Self {
            id: id.to_string(),
            provider: provider.to_string(),
            context: None,
            prices: None,
            caps: Vec::new(),
        }
    }

    /// Set the published context window. Leave unset when it is unknown.
    pub fn context(mut self, c: u32) -> Self {
        self.context = Some(c);
        self
    }

    /// Set the published price per million input and output tokens.
    pub fn prices(mut self, inp: f32, out: f32) -> Self {
        self.prices = Some((inp, out));
        self
    }

    pub fn cap(mut self, c: Capability) -> Self {
        self.caps.push(c);
        self
    }

    pub fn caps(mut self, cs: Vec<Capability>) -> Self {
        self.caps = cs;
        self
    }
}

/// Model picker state: model list, cursor, scroll, and selected model slot.
#[derive(Clone, Debug, Default)]
pub struct ModelPickerState {
    pub models: Vec<ModelInfo>,
    pub selected: usize,
    pub cursor: usize,
    pub offset: usize,
    pub hit: HitBox,
    selection_changed: bool,
}

impl ModelPickerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take_selected(&mut self) -> Option<usize> {
        if self.selection_changed {
            self.selection_changed = false;
            Some(self.selected)
        } else {
            None
        }
    }
}

impl Interactive for ModelPickerState {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if !is_press(&key) {
            return Outcome::Ignored;
        }

        match key.code {
            KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Outcome::Consumed
            }
            KeyCode::Down => {
                let max = self.models.len().saturating_sub(1);
                self.cursor = (self.cursor + 1).min(max);
                Outcome::Consumed
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.selected = self.cursor;
                self.selection_changed = true;
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        if let Some(d) = wheel_delta(&m)
            && self.hit.hover
        {
            let before = self.offset;
            self.offset = (self.offset as i32 - d * 3).max(0) as usize;
            return Outcome::changed_if(before != self.offset);
        }

        match self.hit.mouse(&m) {
            Hit::HoverChanged => Outcome::Consumed,
            Hit::None => Outcome::Ignored,
            _ => Outcome::Consumed,
        }
    }
}

/// Model picker widget rendering models with capability badges and pricing.
pub struct ModelPicker {
    theme: Option<Theme>,
}

impl ModelPicker {
    pub fn new() -> Self {
        Self { theme: None }
    }

    pub fn theme(mut self, th: &Theme) -> Self {
        self.theme = Some(*th);
        self
    }
}

impl Default for ModelPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for ModelPicker {
    type State = ModelPickerState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.unwrap_or_else(theme::current);
        state.hit.set_area(area);
        let visible = area.height as usize;

        if state.cursor >= state.models.len() {
            state.cursor = state.models.len().saturating_sub(1);
        }

        if state.cursor >= state.offset + visible {
            state.offset = state.cursor.saturating_sub(visible - 1);
        } else if state.cursor < state.offset {
            state.offset = state.cursor;
        }

        // One id column wide enough for the longest id on screen, so the rows still line up,
        // but never wider than half the pane — a provider with very long ids must not push the
        // chip and the capability badges off the row. A fixed width truncated ids that fit.
        let widest = state
            .models
            .iter()
            .skip(state.offset)
            .take(visible)
            .map(|m| unicode_width::UnicodeWidthStr::width(m.id.as_str()))
            .max()
            .unwrap_or(0) as u16;
        let id_w = widest.clamp(8, (area.width / 2).max(8));
        for (y, (i, model)) in
            (area.y..area.bottom()).zip(state.models.iter().enumerate().skip(state.offset))
        {
            let is_cursor = i == state.cursor;
            let is_selected = i == state.selected;
            let bg = if is_cursor {
                th.cursor_bg
            } else {
                th.background
            };

            fill(buf, Rect::new(area.x, y, area.width, 1), bg);

            let mut x = area.x;

            // Selected marker
            let marker = if is_selected { "●" } else { "○" };
            put(
                buf,
                x,
                y,
                marker,
                1,
                st(
                    if is_selected {
                        th.primary
                    } else {
                        th.text_muted
                    },
                    bg,
                ),
            );
            x += 2;

            // Model id
            let id_w = id_w.min(area.width.saturating_sub(x - area.x));
            put(
                buf,
                x,
                y,
                &truncate(&model.id, id_w as usize),
                id_w,
                st(th.text, bg),
            );
            x += id_w + 1;

            // Provider chip
            let provider_str = truncate(&model.provider, 10);
            let chip_w = provider_str.len() as u16 + 2;
            if x + chip_w < area.right() {
                fill(buf, Rect::new(x, y, chip_w, 1), th.surface);
                put(
                    buf,
                    x + 1,
                    y,
                    &provider_str,
                    chip_w.saturating_sub(2),
                    st(th.text_muted, th.surface),
                );
                x += chip_w + 1;
            }

            // Context and pricing are published per model, so a model that publishes neither
            // simply takes less width rather than claiming a free zero-token window.
            if let Some(ctx) = model.context {
                let ctx_str = fmt_tokens(ctx);
                put(
                    buf,
                    x,
                    y,
                    &ctx_str,
                    ctx_str.len() as u16,
                    st(th.text_muted, bg),
                );
                x += ctx_str.len() as u16 + 2;
            }

            if let Some((inp, out)) = model.prices {
                let prices = format!("{} / {}", fmt_usd(inp), fmt_usd(out));
                put(
                    buf,
                    x,
                    y,
                    &prices,
                    prices.len() as u16,
                    st(th.text_muted, bg),
                );
                x += prices.len() as u16 + 2;
            }

            // Capabilities
            for cap in &model.caps {
                if x + 8 >= area.right() {
                    break;
                }
                let cap_str = format!("{} {}", cap.glyph(), cap.label());
                put(
                    buf,
                    x,
                    y,
                    &cap_str,
                    cap_str.len() as u16,
                    st(cap.color(&th), bg),
                );
                x += cap_str.len() as u16 + 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fmt_duration() {
        assert_eq!(fmt_duration(Duration::from_millis(850)), "850ms");
        assert_eq!(fmt_duration(Duration::from_secs_f32(1.2)), "1.2s");
        assert_eq!(fmt_duration(Duration::from_secs(12)), "12.0s");
        assert_eq!(fmt_duration(Duration::from_secs(63)), "1m 03s");
        assert_eq!(fmt_duration(Duration::from_secs(7500)), "2h 05m");
    }

    #[test]
    fn test_fmt_usd() {
        assert_eq!(fmt_usd(0.0042), "$0.0042");
        assert_eq!(fmt_usd(0.42), "$0.42");
        assert_eq!(fmt_usd(12.30), "$12");
    }

    #[test]
    fn test_token_meter_segments_sum() {
        let th = Theme::default();
        // cache_write is 5 of 3505 tokens: proportionally it rounds to zero cells, so this
        // input is what exercises the "every non-zero segment gets a cell" guarantee, and
        // the rounding leftover is what exercises filling the bar exactly.
        let b = TokenBreakdown {
            input: 1000,
            output: 500,
            cache_read: 2000,
            cache_write: 5,
        };
        let area = Rect::new(0, 0, 80, 2);
        let mut buf = Buffer::empty(area);
        TokenMeter::new(b).theme(&th).render(area, &mut buf);

        let bar_width = 80u16;
        let segments = [
            ("input", th.primary),
            ("output", th.accent),
            ("cache_read", th.success),
            ("cache_write", th.secondary),
        ];

        // Every non-zero segment gets at least one cell, and the segments together cover the
        // bar with no background showing through. Counting every bg would just recount the row.
        for (name, color) in segments {
            let n = (0..bar_width)
                .filter(|&x| buf.cell((x, 0)).is_some_and(|c| c.bg == color.color()))
                .count();
            assert!(n > 0, "{name} segment is missing from the bar");
        }
        let painted = (0..bar_width)
            .filter(|&x| {
                segments
                    .iter()
                    .any(|(_, col)| buf.cell((x, 0)).is_some_and(|c| c.bg == col.color()))
            })
            .count() as u16;
        assert_eq!(
            painted, bar_width,
            "segments fill the bar, no gap: {painted}"
        );

        // Verify colors appear in order by checking the first cell of each color segment
        let primary_idx = (0..bar_width)
            .find(|&x| {
                buf.cell((x, 0))
                    .map(|c| c.bg == th.primary.color())
                    .unwrap_or(false)
            })
            .unwrap();
        let accent_idx = (0..bar_width)
            .find(|&x| {
                buf.cell((x, 0))
                    .map(|c| c.bg == th.accent.color())
                    .unwrap_or(false)
            })
            .unwrap();
        let success_idx = (0..bar_width)
            .find(|&x| {
                buf.cell((x, 0))
                    .map(|c| c.bg == th.success.color())
                    .unwrap_or(false)
            })
            .unwrap();
        let secondary_idx = (0..bar_width)
            .find(|&x| {
                buf.cell((x, 0))
                    .map(|c| c.bg == th.secondary.color())
                    .unwrap_or(false)
            })
            .unwrap();
        assert!(
            primary_idx < accent_idx,
            "input (primary) before output (accent)"
        );
        assert!(
            accent_idx < success_idx,
            "output (accent) before cache_read (success)"
        );
        assert!(
            success_idx < secondary_idx,
            "cache_read (success) before cache_write (secondary)"
        );
        // Legend row (y=1) should show segments and total
        let legend_row: String = (0..area.width)
            .filter_map(|x| buf.cell((x, 1)).map(|c| c.symbol().to_string()))
            .collect();
        // All segment labels should appear
        assert!(
            legend_row.contains("input"),
            "legend should show input: {legend_row}"
        );
        assert!(
            legend_row.contains("output"),
            "legend should show output: {legend_row}"
        );
        assert!(
            legend_row.contains("cache"),
            "legend should show cache segments: {legend_row}"
        );
        // Total label appears (3800 formats as "3.8k" or might show as "total 3k" depending on formatting)
        assert!(
            legend_row.contains("total") || legend_row.contains("3"),
            "legend should show total: {legend_row}"
        );
    }

    #[test]
    fn test_agent_tree_collapse() {
        let mut state = AgentTreeState::new();
        let child = AgentNode::new("Child", "haiku", AgentStatus::Done);
        let root = AgentNode::new("Main", "opus", AgentStatus::Running).child(child);
        state.root = Some(root);

        // Initially collapsed
        assert_eq!(state.visible_rows().len(), 1);

        // Expand
        state.toggle_expanded(&Vec::<usize>::new());
        assert_eq!(state.visible_rows().len(), 2);

        // Collapse
        state.toggle_expanded(&Vec::<usize>::new());
        assert_eq!(state.visible_rows().len(), 1);
    }

    #[test]
    fn test_agent_lanes_autoscroll() {
        // Create multiple lanes to test vertical capacity and horizontal time scrolling
        let lanes = vec![
            Lane::new("Lane0").span(LaneSpan::new(
                0.0,
                Some(10.0),
                AgentStatus::Running,
                "span0",
            )),
            Lane::new("Lane1").span(LaneSpan::new(2.0, Some(8.0), AgentStatus::Done, "span1")),
            Lane::new("Lane2").span(LaneSpan::new(
                5.0,
                Some(35.0),
                AgentStatus::Running,
                "span2",
            )),
            Lane::new("Lane3").span(LaneSpan::new(10.0, None, AgentStatus::Running, "span3")),
            Lane::new("Lane4").span(LaneSpan::new(15.0, Some(30.0), AgentStatus::Done, "span4")),
        ];

        // Area: 50 wide, 5 high -> lane_height = 5-1 = 4, so only 4 of 5 lanes fit
        let area = Rect::new(0, 0, 50, 5);
        let mut buf = Buffer::empty(area);

        // First render: Clock at 5s, window 30s -> scroll_threshold = 24, view_start = 0
        AgentLanes::new(&lanes)
            .clock(5.0)
            .window(30.0)
            .render(area, &mut buf);

        // Check visible lanes: first 4 lanes fit (Lane0-Lane3), Lane4 is cut off
        let visible_lanes: Vec<String> = (0..area.height.saturating_sub(1))
            .filter_map(|y| {
                let row: String = (0..14)
                    .filter_map(|x| buf.cell((x, y)).map(|c| c.symbol().to_string()))
                    .collect();
                if row.contains("Lane") {
                    Some(row.trim().to_string())
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(
            visible_lanes.len(),
            4,
            "only 4 lanes fit in height 5: {:?}",
            visible_lanes
        );
        assert!(
            visible_lanes.iter().any(|l| l.contains("Lane0")),
            "Lane0 visible: {:?}",
            visible_lanes
        );
        assert!(
            visible_lanes.iter().any(|l| l.contains("Lane3")),
            "Lane3 visible: {:?}",
            visible_lanes
        );

        // Time axis at bottom should show early times (0s, 5s visible with view_start=0)
        let axis_y = area.height.saturating_sub(1);
        let axis_early: String = (0..area.width)
            .filter_map(|x| buf.cell((x, axis_y)).map(|c| c.symbol().to_string()))
            .collect();
        assert!(
            axis_early.contains("0s") || axis_early.contains("5s"),
            "early times visible at clock=5: {axis_early}"
        );

        // Second render: Clock at 30s -> view_start = 30 - 24 = 6, so 0s scrolls out left
        buf.reset();
        AgentLanes::new(&lanes)
            .clock(30.0)
            .window(30.0)
            .render(area, &mut buf);

        let axis_late: String = (0..area.width)
            .filter_map(|x| buf.cell((x, axis_y)).map(|c| c.symbol().to_string()))
            .collect();

        // With view_start=6, first visible tick is 10s, so the "0s" tick label should be scrolled out
        // (Check for "0s" at start or with leading space to avoid matching "10s")
        let has_0s_label = axis_late.starts_with("0s") || axis_late.contains(" 0s");
        assert!(
            !has_0s_label,
            "0s tick scrolled out at clock=30, view_start=6: {axis_late}"
        );
        assert!(
            axis_late.contains("10s") || axis_late.contains("15s") || axis_late.contains("20s"),
            "later times visible at clock=30: {axis_late}"
        );
    }

    #[test]
    fn test_session_filter_ranks_match() {
        let mut state = SessionListState::new();
        state.entries = vec![
            SessionEntry::new("Build feature", "2h ago"),
            SessionEntry::new("Fix bug", "1h ago"),
            SessionEntry::new("Feature request", "3h ago"),
        ];
        state.filter = "feat".to_string();

        let filtered = state.filtered();
        // Should rank "Build feature" and "Feature request" high
        assert!(filtered.len() >= 2);
        assert!(
            filtered[0].1 > 0
                || filtered
                    .iter()
                    .any(|&(idx, _, _)| state.entries[idx].title.contains("feature"))
        );
    }

    /// A session run on a local model has no cost to report. The detail line must then read
    /// "N msgs · model" and not invent "$0.0000".
    #[test]
    fn session_detail_omits_unreported_cost() {
        let screen = |entry: SessionEntry| {
            let mut state = SessionListState::new();
            state.entries = vec![entry];
            let area = Rect::new(0, 0, 60, 4);
            let mut buf = Buffer::empty(area);
            SessionList::new()
                .two_line(true)
                .render(area, &mut buf, &mut state);
            buf.content().iter().map(|c| c.symbol()).collect::<String>()
        };

        let local = screen(
            SessionEntry::new("Local run", "2h ago")
                .messages(7)
                .model("qwen3"),
        );
        assert!(local.contains("7 msgs") && local.contains("qwen3"));
        assert!(!local.contains('$'), "no cost was reported: {local:?}");

        let billed = screen(
            SessionEntry::new("Claude run", "2h ago")
                .messages(7)
                .model("opus")
                .cost(1.25),
        );
        assert!(billed.contains('$'), "a reported cost is shown: {billed:?}");

        // A harness that counts steps on a branch instead of messages and dollars says so in
        // its own words, in the order it chose.
        let stepped = screen(
            SessionEntry::new("Refactor TUI", "2h ago")
                .fact("12 steps")
                .fact("48.2k tok")
                .fact("main"),
        );
        assert!(
            stepped.contains("12 steps · 48.2k tok · main"),
            "{stepped:?}"
        );
        assert!(
            !stepped.contains("msgs"),
            "no message count was claimed: {stepped:?}"
        );
    }

    /// A managed llama-server model publishes neither a window nor a price list.
    #[test]
    fn model_picker_omits_unpublished_context_and_prices() {
        let screen = |model: ModelInfo| {
            let mut state = ModelPickerState::new();
            state.models = vec![model];
            let area = Rect::new(0, 0, 80, 4);
            let mut buf = Buffer::empty(area);
            ModelPicker::new().render(area, &mut buf, &mut state);
            buf.content().iter().map(|c| c.symbol()).collect::<String>()
        };

        let local = screen(ModelInfo::new("qwen3-8b", "llama.cpp"));
        assert!(
            local.contains("qwen3-8b"),
            "the id is always shown: {local:?}"
        );
        assert!(!local.contains('$'), "no pricing was published: {local:?}");

        let published = screen(
            ModelInfo::new("claude-opus-4", "anthropic")
                .context(200_000)
                .prices(15.0, 75.0),
        );
        assert!(
            published.contains('$'),
            "published pricing is shown: {published:?}"
        );
    }

    /// The id column fits the longest id on screen instead of a fixed 20 cells, which used to
    /// cut `granite-embedding:278m` short in a pane with room to spare. It still refuses to eat
    /// more than half the pane, so the provider chip survives a pathological id.
    #[test]
    fn model_picker_id_column_fits_its_content() {
        let screen = |ids: &[&str], width: u16| {
            let mut state = ModelPickerState::new();
            state.models = ids.iter().map(|id| ModelInfo::new(id, "ollama")).collect();
            let area = Rect::new(0, 0, width, ids.len() as u16);
            let mut buf = Buffer::empty(area);
            ModelPicker::new().render(area, &mut buf, &mut state);
            buf.content().iter().map(|c| c.symbol()).collect::<String>()
        };

        let long = screen(&["granite-embedding:278m", "qwen3.5:9b"], 60);
        assert!(
            long.contains("granite-embedding:278m"),
            "the full id fits: {long:?}"
        );
        assert!(
            long.contains("ollama"),
            "and the provider chip still lands: {long:?}"
        );

        // Half of 30 is 15, so a 40-cell id is cut and the chip keeps its place.
        let cramped = screen(&["a-really-very-long-model-identifier-xxxx"], 30);
        assert!(
            cramped.contains('…'),
            "an id too long for the pane is cut: {cramped:?}"
        );
        assert!(cramped.contains("ollama"), "the chip survives: {cramped:?}");
    }

    #[test]
    fn test_no_panic_at_small_sizes() {
        let th = Theme::default();
        let area_1x1 = Rect::new(0, 0, 1, 1);
        let area_3x2 = Rect::new(0, 0, 3, 2);
        let area_10x3 = Rect::new(0, 0, 10, 3);
        let area_60x16 = Rect::new(0, 0, 60, 16);
        let area_130x42 = Rect::new(0, 0, 130, 42);
        let area_250x70 = Rect::new(0, 0, 250, 70);

        for area in [
            area_1x1,
            area_3x2,
            area_10x3,
            area_60x16,
            area_130x42,
            area_250x70,
        ] {
            let mut buf = Buffer::empty(area);
            let now = Instant::now();

            // Test all widgets
            ElapsedTimer::new().now(now).render(area, &mut buf);

            let mut tree_state = AgentTreeState::new();
            tree_state.root = Some(AgentNode::new("Test", "model", AgentStatus::Running));
            AgentTree::new()
                .now(now)
                .render(area, &mut buf, &mut tree_state);

            let lane = Lane::new("Lane");
            AgentLanes::new(&[lane])
                .clock(5.0)
                .now(now)
                .render(area, &mut buf);

            TokenMeter::new(TokenBreakdown::default())
                .theme(&th)
                .render(area, &mut buf);

            let mut cost_state = CostMeterState::new();
            CostMeter::new()
                .theme(&th)
                .render(area, &mut buf, &mut cost_state);

            let seg = ContextSegment::new("test", 1000);
            ContextMap::new(&[seg], 10000)
                .theme(&th)
                .render(area, &mut buf);

            CompactionBanner::new(80.0, 30.0, 5000)
                .now(now)
                .theme(&th)
                .render(area, &mut buf);

            TurnStats::new().theme(&th).render(area, &mut buf);

            RateGraph::new(&[1.0, 2.0])
                .theme(&th)
                .render(area, &mut buf);

            let mut session_state = SessionListState::new();
            SessionList::new()
                .theme(&th)
                .render(area, &mut buf, &mut session_state);

            let mut model_state = ModelPickerState::new();
            ModelPicker::new()
                .theme(&th)
                .render(area, &mut buf, &mut model_state);
        }
    }

    #[test]
    fn vocabulary_overrides_ride_through_to_buffer() {
        let th = Theme::default();
        let area = Rect::new(0, 0, 80, 10);
        let mut buf = Buffer::empty(area);

        // SessionEntry with EUR cost
        let entry = SessionEntry::new("Test", "now").cost_text("€12,34");
        assert!(entry.facts.contains(&"€12,34".to_string()));

        // ContextMap with German text
        let seg = ContextSegment::new("test", 4200);
        ContextMap::new(&[seg], 10000)
            .compact(true)
            .used_text("42% benutzt")
            .theme(&th)
            .render(area, &mut buf);
        let row: String = (0..area.width)
            .filter_map(|x| buf.cell((x, 0)).map(|c| c.symbol().to_string()))
            .collect();
        assert!(row.contains("42% benutzt"), "German used text: {row}");

        // TurnStats with cache text override
        buf = Buffer::empty(area);
        TurnStats::new()
            .cache_text("cache 42%")
            .theme(&th)
            .render(area, &mut buf);
        let row: String = (0..area.width)
            .filter_map(|x| buf.cell((x, 1)).map(|c| c.symbol().to_string()))
            .collect();
        assert!(row.contains("cache 42%"), "cache text override: {row}");
    }

    #[test]
    fn default_vocabulary_unchanged() {
        let th = Theme::default();
        let area = Rect::new(0, 0, 80, 10);
        let mut buf = Buffer::empty(area);

        // ContextMap default "% used"
        let seg = ContextSegment::new("test", 4200);
        ContextMap::new(&[seg], 10000)
            .compact(true)
            .theme(&th)
            .render(area, &mut buf);
        let row: String = (0..area.width)
            .filter_map(|x| buf.cell((x, 0)).map(|c| c.symbol().to_string()))
            .collect();
        assert!(row.contains("42% used"), "default used text: {row}");

        // TurnStats default cache "%"
        buf = Buffer::empty(area);
        TurnStats::new()
            .cache_hit(0.42)
            .theme(&th)
            .render(area, &mut buf);
        let cache_row: String = (0..area.width)
            .filter_map(|x| buf.cell((x, 1)).map(|c| c.symbol().to_string()))
            .collect();
        assert!(cache_row.contains("42%"), "default cache text: {cache_row}");

        // SessionEntry default cost with $
        let entry = SessionEntry::new("Test", "now").cost(12.34);
        assert_eq!(entry.facts, vec!["$12".to_string()]);
    }

    #[test]
    fn session_list_commit_returns_submitted() {
        let mut state = SessionListState::new();
        state.entries = vec![
            SessionEntry::new("Session 1", "2h ago"),
            SessionEntry::new("Session 2", "5h ago"),
        ];

        // Cursor movement returns Consumed
        let outcome = state.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(outcome, Outcome::Consumed);
        assert_eq!(state.cursor, 1);

        // Enter returns Submitted
        let outcome = state.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(outcome, Outcome::Submitted);
        assert_eq!(state.take_activated(), Some(1));
    }

    /// Filled areas are painted as background colour on a space (contract rule 15), so a
    /// symbol-only check reads "nothing drawn" for a widget that did draw.
    fn painted(buf: &Buffer) -> bool {
        buf.content().iter().any(|c| {
            c.symbol() != " "
                || c.bg != ratatui_core::style::Color::Reset
                || c.fg != ratatui_core::style::Color::Reset
        })
    }

    #[test]
    fn draws_at_minimum_and_refuses_visibly_below_it() {
        let th = Theme::default();
        let now = Instant::now();

        // AgentLanes
        let lanes = [Lane::new("Test")];
        let (w, h) = AgentLanes::new(&lanes).min_size();
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        AgentLanes::new(&lanes)
            .clock(5.0)
            .now(now)
            .render(buf.area, &mut buf);
        assert!(
            painted(&buf),
            "AgentLanes should draw at its stated minimum"
        );

        let mut buf = Buffer::empty(Rect::new(0, 0, w, h.saturating_sub(1)));
        AgentLanes::new(&lanes)
            .clock(5.0)
            .now(now)
            .render(buf.area, &mut buf);
        assert!(
            buf.content().iter().any(|c| c.symbol() == "⋯"),
            "one row short must refuse visibly"
        );

        // CostMeter
        let (w, h) = CostMeter::new().min_size();
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        let mut state = CostMeterState::new();
        CostMeter::new()
            .spent(1.23)
            .budget(10.0)
            .theme(&th)
            .render(buf.area, &mut buf, &mut state);
        assert!(painted(&buf), "CostMeter should draw at its stated minimum");

        let mut buf = Buffer::empty(Rect::new(0, 0, w, h.saturating_sub(1)));
        CostMeter::new()
            .spent(1.23)
            .budget(10.0)
            .theme(&th)
            .render(buf.area, &mut buf, &mut state);
        assert!(
            buf.content().iter().any(|c| c.symbol() == "⋯"),
            "one row short must refuse visibly"
        );

        // TurnStats
        let (w, h) = TurnStats::new().min_size();
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        TurnStats::new()
            .cache_hit(0.42)
            .theme(&th)
            .render(buf.area, &mut buf);
        assert!(painted(&buf), "TurnStats should draw at its stated minimum");

        // Shrink height (h=2, so h-1=1 is testable)
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h.saturating_sub(1)));
        TurnStats::new()
            .cache_hit(0.42)
            .theme(&th)
            .render(buf.area, &mut buf);
        assert!(
            buf.content().iter().any(|c| c.symbol() == "⋯"),
            "one row short must refuse visibly"
        );
    }
}
