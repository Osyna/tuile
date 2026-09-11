//! Tables and digits: sortable data table, cell navigation, key-value list, big digits.

use std::time::Instant;

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Constraint};
use ratatui::style::Modifier;
use tuiforge::prelude::*;
use tuiforge::widgets::{
    DataTable, DataTableState, Digits, KeyValueList, TableCell, TableColumn, TableCursor, TableRow,
};

use super::{Ctx, Page};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    MainTable,
    CellTable,
    Counter,
}

pub struct DataPage {
    main_table: DataTableState,
    cell_table: DataTableState,
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
        "Sortable tables, cell navigation, key-value lists, and big digits"
    }

    fn icon(&self) -> &'static str {
        "⊞"
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let focus = self
            .focus
            .get_or_insert_with(|| Focus::new([Id::MainTable, Id::CellTable, Id::Counter]));

        let th = &ctx.theme;
        let rows = tuiforge::layout::stack(area, &[area.height.saturating_sub(11), 10], 1);
        if rows.len() < 2 {
            return;
        }

        // main table card
        let main_card = rows[0];
        let main_inner = Border::Round.draw_titled_with(
            buf,
            main_card,
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
                st(th.text, th.focus_bg()),
            );
        }

        // bottom row: cell table + kvlist + digits
        let bottom_area = rows[1];
        let bottom_cols = tuiforge::layout::cols(
            bottom_area,
            [
                Constraint::Percentage(35),
                Constraint::Percentage(30),
                Constraint::Fill(1),
            ],
        );

        // Cell navigation table
        let cell_card = bottom_cols[0];
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
            .borders(tuiforge::widgets::TableBorders::All)
            .cell_style(heatmap_style)
            .focused(focus.is(Id::CellTable))
            .theme(th)
            .render(cell_inner, buf, &mut self.cell_table);

        // Key-value list
        let kv_card = bottom_cols[1];
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
        let digits_card = bottom_cols[2];
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
            let (h, m, s) = tuiforge::runtime::local_hms();
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
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        match ev {
            Event::Key(k) if is_press(k) => {
                if self.filter_mode {
                    return self.handle_filter_key(*k);
                }

                let Some(focus) = self.focus.as_mut() else {
                    return Outcome::Ignored;
                };

                match k.code {
                    KeyCode::Tab => {
                        focus.next();
                        return Outcome::Consumed;
                    }
                    KeyCode::BackTab => {
                        focus.prev();
                        return Outcome::Consumed;
                    }
                    KeyCode::Char('/') => {
                        self.filter_mode = true;
                        return Outcome::Consumed;
                    }
                    KeyCode::Char(' ') if focus.is(Id::Counter) => {
                        self.counter = (self.counter + 1) % 1000;
                        return Outcome::Changed;
                    }
                    _ => {}
                }

                if focus.is(Id::MainTable) {
                    let out = self.main_table.handle_key(*k);
                    if out.is_changed()
                        && let Some(act) = self.main_table.activated.take()
                    {
                        ctx.notify(format!("Activated row {}", act), Variant::Primary);
                    }
                    return out;
                } else if focus.is(Id::CellTable) {
                    return self.cell_table.handle_key(*k);
                }
            }
            Event::Mouse(m) => {
                let mut out = Outcome::Ignored;
                out |= self.main_table.handle_mouse(*m);
                out |= self.cell_table.handle_mouse(*m);
                if out.is_changed()
                    && let Some(act) = self.main_table.activated.take()
                {
                    ctx.notify(format!("Clicked row {}", act), Variant::Primary);
                }
                return out;
            }
            _ => {}
        }
        Outcome::Ignored
    }

    fn animating(&self, _now: Instant) -> bool {
        true // clock updates every second
    }

    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("Tab", "focus"),
            ("/", "filter"),
            ("s", "sort"),
            ("Space", "select / +1"),
            ("Enter", "activate"),
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
        (
            "srv-web-01",
            "us-west",
            "online",
            45.2,
            "23d 4h",
            "web,nginx",
        ),
        (
            "srv-web-02",
            "us-west",
            "online",
            38.7,
            "23d 4h",
            "web,nginx",
        ),
        (
            "srv-db-01",
            "us-east",
            "online",
            62.3,
            "45d 2h",
            "db,postgres",
        ),
        (
            "srv-db-02",
            "us-east",
            "warning",
            78.1,
            "45d 2h",
            "db,postgres",
        ),
        (
            "srv-cache-01",
            "eu-central",
            "online",
            12.8,
            "12d 8h",
            "cache,redis",
        ),
        (
            "srv-cache-02",
            "eu-central",
            "online",
            15.3,
            "12d 8h",
            "cache,redis",
        ),
        (
            "srv-app-01",
            "ap-south",
            "online",
            51.0,
            "8d 16h",
            "app,node",
        ),
        ("srv-app-02", "ap-south", "offline", 0.0, "0h", "app,node"),
        (
            "srv-queue-01",
            "us-west",
            "online",
            22.4,
            "30d 1h",
            "queue,rabbitmq",
        ),
        (
            "srv-monitor-01",
            "us-east",
            "online",
            8.5,
            "60d 5h",
            "monitor,grafana",
        ),
        (
            "srv-web-03",
            "us-west",
            "online",
            41.2,
            "15d 3h",
            "web,nginx",
        ),
        (
            "srv-web-04",
            "us-west",
            "online",
            39.8,
            "15d 3h",
            "web,nginx",
        ),
        (
            "srv-db-03",
            "eu-west",
            "online",
            55.6,
            "28d 7h",
            "db,postgres",
        ),
        (
            "srv-db-04",
            "eu-west",
            "warning",
            72.9,
            "28d 7h",
            "db,postgres",
        ),
        (
            "srv-worker-01",
            "us-east",
            "online",
            33.1,
            "18d 12h",
            "worker,python",
        ),
        (
            "srv-worker-02",
            "us-east",
            "online",
            31.7,
            "18d 12h",
            "worker,python",
        ),
        (
            "srv-worker-03",
            "us-east",
            "online",
            35.4,
            "18d 12h",
            "worker,python",
        ),
        (
            "srv-lb-01",
            "us-west",
            "online",
            18.9,
            "40d 3h",
            "lb,haproxy",
        ),
        (
            "srv-lb-02",
            "us-east",
            "online",
            17.2,
            "40d 3h",
            "lb,haproxy",
        ),
        (
            "srv-search-01",
            "eu-central",
            "online",
            48.3,
            "22d 9h",
            "search,elastic",
        ),
        (
            "srv-search-02",
            "eu-central",
            "online",
            46.7,
            "22d 9h",
            "search,elastic",
        ),
        (
            "srv-metrics-01",
            "us-west",
            "online",
            14.5,
            "55d 2h",
            "metrics,prometheus",
        ),
        (
            "srv-backup-01",
            "us-east",
            "online",
            6.2,
            "90d 1h",
            "backup,rsync",
        ),
        (
            "srv-backup-02",
            "eu-west",
            "online",
            5.8,
            "90d 1h",
            "backup,rsync",
        ),
        (
            "srv-cdn-01",
            "ap-northeast",
            "online",
            28.3,
            "35d 6h",
            "cdn,nginx",
        ),
        (
            "srv-cdn-02",
            "ap-northeast",
            "online",
            27.1,
            "35d 6h",
            "cdn,nginx",
        ),
        (
            "srv-mail-01",
            "us-west",
            "online",
            9.7,
            "120d 4h",
            "mail,postfix",
        ),
        (
            "srv-dns-01",
            "us-east",
            "online",
            3.2,
            "150d 8h",
            "dns,bind",
        ),
        (
            "srv-dns-02",
            "eu-west",
            "online",
            3.5,
            "150d 8h",
            "dns,bind",
        ),
        (
            "srv-vpn-01",
            "us-west",
            "online",
            11.4,
            "80d 11h",
            "vpn,wireguard",
        ),
        (
            "srv-proxy-01",
            "us-east",
            "online",
            24.6,
            "50d 7h",
            "proxy,squid",
        ),
        (
            "srv-git-01",
            "us-west",
            "online",
            19.8,
            "65d 9h",
            "git,gitlab",
        ),
        (
            "srv-ci-01",
            "us-west",
            "online",
            42.7,
            "25d 5h",
            "ci,jenkins",
        ),
        (
            "srv-ci-02",
            "us-west",
            "warning",
            68.9,
            "25d 5h",
            "ci,jenkins",
        ),
        (
            "srv-log-01",
            "us-east",
            "online",
            31.2,
            "45d 3h",
            "log,fluentd",
        ),
        (
            "srv-log-02",
            "eu-central",
            "online",
            29.8,
            "45d 3h",
            "log,fluentd",
        ),
        (
            "srv-registry-01",
            "us-west",
            "online",
            16.3,
            "70d 2h",
            "registry,harbor",
        ),
        (
            "srv-vault-01",
            "us-east",
            "online",
            7.9,
            "100d 6h",
            "vault,hashicorp",
        ),
        (
            "srv-k8s-master-01",
            "us-west",
            "online",
            54.2,
            "20d 4h",
            "k8s,master",
        ),
        (
            "srv-k8s-node-01",
            "us-west",
            "online",
            61.7,
            "20d 4h",
            "k8s,node",
        ),
    ]
}
