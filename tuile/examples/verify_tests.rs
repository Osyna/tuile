use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tuile::widgets::footer::{FooterBinding, KeyFooter, KeyFooterState};
use tuile::widgets::header::{AppHeader, AppHeaderState};

fn main() {
    println!("=== Test 1: footer_renders_bindings ===");
    {
        let mut state = KeyFooterState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 1));
        let area = buf.area;
        KeyFooter::new()
            .bindings(&[("q", "Quit"), ("^S", "Save")])
            .render(area, &mut buf, &mut state);
        let row: String = (0..area.width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect();
        println!("Rendered row: '{}'", row.trim_end());
    }

    println!("\n=== Test 2: footer_compact_mode ===");
    {
        let mut state = KeyFooterState::new();
        let area = Rect::new(0, 0, 80, 1);

        let mut buf = Buffer::empty(area);
        KeyFooter::new()
            .bindings(&[("q", "Quit"), ("^S", "Save")])
            .render(area, &mut buf, &mut state);
        let normal_row: String = (0..area.width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect();

        let mut buf2 = Buffer::empty(area);
        KeyFooter::new()
            .bindings(&[("q", "Quit"), ("^S", "Save")])
            .compact(true)
            .render(area, &mut buf2, &mut state);
        let compact_row: String = (0..area.width)
            .map(|x| buf2[(x, 0)].symbol().to_string())
            .collect();

        println!("Normal row:  '{}'", normal_row.trim_end());
        println!("Compact row: '{}'", compact_row.trim_end());
    }

    println!("\n=== Test 3: footer_overflow_drops_low_priority ===");
    {
        let mut state = KeyFooterState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
        let area = buf.area;
        let bindings = vec![
            FooterBinding::new("q", "Quit").priority(255),
            FooterBinding::new("^S", "Save").priority(200),
            FooterBinding::new("F1", "Help").priority(100),
            FooterBinding::new("F2", "Info").priority(50),
        ];
        KeyFooter::new()
            .bindings_full(bindings)
            .render(area, &mut buf, &mut state);
        let row: String = (0..area.width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect();
        println!("Rendered row (30 chars): '{}'", row);
    }

    println!("\n=== Test 4: header_tall_mode ===");
    {
        let mut state = AppHeaderState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
        let area = buf.area;
        AppHeader::new()
            .title("Test")
            .subtitle("Sub")
            .tall(true)
            .render(area, &mut buf, &mut state);

        let row0: String = (0..area.width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect();
        let row1: String = (0..area.width)
            .map(|x| buf[(x, 1)].symbol().to_string())
            .collect();
        let row2: String = (0..area.width)
            .map(|x| buf[(x, 2)].symbol().to_string())
            .collect();

        println!("Row 0: '{}'", row0.trim_end());
        println!("Row 1: '{}'", row1.trim_end());
        println!("Row 2: '{}'", row2.trim_end());
    }
}
