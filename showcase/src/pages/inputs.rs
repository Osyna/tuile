//! Showcase for input widgets: Input, TextArea, Select, Combobox, MultiSelect.

use std::time::Instant;

use tuiforge::draw::{Border, fill, put, st};
use tuiforge::prelude::*;
use tuiforge::widgets::input::{Input, InputRestrict, InputState};
use tuiforge::widgets::textarea::{CursorStyle, TextArea, TextAreaState};

use super::{Ctx, Page};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Name,
    Email,
    Password,
    Amount,
    Search,
    Country,
    Language,
    Tags,
    Compact,
    Disabled,
    Editor,
}

pub struct InputsPage {
    focus: Focus<Id>,
    name: InputState,
    email: InputState,
    password: InputState,
    amount: InputState,
    search: InputState,
    compact: InputState,
    disabled: InputState,
    country: SelectState,
    language: ComboboxState,
    tags: MultiSelectState,
    editor: TextAreaState,
    readonly: TextAreaState,
}

impl InputsPage {
    fn new() -> Self {
        let email = InputState::with_value("invalid-email");
        
        let mut s = Self {
            focus: Focus::new([
                Id::Name,
                Id::Email,
                Id::Password,
                Id::Amount,
                Id::Search,
                Id::Country,
                Id::Language,
                Id::Tags,
                Id::Compact,
                Id::Disabled,
                Id::Editor,
            ]),
            name: InputState::new(),
            email,
            password: InputState::with_value("weak"),
            amount: InputState::with_value("99"),
            search: InputState::new(),
            compact: InputState::with_value("Compact field"),
            disabled: InputState::with_value("Disabled"),
            country: SelectState::new(&[
                "United States",
                "United Kingdom",
                "Canada",
                "Australia",
                "Germany",
                "France",
                "Japan",
                "China",
            ]),
            language: ComboboxState::new(&[
                "Rust",
                "Python",
                "JavaScript",
                "TypeScript",
                "Go",
                "C++",
                "Java",
                "Ruby",
            ]),
            tags: MultiSelectState::new(&["Frontend", "Backend", "DevOps", "ML", "Mobile", "Database"]),
            editor: TextAreaState::with_text("fn fibonacci(n: usize) -> usize {\n    match n {\n        0 => 0,\n        1 => 1,\n        _ => fibonacci(n - 1) + fibonacci(n - 2),\n    }\n}\n\nfn main() {\n    for i in 0..10 {\n        println!(\"fib({}) = {}\", i, fibonacci(i));\n    }\n}"),
            readonly: TextAreaState::with_text("This textarea is read-only.\nYou can scroll but not edit.\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7"),
        };
        s.country.set_selected(Some(0));
        s.country.dropdown_width = DropdownWidth::Field; // list opens as wide as the field
        s.language.input.set_value("Rust");
        s.language.selected = Some(0);
        s.tags.selected[0] = true;
        s.tags.selected[1] = true;
        s
    }
}

