//! Tree view with expand/collapse, tree guides, icons, details, scrolling, keyboard navigation,
//! and activation. Supports hierarchical data with `TreeNode` and `TreeId`.
//!
//! ```
//! use tuile::prelude::*;
//! # let area = Rect::new(0, 0, 40, 10);
//! # let mut buf = Buffer::empty(area);
//! let mut roots = vec![TreeNode::new("src").with_children(vec![TreeNode::new("main.rs")])];
//! TreeNode::assign_ids(&mut roots);
//! let mut state = TreeViewState::new();
//! TreeView::new(roots).render(area, &mut buf, &mut state);
//! ```

use std::collections::HashSet;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Alignment, Rect};
use ratatui_core::style::Modifier;
use ratatui_core::widgets::StatefulWidget;
use unicode_width::UnicodeWidthStr;

use crate::core::{Interactive, MinSize, Outcome, is_press, mouse_in, mouse_pos, wheel_delta};
use crate::draw::{Border, fill, put, put_right, refuse, st};
use crate::layout::pad;
use crate::theme::{self, Theme};
use crate::widgets::scrollbar::{Scrollbar, ScrollbarState, keep_visible};

// types

/// Unique tree node identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TreeId(pub usize);

/// One tree node: id, label, optional icon/detail, children.
#[derive(Clone, Debug)]
pub struct TreeNode {
    pub id: TreeId,
    pub label: String,
    pub icon: Option<String>,
    pub detail: Option<String>,
    pub disabled: bool,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: TreeId(0),
            label: label.into(),
            icon: None,
            detail: None,
            disabled: false,
            children: Vec::new(),
        }
    }

    pub fn icon(mut self, i: &str) -> Self {
        self.icon = Some(i.to_string());
        self
    }

    pub fn detail(mut self, d: &str) -> Self {
        self.detail = Some(d.to_string());
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn with_children(mut self, children: Vec<TreeNode>) -> Self {
        self.children = children;
        self
    }

    /// Assign unique sequential ids to a forest, depth-first.
    pub fn assign_ids(roots: &mut [TreeNode]) {
        let mut next_id = 0;
        fn go(node: &mut TreeNode, next: &mut usize) {
            node.id = TreeId(*next);
            *next += 1;
            for child in &mut node.children {
                go(child, next);
            }
        }
        for root in roots {
            go(root, &mut next_id);
        }
    }
}

// visible rows

/// One visible row in the flattened tree.
#[derive(Clone, Debug)]
pub struct TreeRow {
    pub id: TreeId,
    pub depth: usize,
    pub has_children: bool,
    pub is_last: Vec<bool>, // for each ancestor: is it the last child?
}

/// Flatten visible tree rows (respecting expanded set).
pub fn visible_rows(roots: &[TreeNode], expanded: &HashSet<TreeId>) -> Vec<TreeRow> {
    let mut out = Vec::new();
    fn go(
        node: &TreeNode,
        depth: usize,
        is_last: &mut Vec<bool>,
        expanded: &HashSet<TreeId>,
        out: &mut Vec<TreeRow>,
    ) {
        out.push(TreeRow {
            id: node.id,
            depth,
            has_children: !node.children.is_empty(),
            is_last: is_last.clone(),
        });
        if expanded.contains(&node.id) {
            for (i, child) in node.children.iter().enumerate() {
                let last = i == node.children.len() - 1;
                is_last.push(last);
                go(child, depth + 1, is_last, expanded, out);
                is_last.pop();
            }
        }
    }
    for (i, root) in roots.iter().enumerate() {
        let mut is_last = vec![i == roots.len() - 1];
        go(root, 0, &mut is_last, expanded, &mut out);
    }
    out
}

// builder

/// Hierarchical tree with collapse/expand markers, guides, border, and title.
#[derive(Clone, Debug)]
pub struct TreeView {
    roots: Vec<TreeNode>,
    guides: bool,
    markers: (String, String), // collapsed, expanded
    border: Option<Border>,
    title: String,
    focused: bool,
    enabled: bool,
    now: Option<Instant>,
    theme: Option<Theme>,
}

impl TreeView {
    pub fn new(roots: Vec<TreeNode>) -> Self {
        Self {
            roots,
            guides: true,
            markers: ("▸".to_string(), "▾".to_string()),
            border: Some(Border::Round),
            title: String::new(),
            focused: false,
            enabled: true,
            now: None,
            theme: None,
        }
    }

