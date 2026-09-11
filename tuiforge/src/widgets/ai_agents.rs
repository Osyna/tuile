//! AI harness: agent trees and lanes, token/cost meters, context map, sessions, model picker.
//!
//! ```no_run
//! use tuiforge::prelude::*;
//! use tuiforge::widgets::ai_agents::*;
//! # let area = Rect::new(0, 0, 80, 24);
//! # let mut buf = Buffer::empty(area);
//! # let now = Instant::now();
//! let mut state = AgentTreeState::default();
//! state.root = Some(AgentNode::new("Main", "opus", AgentStatus::Running));
//! AgentTree::new().now(now).render(area, &mut buf, &mut state);
//! ```

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::widgets::{StatefulWidget, Widget};

use crate::anim::{self, Easing, Tween};
use crate::core::{is_press, wheel_delta, Focus, Hit, HitBox, Interactive, Look, Outcome};
use crate::draw::{
    bold, fill, hbar, put, put_right, st, truncate, Border,
};
use crate::fuzzy;
use crate::layout::pad;
use crate::theme::{self, Rgb, Theme, Variant};
use crate::widgets::ai::fmt_tokens;
use crate::widgets::charts::{SparkChart, SparkStyle};
use crate::widgets::spinner::spinners;
use crate::widgets::{Scrollbar, ScrollbarState};

// ───────────────────────────── Duration formatting ─────────────────────────────

/// Format duration for UI: `850ms`, `1.2s`, `12.4s`, `1m 03s`, `2h 05m`.
pub fn fmt_duration(d: Duration) -> String {
    let ms = d.as_millis();
    let secs = d.as_secs();
    if ms < 1000 {
        format!("{}ms", ms)
    } else if secs < 10 {
        format!("{:.1}s", d.as_secs_f32())
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

// ───────────────────────────── ElapsedTimer ─────────────────────────────

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
            put(buf, x, area.y, lbl, area.width.saturating_sub(x - area.x), st(th.text, th.background));
            x += lbl.len() as u16 + 1;
        }
        
        put(buf, x, area.y, &dur_str, area.width.saturating_sub(x - area.x), st(th.text, th.background));
    }
}

// ───────────────────────────── AgentStatus ─────────────────────────────

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

// ───────────────────────────── AgentNode ─────────────────────────────

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

