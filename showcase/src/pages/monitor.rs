//! Monitor: a btop-style system view from stock widgets - LED meters, braille dot-field
//! graphs, a mirrored network graph, a process tree, boxed panels with right titles and footer
//! keys, and a big-font menu overlay (`m`).

use std::time::Instant;

use tuiforge::draw::{blend_area, hline, put, put_right, st};
use tuiforge::prelude::*;

use super::{Ctx, Page};

const CORES: usize = 16;
const HIST: usize = 120;
const TICK: f32 = 0.25;

/// Deterministic fake telemetry; one sample every `TICK` seconds.
struct Sim {
    ticks: u64,
    cores: Vec<Vec<f64>>,
    total: Vec<f64>,
    down: Vec<f64>,
    up: Vec<f64>,
    mem: [Vec<f64>; 4],
}

fn noise(seed: u64, t: u64) -> f64 {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ t.wrapping_mul(0xD1B5_4A32_D192_ED03);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    (x % 10_000) as f64 / 10_000.0
}

impl Sim {
    fn new() -> Self {
        let mut s = Self {
            ticks: 0,
            cores: vec![vec![0.0; HIST]; CORES],
            total: vec![0.0; HIST],
            down: vec![0.0; HIST],
            up: vec![0.0; HIST],
            mem: [
                vec![0.35; HIST],
                vec![0.65; HIST],
                vec![0.50; HIST],
                vec![0.18; HIST],
            ],
        };
        for _ in 0..HIST {
            s.sample();
        }
        s
    }

    fn sample(&mut self) {
        let t = self.ticks;
        self.ticks += 1;
        let ts = t as f64 * TICK as f64;
        let mut sum = 0.0;
        for (i, hist) in self.cores.iter_mut().enumerate() {
            let base = 0.03 + 0.05 * ((ts * 0.3 + i as f64).sin() * 0.5 + 0.5);
            let spike = if noise(i as u64 + 1, t / 3) > 0.85 {
                0.5 * noise(i as u64 + 99, t)
            } else {
                0.0
            };
            let v = (base + spike + 0.04 * noise(i as u64 + 7, t)).clamp(0.0, 1.0);
            hist.remove(0);
            hist.push(v);
            sum += v;
        }
        self.total.remove(0);
        self.total.push(sum / CORES as f64);
        let burst = |seed: u64| {
            if noise(seed, t / 4) > 0.9 {
                0.4 + 0.6 * noise(seed + 1, t)
            } else {
                0.02 + 0.06 * noise(seed + 2, t)
            }
        };
        self.down.remove(0);
        self.down.push(burst(500));
        self.up.remove(0);
        self.up.push(burst(700));
        for (k, m) in self.mem.iter_mut().enumerate() {
            let last = m[HIST - 1];
            let drift = (noise(900 + k as u64, t) - 0.5) * 0.02;
            m.remove(0);
            m.push((last + drift).clamp(0.05, 0.95));
        }
    }

    fn advance_to(&mut self, elapsed: f32) {
        let target = (elapsed / TICK) as u64 + HIST as u64;
        while self.ticks < target {
            self.sample();
        }
    }
}

pub struct MonitorPage {
    sim: Sim,
    procs: Vec<TreeNode>,
    tree: TreeViewState,
    menu_open: bool,
    menu: BigMenuState,
}