    pub fn guides(mut self, g: bool) -> Self {
        self.guides = g;
        self
    }

    pub fn markers(mut self, collapsed: &str, expanded: &str) -> Self {
        self.markers = (collapsed.to_string(), expanded.to_string());
        self
    }

    pub fn border(mut self, b: Border) -> Self {
        self.border = Some(b);
        self
    }

    pub fn title(mut self, t: &str) -> Self {
        self.title = t.to_string();
        self
    }

    pub fn focused(mut self, f: bool) -> Self {
        self.focused = f;
        self
    }

    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }

    pub fn now(mut self, n: Instant) -> Self {
        self.now = Some(n);
        self
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.theme = Some(*t);
        self
    }
}

impl MinSize for TreeView {
    /// (3, 3) with border (default), (1, 1) without.
    fn min_size(&self) -> (u16, u16) {
        if self.border.is_some() {
            (3, 3)
        } else {
            (1, 1)
        }
    }
}

// state

/// Tree view state: cursor (visible row index), expanded set, scroll, hover, activation.
#[derive(Clone, Debug)]
pub struct TreeViewState {
    pub cursor: usize,
    pub scroll: usize,
    pub expanded: HashSet<TreeId>,
    pub hover: Option<usize>,
    pub body: Rect,
    pub scrollbar: ScrollbarState,
    pub activated: Option<TreeId>,
    pub row_ids: Vec<TreeId>,
    marker_hits: Vec<Rect>,
    last_click: Option<(usize, Instant)>,
}

impl TreeViewState {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            scroll: 0,
            expanded: HashSet::new(),
            hover: None,
            body: Rect::ZERO,
            scrollbar: ScrollbarState::default(),
            activated: None,
            row_ids: Vec::new(),
            marker_hits: Vec::new(),
            last_click: None,
        }
    }

    pub fn expand(&mut self, id: TreeId) {
        self.expanded.insert(id);
    }

    pub fn collapse(&mut self, id: TreeId) {
        self.expanded.remove(&id);
    }

    pub fn toggle(&mut self, id: TreeId) {
        if self.expanded.contains(&id) {
            self.collapse(id);
        } else {
            self.expand(id);
        }
    }

    pub fn expand_all(&mut self, roots: &[TreeNode]) {
        fn go(node: &TreeNode, set: &mut HashSet<TreeId>) {
            if !node.children.is_empty() {
                set.insert(node.id);
            }
            for child in &node.children {
                go(child, set);
            }
        }
        for root in roots {
            go(root, &mut self.expanded);
        }
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
    }

    pub fn selected(&self) -> Option<TreeId> {
        self.row_ids.get(self.cursor).copied()
    }

    pub fn select(&mut self, id: TreeId, roots: &[TreeNode]) {
        // expand ancestors
        fn find_path(node: &TreeNode, target: TreeId, path: &mut Vec<TreeId>) -> bool {
            if node.id == target {
                return true;
            }
            for child in &node.children {
                path.push(node.id);
                if find_path(child, target, path) {
                    return true;
                }
                path.pop();
            }
            false
        }
        let mut path = Vec::new();
        for root in roots {
            if find_path(root, id, &mut path) {
                for ancestor in path {
                    self.expanded.insert(ancestor);
                }
                break;
            }
            path.clear();
        }
    }

    pub fn take_activated(&mut self) -> Option<TreeId> {
        self.activated.take()
    }
}

impl Default for TreeViewState {
    fn default() -> Self {
        Self::new()
    }
}