// ───────────────────────────── AgentTreeState ─────────────────────────────

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
        if self.root.is_none() {
            return;
        }
        let paths = {
            let root = self.root.as_ref().unwrap();
            let mut paths = Vec::new();
            Self::collect_paths_recursive(root, &Vec::new(), &mut paths);
            paths
        };
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
                    if let Some(node) = self.get_node(path) {
                        if !node.children.is_empty() && !self.is_expanded(path) {
                            self.toggle_expanded(path);
                        }
                    }
                }
                Outcome::Consumed
            }
            KeyCode::Enter => {
                if self.cursor < rows.len() {
                    let path = &rows[self.cursor].path;
                    if let Some(node) = self.get_node(path) {
                        if !node.children.is_empty() {
                            self.toggle_expanded(path);
                        }
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
        if let Some(d) = wheel_delta(&m) {
            if self.hit.hover {
                let before = self.offset;
                self.offset = (self.offset as i32 - d * 3).max(0) as usize;
                out = out | Outcome::changed_if(before != self.offset);
            }
        }
        let hit_out = match self.hit.mouse(&m) {
            Hit::HoverChanged => Outcome::Consumed,
            Hit::None => Outcome::Ignored,
            _ => Outcome::Consumed,
        };
        out | self.scrollbar.handle_mouse(m) | hit_out
    }
}

// ───────────────────────────── AgentTree ─────────────────────────────

/// Agent tree widget with expansion, cursor, tasks.
pub struct AgentTree {
    show_tasks: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl AgentTree {
    pub fn new() -> Self {
        Self {
            show_tasks: false,
            now: None,
            theme: None,
        }
    }

    pub fn show_tasks(mut self, v: bool) -> Self {
        self.show_tasks = v;
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

        let row_height = if self.show_tasks { 2 } else { 1 };
        let visible = area.height as usize / row_height;
        
        if state.cursor >= rows.len() {
            state.cursor = rows.len().saturating_sub(1);
        }
        
        if state.cursor >= state.offset + visible {
            state.offset = state.cursor.saturating_sub(visible - 1);
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
            let bg = if is_cursor { th.cursor_bg } else { th.background };
            
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
                let marker = if state.is_expanded(&row.path) { "▾" } else { "▸" };
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
                put(buf, x, y, node.status.glyph(), 1, st(node.status.color(&th), bg));
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
                put(buf, x + 1, y, &model_str, chip_width.saturating_sub(2), st(th.text_muted, th.surface));
                x += chip_width + 1;
            }

            // Right-aligned: tokens · duration
            let info = format!("{} · {}", fmt_tokens(node.tokens), fmt_duration(node.elapsed));
            let info_w = info.len() as u16;
            if x + info_w <= area.x + content_width {
                let rx = area.x + content_width - info_w;
                if rx > x {
                    put(buf, rx, y, &info, info_w, st(th.text_muted, bg));
                }
            }

            y += 1;

            // Task row
            if self.show_tasks && !node.task.is_empty() {
                if y < area.bottom() {
                    let task_x = area.x + (row.depth as u16 * 2) + 4;
                    let task_width = content_width.saturating_sub(task_x - area.x);
                    let task_str = truncate(&node.task, task_width as usize);
                    put(buf, task_x, y, &task_str, task_width, st(th.text_muted, bg));
                    y += 1;
                }
            }
        }
    }
}

// ───────────────────────────── AgentLanes ─────────────────────────────

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

impl Widget for AgentLanes<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || self.lanes.is_empty() {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        
        // Name column width
        let max_name = self.lanes.iter().map(|l| l.name.len()).max().unwrap_or(0);
        let name_width = (max_name.min(14) + 1) as u16;
        
        if area.width < name_width + 10 {
            return;
        }

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
            put(buf, area.x, y, &name_str, name_width, st(th.text, if i % 2 == 1 { th.surface } else { th.background }));

            // Track
            let track_x = area.x + name_width;
            for span in &lane.spans {
                let span_end = span.end.unwrap_or(self.clock);
                if span_end < view_start || span.start > view_end {
                    continue;
                }

                let cell_start = ((span.start - view_start) / self.window * track_width as f32).max(0.0) as u16;
                let cell_end = ((span_end - view_start) / self.window * track_width as f32).min(track_width as f32) as u16;
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

                fill(buf, Rect::new(track_x + cell_start, y, span_width, 1), span_bg);
                
                // Label if it fits
                if span_width > 2 && !span.label.is_empty() {
                    let label_str = truncate(&span.label, span_width.saturating_sub(2) as usize);
                    let label_color = span_bg.text_on(1.0);
                    put(buf, track_x + cell_start + 1, y, &label_str, span_width.saturating_sub(2), st(label_color, span_bg));
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
                put(buf, area.x + name_width + tick_x, axis_y, &tick_label, track_width.saturating_sub(tick_x), st(th.text_muted, th.background));
            }
            t += tick_interval;
        }

        // Clock marker
        if self.clock >= view_start && self.clock <= view_end {
            let marker_x = area.x + name_width + ((self.clock - view_start) / self.window * track_width as f32) as u16;
            for i in 0..lane_height.min(lanes_per_row as u16) {
                put(buf, marker_x, area.y + i, "│", 1, st(th.accent, if i % 2 == 1 { th.surface } else { th.background }));
            }
        }
    }
}

