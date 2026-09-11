//! Navigation widgets: Tabs, List, Tree, Menu, Breadcrumbs, Paginator.

use std::time::Instant;

use tuile::draw::{Border, fill, put};
use tuile::prelude::*;

use super::{Ctx, Page};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Menu,
    Breadcrumbs,
    UnderlineTabs,
    BoxedTabs,
    PillsTabs,
    SegmentedTabs,
    MinimalTabs,
    TabbedContent,
    List,
    Paginator,
    BigMenu,
}

pub struct NavigationPage {
    focus: Focus<Id>,
    menu: MenuBarState,
    breadcrumbs: BreadcrumbsState,
    underline_tabs: TabBarState,
    boxed_tabs: TabBarState,
    /// Owned so the closable tab can actually be removed when its × is clicked.
    boxed_items: Vec<TabItem>,
    pills_tabs: TabBarState,
    segmented_tabs: TabBarState,
    minimal_tabs: TabBarState,
    tabbed: TabBarState,
    list: ListViewState,
    tree: TreeViewState,
    multi_list: ListViewState,
    paginator: PaginatorState,
    context_menu: ContextMenuState,
    tree_roots: Vec<TreeNode>,
    list_filter: String,
    big_menu: BigMenuState,
}

impl NavigationPage {
    fn new() -> Self {
        let mut tree_roots = vec![
            TreeNode::new("src").icon("").with_children(vec![
                TreeNode::new("main.rs").icon("").detail("2.4K"),
                TreeNode::new("lib.rs").icon("").detail("1.8K"),
                TreeNode::new("widgets").icon("").with_children(vec![
                    TreeNode::new("tabs.rs").icon("").detail("12K"),
                    TreeNode::new("list.rs").icon("").detail("8K"),
                    TreeNode::new("tree.rs").icon("").detail("10K"),
                ]),
            ]),
            TreeNode::new("tests").icon("").with_children(vec![
                TreeNode::new("integration.rs").icon("").detail("3.2K"),
            ]),
            TreeNode::new("docs").icon(""),
            TreeNode::new("Cargo.toml").icon("").detail("1.2K"),
            TreeNode::new("README.md").icon("").detail("4.5K"),
        ];
        TreeNode::assign_ids(&mut tree_roots);

        let mut tree = TreeViewState::new();
        tree.expand(TreeId(0));

        Self {
            focus: Focus::new([
                Id::Menu,
                Id::Breadcrumbs,
                Id::UnderlineTabs,
                Id::BoxedTabs,
                Id::PillsTabs,
                Id::SegmentedTabs,
                Id::MinimalTabs,
                Id::TabbedContent,
                Id::List,
                Id::Paginator,
                Id::BigMenu,
            ]),
            menu: MenuBarState::new(),
            breadcrumbs: BreadcrumbsState::new(),
            underline_tabs: TabBarState::new(0),
            boxed_tabs: TabBarState::new(1),
            boxed_items: vec![
                "Files".into(),
                "Search".into(),
                TabItem::new("Git").closable(true),
                "Debug".into(),
            ],
            pills_tabs: TabBarState::new(0),
            segmented_tabs: TabBarState::new(2),
            minimal_tabs: TabBarState::new(0),
            tabbed: TabBarState::new(0),
            list: ListViewState::new(),
            tree,
            multi_list: ListViewState::new(),
            paginator: PaginatorState::new(),
            context_menu: ContextMenuState::new(),
            tree_roots,
            list_filter: String::new(),
            big_menu: BigMenuState::default(),
        }
    }
}

