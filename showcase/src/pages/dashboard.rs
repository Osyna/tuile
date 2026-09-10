//! Dashboard: a realistic ops screen composed from widgets of every family — stat cards,
//! line/bar graphs, a live data table, meters, a deploy button driving a progress bar,
//! a switch, and an event log. Everything updates from a deterministic simulation.

use tuiforge::draw::{fill, put, put_right, st};
use tuiforge::prelude::*;
use tuiforge::widgets::{LineSeries, LineStyle, LegendPos, BarGroup, MeterStyle};

use super::{Ctx, Page, card};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Services,
    Deploy,
    Restart,
    Auto,
    Log,
}

const SERVICES: [(&str, &str, &str); 9] = [
    ("api-gateway", "1.42.0", "edge"),
    ("auth", "3.9.1", "core"),
    ("billing", "2.0.7", "core"),
    ("search", "0.31.2", "data"),
    ("mailer", "1.3.3", "async"),
    ("scheduler", "1.3.3", "async"),
    ("thumbnailer", "0.9.0", "media"),
    ("analytics", "4.1.0", "data"),
    ("webhooks", "1.0.5", "edge"),
];

/// Tiny deterministic PRNG so the demo looks alive without a `rand` dependency.
struct Lcg(u64);
impl Lcg {
    fn next_f(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f64) / (u32::MAX as f64 / 2.0)
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f()
    }
}

pub struct DashboardPage {
    focus: Focus<Id>,
    rng: Lcg,
    last_tick: Option<Instant>,
    // time series (last 120 samples)
    rps: Vec<f64>,
    errors: Vec<f64>,
    latency: Vec<f64>,
    users: Vec<f64>,
    regions: [f64; 5],
    cpu: [f64; 9],
    resources: [f32; 4],
    table: DataTableState,
    deploy_btn: ButtonState,
    restart_btn: ButtonState,
    auto: SwitchState,
    deploy: ProgressState,
    deploying: Option<Instant>,
    log: LogViewState,
}

impl Default for DashboardPage {
    fn default() -> Self {
        let mut p = DashboardPage {
            focus: Focus::new([Id::Services, Id::Deploy, Id::Restart, Id::Auto, Id::Log]),
            rng: Lcg(0x5eed_1234),
            last_tick: None,
            rps: Vec::new(),
            errors: Vec::new(),
            latency: Vec::new(),
            users: Vec::new(),
            regions: [420.0, 310.0, 205.0, 160.0, 95.0],
            cpu: [0.0; 9],
            resources: [0.62, 0.71, 0.48, 0.23],
            table: DataTableState::new(),
            deploy_btn: ButtonState::new(),
            restart_btn: ButtonState::new(),
            auto: SwitchState::new(true),
            deploy: ProgressState::new(),
            deploying: None,
            log: LogViewState::new(),
        };
        for _ in 0..90 {
            p.step();
        }
        for (lvl, msg) in [
            (LogLevel::Info, "dashboard connected to metrics stream"),
            (LogLevel::Success, "auth 3.9.1 rolled out to 12/12 pods"),
            (LogLevel::Warn, "search p95 latency above 180 ms for 5 min"),
            (LogLevel::Info, "autoscaler: billing 4 → 6 replicas"),
        ] {
            p.log.push(lvl, msg);
        }
        p.log.set_timestamps(true);
        p
    }
}

impl DashboardPage {
    fn step(&mut self) {
        let last = |v: &Vec<f64>, d: f64| v.last().copied().unwrap_or(d);
        let rps = (last(&self.rps, 1200.0) + self.rng.range(-60.0, 60.0)).clamp(600.0, 2200.0);
        let err = (last(&self.errors, 8.0) + self.rng.range(-2.5, 2.5)).clamp(0.0, 40.0);
        let lat = (last(&self.latency, 140.0) + self.rng.range(-12.0, 12.0)).clamp(60.0, 320.0);
        let usr = (last(&self.users, 830.0) + self.rng.range(-15.0, 16.0)).clamp(300.0, 1500.0);
        for (v, x) in [(&mut self.rps, rps), (&mut self.errors, err), (&mut self.latency, lat), (&mut self.users, usr)] {
            v.push(x);
            if v.len() > 120 {
                v.remove(0);
            }
        }
        for r in &mut self.regions {
            *r = (*r + self.rng.range(-8.0, 8.0)).clamp(40.0, 600.0);
        }
        for c in &mut self.cpu {
            *c = (*c + self.rng.range(-6.0, 6.0)).clamp(3.0, 97.0);
        }
        for (i, r) in self.resources.iter_mut().enumerate() {
            let drift = self.rng.range(-0.02, 0.02) as f32;
            *r = (*r + drift).clamp(0.05, 0.98);
            if i == 3 {
                *r = (*r + 0.01).min(0.98);
            }
        }
    }