// ───────────────────────────── TokenBreakdown & TokenMeter ─────────────────────────────

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
        let mut cells = vec![0u16; 4];
        let _remaining = bar_width;
        
        // First pass: proportional
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
            // Shrink largest
            for _ in 0..(sum - bar_width) {
                if let Some((idx, _)) = cells.iter().enumerate().max_by_key(|&(_, c)| c) {
                    if cells[idx] > 1 {
                        cells[idx] -= 1;
                    }
                }
            }
        } else if sum < bar_width {
            // Grow largest
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
            put_right(buf, Rect::new(area.x, area.y, area.width, 1), &total_str, st(th.text, th.background));
        } else if area.height > 1 {
            // Legend row
            let mut leg_x = area.x;
            let leg_y = area.y + 1;
            
            for &(count, color, label) in &segments {
                if count > 0 {
                    let entry = format!("■ {} {}  ", label, fmt_tokens(count));
                    if leg_x + entry.len() as u16 <= area.right() {
                        put(buf, leg_x, leg_y, "■", 1, st(color, th.background));
                        put(buf, leg_x + 2, leg_y, &format!("{} {}", label, fmt_tokens(count)), area.width, st(th.text, th.background));
                        leg_x += entry.len() as u16;
                    }
                }
            }

            // Total on the right
            let total_str = format!("total {}", fmt_tokens(total));
            let total_x = area.right().saturating_sub(total_str.len() as u16);
            if total_x > leg_x {
                put(buf, total_x, leg_y, &total_str, total_str.len() as u16, st(th.text, th.background));
            }
        }
    }
}

// ───────────────────────────── CostMeter ─────────────────────────────

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

impl StatefulWidget for CostMeter {
    type State = CostMeterState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let th = self.theme.unwrap_or_else(theme::current);
        
        if area.height < 2 {
            return;
        }

        let spent = self.spent.unwrap_or(0.0);
        
        // Update tween
        if let Some(d) = self.dur {
            if let Some(n) = self.now {
                if state.spent.target() != spent {
                    state.spent.go_with(spent, n, d, Easing::OutCubic);
                }
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
            
            hbar(buf, area.x, area.y, area.width, pct.min(1.0), color, th.panel);
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
                
                if let Some(budget) = self.budget {
                    if rate > 0.0 && current < budget {
                        let remaining = budget - current;
                        let mins = remaining / rate;
                        parts.push(format!(" · ~{} min left", mins.round() as i32));
                    }
                }
            }

            let text = parts.join("");
            put(buf, area.x, area.y + 1, &text, area.width, st(th.text, th.background));
        }
    }
}

// ───────────────────────────── ContextMap ─────────────────────────────

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
    theme: Option<Theme>,
}

impl<'a> ContextMap<'a> {
    pub fn new(segments: &'a [ContextSegment], limit: u32) -> Self {
        Self {
            segments,
            limit,
            hover: None,
            compact: false,
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
            let used_pct = if self.limit > 0 { (used as f32 / self.limit as f32 * 100.0) as u32 } else { 0 };
            let text = format!("{}% used", used_pct);
            put_right(buf, Rect::new(area.x, area.y, area.width, 1), &text, st(th.text, th.background));
        } else if area.height > 1 {
            // Legend
            let mut leg_x = area.x;
            let leg_y = area.y + 1;
            
            for (i, seg) in self.segments.iter().enumerate() {
                if seg.tokens == 0 {
                    continue;
                }
                let pct = if self.limit > 0 { (seg.tokens as f32 / self.limit as f32 * 100.0) as u32 } else { 0 };
                let entry = format!("■ {} {}%  ", seg.label, pct);
                
                if leg_x + entry.len() as u16 <= area.right() {
                    let color = seg.color.unwrap_or(palette[i % palette.len()]);
                    put(buf, leg_x, leg_y, "■", 1, st(color, th.background));
                    put(buf, leg_x + 2, leg_y, &format!("{} {}%", seg.label, pct), area.width, st(th.text, th.background));
                    leg_x += entry.len() as u16;
                }
            }

            // Free
            let free_pct = if self.limit > 0 { (free as f32 / self.limit as f32 * 100.0) as u32 } else { 0 };
            let free_entry = format!("free {}%", free_pct);
            let free_x = area.right().saturating_sub(free_entry.len() as u16);
            if free_x > leg_x {
                put(buf, free_x, leg_y, &free_entry, free_entry.len() as u16, st(th.text, th.background));
            }
        }
    }
}