impl Default for InputsPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Page for InputsPage {
    fn title(&self) -> &'static str {
        "Inputs"
    }

    fn subtitle(&self) -> &'static str {
        "Text input, textarea, select, combobox, multiselect"
    }

    fn icon(&self) -> &'static str {
        ">"
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        ctx.area = area;
        let th = ctx.theme;

        if area.width < 80 || area.height < 30 {
            put(buf, area.x + 2, area.y + 2, "Window too small", area.width.saturating_sub(4), st(th.text_muted, th.background));
            return;
        }

        // Top: form (left) + controls (right)
        // Bottom: editors
        let [top_area, editor_area] = Layout::vertical([Constraint::Length(22), Constraint::Fill(1)]).areas(area);
        let [form_area, controls_area] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(top_area);

        self.draw_form(form_area, buf, &th, ctx.now);
        self.draw_controls(controls_area, buf, &th, ctx.now);
        self.draw_editors(editor_area, buf, &th, ctx.now);

        // Overlays last
        self.country.render_overlay(buf, area, &th, 8);
        self.language.render_overlay(buf, area, &th, 8);
        self.tags.render_overlay(buf, area, &th, 6);
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        // Close dropdowns on outside click
        if let Event::Mouse(m) = ev
            && is_left_down(m) {
                let in_country = mouse_in(self.country.hit.area, m) || mouse_in(self.country.dropdown_area, m);
                let in_language = mouse_in(self.language.hit.area, m) || mouse_in(self.language.dropdown_area, m);
                let in_tags = mouse_in(self.tags.hit.area, m) || mouse_in(self.tags.dropdown_area, m);

                if !in_country && self.country.open {
                    self.country.close();
                }
                if !in_language && self.language.open {
                    self.language.close();
                }
                if !in_tags && self.tags.open {
                    self.tags.close();
                }
            }

        // Tab navigation
        if let Event::Key(k) = ev {
            if k.code == KeyCode::Tab && !k.modifiers.contains(KeyModifiers::SHIFT) && is_press(k) {
                self.focus.next();
                return Outcome::Consumed;
            }
            if k.code == KeyCode::BackTab || (k.code == KeyCode::Tab && k.modifiers.contains(KeyModifiers::SHIFT)) && is_press(k) {
                self.focus.prev();
                return Outcome::Consumed;
            }
        }

        // Forward to focused widget
        if let Event::Key(k) = ev {
            let out = match self.focus.current() {
                Some(Id::Name) => self.name.handle_key(*k),
                Some(Id::Email) => self.email.handle_key(*k),
                Some(Id::Password) => self.password.handle_key(*k),
                Some(Id::Amount) => self.amount.handle_key(*k),
                Some(Id::Search) => self.search.handle_key(*k),
                Some(Id::Country) => self.country.handle_key(*k),
                Some(Id::Language) => self.language.handle_key(*k),
                Some(Id::Tags) => self.tags.handle_key(*k),
                Some(Id::Compact) => self.compact.handle_key(*k),
                Some(Id::Disabled) => Outcome::Ignored,
                Some(Id::Editor) => self.editor.handle_key(*k),
                None => Outcome::Ignored,
            };
            if out.is_changed() {
                match self.focus.current() {
                    Some(Id::Country) => {
                        if let Some(label) = self.country.selected_label() {
                            ctx.notify(format!("Country: {}", label), Variant::Success);
                        }
                    }
                    Some(Id::Language) => {
                        if let Some(label) = self.language.selected_label() {
                            ctx.notify(format!("Language: {}", label), Variant::Success);
                        }
                    }
                    Some(Id::Tags) => {
                        ctx.notify(format!("Tags: {}", self.tags.summary()), Variant::Success);
                    }
                    _ => {}
                }
                return Outcome::Changed;
            }
            if out.is_consumed() {
                return Outcome::Consumed;
            }
        }

        // Forward mouse to all widgets
        if let Event::Mouse(m) = ev {
            let mut result = Outcome::Ignored;

            macro_rules! handle_mouse {
                ($state:expr, $id:expr) => {
                    let out = $state.handle_mouse(*m);
                    if matches!(out, Outcome::Changed | Outcome::Consumed) && matches!($state.hit.mouse(m), Hit::Press) {
                        self.focus.set($id);
                    }
                    result |= out;
                };
            }

            handle_mouse!(self.name, Id::Name);
            handle_mouse!(self.email, Id::Email);
            handle_mouse!(self.password, Id::Password);
            handle_mouse!(self.amount, Id::Amount);
            handle_mouse!(self.search, Id::Search);
            handle_mouse!(self.country, Id::Country);
            handle_mouse!(self.language, Id::Language);
            handle_mouse!(self.tags, Id::Tags);
            handle_mouse!(self.compact, Id::Compact);
            handle_mouse!(self.editor, Id::Editor);

            if result.is_changed() || result.is_consumed() {
                return result;
            }
        }

        Outcome::Ignored
    }

    fn animating(&self, now: Instant) -> bool {
        self.name.animating(now)
            || self.email.animating(now)
            || self.password.animating(now)
            || self.amount.animating(now)
            || self.search.animating(now)
            || self.compact.animating(now)
    }

    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[("Tab", "next field"), ("Enter", "submit"), ("click", "Move cursor")]
    }
}