impl Interactive for TreeViewState {
    fn handle_key(&mut self, k: KeyEvent) -> Outcome {
        if !is_press(&k) {
            return Outcome::Ignored;
        }
        let current_id = self.row_ids.get(self.cursor).copied();
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    return Outcome::Changed;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.cursor + 1 < self.row_ids.len() {
                    self.cursor += 1;
                    return Outcome::Changed;
                }
            }
            KeyCode::Right => {
                // expand if collapsed, or move to first child if already expanded
                if let Some(id) = current_id {
                    if self.expanded.contains(&id) {
                        // already expanded, move to first child
                        if self.cursor + 1 < self.row_ids.len() {
                            self.cursor += 1;
                            return Outcome::Changed;
                        }
                    } else {
                        self.expand(id);
                        return Outcome::Changed;
                    }
                }
            }
            KeyCode::Left => {
                // collapse if expanded, or jump to parent if collapsed/leaf
                if let Some(id) = current_id {
                    if self.expanded.contains(&id) {
                        self.collapse(id);
                        return Outcome::Changed;
                    } else {
                        // find parent (previous row with smaller depth)
                        // simplified: just move up
                        if self.cursor > 0 {
                            self.cursor -= 1;
                            return Outcome::Changed;
                        }
                    }
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(id) = current_id {
                    self.toggle(id);
                    let outcome = if matches!(k.code, KeyCode::Enter) {
                        self.activated = Some(id);
                        Outcome::Submitted
                    } else {
                        Outcome::Changed
                    };
                    return outcome;
                }
            }
            KeyCode::Char('*') => {
                // expand all under cursor (not implemented fully, just toggle)
                if let Some(id) = current_id {
                    self.toggle(id);
                    return Outcome::Changed;
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                if self.cursor != 0 {
                    self.cursor = 0;
                    return Outcome::Changed;
                }
            }
            KeyCode::End | KeyCode::Char('G') => {
                let last = self.row_ids.len().saturating_sub(1);
                if self.cursor != last {
                    self.cursor = last;
                    return Outcome::Changed;
                }
            }
            KeyCode::PageUp => {
                self.cursor = self.cursor.saturating_sub(10);
                return Outcome::Changed;
            }
            KeyCode::PageDown => {
                self.cursor = (self.cursor + 10).min(self.row_ids.len().saturating_sub(1));
                return Outcome::Changed;
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Ignored
    }

    fn handle_mouse(&mut self, m: MouseEvent) -> Outcome {
        let pos = mouse_pos(&m);
        let mut out = Outcome::Ignored;

        // scrollbar
        out |= self.scrollbar.handle_mouse(m);
        self.scroll = self.scrollbar.offset;

        // hover
        if self.body.contains(pos) {
            let row = (pos.y - self.body.y) as usize + self.scroll;
            if self.hover != Some(row) {
                self.hover = Some(row);
                out = Outcome::Consumed;
            }

            // click marker
            if matches!(m.kind, MouseEventKind::Down(MouseButton::Left)) {
                if row < self.marker_hits.len()
                    && self.marker_hits[row].contains(pos)
                    && let Some(id) = self.row_ids.get(row)
                {
                    self.toggle(*id);
                    return Outcome::Changed;
                }

                // click row: double-click activates
                if row < self.row_ids.len() {
                    let now = Instant::now();
                    if let Some((last_row, last_time)) = self.last_click
                        && last_row == row
                        && now.duration_since(last_time).as_millis() < 400
                        && let Some(id) = self.row_ids.get(row)
                    {
                        self.activated = Some(*id);
                        self.last_click = None;
                        return Outcome::Submitted;
                    }
                    self.last_click = Some((row, now));
                    self.cursor = row;
                    return Outcome::Changed;
                }
            }
        } else {
            if self.hover.is_some() {
                self.hover = None;
                out = Outcome::Consumed;
            }
        }

        // wheel
        if let Some(delta) = wheel_delta(&m)
            && mouse_in(self.body, &m)
        {
            if delta > 0 {
                self.scroll = self.scroll.saturating_add(1);
            } else {
                self.scroll = self.scroll.saturating_sub(1);
            }
            return Outcome::Consumed;
        }

        out
    }
}

// render

impl StatefulWidget for TreeView {
    type State = TreeViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let th = self.theme.unwrap_or_else(theme::current);
        if refuse(buf, area, self.min_size(), th.text_disabled) {
            return;
        }
        let bg = th.surface;
        fill(buf, area, bg);

        // border
        let mut inner = area;
        if let Some(border) = self.border {
            let border_color = if self.focused {
                th.border
            } else {
                th.border_blurred
            };
            if !self.title.is_empty() {
                border.draw_titled(buf, area, border_color, bg, &self.title, Alignment::Left);
            } else {
                border.draw(buf, area, border_color, bg);
            }
            inner = pad(area, 1, 1);
        }

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        // compute visible rows
        let rows = visible_rows(&self.roots, &state.expanded);
        state.row_ids = rows.iter().map(|r| r.id).collect();

        if rows.is_empty() {
            return;
        }

        // clamp cursor
        state.cursor = state.cursor.min(rows.len().saturating_sub(1));

        // scrollbar area
        let sb_area = Rect::new(inner.right().saturating_sub(1), inner.y, 1, inner.height);
        let body = Rect::new(
            inner.x,
            inner.y,
            inner.width.saturating_sub(1),
            inner.height,
        );
        state.body = body;

        // scroll to cursor
        state.scroll = keep_visible(state.scroll, state.cursor, body.height as usize);

        state.marker_hits.clear();
        state.marker_hits.resize(rows.len(), Rect::ZERO);

        // render rows
        for row_idx in 0..body.height as usize {
            let vis_idx = row_idx + state.scroll;
            if vis_idx >= rows.len() {
                break;
            }

            let row = &rows[vis_idx];
            let y = body.y + row_idx as u16;

            let is_cursor = vis_idx == state.cursor;
            let is_hover = state.hover == Some(vis_idx);

            let Some(node) = find_node(&self.roots, row.id) else {
                continue;
            };

            let row_bg = if is_cursor && self.focused {
                th.cursor_bg
            } else if is_hover {
                th.hover_bg
            } else {
                bg
            };

            let row_fg = if node.disabled {
                th.text_disabled
            } else if is_cursor && self.focused {
                th.cursor_fg
            } else {
                th.text
            };

            // background
            fill(buf, Rect::new(body.x, y, body.width, 1), row_bg);

            let mut x = body.x;

            // guides
            if self.guides && row.depth > 0 {
                for d in 0..row.depth {
                    let is_last_at_depth = row.is_last.get(d).copied().unwrap_or(false);
                    let sym = if d == row.depth - 1 {
                        if is_last_at_depth { "└─" } else { "├─" }
                    } else {
                        if is_last_at_depth { "  " } else { "│ " }
                    };
                    put(buf, x, y, sym, 2, st(th.border_blurred, row_bg));
                    x += 2;
                }
            } else {
                x += row.depth as u16 * 2;
            }

            // marker
            if row.has_children {
                let marker = if state.expanded.contains(&row.id) {
                    &self.markers.1
                } else {
                    &self.markers.0
                };
                let marker_w = marker.width() as u16;
                put(buf, x, y, marker, marker_w, st(th.accent, row_bg));
                state.marker_hits[vis_idx] = Rect::new(x, y, marker_w, 1);
                x += marker_w + 1;
            } else {
                x += 1;
            }

            // icon
            if let Some(icon) = &node.icon {
                let icon_w = icon.width() as u16;
                put(buf, x, y, icon, icon_w, st(row_fg, row_bg));
                x += icon_w + 1;
            }

            // label
            let label_w = body.right().saturating_sub(x);
            let mut style = st(row_fg, row_bg);
            if is_cursor && !node.disabled {
                style = style.add_modifier(Modifier::BOLD);
            }
            let used = put(buf, x, y, &node.label, label_w, style);

            // detail: right-aligned when there is room after the label, else inline and truncated
            if let Some(detail) = &node.detail {
                let after = x + used + 2;
                let free = body.right().saturating_sub(after);
                if (detail.width() as u16) < free {
                    put_right(
                        buf,
                        Rect::new(after, y, free, 1),
                        detail,
                        st(th.text_muted, row_bg),
                    );
                } else if free > 3 {
                    put(
                        buf,
                        after,
                        y,
                        &crate::draw::truncate(detail, free as usize),
                        free,
                        st(th.text_muted, row_bg),
                    );
                }
            }
        }

        // scrollbar
        if rows.len() > body.height as usize {
            Scrollbar::vertical(rows.len(), body.height as usize)
                .offset(state.scroll)
                .theme(&th)
                .render(sb_area, buf, &mut state.scrollbar);
        }
    }
}

fn find_node(roots: &[TreeNode], id: TreeId) -> Option<&TreeNode> {
    fn go(node: &TreeNode, id: TreeId) -> Option<&TreeNode> {
        if node.id == id {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = go(child, id) {
                return Some(found);
            }
        }
        None
    }
    for root in roots {
        if let Some(found) = go(root, id) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_id_assignment_is_depth_first() {
        let mut roots = vec![
            TreeNode::new("a").with_children(vec![TreeNode::new("a1"), TreeNode::new("a2")]),
            TreeNode::new("b"),
        ];
        TreeNode::assign_ids(&mut roots);
        assert_eq!(roots[0].id, TreeId(0));
        assert_eq!(roots[0].children[0].id, TreeId(1));
        assert_eq!(roots[0].children[1].id, TreeId(2));
        assert_eq!(roots[1].id, TreeId(3));
    }

    #[test]
    fn visible_rows_respects_expanded() {
        let mut roots = vec![TreeNode::new("a").with_children(vec![TreeNode::new("a1")])];
        TreeNode::assign_ids(&mut roots);
        let rows = visible_rows(&roots, &HashSet::new());
        assert_eq!(rows.len(), 1); // only root

        let mut expanded = HashSet::new();
        expanded.insert(TreeId(0));
        let rows = visible_rows(&roots, &expanded);
        assert_eq!(rows.len(), 2); // root + child
    }

    #[test]
    fn tree_state_toggles_expand() {
        let mut state = TreeViewState::new();
        let id = TreeId(42);
        state.toggle(id);
        assert!(state.expanded.contains(&id));
        state.toggle(id);
        assert!(!state.expanded.contains(&id));
    }

    fn painted(buf: &Buffer) -> bool {
        buf.content().iter().any(|c| {
            c.symbol() != " "
                || c.bg != ratatui_core::style::Color::Reset
                || c.fg != ratatui_core::style::Color::Reset
        })
    }

    #[test]
    fn draws_at_its_minimum_and_refuses_visibly_below_it() {
        let mut roots = vec![TreeNode::new("root")];
        TreeNode::assign_ids(&mut roots);

        // With border (default): minimum is (3, 3)
        let with_border = TreeView::new(roots.clone());
        let (w, h) = with_border.min_size();
        assert_eq!((w, h), (3, 3), "TreeView with border should be (3, 3)");

        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        let mut state = TreeViewState::new();
        TreeView::new(roots.clone()).render(buf.area, &mut buf, &mut state);
        assert!(painted(&buf), "should draw at minimum with border");

        // One cell short
        let mut buf = Buffer::empty(Rect::new(0, 0, w - 1, h));
        let mut state = TreeViewState::new();
        TreeView::new(roots.clone()).render(buf.area, &mut buf, &mut state);
        assert!(
            buf.content().iter().any(|c| c.symbol() == "⋯"),
            "one cell short must refuse visibly with border"
        );

        // Without border: minimum is (1, 1)
        let mut no_border_tree = TreeView::new(roots.clone());
        no_border_tree.border = None;
        let (w2, h2) = no_border_tree.min_size();
        assert_eq!((w2, h2), (1, 1), "TreeView without border should be (1, 1)");

        let mut buf = Buffer::empty(Rect::new(0, 0, w2, h2));
        let mut state = TreeViewState::new();
        let mut tree = TreeView::new(roots.clone());
        tree.border = None;
        tree.render(buf.area, &mut buf, &mut state);
        assert!(painted(&buf), "should draw at minimum without border");

        // Zero area refuses
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 1));
        let mut state = TreeViewState::new();
        let mut tree = TreeView::new(roots);
        tree.border = None;
        tree.render(buf.area, &mut buf, &mut state);
        // Empty buffer with 0 width can't paint ⋯, but painted() will be false
        assert!(!painted(&buf), "zero width draws nothing");
    }