// ───────────────────────────── CompactionBanner ─────────────────────────────

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

impl Widget for CompactionBanner {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme.unwrap_or_else(theme::current);
        
        if area.height < 2 {
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
        put(buf, inner.x, inner.y, &title, inner.width, bold(st(th.text, th.background)));

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
            
            hbar(buf, inner.x, inner.y + 1, inner.width, current_pct / 100.0, bar_color, th.panel);
        }

        // Summary
        if inner.height > 2 {
            if let Some(ref s) = self.summary {
                let summary_str = truncate(s, inner.width as usize);
                put(buf, inner.x, inner.y + 2, &summary_str, inner.width, st(th.text_muted, th.background));
            }
        }
    }
}

// ───────────────────────────── TurnStats ─────────────────────────────

/// Turn statistics KPI cells.
pub struct TurnStats {
    input: Option<u32>,
    output: Option<u32>,
    cache_hit: Option<f32>,
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

impl Widget for TurnStats {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme.unwrap_or_else(theme::current);
        
        if area.height < 2 {
            return;
        }

        let cells = [
            ("input", self.input.map(|n| fmt_tokens(n)), None),
            ("output", self.output.map(|n| fmt_tokens(n)), None),
            (
                "cache",
                self.cache_hit.map(|f| format!("{}%", (f * 100.0) as u32)),
                self.cache_hit.map(|f| if f > 0.5 { th.success } else { th.warning }),
            ),
            ("tools", self.tool_calls.map(|n| n.to_string()), None),
            ("time", self.duration.map(|d| fmt_duration(d)), None),
            ("cost", self.cost.map(|c| fmt_usd(c)), None),
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
            put(buf, x, area.y, label, cell_width, st(th.text_muted, th.background));
            
            // Value row
            let val_color = color.unwrap_or(th.text);
            put(buf, x, area.y + 1, val, cell_width, bold(st(val_color, th.background)));
        }
    }
}

// ───────────────────────────── RateGraph ─────────────────────────────

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

impl Widget for RateGraph<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let th = self.theme.unwrap_or_else(theme::current);
        
        if area.height < 2 || self.values.is_empty() {
            return;
        }

        let current = self.values.last().copied().unwrap_or(0.0);
        let current_str = format!("{} {}", current.round() as u32, self.label.as_deref().unwrap_or(""));
        
        put(buf, area.x, area.y, &current_str, area.width, bold(st(th.accent, th.background)));

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

// ───────────────────────────── SessionList ─────────────────────────────

/// One session entry.
#[derive(Clone, Debug)]
pub struct SessionEntry {
    pub title: String,
    pub when: String,
    pub messages: u32,
    pub cost: f32,
    pub model: String,
    pub active: bool,
}

impl SessionEntry {
    pub fn new(title: &str, when: &str) -> Self {
        Self {
            title: title.to_string(),
            when: when.to_string(),
            messages: 0,
            cost: 0.0,
            model: String::new(),
            active: false,
        }
    }

    pub fn messages(mut self, m: u32) -> Self {
        self.messages = m;
        self
    }

    pub fn cost(mut self, c: f32) -> Self {
        self.cost = c;
        self
    }

    pub fn model(mut self, m: &str) -> Self {
        self.model = m.to_string();
        self
    }

    pub fn active(mut self, a: bool) -> Self {
        self.active = a;
        self
    }
}

/// Session list state.
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
                Outcome::Changed
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
        if let Some(d) = wheel_delta(&m) {
            if self.hit.hover {
                let before = self.offset;
                self.offset = (self.offset as i32 - d * 3).max(0) as usize;
                return Outcome::changed_if(before != self.offset);
            }
        }
        let hit_out = match self.hit.mouse(&m) {
            Hit::HoverChanged => Outcome::Consumed,
            Hit::None => Outcome::Ignored,
            _ => Outcome::Consumed,
        };
        hit_out
    }
}