impl InputsPage {
    fn draw_form(&mut self, area: Rect, buf: &mut Buffer, th: &Theme, now: Instant) {
        let bg = th.background;
        fill(buf, area, bg);
        let title_style = st(th.text, bg).add_modifier(Modifier::BOLD);
        let inner = Border::Round.draw_titled_with(buf, area, th.border_blurred, bg, " Form ", Alignment::Left, title_style);

        if inner.height < 16 {
            return;
        }

        let rows = stack(inner, &[3, 4, 3, 3, 3], 1);

        // Name
        if let Some(r) = rows.first() {
            let [label, field] = tuiforge::layout::cols(*r, [Constraint::Length(12), Constraint::Length(44)]);
            put(buf, label.x, label.y + 1, "Name:", label.width, st(th.text, bg));
            Input::new()
                .placeholder("Enter your name")
                .max_len(32)
                .focused(self.focus.is(Id::Name))
                .now(now)
                .theme(th)
                .render(field, buf, &mut self.name);
        }

        // Email with validation error (4 rows: label + field + error line)
        if let Some(r) = rows.get(1) {
            let [label, field] = tuiforge::layout::cols(*r, [Constraint::Length(12), Constraint::Length(44)]);
            put(buf, label.x, label.y + 1, "Email:", label.width, st(th.text, bg));
            
            let field_area = Rect { height: 3, ..field };
            Input::new()
                .placeholder("you@example.com")
                .validator(validate_email)
                .focused(self.focus.is(Id::Email))
                .now(now)
                .theme(th)
                .render(field_area, buf, &mut self.email);
            
            // Show error message below field
            if let Some(err) = &self.email.error {
                put(buf, field.x + 3, field.y + 3, err, field.width.saturating_sub(6), st(th.error, bg));
            }
        }

        // Password with strength hint
        if let Some(r) = rows.get(2) {
            let [label, field] = tuiforge::layout::cols(*r, [Constraint::Length(12), Constraint::Length(44)]);
            put(buf, label.x, label.y + 1, "Password:", label.width, st(th.text, bg));
            Input::new()
                .password(true)
                .placeholder("Enter password")
                .focused(self.focus.is(Id::Password))
                .now(now)
                .theme(th)
                .render(field, buf, &mut self.password);
            let strength = password_strength(self.password.value());
            let (color, text) = match strength {
                0 => (th.error, "Weak"),
                1 => (th.warning, "Fair"),
                _ => (th.success, "Strong"),
            };
            if !self.password.value().is_empty() {
                put(buf, field.right().saturating_sub(8), field.y + 1, text, 6, st(color, th.surface));
            }
        }

        // Amount with prefix/suffix
        if let Some(r) = rows.get(3) {
            let [label, field] = tuiforge::layout::cols(*r, [Constraint::Length(12), Constraint::Length(20)]);
            put(buf, label.x, label.y + 1, "Amount:", label.width, st(th.text, bg));
            Input::new()
                .prefix("$")
                .suffix(".00")
                .restrict(InputRestrict::Digits)
                .max_len(6)
                .focused(self.focus.is(Id::Amount))
                .now(now)
                .theme(th)
                .render(field, buf, &mut self.amount);
        }

        // Search with suggester (ghost text)
        if let Some(r) = rows.get(4) {
            let [label, field] = tuiforge::layout::cols(*r, [Constraint::Length(12), Constraint::Length(44)]);
            put(buf, label.x, label.y + 1, "Search:", label.width, st(th.text, bg));
            Input::new()
                .placeholder("Type for suggestions")
                .suggester(suggest_search)
                .tab_accepts(true)
                .focused(self.focus.is(Id::Search))
                .now(now)
                .theme(th)
                .render(field, buf, &mut self.search);
        }
    }

    fn draw_controls(&mut self, area: Rect, buf: &mut Buffer, th: &Theme, now: Instant) {
        let bg = th.background;
        fill(buf, area, bg);
        let title_style = st(th.text, bg).add_modifier(Modifier::BOLD);
        let inner = Border::Round.draw_titled_with(buf, area, th.border_blurred, bg, " Controls ", Alignment::Left, title_style);

        if inner.height < 18 {
            return;
        }

        let rows = stack(inner, &[3, 3, 3, 1, 1, 3], 1);

        // Country select
        if let Some(r) = rows.first() {
            let [label, field] = tuiforge::layout::cols(*r, [Constraint::Length(10), Constraint::Fill(1)]);
            put(buf, label.x, label.y + 1, "Country:", label.width, st(th.text, bg));
            Select::new()
                .placeholder("Select")
                .focused(self.focus.is(Id::Country))
                .now(now)
                .theme(th)
                .render(field, buf, &mut self.country);
        }

        // Language combobox
        if let Some(r) = rows.get(1) {
            let [label, field] = tuiforge::layout::cols(*r, [Constraint::Length(10), Constraint::Fill(1)]);
            put(buf, label.x, label.y + 1, "Language:", label.width, st(th.text, bg));
            Combobox::new()
                .placeholder("Type to filter")
                .focused(self.focus.is(Id::Language))
                .now(now)
                .theme(th)
                .render(field, buf, &mut self.language);
        }

        // Tags multiselect
        if let Some(r) = rows.get(2) {
            let [label, field] = tuiforge::layout::cols(*r, [Constraint::Length(10), Constraint::Fill(1)]);
            put(buf, label.x, label.y + 1, "Tags:", label.width, st(th.text, bg));
            MultiSelect::new()
                .focused(self.focus.is(Id::Tags))
                .now(now)
                .theme(th)
                .render(field, buf, &mut self.tags);
        }

        // Compact input (1 row, label on same row)
        if let Some(r) = rows.get(3) {
            let [label, field] = tuiforge::layout::cols(*r, [Constraint::Length(10), Constraint::Fill(1)]);
            put(buf, label.x, label.y, "Compact:", label.width, st(th.text, bg));
            Input::new()
                .compact(true)
                .focused(self.focus.is(Id::Compact))
                .now(now)
                .theme(th)
                .render(field, buf, &mut self.compact);
        }

        // Disabled input
        if let Some(r) = rows.get(5) {
            let [label, field] = tuiforge::layout::cols(*r, [Constraint::Length(10), Constraint::Fill(1)]);
            put(buf, label.x, label.y + 1, "Disabled:", label.width, st(th.text, bg));
            Input::new()
                .enabled(false)
                .now(now)
                .theme(th)
                .render(field, buf, &mut self.disabled);
        }
    }