    #[test]
    fn activation_is_a_commit_and_motion_is_not() {
        use crossterm::event::{MouseButton, MouseEventKind};
        let mut roots = vec![TreeNode::new("a"), TreeNode::new("b")];
        TreeNode::assign_ids(&mut roots);
        let mut state = TreeViewState::new();

        // First render to populate row_ids
        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        TreeView::new(roots.clone()).render(area, &mut buf, &mut state);

        // Arrow movement is changed, not submitted
        let moved = state.handle_key(KeyEvent::from(KeyCode::Down));
        assert!(moved.is_changed() && !moved.is_submitted(), "{moved:?}");

        // Space toggles but doesn't activate
        state.cursor = 0;
        let toggled = state.handle_key(KeyEvent::from(KeyCode::Char(' ')));
        assert!(
            toggled.is_changed() && !toggled.is_submitted(),
            "{toggled:?}"
        );
        assert_eq!(state.take_activated(), None);

        // Enter toggles AND activates
        let out = state.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(out.is_submitted(), "{out:?}");
        assert_eq!(state.take_activated(), Some(TreeId(0)));
        assert_eq!(state.take_activated(), None, "drains once");

        // Double-click activates: render again to cache rects
        TreeView::new(roots.clone()).render(area, &mut buf, &mut state);

        // First click on row: changed
        let m1 = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x + 2,
            row: area.y + 1, // inside border
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        let click1 = state.handle_mouse(m1);
        assert!(click1.is_changed() && !click1.is_submitted(), "{click1:?}");

        // Second click within 400ms: submitted
        let click2 = state.handle_mouse(m1);
        assert!(click2.is_submitted(), "{click2:?}");
        assert_eq!(state.take_activated(), Some(TreeId(0)));
    }
}