impl Default for MonitorPage {
    fn default() -> Self {
        let mut procs = vec![
            TreeNode::new("1 systemd")
                .detail("/usr/lib/systemd/systemd --switched-root --system")
                .with_children(vec![
                    TreeNode::new("1042971 taix-gui")
                        .detail("/home/irvin/.local/bin/taix-gui")
                        .with_children(vec![
                            TreeNode::new("1042984 tmux: client")
                                .detail("tmux -L taix -CC attach -t taix"),
                            TreeNode::new("1046482 bwrap")
                                .detail("/usr/bin/bwrap --args 45 -- /usr/bin/xdg-dbus-proxy")
                                .with_children(vec![
                                    TreeNode::new("1046484 xdg-dbus-proxy")
                                        .detail("/usr/bin/xdg-dbus-proxy --args=41"),
                                ]),
                            TreeNode::new("1046487 bwrap")
                                .detail("/usr/bin/bwrap --args 41 -- WebKitWebProcess 4 35")
                                .with_children(vec![
                                    TreeNode::new("1046493 WebKitWebProcess")
                                        .detail("/usr/lib/webkitgtk-6.0/WebKitWebProcess 4 35"),
                                ]),
                        ]),
                    TreeNode::new("2992 tmux: server")
                        .detail("tmux -L taix new-session -d -s taix -x 240 -y 60")
                        .with_children(vec![
                            TreeNode::new("3956 omp").detail("omp").with_children(vec![
                                TreeNode::new("4046 node")
                                    .detail("node /home/irvin/Projects/MCPManager/src/mcp.js"),
                                TreeNode::new("4047 tanuki-context")
                                    .detail("/home/irvin/.local/bin/tanuki-context"),
                                TreeNode::new("555363 omp daemon")
                                    .detail("omp __omp_worker_daemon_broker")
                                    .with_children(vec![
                                        TreeNode::new("555394 omp lsp mux")
                                            .detail("omp __omp_worker_lsp_mux")
                                            .with_children(vec![
                                                TreeNode::new("555419 ruff")
                                                    .detail("/home/irvin/.local/bin/ruff server"),
                                            ]),
                                    ]),
                                TreeNode::new("572559 python")
                                    .detail("python -u /tmp/omp-python-runner/runner.py"),
                            ]),
                            TreeNode::new("2993 zsh").detail("-zsh"),
                            TreeNode::new("7270 zsh").detail("-zsh").with_children(vec![
                                TreeNode::new("7559 omp").detail("omp").with_children(vec![
                                    TreeNode::new("71897 rust-analyzer")
                                        .detail("/home/irvin/.local/bin/rust-analyzer"),
                                    TreeNode::new("314541 chromium").detail(
                                        "/usr/lib/chromium/chromium --headless --disable-gpu",
                                    ),
                                ]),
                            ]),
                        ]),
                ]),
        ];
        TreeNode::assign_ids(&mut procs);
        let mut tree = TreeViewState::new();
        tree.expand_all(&procs);
        Self {
            sim: Sim::new(),
            procs,
            tree,
            menu_open: false,
            menu: BigMenuState::default(),
        }
    }
}