    fn pct(v: f64, prev: f64) -> (f64, bool) {
        // small bases (errors/min near zero) would explode: never divide by less than 1
        let d = (v - prev) / prev.abs().max(1.0) * 100.0;
        (d.abs(), d >= 0.0)
    }

    fn rows(&self) -> Vec<TableRow> {
        SERVICES
            .iter()
            .enumerate()
            .map(|(i, (name, ver, tier))| {
                let cpu = self.cpu[i];
                let (status, v) = if cpu > 85.0 {
                    ("degraded", Variant::Error)
                } else if cpu > 65.0 {
                    ("busy", Variant::Warning)
                } else {
                    ("healthy", Variant::Success)
                };
                let p95 = 40.0 + cpu * 2.1;
                TableRow::new(vec![
                    TableCell::new(*name),
                    TableCell::new(status).style(Style::new().fg(theme::current().text_variant(v).color()).add_modifier(Modifier::BOLD)),
                    TableCell::new(format!("{cpu:.0}%")).sort_key(cpu),
                    TableCell::new(format!("{p95:.0} ms")).sort_key(p95),
                    TableCell::new(*ver),
                    TableCell::new(*tier),
                ])
            })
            .collect()
    }

    fn columns() -> Vec<TableColumn> {
        vec![
            TableColumn::new("Service").width(Constraint::Fill(2)).sortable(true),
            TableColumn::new("Status").width(Constraint::Length(9)).sortable(true),
            TableColumn::new("CPU").width(Constraint::Length(5)).align(Alignment::Right).sortable(true),
            TableColumn::new("p95").width(Constraint::Length(7)).align(Alignment::Right).sortable(true),
            TableColumn::new("Version").width(Constraint::Length(8)),
            TableColumn::new("Tier").width(Constraint::Length(6)),
        ]
    }

    fn start_deploy(&mut self, ctx: &mut Ctx) {
        if self.deploying.is_some() {
            ctx.notify("A deploy is already running", Variant::Warning);
            return;
        }
        self.deploying = Some(ctx.now);
        self.deploy.set(0.0, ctx.now, Duration::ZERO);
        self.log.push(LogLevel::Info, "deploy api-gateway 1.43.0 started (canary 10%)");
        ctx.notify("Deploy started: api-gateway 1.43.0", Variant::Primary);
    }
}

