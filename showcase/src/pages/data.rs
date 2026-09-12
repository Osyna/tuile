//! Tables and catalogs: sortable data table, cell navigation, key-value list, big digits, and generic catalogues.

use std::time::Instant;

use tuile::crossterm::event::{Event, KeyCode, KeyEvent};
use tuile::prelude::*;
use tuile::ratatui_core::layout::{Alignment, Constraint};
use tuile::ratatui_core::style::Modifier;
use tuile::widgets::{
    Catalog, CatalogState, DataTable, DataTableState, Digits, KeyValueList, TableCell, TableColumn,
    TableCursor, TableRow,
};

use super::{Ctx, Page};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    MainTable,
    CellTable,
    Counter,
    McpCatalog,
    RecordCatalog,
}

pub struct DataPage {
    main_table: DataTableState,
    cell_table: DataTableState,
    mcp_catalog: CatalogState,
    record_catalog: CatalogState,
    focus: Option<Focus<Id>>,
    counter: u32,
    filter_mode: bool,
    filter_buf: String,
}

impl Default for DataPage {
    fn default() -> Self {
        Self {
            main_table: DataTableState::new(),
            cell_table: DataTableState::new(),
            mcp_catalog: CatalogState::new(),
            record_catalog: CatalogState::new(),
            focus: None,
            counter: 0,
            filter_mode: false,
            filter_buf: String::new(),
        }
    }
}

impl DataPage {
    fn handle_filter_key(&mut self, k: KeyEvent) -> Outcome {
        match k.code {
            KeyCode::Esc => {
                self.filter_mode = false;
                self.filter_buf.clear();
                self.main_table.set_filter("");
                Outcome::Consumed
            }
            KeyCode::Char(c) => {
                self.filter_buf.push(c);
                self.main_table.set_filter(&self.filter_buf);
                Outcome::Consumed
            }
            KeyCode::Backspace => {
                self.filter_buf.pop();
                self.main_table.set_filter(&self.filter_buf);
                Outcome::Consumed
            }
            KeyCode::Enter => {
                self.filter_mode = false;
                Outcome::Consumed
            }
            _ => Outcome::Ignored,
        }
    }
}

