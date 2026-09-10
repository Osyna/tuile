//! Options: an omp-style settings screen from stock parts - icon tabs, a group sidebar, an
//! `OptionList` with cursor + value cycling, and a live preview of the composer shape and
//! status line the options describe.

use std::time::Instant;

use tuiforge::draw::{hline, put, st};
use tuiforge::prelude::*;

use super::{Ctx, Page};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Id {
    Tabs,
    Groups,
    Options,
}

const SHAPES: &[&str] = &["bars", "band", "bar", "tall", "tall-thin", "rule", "round", "prompt", "none"];
const SEPS: &[&str] = &["chevron", "dot", "pipe", "arrow", "space"];

pub struct OptionsPage {
    focus: Focus<Id>,
    tabs: TabBarState,
    groups: ListViewState,
    options: OptionListState,
    composer: ComposerState,
}

impl Default for OptionsPage {
    fn default() -> Self {
        let options = OptionListState::new(vec![
            OptionGroup::new(
                "Theme",
                vec![
                    OptionItem::choice("dark", "Dark Theme", &["textual-dark", "nord", "dracula", "tokyo-night", "gruvbox", "catppuccin-mocha"], 0)
                        .hint("applied live"),
                    OptionItem::choice("light", "Light Theme", &["textual-light", "catppuccin-latte", "solarized-light"], 0),
                    OptionItem::choice("symbols", "Symbol Preset", &["unicode", "nerd", "ascii"], 0),
                    OptionItem::bool("colorblind", "Color-Blind Mode", false),
                ],
            ),
            OptionGroup::new(
                "Composer",
                vec![
                    OptionItem::choice("shape", "Composer Shape", SHAPES, 0).hint("see preview"),
                    OptionItem::choice("edge", "Bar Thickness", &["hair", "thin", "half", "full"], 1),
                    OptionItem::text("placeholder", "Placeholder", "Ask anything, edit files, run tools"),
                ],
            ),
            OptionGroup::new(
                "Status Line",
                vec![
                    OptionItem::choice("preset", "Status Line Preset", &["default", "minimal", "full"], 0),
                    OptionItem::choice("sep", "Status Line Separator", SEPS, 0),
                    OptionItem::bool("accent", "Session Accent", true),
                    OptionItem::bool("transparent", "Transparent Status Line", false),
                    OptionItem::bool("hooks", "Show Hook Status", true),
                ],
            ),
            OptionGroup::new(
                "Display",
                vec![
                    OptionItem::choice("scrollback", "Resize Scrollback", &["rebuild", "keep", "clear"], 0),
                    OptionItem::bool("headings", "Large Headings (Kitty)", true),
                    OptionItem::bool("mermaid", "Render Mermaid Diagrams", true),
                    OptionItem::bool("reactions", "Agent Reactions", true),
                    OptionItem::bool("tight", "Tight Layout", false),
                    OptionItem::choice("shimmer", "Shimmer", &["classic", "sweep", "off"], 0),
                    OptionItem::bool("smooth", "Smooth Streaming", true),
                    OptionItem::bool("tokens", "Show Token Usage", false),
                    OptionItem::int("fps", "Max Frame Rate", 60, 15, 120, 15),
                    OptionItem::bool("hw_cursor", "Show Hardware Cursor", true),
                ],
            ),
            OptionGroup::new(
                "Images",
                vec![OptionItem::bool("autoresize", "Auto-Resize Images", true), OptionItem::bool("block", "Block Images", false), OptionItem::action("clear_cache", "Clear image cache…")],
            ),
        ]);
        Self { focus: Focus::new([Id::Options, Id::Groups, Id::Tabs]), tabs: TabBarState::new(0), groups: ListViewState::new(), options, composer: ComposerState::default() }
    }
}

impl OptionsPage {
    fn shape(&self) -> FieldShape {
        let edge = match self.options.choice("edge").unwrap_or("thin") {
            "hair" => Edge::Hair,
            "half" => Edge::Half,
            "full" => Edge::Full,
            _ => Edge::Thin,
        };
        match self.options.choice("shape").unwrap_or("bars") {
            "bar" => FieldShape::Bar(edge),
            "band" => FieldShape::Band,
            "tall" => FieldShape::Tall(Edge::Full),
            "tall-thin" => FieldShape::Tall(edge),
            "rule" => FieldShape::Rule,
            "round" => FieldShape::Round,
            "prompt" => FieldShape::Prompt,
            "none" => FieldShape::None,
            _ => FieldShape::Bars(edge),
        }
    }

    fn sep(&self) -> StatusSep {
        match self.options.choice("sep").unwrap_or("chevron") {
            "dot" => StatusSep::Dot,
            "pipe" => StatusSep::Pipe,
            "arrow" => StatusSep::Arrow,
            "space" => StatusSep::Space,
            _ => StatusSep::Chevron,
        }
    }
}