impl Page for MonitorPage {
    fn title(&self) -> &'static str {
        "Monitor"
    }
    fn subtitle(&self) -> &'static str {
        "btop-style: LED meters, dot-field graphs, mirrored net graph, process tree, big menu"
    }
    fn icon(&self) -> &'static str {
        "◆"
    }
    fn bindings(&self) -> &'static [(&'static str, &'static str)] {
        &[("m", "Menu"), ("↑↓", "Process"), ("Enter", "Fold")]
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = ctx.theme;
        self.sim.advance_to(ctx.elapsed());
        if area.width < 60 || area.height < 16 {
            return;
        }
        let heat: [Rgb; 3] = [th.success, th.warning, th.error];
        let cool: [Rgb; 2] = [th.primary.blend(th.background, 0.5), th.primary];
        let purple: [Rgb; 2] = [th.accent.blend(th.background, 0.5), th.accent];

        let cpu_h = if area.height >= 36 { 13 } else { 8 };
        let [cpu_a, rest] =
            Layout::vertical([Constraint::Length(cpu_h), Constraint::Fill(1)]).areas(area);
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Fill(1)]).areas(rest);
        let [mem_a, net_a] =
            Layout::vertical([Constraint::Percentage(55), Constraint::Fill(1)]).areas(left);
        let [proc_a, disk_a] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(13.min(right.height / 2)),
        ])
        .areas(right);

        // ── cpu ──
        let boxed = |title: &str| {
            Panel::new()
                .title(title)
                .border_color(th.text_disabled)
                .theme(&th)
        };
        let c = boxed("cpu")
            .title_right("Ryzen 7 9700X · 3.0 GHz")
            .footer("preset 1 · toggle t")
            .render(cpu_a, buf);
        let total = *self.sim.total.last().unwrap_or(&0.0) as f32;
        let mut y = c.y;
        let meter_w = c.width.saturating_sub(34);
        Meter::new()
            .value(total)
            .label("CPU")
            .style(MeterStyle::Blocks)
            .gradient(&heat)
            .show_percent(true)
            .theme(&th)
            .render(
                Rect {
                    x: c.x,
                    y,
                    width: meter_w + 9,
                    height: 1,
                },
                buf,
            );
        SparkChart::new(&self.sim.total)
            .min(0.0)
            .max(1.0)
            .style(SparkStyle::Field)
            .gradient(&cool)
            .theme(&th)
            .render(
                Rect {
                    x: c.x + meter_w + 10,
                    y,
                    width: 12,
                    height: 1,
                },
                buf,
            );
        put_right(
            buf,
            Rect { y, height: 1, ..c },
            &format!("{:>2.0}°C {:.1}W", 38.0 + total * 40.0, 20.0 + total * 60.0),
            st(th.text_primary, th.background),
        );
        y += 1;
        let per_col = CORES.div_ceil(2);
        let col_w = c.width / 2;
        let rows_avail = c.height.saturating_sub(3) as usize;
        for (i, hist) in self.sim.cores.iter().enumerate() {
            let (col, row) = (i / per_col, i % per_col);
            if row >= rows_avail {
                break;
            }
            let x = c.x + col as u16 * col_w;
            let ry = y + row as u16;
            let v = *hist.last().unwrap_or(&0.0) as f32;
            put(
                buf,
                x,
                ry,
                &format!("C{i:<2}"),
                3,
                st(th.text, th.background).add_modifier(Modifier::BOLD),
            );
            let graph_w = col_w.saturating_sub(24);
            SparkChart::new(hist)
                .min(0.0)
                .max(1.0)
                .style(SparkStyle::Field)
                .gradient(&heat)
                .theme(&th)
                .render(
                    Rect {
                        x: x + 4,
                        y: ry,
                        width: graph_w,
                        height: 1,
                    },
                    buf,
                );
            put(
                buf,
                x + 5 + graph_w,
                ry,
                &format!("{:>3.0}%", v * 100.0),
                4,
                st(th.text, th.background),
            );
            Meter::new()
                .value(v)
                .style(MeterStyle::Dots)
                .gradient(&cool)
                .theme(&th)
                .render(
                    Rect {
                        x: x + 10 + graph_w,
                        y: ry,
                        width: 8,
                        height: 1,
                    },
                    buf,
                );
            put(
                buf,
                x + 19 + graph_w,
                ry,
                &format!("{:>2.0}°C", 38.0 + v * 30.0),
                4,
                st(th.text_primary, th.background),
            );
        }
        if c.height >= 10 {
            let gy = c.bottom() - 2;
            Meter::new()
                .value(0.68)
                .label("GPU")
                .style(MeterStyle::Blocks)
                .gradient(&heat)
                .show_percent(true)
                .theme(&th)
                .render(
                    Rect {
                        x: c.x,
                        y: gy,
                        width: meter_w + 9,
                        height: 1,
                    },
                    buf,
                );
            put(
                buf,
                c.x + meter_w + 10,
                gy,
                "4.9G/24G",
                8,
                st(th.text, th.background),
            );
            put_right(
                buf,
                Rect {
                    y: gy,
                    height: 1,
                    ..c
                },
                "47°C 48.6W",
                st(th.text_primary, th.background),
            );
            let load = format!(
                "Load avg: {:.2} {:.2} {:.2}",
                1.0 + total * 2.0,
                1.1 + total,
                1.05
            );
            put_right(
                buf,
                Rect {
                    y: c.bottom() - 1,
                    height: 1,
                    ..c
                },
                &load,
                st(th.text_muted, th.background),
            );
        }

        // ── mem ──
        let m = boxed("mem").title_right("60.2 GiB").render(mem_a, buf);
        let labels = ["Used:", "Available:", "Cached:", "Free:"];
        let colors = [
            [th.error.blend(th.background, 0.4), th.error],
            [th.warning.blend(th.background, 0.4), th.warning],
            cool,
            [th.success.blend(th.background, 0.4), th.success],
        ];
        let slot = m.height / 4;
        for (k, hist) in self.sim.mem.iter().enumerate() {
            let sy = m.y + k as u16 * slot;
            if slot < 2 || sy + slot > m.bottom() {
                break;
            }
            let v = *hist.last().unwrap_or(&0.0);
            put(
                buf,
                m.x,
                sy,
                labels[k],
                11,
                st(th.text, th.background).add_modifier(Modifier::BOLD),
            );
            put(
                buf,
                m.x + 11,
                sy,
                &format!("{:>3.0}%", v * 100.0),
                4,
                st(th.text_muted, th.background),
            );
            put_right(
                buf,
                Rect {
                    y: sy,
                    height: 1,
                    ..m
                },
                &format!("{:.1} GiB", v * 60.2),
                st(th.text, th.background).add_modifier(Modifier::BOLD),
            );
            SparkChart::new(hist)
                .min(0.0)
                .max(1.0)
                .style(SparkStyle::Field)
                .gradient(&colors[k])
                .theme(&th)
                .render(
                    Rect {
                        x: m.x,
                        y: sy + 1,
                        width: m.width,
                        height: slot - 1,
                    },
                    buf,
                );
        }

        // ── net: download grows up from the middle, upload hangs down ──
        let n = boxed("net")
            .title_right("enp12s0")
            .footer("sync · auto · zero")
            .render(net_a, buf);
        let stats_w = 24.min(n.width / 2);
        let graph = Rect {
            width: n.width.saturating_sub(stats_w + 1),
            ..n
        };
        let half = graph.height / 2;
        SparkChart::new(&self.sim.down)
            .min(0.0)
            .max(1.0)
            .style(SparkStyle::Field)
            .gradient(&cool)
            .theme(&th)
            .render(
                Rect {
                    height: half,
                    ..graph
                },
                buf,
            );
        SparkChart::new(&self.sim.up)
            .min(0.0)
            .max(1.0)
            .style(SparkStyle::Field)
            .gradient(&purple)
            .mirrored(true)
            .theme(&th)
            .render(
                Rect {
                    y: graph.y + half,
                    height: graph.height - half,
                    ..graph
                },
                buf,
            );
        let s = Rect {
            x: graph.right() + 1,
            width: stats_w,
            ..n
        };
        let d = *self.sim.down.last().unwrap_or(&0.0);
        let u = *self.sim.up.last().unwrap_or(&0.0);
        let rows = [
            ("▼", format!("{:.2} MiB/s", d * 12.0), th.primary),
            ("▼", "Top:  19.6 Mibps".to_string(), th.primary),
            ("▼", "Total:  1.72 GiB".to_string(), th.primary),
            ("▲", format!("{:.2} MiB/s", u * 12.0), th.accent),
            ("▲", "Top:  21.9 Mibps".to_string(), th.accent),
            ("▲", "Total:  7.20 GiB".to_string(), th.accent),
        ];
        for (i, (glyph, text, color)) in rows.iter().enumerate() {
            let ry = s.y + i as u16 + u16::from(i >= 3 && s.height > 7);
            if ry >= s.bottom() {
                break;
            }
            put(buf, s.x, ry, glyph, 1, st(*color, th.background));
            put(
                buf,
                s.x + 2,
                ry,
                text,
                s.width.saturating_sub(2),
                st(th.text, th.background),
            );
        }

        // ── proc ──
        let p = boxed("proc")
            .title_right("24 procs")
            .footer("tree · filter /")
            .render(proc_a, buf);
        TreeView::new(self.procs.clone())
            .border(Border::None)
            .markers("[+]", "[-]")
            .guides(true)
            .focused(!self.menu_open)
            .theme(&th)
            .render(p, buf, &mut self.tree);

        // ── disks ──
        let dk = boxed("disks").title_right("io").render(disk_a, buf);
        let disks: [(&str, &str, f32, &str, &str, bool); 3] = [
            ("root", "914 GiB", 0.73, "665 GiB", "249 GiB", true),
            ("swap", "3.99 GiB", 0.0, "0 Byte", "3.99 GiB", false),
            ("boot", "1021 MiB", 0.35, "353 MiB", "668 MiB", true),
        ];
        let mut dy = dk.y;
        for (name, size, used, used_s, free_s, io) in disks {
            if dy + 3 > dk.bottom() {
                break;
            }
            hline(
                buf,
                dk.x,
                dy,
                dk.width,
                "─",
                st(th.border_blurred, th.background),
            );
            put(
                buf,
                dk.x,
                dy,
                &format!("{name} "),
                name.len() as u16 + 1,
                st(th.text, th.background).add_modifier(Modifier::BOLD),
            );
            put_right(
                buf,
                Rect {
                    y: dy,
                    height: 1,
                    ..dk
                },
                &format!(" {size}"),
                st(th.text, th.background).add_modifier(Modifier::BOLD),
            );
            dy += 1;
            if io && dy + 3 <= dk.bottom() {
                put(buf, dk.x, dy, "IO%", 3, st(th.text_muted, th.background));
                SparkChart::new(&self.sim.total)
                    .min(0.0)
                    .max(1.0)
                    .style(SparkStyle::Line)
                    .color(th.text_muted)
                    .theme(&th)
                    .render(
                        Rect {
                            x: dk.x + 4,
                            y: dy,
                            width: dk.width.saturating_sub(4),
                            height: 1,
                        },
                        buf,
                    );
                dy += 1;
            }
            Meter::new()
                .value(used)
                .label("Used:")
                .show_percent(true)
                .suffix(used_s)
                .style(MeterStyle::Blocks)
                .gradient(&heat)
                .theme(&th)
                .render(
                    Rect {
                        y: dy,
                        height: 1,
                        ..dk
                    },
                    buf,
                );
            dy += 1;
            Meter::new()
                .value(1.0 - used)
                .label("Free:")
                .show_percent(true)
                .suffix(free_s)
                .style(MeterStyle::Blocks)
                .color(th.success)
                .theme(&th)
                .render(
                    Rect {
                        y: dy,
                        height: 1,
                        ..dk
                    },
                    buf,
                );
            dy += 1;
        }

        // ── menu overlay ──
        if self.menu_open {
            blend_area(buf, area, th.background, 0.6);
            let fire = [th.error, th.warning];
            let title = BigText::new("TUIFORGE").gradient(&fire);
            let items = ["OPTIONS", "HELP", "QUIT"];
            let menu = BigMenu::new(&items)
                .focused(true)
                .selected_gradient(&fire)
                .theme(&th);
            let menu_h = menu.height();
            let w = title.width().max(menu.width()) + 4;
            let m_area = center(area, w, 3 + 2 + menu_h);
            fill(buf, m_area, th.background);
            title.align(Alignment::Center).render(
                Rect {
                    height: 3,
                    ..m_area
                },
                buf,
            );
            put_right(
                buf,
                Rect {
                    y: m_area.y + 3,
                    height: 1,
                    ..m_area
                },
                "v0.1.0",
                st(th.text_muted, th.background),
            );
            menu.render(
                Rect {
                    y: m_area.y + 5,
                    height: menu_h,
                    ..m_area
                },
                buf,
                &mut self.menu,
            );
        }
    }

    fn event(&mut self, ev: &Event, ctx: &mut Ctx) -> Outcome {
        if self.menu_open {
            let out = match ev {
                Event::Key(k)
                    if is_press(k) && (k.code == KeyCode::Esc || k.code == KeyCode::Char('m')) =>
                {
                    self.menu_open = false;
                    Outcome::Changed
                }
                _ => self.menu.handle(ev),
            };
            if let Some(i) = self.menu.take_activated() {
                ctx.notify(
                    format!("Menu: {}", ["Options", "Help", "Quit"][i.min(2)]),
                    Variant::Primary,
                );
                self.menu_open = false;
            }
            return out;
        }
        if let Event::Key(k) = ev
            && is_press(k)
            && k.code == KeyCode::Char('m')
        {
            self.menu_open = true;
            return Outcome::Changed;
        }
        match ev {
            Event::Key(k) => self.tree.handle_key(*k),
            Event::Mouse(m) => self.tree.handle_mouse(*m),
            _ => Outcome::Ignored,
        }
    }

    fn animating(&self, _now: Instant) -> bool {
        true
    }
}