    fn draw_editors(&mut self, area: Rect, buf: &mut Buffer, th: &Theme, now: Instant) {
        let [editor_area, readonly_area] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

        // Editor with syntax highlighting
        let editor_card = pad(editor_area, 1, 0);
        let bg = th.background;
        fill(buf, editor_card, bg);
        let title_style = st(th.text, bg).add_modifier(Modifier::BOLD);
        let editor_inner = Border::Round.draw_titled_with(buf, editor_card, th.border_blurred, bg, " Editor (line numbers, syntax) ", Alignment::Left, title_style);

        if editor_inner.height >= 3 {
            let text_area = Rect { width: editor_inner.width, height: editor_inner.height.saturating_sub(1), ..editor_inner };
            TextArea::new()
                .line_numbers(true)
                .highlight_line(true)
                .highlighter(highlight_rust)
                .shape(FieldShape::Tall(Edge::Thin))
                .cursor(CursorStyle::Bar)
                .cursor_blink(true)
                .cursor_when_unfocused(true)
                .show_position(true)
                .focused(self.focus.is(Id::Editor))
                .now(now)
                .theme(th)
                .render(text_area, buf, &mut self.editor);

            // Position indicator is now shown by the widget itself
        }

        // Read-only textarea
        let readonly_card = pad(readonly_area, 1, 0);
        fill(buf, readonly_card, bg);
        let readonly_inner = Border::Round.draw_titled_with(buf, readonly_card, th.border_blurred, bg, " Read-Only ", Alignment::Left, title_style);

        if readonly_inner.height >= 3 {
            let text_area = Rect { width: readonly_inner.width, height: readonly_inner.height.saturating_sub(1), ..readonly_inner };
            TextArea::new()
                .read_only(true)
                .now(now)
                .theme(th)
                .render(text_area, buf, &mut self.readonly);
            
            let note_y = readonly_inner.bottom().saturating_sub(1);
            put(buf, readonly_inner.x, note_y, " Read-only mode", readonly_inner.width, st(th.text_muted, bg));
        }
    }
}

fn validate_email(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Ok(());
    }
    if s.contains('@') && s.contains('.') && s.len() > 5 {
        Ok(())
    } else {
        Err("Invalid email format".to_string())
    }
}

fn password_strength(s: &str) -> usize {
    if s.len() < 4 {
        0
    } else if s.len() < 8 {
        1
    } else {
        2
    }
}

fn suggest_search(query: &str) -> Option<String> {
    let suggestions = ["hello", "world", "rust", "ratatui", "tuiforge"];
    for sug in suggestions {
        if sug.starts_with(query) && sug.len() > query.len() {
            return Some(sug[query.len()..].to_string());
        }
    }
    None
}

fn highlight_rust(line: &str) -> Vec<(usize, usize, Style)> {
    let keywords = ["fn", "let", "mut", "pub", "use", "struct", "impl", "for", "in", "return", "if", "else", "match", "usize"];
    let mut spans = Vec::new();

    let th = theme::current();
    let keyword_style = st(th.primary, th.surface);
    let string_style = st(th.success, th.surface);

    // Simple keyword matching
    for kw in keywords {
        let mut start = 0;
        while let Some(pos) = line[start..].find(kw) {
            let abs_pos = start + pos;
            // Check word boundary
            let before_ok = abs_pos == 0 || !line.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
            let after_ok = abs_pos + kw.len() >= line.len() || !line.as_bytes()[abs_pos + kw.len()].is_ascii_alphanumeric();
            if before_ok && after_ok {
                spans.push((abs_pos, abs_pos + kw.len(), keyword_style));
            }
            start = abs_pos + kw.len();
        }
    }

    // String literals
    let mut in_string = false;
    let mut string_start = 0;
    for (i, ch) in line.chars().enumerate() {
        if ch == '"' {
            if in_string {
                spans.push((string_start, i + 1, string_style));
                in_string = false;
            } else {
                string_start = i;
                in_string = true;
            }
        }
    }
    if in_string {
        spans.push((string_start, line.len(), string_style));
    }

    spans.sort_by_key(|s| s.0);
    spans
}