impl Default for NavigationPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Page for NavigationPage {
    fn title(&self) -> &'static str {
        "Navigation"
    }

    fn subtitle(&self) -> &'static str {
        "Tabs, Lists, Trees, Menus, Breadcrumbs, Paginators"
    }

    fn icon(&self) -> &'static str {
        "≡"
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = &ctx.theme;
        fill(buf, area, th.background);

        if area.height < 10 {
            return;
        }

        // top row: menu bar
        let menu_area = Rect::new(area.x, area.y, area.width, 1);
        let menus = vec![
            MenuDef {
                title: "File".to_string(),
                items: vec![
                    MenuItem::action(1, "New"),
                    MenuItem::action(2, "Open"),
                    MenuItem::submenu(
                        "Recent",
                        vec![
                            MenuItem::action(10, "project-1.rs"),
                            MenuItem::action(11, "project-2.rs"),
                            MenuItem::action(12, "project-3.rs"),
                        ],
                    ),
                    MenuItem::separator(),
                    MenuItem::action(3, "Save").shortcut("^S"),
                    MenuItem::action(4, "Save As"),
                    MenuItem::separator(),
                    MenuItem::action(5, "Quit").shortcut("^Q"),
                ],
            },
            MenuDef {
                title: "Edit".to_string(),
                items: vec![
                    MenuItem::action(20, "Undo"),
                    MenuItem::action(21, "Redo"),
                    MenuItem::separator(),
                    MenuItem::action(22, "Cut"),
                    MenuItem::action(23, "Copy"),
                    MenuItem::action(24, "Paste"),
                    MenuItem::separator(),
                    MenuItem::action(25, "Preferences…"),
                ],
            },
            MenuDef {
                title: "View".to_string(),
                items: vec![
                    MenuItem::action(30, "Sidebar").shortcut("^B").checked(true),
                    MenuItem::action(31, "Status bar").checked(true),
                    MenuItem::separator(),
                    MenuItem::action(32, "Zoom In"),
                    MenuItem::action(33, "Zoom Out"),
                ],
            },
            MenuDef {
                title: "Help".to_string(),
                items: vec![MenuItem::action(40, "Docs"), MenuItem::action(41, "About")],
            },
        ];
        MenuBar::new(menus)
            .focused(self.focus.is(Id::Menu))
            .theme(th)
            .render(menu_area, buf, &mut self.menu);

        let breadcrumb_area = Rect::new(area.x, area.y + 1, area.width, 1);
        Breadcrumbs::new(vec![
            "Home".to_string(),
            "Projects".to_string(),
            "tuile".to_string(),
            "showcase".to_string(),
            "pages".to_string(),
            "navigation.rs".to_string(),
        ])
        .focused(self.focus.is(Id::Breadcrumbs))
        .theme(th)
        .render(breadcrumb_area, buf, &mut self.breadcrumbs);

        // main content area
        let content_y = area.y + 2;
        let content_h = area.height.saturating_sub(2);
        let content = Rect::new(area.x, content_y, area.width, content_h);

        if content.width < 60 || content.height < 10 {
            return;
        }

        // two columns
        let col_w = content.width / 2;
        let left = Rect::new(
            content.x,
            content.y,
            col_w.saturating_sub(1),
            content.height,
        );
        let right = Rect::new(
            content.x + col_w,
            content.y,
            content.width.saturating_sub(col_w),
            content.height,
        );

        // LEFT column: split into cards
        let left_half = left.height / 2;
        let left_card1 = Rect::new(left.x, left.y, left.width, left_half.saturating_sub(1));
        let left_card2 = Rect::new(
            left.x,
            left.y + left_half,
            left.width,
            left.height.saturating_sub(left_half),
        );

        // Tab styles card
        let left_inner = Border::Round.draw_titled_with(
            buf,
            left_card1,
            th.border_blurred,
            th.background,
            "Tab styles",
            Alignment::Left,
            tuile::draw::st(th.text, th.background).add_modifier(Modifier::BOLD),
        );
        let mut tab_y = left_inner.y;

        // Underline
        if tab_y + 2 <= left_inner.bottom() {
            let items: Vec<TabItem> = vec!["Home".into(), "Settings".into(), "Help".into()];
            TabBar::new(items)
                .style(TabStyle::Underline)
                .focused(self.focus.is(Id::UnderlineTabs))
                .now(ctx.now)
                .theme(th)
                .render(
                    Rect::new(left_inner.x, tab_y, left_inner.width, 2),
                    buf,
                    &mut self.underline_tabs,
                );
            tab_y += 3;
        }

        // Boxed
        if tab_y + 2 <= left_inner.bottom() && !self.boxed_items.is_empty() {
            TabBar::new(self.boxed_items.clone())
                .style(TabStyle::Boxed)
                .focused(self.focus.is(Id::BoxedTabs))
                .theme(th)
                .render(
                    Rect::new(left_inner.x, tab_y, left_inner.width, 3),
                    buf,
                    &mut self.boxed_tabs,
                );
            tab_y += 4;
        }

        // Pills
        if tab_y < left_inner.bottom() {
            let items: Vec<TabItem> = vec![
                TabItem::new("All").icon("■"),
                TabItem::new("Active").badge("3"),
                TabItem::new("Done"),
            ];
            TabBar::new(items)
                .style(TabStyle::Pills)
                .focused(self.focus.is(Id::PillsTabs))
                .theme(th)
                .render(
                    Rect::new(left_inner.x, tab_y, left_inner.width, 1),
                    buf,
                    &mut self.pills_tabs,
                );
            tab_y += 2;
        }

        // Segmented
        if tab_y < left_inner.bottom() {
            let items: Vec<TabItem> =
                vec!["Day".into(), "Week".into(), "Month".into(), "Year".into()];
            TabBar::new(items)
                .style(TabStyle::Segmented)
                .focused(self.focus.is(Id::SegmentedTabs))
                .theme(th)
                .render(
                    Rect::new(left_inner.x, tab_y, left_inner.width, 1),
                    buf,
                    &mut self.segmented_tabs,
                );
            tab_y += 2;
        }

        // Minimal
        if tab_y < left_inner.bottom() {
            let items: Vec<TabItem> = vec![
                "Overview".into(),
                "Details".into(),
                TabItem::new("Settings").disabled(true),
            ];
            TabBar::new(items)
                .style(TabStyle::Minimal)
                .focused(self.focus.is(Id::MinimalTabs))
                .theme(th)
                .render(
                    Rect::new(left_inner.x, tab_y, left_inner.width, 1),
                    buf,
                    &mut self.minimal_tabs,
                );
        }

        // Lower card: TreeView and Breadcrumbs/Paginator
        let lower_inner = Border::Round.draw_titled_with(
            buf,
            left_card2,
            th.border_blurred,
            th.background,
            "TreeView & Navigation",
            Alignment::Left,
            tuile::draw::st(th.text, th.background).add_modifier(Modifier::BOLD),
        );
        let tree_h = (lower_inner.height * 2) / 3;
        let tree_area = Rect::new(lower_inner.x, lower_inner.y, lower_inner.width, tree_h);
        let nav_area = Rect::new(
            lower_inner.x,
            lower_inner.y + tree_h,
            lower_inner.width,
            lower_inner.height.saturating_sub(tree_h),
        );

        // Mini tree
        TreeView::new(self.tree_roots.clone())
            .focused(false)
            .border(Border::None)
            .theme(th)
            .render(tree_area, buf, &mut self.tree);

        // Breadcrumbs
        if nav_area.height >= 3 {
            Breadcrumbs::new(vec![
                "Home".to_string(),
                "Docs".to_string(),
                "API".to_string(),
            ])
            .focused(false)
            .theme(th)
            .render(
                Rect::new(nav_area.x, nav_area.y, nav_area.width, 1),
                buf,
                &mut BreadcrumbsState::new(),
            );

            // Paginator
            Paginator::new(20)
                .window(1)
                .focused(false)
                .theme(th)
                .render(
                    Rect::new(nav_area.x, nav_area.y + 2, nav_area.width, 1),
                    buf,
                    &mut PaginatorState::new(),
                );
        }

        // RIGHT: tabbed content on top, the btop-style big-font menu below when there is room
        let menu_h = if right.height >= 30 { 13 } else { 0 };
        let (right, big_area) = (
            Rect {
                height: right.height - menu_h,
                ..right
            },
            Rect {
                y: right.bottom() - menu_h,
                height: menu_h,
                ..right
            },
        );
        let right_inner = Border::Round.draw_titled_with(
            buf,
            right,
            th.border_blurred,
            th.background,
            "Tabbed Content",
            Alignment::Left,
            tuile::draw::st(th.text, th.background).add_modifier(Modifier::BOLD),
        );
        if menu_h > 0 {
            let inner = Border::Round.draw_titled_with(
                buf,
                big_area,
                th.border_blurred,
                th.background,
                "Big menu (BigMenu · btop style, also on Monitor: m)",
                Alignment::Left,
                tuile::draw::st(th.text, th.background).add_modifier(Modifier::BOLD),
            );
            let items = ["OPTIONS", "HELP", "QUIT"];
            BigMenu::new(&items)
                .gap(1)
                .focused(self.focus.is(Id::BigMenu))
                .theme(th)
                .render(inner, buf, &mut self.big_menu);
        }

        let tabs_items = vec!["List".into(), "Tree".into(), "Multi-select".into()];

        let content_rect = TabbedContent::new(tabs_items)
            .style(TabStyle::Underline)
            .bordered(true)
            .focused(self.focus.is(Id::TabbedContent))
            .now(ctx.now)
            .theme(th)
            .render(right_inner, buf, &mut self.tabbed);

        // tab content
        if content_rect.width > 4 && content_rect.height > 4 {
            match self.tabbed.active {
                0 => {
                    // List
                    let mut list_area = content_rect;
                    if !self.list_filter.is_empty() {
                        let filter_y = list_area.y;
                        put(
                            buf,
                            list_area.x,
                            filter_y,
                            &format!("Filter: /{}", self.list_filter),
                            list_area.width,
                            tuile::draw::st(th.text_muted, th.surface),
                        );
                        list_area = Rect::new(
                            list_area.x,
                            list_area.y + 1,
                            list_area.width,
                            list_area.height.saturating_sub(1),
                        );
                    }

                    let mut entries: Vec<ListEntry> = Vec::with_capacity(36);
                    for i in 1..=30 {
                        if i > 1 && i % 5 == 1 {
                            entries.push(ListEntry::new("").separator(true));
                        }
                        let mut e = ListEntry::new(format!("Item {}", i))
                            .icon("◦")
                            .detail(&format!("{}ms", i * 10));
                        if i == 15 {
                            e = e.disabled(true);
                        }
                        entries.push(e);
                    }
                    ListView::new(entries)
                        .filter(&self.list_filter)
                        .focused(self.focus.is(Id::List))
                        .details(ListDetail::Right)
                        .theme(th)
                        .render(list_area, buf, &mut self.list);
                }
                1 => {
                    // Tree
                    TreeView::new(self.tree_roots.clone())
                        .focused(self.focus.is(Id::List))
                        .theme(th)
                        .render(content_rect, buf, &mut self.tree);
                }
                2 => {
                    // Multi-select
                    let entries: Vec<ListEntry> = vec![
                        "Option A", "Option B", "Option C", "Option D", "Option E", "Option F",
                        "Option G", "Option H", "Option I", "Option J",
                    ]
                    .into_iter()
                    .map(ListEntry::new)
                    .collect();
                    ListView::new(entries)
                        .multi_select(true)
                        .focused(self.focus.is(Id::List))
                        .highlight(ListHighlight::Bar)
                        .theme(th)
                        .render(content_rect, buf, &mut self.multi_list);
                }
                _ => {}
            }

            // paginator at bottom
            let pag_y = right_inner.bottom().saturating_sub(1);
            if pag_y > right_inner.y {
                Paginator::new(42)
                    .window(2)
                    .focused(self.focus.is(Id::Paginator))
                    .theme(th)
                    .render(
                        Rect::new(right_inner.x, pag_y, right_inner.width, 1),
                        buf,
                        &mut self.paginator,
                    );
            }
        }

        // overlays (last)
        self.menu.render_overlay(buf, buf.area);
        self.context_menu.render_overlay(buf, buf.area);
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        let mut out = Outcome::Ignored;

        match ev {
            Event::Key(k) => {
                if k.kind != tuile::crossterm::event::KeyEventKind::Press {
                    return Outcome::Ignored;
                }

                // F10 toggles menu
                if matches!(k.code, KeyCode::F(10)) {
                    if self.menu.is_open() {
                        self.menu.close();
                    } else {
                        self.menu.open(0);
                    }
                    return Outcome::Changed;
                }

                // / starts filter
                if matches!(k.code, KeyCode::Char('/')) && !self.focus.is(Id::List) {
                    self.list_filter.clear();
                    return Outcome::Consumed;
                }

                // filter typing
                if !self.list_filter.is_empty() || matches!(k.code, KeyCode::Char('/')) {
                    match k.code {
                        KeyCode::Char(c) if c != '/' => {
                            self.list_filter.push(c);
                            return Outcome::Changed;
                        }
                        KeyCode::Backspace => {
                            self.list_filter.pop();
                            return Outcome::Changed;
                        }
                        KeyCode::Esc => {
                            self.list_filter.clear();
                            return Outcome::Changed;
                        }
                        _ => {}
                    }
                }

                // menu gets keys when open or focused
                if self.menu.is_open() || self.focus.is(Id::Menu) {
                    out |= self.menu.handle_key(*k);
                    if out.is_changed() {
                        return out;
                    }
                }

                // context menu
                if self.context_menu.open {
                    out |= self.context_menu.handle_key(*k);
                    if out.is_changed() {
                        return out;
                    }
                }

                // 'm' key opens context menu at cursor position
                if matches!(k.code, KeyCode::Char('m')) && self.focus.is(Id::List) {
                    let cursor_y = self.list.hits.y
                        + (self.list.cursor.saturating_sub(self.list.scroll)) as u16;
                    self.context_menu.open_at(Position {
                        x: self.list.hits.x + 10,
                        y: cursor_y,
                    });
                    return Outcome::Changed;
                }

                // focus navigation
                if matches!(k.code, KeyCode::Tab) {
                    self.focus.next();
                    return Outcome::Changed;
                }
                if matches!(k.code, KeyCode::BackTab) {
                    self.focus.prev();
                    return Outcome::Changed;
                }

                // focused widget
                if self.focus.is(Id::UnderlineTabs) {
                    out |= self.underline_tabs.handle_key(*k);
                } else if self.focus.is(Id::BoxedTabs) {
                    out |= self.boxed_tabs.handle_key(*k);
                } else if self.focus.is(Id::PillsTabs) {
                    out |= self.pills_tabs.handle_key(*k);
                } else if self.focus.is(Id::SegmentedTabs) {
                    out |= self.segmented_tabs.handle_key(*k);
                } else if self.focus.is(Id::MinimalTabs) {
                    out |= self.minimal_tabs.handle_key(*k);
                } else if self.focus.is(Id::TabbedContent) {
                    out |= self.tabbed.handle_key(*k);
                } else if self.focus.is(Id::List) {
                    match self.tabbed.active {
                        0 => out |= self.list.handle_key(*k),
                        1 => out |= self.tree.handle_key(*k),
                        2 => out |= self.multi_list.handle_key(*k),
                        _ => {}
                    }
                } else if self.focus.is(Id::Paginator) {
                    out |= self.paginator.handle_key(*k);
                } else if self.focus.is(Id::Breadcrumbs) {
                    out |= self.breadcrumbs.handle_key(*k);
                } else if self.focus.is(Id::BigMenu) {
                    out |= self.big_menu.handle_key(*k);
                    if let Some(i) = self.big_menu.take_activated() {
                        ctx.notify(
                            format!("Big menu: {}", ["Options", "Help", "Quit"][i.min(2)]),
                            Variant::Primary,
                        );
                    }
                }
            }
            Event::Mouse(m) => {
                out |= self.menu.handle_mouse(*m);
                out |= self.context_menu.handle_mouse(*m);
                out |= self.breadcrumbs.handle_mouse(*m);
                out |= self.underline_tabs.handle_mouse(*m);
                out |= self.boxed_tabs.handle_mouse(*m);
                if let Some(i) = self.boxed_tabs.take_closed()
                    && i < self.boxed_items.len()
                {
                    let label = self.boxed_items.remove(i).label;
                    let last = self.boxed_items.len().saturating_sub(1);
                    self.boxed_tabs.set_active(self.boxed_tabs.active.min(last));
                    ctx.notify(format!("Closed {label}"), Variant::Default);
                }
                out |= self.pills_tabs.handle_mouse(*m);
                out |= self.segmented_tabs.handle_mouse(*m);
                out |= self.minimal_tabs.handle_mouse(*m);
                out |= self.tabbed.handle_mouse(*m);

                match self.tabbed.active {
                    0 => out |= self.list.handle_mouse(*m),
                    1 => out |= self.tree.handle_mouse(*m),
                    2 => out |= self.multi_list.handle_mouse(*m),
                    _ => {}
                }

                out |= self.paginator.handle_mouse(*m);
                out |= self.big_menu.handle_mouse(*m);
                if let Some(i) = self.big_menu.take_activated() {
                    self.focus.set(Id::BigMenu);
                    ctx.notify(
                        format!("Big menu: {}", ["Options", "Help", "Quit"][i.min(2)]),
                        Variant::Primary,
                    );
                }

                // right-click in list opens context menu
                if matches!(
                    m.kind,
                    tuile::crossterm::event::MouseEventKind::Down(
                        tuile::crossterm::event::MouseButton::Right
                    )
                ) {
                    self.context_menu.open_at(Position {
                        x: m.column,
                        y: m.row,
                    });
                    return Outcome::Changed;
                }

                // click outside context menu closes it
                if self.context_menu.open
                    && matches!(
                        m.kind,
                        tuile::crossterm::event::MouseEventKind::Down(
                            tuile::crossterm::event::MouseButton::Left
                        )
                    )
                {
                    let pos = Position {
                        x: m.column,
                        y: m.row,
                    };
                    if !self.context_menu.area.contains(pos) {
                        self.context_menu.close();
                        return Outcome::Changed;
                    }
                }
            }
            _ => {}
        }

        // handle actions
        if let Some(action) = self.menu.take_action() {
            ctx.notify(
                format!("Menu action: {}", action),
                tuile::theme::Variant::Primary,
            );
            out = Outcome::Changed;
        }

        if let Some(action) = self.context_menu.take_action() {
            ctx.notify(
                format!("Context action: {}", action),
                tuile::theme::Variant::Primary,
            );
            out = Outcome::Changed;
        }

        if let Some(seg) = self.breadcrumbs.take_clicked() {
            ctx.notify(
                format!("Breadcrumb: {}", seg),
                tuile::theme::Variant::Primary,
            );
            out = Outcome::Changed;
        }

        if let Some(activated) = self.list.take_activated() {
            ctx.notify(
                format!("List activated: {}", activated),
                tuile::theme::Variant::Primary,
            );
            out = Outcome::Changed;
        }

        if let Some(activated) = self.tree.take_activated() {
            ctx.notify(
                format!("Tree activated: {:?}", activated),
                tuile::theme::Variant::Primary,
            );
            out = Outcome::Changed;
        }

        out
    }

    fn animating(&self, _now: Instant) -> bool {
        false
    }

    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("F10", "Menu"),
            ("/", "Filter list"),
            ("Tab", "Next focus"),
            ("←/→/↑/↓", "Navigate"),
            ("Enter", "Activate"),
        ]
    }
}