impl Page for OptionsPage {
    fn title(&self) -> &'static str {
        "Options"
    }
    fn subtitle(&self) -> &'static str {
        "Settings screen: icon tabs + group sidebar + OptionList, live composer & status-line preview"
    }
    fn icon(&self) -> &'static str {
        "⚙"
    }
    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[("↑↓", "Row"), ("←→/Enter", "Change"), ("Tab", "Focus")]
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        let inner = pad(area, 1, 0);
        if inner.width < 40 || inner.height < 8 {
            return;
        }
        let preview_h = if inner.height >= 24 { 6 } else { 0 };
        let [tabs_a, body, preview_a] = Layout::vertical([Constraint::Length(2), Constraint::Fill(1), Constraint::Length(preview_h)]).areas(inner);

        // icon tabs
        let tabs: Vec<TabItem> = [("◐", "Appearance"), ("◆", "Model"), ("⌨", "Interaction"), ("▤", "Context"), ("⌘", "Memory"), ("▣", "Files"), ("›_", "Shell"), ("⚒", "Tools"), ("☰", "Tasks"), ("⊕", "Providers"), ("⧉", "Plugins")]
            .iter()
            .map(|(i, t)| TabItem::new(*t).icon(i))
            .collect();
        TabBar::new(tabs).style(TabStyle::Pills).focused(self.focus.is(Id::Tabs)).now(ctx.now).theme(&th).render(Rect { height: 1, ..tabs_a }, buf, &mut self.tabs);
        hline(buf, tabs_a.x, tabs_a.y + 1, tabs_a.width, "─", st(th.border_blurred, th.background));

        // sidebar of groups (follows the cursor; clicking jumps)
        let side_w = 22.min(body.width / 3);
        let [side, main] = Layout::horizontal([Constraint::Length(side_w), Constraint::Fill(1)]).areas(body);
        self.groups.cursor = self.options.group_index();
        let entries: Vec<ListEntry> = self.options.groups.iter().map(|g| ListEntry::new(g.title.clone())).collect();
        ListView::new(entries).highlight(ListHighlight::Bar).focused(self.focus.is(Id::Groups)).theme(&th).render(pad(side, 1, 0), buf, &mut self.groups);
        for y in main.top()..main.bottom() {
            put(buf, main.x, y, "│", 1, st(th.border_blurred, th.background));
        }

        OptionList::new().focused(self.focus.is(Id::Options)).theme(&th).render(pad(Rect { x: main.x + 1, width: main.width - 1, ..main }, 1, 0), buf, &mut self.options);

        // live preview of what the Composer / Status Line options describe
        if preview_h > 0 {
            hline(buf, preview_a.x, preview_a.y, preview_a.width, "─", st(th.border_blurred, th.background));
            put(buf, preview_a.x, preview_a.y + 1, "Preview:", 8, st(th.text_muted, th.background));
            let shape = self.shape();
            let composer_h = 1 + shape.vertical_chrome();
            let field = Rect { x: preview_a.x + 1, y: preview_a.y + 2, width: preview_a.width.saturating_sub(2), height: composer_h };
            let placeholder = match self.options.get("placeholder") {
                Some(OptionValue::Text(t)) => t.clone(),
                _ => String::new(),
            };
            TextArea::new().shape(shape).placeholder(&placeholder).focused(true).now(ctx.now).theme(&th).render(field, buf, &mut self.composer.editor);
            let segs = [
                StatusSegment::new("π"),
                StatusSegment::new("Opus 5").icon("◕").color(th.warning),
                StatusSegment::new("/tmp").icon("⌂"),
                StatusSegment::new("4.0%/1M").icon("▤"),
                StatusSegment::new("(sub)"),
            ];
            let n = match self.options.choice("preset") {
                Some("minimal") => 2,
                Some("full") => 5,
                _ => 4,
            };
            let mut line = StatusLine::new(&segs[..n]).sep(self.sep()).theme(&th);
            if self.options.bool("accent").unwrap_or(true) {
                line = line.right("omp");
            }
            if !self.options.bool("transparent").unwrap_or(false) {
                line = line.bg(th.surface);
            }
            line.render(Rect { x: field.x, y: field.bottom(), width: field.width, height: 1 }, buf);
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        match ev {
            Event::Key(k) => {
                if self.focus.handle_key(*k).is_consumed() {
                    return Outcome::Consumed;
                }
                let out = match self.focus.current() {
                    Some(Id::Tabs) => self.tabs.handle_key(*k),
                    Some(Id::Groups) => {
                        let out = self.groups.handle_key(*k);
                        if out.is_consumed() || out.is_changed() {
                            self.options.jump_to_group(self.groups.cursor);
                        }
                        out
                    }
                    _ => self.options.handle_key(*k),
                };
                self.apply(ctx);
                out
            }
            Event::Mouse(m) => {
                let mut out = self.tabs.handle_mouse(*m) | self.options.handle_mouse(*m);
                let before = self.groups.cursor;
                out |= self.groups.handle_mouse(*m);
                if self.groups.cursor != before && is_left_down(m) {
                    self.options.jump_to_group(self.groups.cursor);
                }
                self.apply(ctx);
                out
            }
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self, _now: Instant) -> bool {
        true // cursor blink in the preview field
    }
}

impl OptionsPage {
    /// React to changed rows: the theme switches live, actions toast.
    fn apply(&mut self, ctx: &mut Ctx) {
        let Some(key) = self.options.take_changed() else { return };
        match key.as_str() {
            "dark" | "light" => {
                if let Some(name) = self.options.choice(&key)
                    && theme::set_by_name(name)
                {
                    ctx.theme = theme::current();
                }
            }
            "clear_cache" => ctx.notify("Image cache cleared", Variant::Success),
            _ => {}
        }
    }
}