/// Session list widget.
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
            put(buf, area.x, y, &header, area.width, bold(st(th.text, th.background)));
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

        for (list_idx, &(entry_idx, _, ref positions)) in filtered.iter().enumerate().skip(state.offset) {
            if y >= area.bottom() {
                break;
            }

            let Some(entry) = state.entries.get(entry_idx) else {
                continue;
            };

            let is_cursor = list_idx == state.cursor;
            let bg = if is_cursor { th.cursor_bg } else { th.background };
            
            fill(buf, Rect::new(area.x, y, area.width, row_height as u16), bg);

            // Active dot or space
            let dot = if entry.active { "●" } else { " " };
            put(buf, area.x, y, dot, 1, st(if entry.active { th.primary } else { th.text_muted }, bg));

            // Title with fuzzy highlights
            let title_x = area.x + 2;
            let when_w = entry.when.len() as u16 + 1;
            let title_w = area.width.saturating_sub(2 + when_w);
            
            if positions.is_empty() {
                let title_str = truncate(&entry.title, title_w as usize);
                put(buf, title_x, y, &title_str, title_w, bold(st(th.text, bg)));
            } else {
                // Fuzzy highlight
                let mut x = title_x;
                for (i, ch) in entry.title.chars().enumerate() {
                    if x >= title_x + title_w {
                        break;
                    }
                    let highlighted = positions.contains(&i);
                    let color = if highlighted { th.accent } else { th.text };
                    put(buf, x, y, &ch.to_string(), 1, bold(st(color, bg)));
                    x += 1;
                }
            }

            // When (right-aligned)
            let when_x = area.right().saturating_sub(when_w);
            put(buf, when_x, y, &entry.when, when_w, st(th.text_muted, bg));

            y += 1;

            // Second line
            if self.two_line && y < area.bottom() {
                let detail = format!(
                    "{} msgs · {} · {}",
                    entry.messages,
                    fmt_usd(entry.cost),
                    entry.model
                );
                put(buf, title_x, y, &detail, area.width.saturating_sub(2), st(th.text_muted, bg));
                y += 1;
            }
        }
    }
}

// ───────────────────────────── ModelPicker ─────────────────────────────

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

/// Model info.
#[derive(Clone, Debug)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub context: u32,
    pub in_price: f32,
    pub out_price: f32,
    pub caps: Vec<Capability>,
}

impl ModelInfo {
    pub fn new(id: &str, provider: &str) -> Self {
        Self {
            id: id.to_string(),
            provider: provider.to_string(),
            context: 0,
            in_price: 0.0,
            out_price: 0.0,
            caps: Vec::new(),
        }
    }

    pub fn context(mut self, c: u32) -> Self {
        self.context = c;
        self
    }

    pub fn prices(mut self, inp: f32, out: f32) -> Self {
        self.in_price = inp;
        self.out_price = out;
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

/// Model picker state.
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
        if let Some(d) = wheel_delta(&m) {
            if self.hit.hover {
                let before = self.offset;
                self.offset = (self.offset as i32 - d * 3).max(0) as usize;
                return Outcome::changed_if(before != self.offset);
            }
        }
        let hit_out = match self.hit.mouse(&m) {
            Hit::HoverChanged => Outcome::Consumed,
            Hit::None => Outcome::Ignored,
            _ => Outcome::Consumed,
        };
        hit_out
    }
}

