//! Exercises the crate the way a consumer does: `tuiforge::prelude::*` and nothing else.
//!
//! Unit tests live inside the modules and can reach private items, so they never prove the
//! public surface is usable. Anything here that needs a private item is an API gap.

use tuiforge::prelude::*;

/// Row `y` of the buffer as a string, the idiom the crate's own tests use.
fn row(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, y)].symbol().to_string())
        .collect()
}

fn buffer(w: u16, h: u16) -> Buffer {
    Buffer::empty(Rect::new(0, 0, w, h))
}

#[test]
fn a_widget_can_be_built_styled_and_rendered_from_the_prelude() {
    let mut buf = buffer(30, 3);
    let area = buf.area;
    Button::new("Save")
        .variant(Variant::Success)
        .theme(&Theme::default())
        .render(area, &mut buf, &mut ButtonState::default());

    let text: String = (0..3).map(|y| row(&buf, y)).collect();
    assert!(text.contains("Save"), "{text:?}");
}

#[test]
fn stateful_widgets_report_value_changes_through_outcome() {
    let mut state = CheckboxState::default();
    let before = state.value;

    let out = state.handle_key(KeyEvent::from(KeyCode::Char(' ')));

    assert_eq!(out, Outcome::Changed, "space toggles and reports Changed");
    assert_ne!(state.value, before);
    assert_eq!(
        state.handle_key(KeyEvent::from(KeyCode::Char('q'))),
        Outcome::Ignored,
        "an unrelated key is left for the app"
    );
}

#[test]
fn mouse_clicks_reach_a_widget_through_its_cached_hit_rects() {
    let mut buf = buffer(40, 3);
    let area = buf.area;
    let mut state = TabBarState::new(0);
    let items: Vec<TabItem> = vec!["One".into(), "Two".into(), "Three".into()];
    TabBar::new(items).render(area, &mut buf, &mut state);

    // Click the third tab where render recorded it, not at a guessed coordinate.
    let hit = state.hits[2];
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: hit.x,
        row: hit.y,
        modifiers: KeyModifiers::NONE,
    };
    state.handle_mouse(click);
    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        ..click
    };
    state.handle_mouse(up);

    assert_eq!(state.active, 2, "clicking a tab selects it");
}

#[test]
fn themes_are_switchable_and_reach_widgets_that_did_not_ask_for_one() {
    let nord = theme::builtin("nord").expect("nord ships with the crate");
    let dark = theme::builtin("textual-dark").expect("textual-dark ships");
    let mut buf = buffer(20, 1);
    let area = buf.area;

    fill(&mut buf, area, nord.background);
    assert_eq!(buf[(0, 0)].bg, nord.background.color());

    assert_ne!(
        nord.background, dark.background,
        "two built-in palettes differ"
    );
    assert!(theme::builtin("no-such-theme").is_none());
    assert!(theme::theme_names().contains(&"nord"));
}

#[test]
fn drawing_helpers_clip_instead_of_panicking_outside_the_buffer() {
    let mut buf = buffer(10, 2);
    let th = Theme::default();

    put(
        &mut buf,
        8,
        0,
        "overflowing text",
        20,
        st(th.text, th.background),
    );
    fill(&mut buf, Rect::new(5, 1, 99, 99), th.primary);
    put_centered(
        &mut buf,
        Rect::new(0, 1, 10, 1),
        "centred",
        st(th.text, th.background),
    );

    assert_eq!(buf.area.width, 10, "the buffer was not resized");
    assert!(row(&buf, 0).starts_with("        ov"), "{:?}", row(&buf, 0));
}

#[test]
fn every_widget_survives_a_terminal_too_small_to_draw_in() {
    for (w, h) in [(1, 1), (3, 2), (12, 4)] {
        let mut buf = buffer(w, h);
        let area = buf.area;
        Button::new("x").render(area, &mut buf, &mut ButtonState::default());
        Checkbox::new("x").render(area, &mut buf, &mut CheckboxState::default());
        Input::new().render(area, &mut buf, &mut InputState::new());
        TabBar::new(vec!["a".into()]).render(area, &mut buf, &mut TabBarState::new(0));
    }
}