impl Page for DataPage {
    fn title(&self) -> &'static str {
        "Tables"
    }

    fn subtitle(&self) -> &'static str {
        "Sortable tables, cell navigation, key-value lists, and generic catalogs"
    }

    fn icon(&self) -> &'static str {
        "⊞"
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let focus = self.focus.get_or_insert_with(|| {
            Focus::new([
                Id::MainTable,
                Id::CellTable,
                Id::Counter,
                Id::McpCatalog,
                Id::RecordCatalog,
            ])
        });

        let th = &ctx.theme;
        let [main_area, middle_area, bottom_area] = tuile::layout::rows(
            area,
            [
                Constraint::Fill(1),
                Constraint::Length(10),
                Constraint::Length(10),
            ],
        );

        // main table card
        let main_inner = Border::Round.draw_titled_with(
            buf,
            main_area,
            th.border_blurred,
            th.background,
            "Server Status",
            Alignment::Left,
            st(th.text, th.background).add_modifier(Modifier::BOLD),
        );

        if main_inner.height < 3 {
            return;
        }

        let cols = vec![
            TableColumn::new("Name")
                .width(Constraint::Length(12))
                .sortable(true),
            TableColumn::new("Region")
                .width(Constraint::Length(10))
                .sortable(true),
            TableColumn::new("Status")
                .width(Constraint::Length(10))
                .sortable(true),
            TableColumn::new("CPU %")
                .width(Constraint::Length(8))
                .align(Alignment::Right)
                .sortable(true),
            TableColumn::new("Uptime").width(Constraint::Length(10)),
            TableColumn::new("Tags").width(Constraint::Fill(1)),
        ];

        let data = fake_server_data();
        let mut rows_vec = vec![];
        for (name, region, status, cpu, uptime, tags) in data {
            let status_variant = match status {
                "online" => Variant::Success,
                "warning" => Variant::Warning,
                "offline" => Variant::Error,
                _ => Variant::Default,
            };
            let cpu_cell = TableCell::new(format!("{:.1}", cpu)).sort_key(cpu);
            let status_cell =
                TableCell::new(status).style(st(th.text_variant(status_variant), th.surface));
            rows_vec.push(TableRow::new(vec![
                name.into(),
                region.into(),
                status_cell,
                cpu_cell,
                uptime.into(),
                tags.into(),
            ]));
        }

        // Reserve last line for status
        let table_area = Rect {
            height: main_inner.height.saturating_sub(1),
            ..main_inner
        };

        DataTable::new(cols, rows_vec)
            .cursor(TableCursor::Row)
            .zebra(true)
            .multi_select(true)
            .focused(focus.is(Id::MainTable))
            .theme(th)
            .render(table_area, buf, &mut self.main_table);

        // Status line inside the card
        let status_y = main_inner.y + main_inner.height - 1;
        if !self.filter_mode {
            let mut status_parts = vec![format!(
                "row {}/{}",
                self.main_table.cursor_row + 1,
                self.main_table.visible_len()
            )];
            if let Some((col, asc)) = self.main_table.sort {
                let col_name = ["Name", "Region", "Status", "CPU %", "Uptime", "Tags"]
                    .get(col)
                    .unwrap_or(&"?");
                status_parts.push(format!(
                    "sorted by {} {}",
                    col_name,
                    if asc { "▲" } else { "▼" }
                ));
            }
            if !self.main_table.selected_rows().is_empty() {
                status_parts.push(format!(
                    "{} selected",
                    self.main_table.selected_rows().len()
                ));
            }
            let status = status_parts.join(" • ");
            put(
                buf,
                main_inner.x,
                status_y,
                &status,
                main_inner.width,
                st(th.text_muted, th.background),
            );
        } else {
            let filter_prompt = format!("Filter: {}", self.filter_buf);
            put(
                buf,
                main_inner.x,
                status_y,
                &filter_prompt,
                main_inner.width,
                st(th.text, th.background),
            );
        }

        // middle row: cell table + kvlist + digits
        let [cell_card, kv_card, digits_card] = tuile::layout::cols(
            middle_area,
            [
                Constraint::Percentage(35),
                Constraint::Percentage(30),
                Constraint::Fill(1),
            ],
        );

        // Cell navigation table
        let cell_inner = Border::Round.draw_titled_with(
            buf,
            cell_card,
            th.border_blurred,
            th.background,
            "Cell Navigation",
            Alignment::Left,
            st(th.text, th.background).add_modifier(Modifier::BOLD),
        );

        let cell_cols = vec![
            TableColumn::new("Q1")
                .width(Constraint::Fill(1))
                .align(Alignment::Right),
            TableColumn::new("Q2")
                .width(Constraint::Fill(1))
                .align(Alignment::Right),
            TableColumn::new("Q3")
                .width(Constraint::Fill(1))
                .align(Alignment::Right),
            TableColumn::new("Q4")
                .width(Constraint::Fill(1))
                .align(Alignment::Right),
        ];
        let heatmap_data = vec![
            vec![45.2, 52.1, 48.3, 61.5],
            vec![38.7, 41.2, 55.8, 49.1],
            vec![62.3, 58.9, 67.4, 71.2],
            vec![51.0, 47.6, 53.2, 58.8],
        ];
        let mut cell_rows = vec![];
        for row in heatmap_data {
            cell_rows.push(TableRow::from(
                row.into_iter().map(TableCell::from).collect::<Vec<_>>(),
            ));
        }

        fn heatmap_style(_row: usize, _col: usize, cell: &TableCell, th: &Theme) -> Option<Style> {
            if let Some(v) = cell.sort_key {
                let intensity = ((v - 30.0) / 50.0).clamp(0.0, 1.0) as f32;
                let color = th.primary.blend(th.error, intensity);
                Some(st(color, th.surface))
            } else {
                None
            }
        }

        DataTable::new(cell_cols, cell_rows)
            .cursor(TableCursor::Cell)
            .borders(tuile::widgets::TableBorders::All)
            .cell_style(heatmap_style)
            .focused(focus.is(Id::CellTable))
            .theme(th)
            .render(cell_inner, buf, &mut self.cell_table);

        // Key-value list
        let kv_inner = Border::Round.draw_titled_with(
            buf,
            kv_card,
            th.border_blurred,
            th.background,
            "Details",
            Alignment::Left,
            st(th.text, th.background).add_modifier(Modifier::BOLD),
        );

        let kv_items = vec![
            ("Version".to_string(), "1.0.0".to_string()),
            ("Uptime".to_string(), "23d 4h 12m".to_string()),
            ("Load".to_string(), "0.42 0.38 0.35".to_string()),
            ("Memory".to_string(), "8.2 / 16.0 GB".to_string()),
        ];
        KeyValueList::new(kv_items).theme(th).render(kv_inner, buf);

        // Digits
        let digits_inner = Border::Round.draw_titled_with(
            buf,
            digits_card,
            th.border_blurred,
            th.background,
            "Digits",
            Alignment::Left,
            st(th.text, th.background).add_modifier(Modifier::BOLD),
        );

        if digits_inner.height >= 3 {
            let (h, m, s) = tuile::runtime::local_hms();
            let time_str = format!("{:02}:{:02}:{:02}", h, m, s);
            let time_rect = Rect {
                x: digits_inner.x,
                y: digits_inner.y,
                width: digits_inner.width,
                height: 3,
            };
            Digits::new(&time_str)
                .color(th.primary)
                .align(Alignment::Center)
                .theme(th)
                .render(time_rect, buf);
        }

        if digits_inner.height >= 7 {
            let counter_str = format!("{:03}", self.counter);
            let counter_rect = Rect {
                x: digits_inner.x,
                y: digits_inner.y + 4,
                width: digits_inner.width,
                height: 3,
            };
            let counter_color = if focus.is(Id::Counter) {
                th.accent
            } else {
                th.text_muted
            };
            Digits::new(&counter_str)
                .color(counter_color)
                .align(Alignment::Center)
                .bold(focus.is(Id::Counter))
                .theme(th)
                .render(counter_rect, buf);
        }

        // bottom row: two catalogs (MCP servers and saved records)
        let [mcp_card, record_card] = tuile::layout::cols(
            bottom_area,
            [Constraint::Percentage(50), Constraint::Fill(1)],
        );

        // MCP Server Catalog
        let mcp_inner = Border::Round.draw_titled_with(
            buf,
            mcp_card,
            if focus.is(Id::McpCatalog) {
                th.border
            } else {
                th.border_blurred
            },
            th.background,
            "MCP Servers",
            Alignment::Left,
            st(th.text, th.background).add_modifier(Modifier::BOLD),
        );

        let mcp_cols = vec![
            TableColumn::new("Name").width(Constraint::Fill(1)),
            TableColumn::new("Status").width(Constraint::Length(8)),
        ];
        let mcp_rows = vec![
            TableRow::from(vec!["aisandbox", "enabled"]),
            TableRow::from(vec!["context-mode", "enabled"]),
            TableRow::from(vec!["mcp-manager", "enabled"]),
            TableRow::from(vec!["tanuki-context", "enabled"]),
        ];

        Catalog::new(mcp_cols, mcp_rows)
            .focused(focus.is(Id::McpCatalog))
            .theme(th)
            .render(mcp_inner, buf, &mut self.mcp_catalog);

        // Saved Records Catalog (different schema)
        let record_inner = Border::Round.draw_titled_with(
            buf,
            record_card,
            if focus.is(Id::RecordCatalog) {
                th.border
            } else {
                th.border_blurred
            },
            th.background,
            "Recent Records",
            Alignment::Left,
            st(th.text, th.background).add_modifier(Modifier::BOLD),
        );

        let record_cols = vec![
            TableColumn::new("ID").width(Constraint::Length(6)),
            TableColumn::new("Title").width(Constraint::Fill(1)),
            TableColumn::new("Size")
                .width(Constraint::Length(8))
                .align(Alignment::Right),
        ];
        let record_rows = vec![
            TableRow::from(vec!["r-001", "Project setup", "2.4 KB"]),
            TableRow::from(vec!["r-002", "API design", "5.1 KB"]),
            TableRow::from(vec!["r-003", "Implementation", "12.8 KB"]),
            TableRow::from(vec!["r-004", "Review notes", "3.2 KB"]),
            TableRow::from(vec!["r-005", "Final report", "8.7 KB"]),
        ];

        Catalog::new(record_cols, record_rows)
            .focused(focus.is(Id::RecordCatalog))
            .theme(th)
            .render(record_inner, buf, &mut self.record_catalog);
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        match ev {
            Event::Key(k) if is_press(k) => {
                if self.filter_mode {
                    return self.handle_filter_key(*k);
                }

                let focus = self.focus.as_mut().unwrap();

                match k.code {
                    KeyCode::Char('/') => {
                        if focus.is(Id::MainTable) {
                            self.filter_mode = true;
                            return Outcome::Consumed;
                        }
                    }
                    KeyCode::Char('+') if focus.is(Id::Counter) => {
                        self.counter = self.counter.saturating_add(1);
                        return Outcome::Consumed;
                    }
                    KeyCode::Char('-') if focus.is(Id::Counter) => {
                        self.counter = self.counter.saturating_sub(1);
                        return Outcome::Consumed;
                    }
                    KeyCode::Tab => {
                        focus.next();
                        return Outcome::Consumed;
                    }
                    KeyCode::BackTab => {
                        focus.prev();
                        return Outcome::Consumed;
                    }
                    _ => {}
                }

                if focus.is(Id::MainTable) {
                    let out = self.main_table.handle_key(*k);
                    if out.is_consumed() {
                        return out;
                    }
                } else if focus.is(Id::CellTable) {
                    let out = self.cell_table.handle_key(*k);
                    if out.is_consumed() {
                        return out;
                    }
                } else if focus.is(Id::McpCatalog) {
                    let out = self.mcp_catalog.handle_key(*k);
                    if out.is_submitted()
                        && let Some(idx) = self.mcp_catalog.take_activated()
                    {
                        let servers =
                            ["aisandbox", "context-mode", "mcp-manager", "tanuki-context"];
                        if let Some(name) = servers.get(idx) {
                            ctx.notify(format!("MCP server: {}", name), Variant::Default);
                        }
                    }
                    if out.is_consumed() {
                        return out;
                    }
                } else if focus.is(Id::RecordCatalog) {
                    let out = self.record_catalog.handle_key(*k);
                    if out.is_submitted()
                        && let Some(idx) = self.record_catalog.take_activated()
                    {
                        let records = ["r-001", "r-002", "r-003", "r-004", "r-005"];
                        if let Some(id) = records.get(idx) {
                            ctx.notify(format!("Record: {}", id), Variant::Default);
                        }
                    }
                    if out.is_consumed() {
                        return out;
                    }
                }

                Outcome::Ignored
            }
            Event::Mouse(m) => {
                let mut out = Outcome::Ignored;
                let focus = self.focus.as_mut().unwrap();

                out |= self.main_table.handle_mouse(*m);
                if out.is_changed() {
                    focus.set(Id::MainTable);
                }

                out |= self.cell_table.handle_mouse(*m);
                if out.is_changed() {
                    focus.set(Id::CellTable);
                }

                let mcp_out = self.mcp_catalog.handle_mouse(*m);
                if mcp_out.is_submitted()
                    && let Some(idx) = self.mcp_catalog.take_activated()
                {
                    let servers = ["aisandbox", "context-mode", "mcp-manager", "tanuki-context"];
                    if let Some(name) = servers.get(idx) {
                        ctx.notify(format!("MCP server: {}", name), Variant::Default);
                    }
                }
                if mcp_out.is_changed() {
                    focus.set(Id::McpCatalog);
                }
                out |= mcp_out;

                let record_out = self.record_catalog.handle_mouse(*m);
                if record_out.is_submitted()
                    && let Some(idx) = self.record_catalog.take_activated()
                {
                    let records = ["r-001", "r-002", "r-003", "r-004", "r-005"];
                    if let Some(id) = records.get(idx) {
                        ctx.notify(format!("Record: {}", id), Variant::Default);
                    }
                }
                if record_out.is_changed() {
                    focus.set(Id::RecordCatalog);
                }
                out |= record_out;

                out
            }
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self, _now: Instant) -> bool {
        false
    }

    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("Tab", "next"),
            ("↑↓", "move"),
            ("/", "filter"),
            ("Enter", "activate"),
            ("Space", "select"),
        ]
    }
}

fn fake_server_data() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    f64,
    &'static str,
    &'static str,
)> {
    vec![
        ("server-01", "us-east", "online", 45.2, "2d 4h", "web,api"),
        ("server-02", "us-west", "online", 12.8, "5d 12h", "db"),
        ("server-03", "eu-west", "warning", 87.3, "1d 2h", "cache"),
        ("server-04", "ap-south", "online", 23.5, "8d 6h", "web"),
        ("server-05", "us-east", "offline", 0.0, "—", "backup"),
        ("server-06", "eu-north", "online", 56.1, "3d 8h", "api,cdn"),
        ("server-07", "ap-east", "online", 34.7, "12d 4h", "web"),
        ("server-08", "us-west", "warning", 91.2, "6h", "ml"),
    ]
}