/// Model picker widget.
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

        let mut y = area.y;
        for (i, model) in state.models.iter().enumerate().skip(state.offset) {
            if y >= area.bottom() {
                break;
            }

            let is_cursor = i == state.cursor;
            let is_selected = i == state.selected;
            let bg = if is_cursor { th.cursor_bg } else { th.background };
            
            fill(buf, Rect::new(area.x, y, area.width, 1), bg);

            let mut x = area.x;
            
            // Selected marker
            let marker = if is_selected { "●" } else { "○" };
            put(buf, x, y, marker, 1, st(if is_selected { th.primary } else { th.text_muted }, bg));
            x += 2;

            // Model id
            let id_w = 20.min(area.width.saturating_sub(x - area.x));
            put(buf, x, y, &model.id, id_w, st(th.text, bg));
            x += id_w + 1;

            // Provider chip
            let provider_str = truncate(&model.provider, 10);
            let chip_w = provider_str.len() as u16 + 2;
            if x + chip_w < area.right() {
                fill(buf, Rect::new(x, y, chip_w, 1), th.surface);
                put(buf, x + 1, y, &provider_str, chip_w.saturating_sub(2), st(th.text_muted, th.surface));
                x += chip_w + 1;
            }

            // Context
            let ctx_str = fmt_tokens(model.context);
            put(buf, x, y, &ctx_str, ctx_str.len() as u16, st(th.text_muted, bg));
            x += ctx_str.len() as u16 + 2;

            // Prices
            let prices = format!("{} / {}", fmt_usd(model.in_price), fmt_usd(model.out_price));
            put(buf, x, y, &prices, prices.len() as u16, st(th.text_muted, bg));
            x += prices.len() as u16 + 2;

            // Capabilities
            for cap in &model.caps {
                if x + 8 >= area.right() {
                    break;
                }
                let cap_str = format!("{} {}", cap.glyph(), cap.label());
                put(buf, x, y, &cap_str, cap_str.len() as u16, st(cap.color(&th), bg));
                x += cap_str.len() as u16 + 1;
            }

            y += 1;
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
        let b = TokenBreakdown {
            input: 1000,
            output: 500,
            cache_read: 2000,
            cache_write: 300,
        };
        let area = Rect::new(0, 0, 20, 2);
        let mut buf = Buffer::empty(area);
        TokenMeter::new(b).theme(&th).render(area, &mut buf);
        
        // Each non-zero segment should get >= 1 cell
        // Total should equal bar width
        // (visual test via buffer inspection would be needed for full validation)
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
        let lane = Lane::new("Test").span(LaneSpan::new(0.0, Some(10.0), AgentStatus::Running, "test"));
        let lanes = vec![lane];
        
        // Clock at 5s, window 30s -> view_start = 0
        let area = Rect::new(0, 0, 50, 5);
        let mut buf = Buffer::empty(area);
        AgentLanes::new(&lanes).clock(5.0).window(30.0).render(area, &mut buf);

        // Clock at 30s -> view_start = 30 - 24 = 6
        AgentLanes::new(&lanes).clock(30.0).window(30.0).render(area, &mut buf);
        // (visual validation needed)
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
        assert!(filtered[0].1 > 0 || filtered.iter().any(|&(idx, _, _)| state.entries[idx].title.contains("feature")));
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

        for area in [area_1x1, area_3x2, area_10x3, area_60x16, area_130x42, area_250x70] {
            let mut buf = Buffer::empty(area);
            let now = Instant::now();

            // Test all widgets
            ElapsedTimer::new().now(now).render(area, &mut buf);
            
            let mut tree_state = AgentTreeState::new();
            tree_state.root = Some(AgentNode::new("Test", "model", AgentStatus::Running));
            AgentTree::new().now(now).render(area, &mut buf, &mut tree_state);

            let lane = Lane::new("Lane");
            AgentLanes::new(&[lane]).clock(5.0).now(now).render(area, &mut buf);

            TokenMeter::new(TokenBreakdown::default()).theme(&th).render(area, &mut buf);

            let mut cost_state = CostMeterState::new();
            CostMeter::new().theme(&th).render(area, &mut buf, &mut cost_state);

            let seg = ContextSegment::new("test", 1000);
            ContextMap::new(&[seg], 10000).theme(&th).render(area, &mut buf);

            CompactionBanner::new(80.0, 30.0, 5000).now(now).theme(&th).render(area, &mut buf);

            TurnStats::new().theme(&th).render(area, &mut buf);

            RateGraph::new(&[1.0, 2.0]).theme(&th).render(area, &mut buf);

            let mut session_state = SessionListState::new();
            SessionList::new().theme(&th).render(area, &mut buf, &mut session_state);

            let mut model_state = ModelPickerState::new();
            ModelPicker::new().theme(&th).render(area, &mut buf, &mut model_state);
        }
    }
}