impl Page for DashboardPage {
    fn title(&self) -> &'static str {
        "Dashboard"
    }
    fn subtitle(&self) -> &'static str {
        "One screen composed from every widget family"
    }
    fn icon(&self) -> &'static str {
        "▦"
    }
    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[("Tab", "Focus"), ("Enter", "Deploy / restart"), ("s", "Sort table"), ("Space", "Toggle auto-refresh")]
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        let now = ctx.now;
        // simulation tick
        if self.auto.on && self.last_tick.is_none_or(|t| now.duration_since(t) >= Duration::from_millis(600)) {
            self.step();
            self.last_tick = Some(now);
        }
        if let Some(started) = self.deploying {
            let p = (now.duration_since(started).as_secs_f32() / 6.0).min(1.0);
            self.deploy.set(p, now, Duration::ZERO);
            if p >= 1.0 {
                self.deploying = None;
                self.log.push(LogLevel::Success, "deploy api-gateway 1.43.0 complete — 100% traffic");
                ctx.notify("Deploy complete", Variant::Success);
            }
        }

        let inner = pad(area, 1, 0);
        let [stats_a, mid_a, bottom_a] = Layout::vertical([Constraint::Length(5), Constraint::Length(13), Constraint::Fill(1)]).areas(inner);

        // ── stat cards ──
        let cards = columns(stats_a, 4, 1);
        let n = self.rps.len();
        // delta vs the trailing 30-sample average (a single old sample makes noisy metrics swing wildly)
        let prev = |v: &Vec<f64>| {
            let w = &v[n.saturating_sub(30)..];
            w.iter().sum::<f64>() / w.len().max(1) as f64
        };
        let stat = |v: &Vec<f64>| (v[n - 1], prev(v));
        let (rps, rps_p) = stat(&self.rps);
        let (err, err_p) = stat(&self.errors);
        let (lat, lat_p) = stat(&self.latency);
        let (usr, usr_p) = stat(&self.users);
        let specs = [
            (format!("{rps:.0}"), "requests / s", Self::pct(rps, rps_p), Variant::Primary, &self.rps, true),
            (format!("{err:.1}"), "errors / min", Self::pct(err, err_p), Variant::Error, &self.errors, false),
            (format!("{lat:.0} ms"), "latency p95", Self::pct(lat, lat_p), Variant::Warning, &self.latency, false),
            (format!("{usr:.0}"), "active users", Self::pct(usr, usr_p), Variant::Success, &self.users, true),
        ];
        for (rect, (value, label, (d, up), v, series, up_is_good)) in cards.iter().zip(specs) {
            let trend: Vec<f64> = series[n.saturating_sub(24)..].to_vec();
            // for errors/latency going up is bad: flip the colour semantics via `positive`
            let positive = if up_is_good { up } else { !up };
            StatCard::new(value, label).delta(d, positive).trend(&trend).variant(v).bordered(true).theme(&th).render(*rect, buf);
        }

        // ── graphs ──
        let [traffic_a, regions_a] = Layout::horizontal([Constraint::Percentage(60), Constraint::Fill(1)]).areas(mid_a);
        let t_in = card(buf, traffic_a, &th, "Traffic (last 2 min)");
        let pts = |v: &Vec<f64>| v.iter().enumerate().map(|(i, y)| (i as f64, *y)).collect::<Vec<_>>();
        let series = [
            LineSeries { name: "req/s".into(), points: pts(&self.rps), color: Some(th.primary), style: LineStyle::Area },
            LineSeries { name: "users".into(), points: pts(&self.users), color: Some(th.success), style: LineStyle::Line },
        ];
        LineGraph::new(&series)
            .y_labels(4)
            .x_labels(4)
            .label_fmt(|v| format!("{v:.0}"))
            .grid(true)
            .legend(LegendPos::TopLeft)
            .theme(&th)
            .render(pad(t_in, 1, 0), buf);
        let r_in = card(buf, regions_a, &th, "Requests by region");
        let groups: Vec<BarGroup> = ["us-east", "us-west", "eu-west", "ap-south", "sa-east"]
            .iter()
            .zip(self.regions)
            .map(|(l, v)| BarGroup { label: (*l).to_string(), values: vec![v] })
            .collect();
        BarGraph::new(&groups).horizontal(true).show_values(true).colors(&[th.secondary]).theme(&th).render(pad(r_in, 1, 0), buf);

        // ── bottom: services table | resources ──
        let [services_a, right_a] = Layout::horizontal([Constraint::Percentage(60), Constraint::Fill(1)]).areas(bottom_a);
        let s_in = card(buf, services_a, &th, "Services");
        let focused = self.focus.is(Id::Services);
        DataTable::new(Self::columns(), self.rows())
            .zebra(true)
            .cursor(TableCursor::Row)
            .focused(focused)
            .theme(&th)
            .render(pad(s_in, 1, 0), buf, &mut self.table);

        let [res_a, log_a] = Layout::vertical([Constraint::Length(15), Constraint::Fill(1)]).areas(right_a);
        let res_in = pad(card(buf, res_a, &th, "Resources"), 1, 0);
        let labels = ["CPU", "Memory", "Disk", "Network"];
        let thresholds = [(0.7, Variant::Success), (0.9, Variant::Warning), (1.01, Variant::Error)];
        for (i, label) in labels.iter().enumerate() {
            let row = Rect { y: res_in.y + i as u16, height: 1, ..res_in };
            if row.y >= res_in.bottom() {
                break;
            }
            let meter = Meter::new()
                .value(self.resources[i])
                .label(*label)
                .show_percent(true)
                .thresholds(&thresholds)
                .style(if i % 2 == 0 { MeterStyle::Line } else { MeterStyle::Block })
                .theme(&th);
            Widget::render(meter, row, buf);
        }
        // deploy progress + controls
        let py = res_in.y + 5;
        if py + 1 < res_in.bottom() {
            put(buf, res_in.x, py, "Deploy", 6, st(th.text_muted, th.background));
            let bar = Rect { x: res_in.x + 7, y: py, width: res_in.width.saturating_sub(7), height: 1 };
            ProgressBar::new().show_eta(self.deploying.is_some()).now(now).theme(&th).render(bar, buf, &mut self.deploy);
        }
        let by = py + 2;
        if by + 3 <= res_in.bottom() {
            let bw = ((res_in.width.saturating_sub(2)) / 3).clamp(10, 18);
            let b1 = Rect { x: res_in.x, y: by, width: bw, height: 3 };
            let b2 = Rect { x: b1.right() + 1, y: by, width: bw, height: 3 };
            Button::new("Deploy").variant(Variant::Primary).min_width(bw).focused(self.focus.is(Id::Deploy)).now(now).theme(&th).render(b1, buf, &mut self.deploy_btn);
            Button::new("Restart").min_width(bw).focused(self.focus.is(Id::Restart)).now(now).theme(&th).render(b2, buf, &mut self.restart_btn);
            let sw = Rect { x: res_in.x, y: by + 3, width: res_in.width, height: 3 };
            if sw.bottom() <= res_in.bottom() {
                Switch::new().label("Auto-refresh every 600 ms").focused(self.focus.is(Id::Auto)).now(now).theme(&th).render(sw, buf, &mut self.auto);
            }
        }

        let l_in = card(buf, log_a, &th, "Events");
        fill(buf, l_in, th.surface);
        LogView::new().timestamps(true).level_column(true).theme(&th).render(l_in, buf, &mut self.log);
        if self.focus.is(Id::Log) {
            put_right(buf, Rect { y: log_a.y, ..log_a }, " ↑↓ scroll ", st(th.text_muted, th.background));
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        match ev {
            Event::Key(k) => {
                if self.focus.handle_key(*k).is_consumed() {
                    return Outcome::Consumed;
                }
                
                match self.focus.current() {
                    Some(Id::Services) => {
                        let o = self.table.handle_key(*k);
                        if let Some(row) = self.table.take_activated() {
                            let name = SERVICES.get(row).map(|s| s.0).unwrap_or("?");
                            ctx.notify(format!("Opened {name}"), Variant::Primary);
                        }
                        o
                    }
                    Some(Id::Deploy) => {
                        let o = self.deploy_btn.handle_key(*k);
                        if o.is_changed() {
                            self.start_deploy(ctx);
                        }
                        o
                    }
                    Some(Id::Restart) => {
                        let o = self.restart_btn.handle_key(*k);
                        if o.is_changed() {
                            self.log.push(LogLevel::Warn, "restart requested for search (rolling)");
                            ctx.notify("Rolling restart: search", Variant::Warning);
                        }
                        o
                    }
                    Some(Id::Auto) => self.auto.handle_key(*k),
                    Some(Id::Log) => self.log.handle_key(*k),
                    None => Outcome::Ignored,
                }
            }
            Event::Mouse(m) => {
                let mut out = Outcome::Ignored;
                let t = self.table.handle_mouse(*m);
                if t.is_changed() {
                    self.focus.set(Id::Services);
                }
                out |= t;
                if let Some(row) = self.table.take_activated() {
                    let name = SERVICES.get(row).map(|s| s.0).unwrap_or("?");
                    ctx.notify(format!("Opened {name}"), Variant::Primary);
                }
                let d = self.deploy_btn.handle_mouse(*m);
                if d.is_changed() {
                    self.focus.set(Id::Deploy);
                    self.start_deploy(ctx);
                }
                out |= d;
                let r = self.restart_btn.handle_mouse(*m);
                if r.is_changed() {
                    self.focus.set(Id::Restart);
                    self.log.push(LogLevel::Warn, "restart requested for search (rolling)");
                    ctx.notify("Rolling restart: search", Variant::Warning);
                }
                out |= r;
                let a = self.auto.handle_mouse(*m);
                if a.is_changed() {
                    self.focus.set(Id::Auto);
                }
                out |= a;
                out |= self.log.handle_mouse(*m);
                out
            }
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self, now: Instant) -> bool {
        self.auto.on || self.deploying.is_some() || self.auto.animating(now) || self.deploy_btn.animating(now) || self.restart_btn.animating(now)
    }
}
