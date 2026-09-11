//! Headless TUI screenshots: run a binary in a private tmux server, drive it with keys and
//! mouse, capture with colours and rasterize to PNG (or print plain text).
//!
//! Key names are tmux send-keys names (Tab, BTab, Enter, Escape, Up, Down, Left, Right,
//! Space, PageUp, PageDown, Home, End, F1.., C-p for ctrl, M-1 for alt, plain characters).
//! `Escape` is always sent alone with a pause, because crossterm reads ESC immediately
//! followed by another key as Alt+key.

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use std::process::Command;
use std::time::Duration;
use unicode_width::UnicodeWidthChar;

const FONT_CANDIDATES: [&str; 4] = [
    "/usr/share/fonts/TTF/JetBrainsMonoNerdFontMono-Regular.ttf",
    "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
];
const FONT_BOLD: [&str; 4] = [
    "/usr/share/fonts/TTF/JetBrainsMonoNerdFontMono-Bold.ttf",
    "/usr/share/fonts/TTF/JetBrainsMono-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono-Bold.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf",
];

const CELL_W: u32 = 11;
const CELL_H: u32 = 22;
const DEFAULT_FG: [u8; 3] = [229, 229, 229];
const DEFAULT_BG: [u8; 3] = [0, 0, 0];

pub fn run(args: &[String]) -> Result<(), String> {
    let mut size = (130u32, 42u32);
    let mut keys = String::new();
    let mut mouse: Vec<String> = Vec::new();
    let mut out = "/tmp/shot.png".to_string();
    let mut text_only = false;
    let mut startup = 0.8f32;
    let mut cmd: Vec<String> = Vec::new();

    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut next = || it.next().cloned().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "-s" | "--size" => {
                let v = next()?;
                let (c, r) = v
                    .to_lowercase()
                    .split_once('x')
                    .map(|(c, r)| (c.to_string(), r.to_string()))
                    .ok_or("size must look like 130x42")?;
                size = (
                    c.parse().map_err(|_| "bad columns")?,
                    r.parse().map_err(|_| "bad rows")?,
                );
            }
            "-k" | "--keys" => keys = next()?,
            "-m" | "--mouse" => mouse.push(next()?),
            "-o" | "--out" => out = next()?,
            "--text" => text_only = true,
            "--startup" => startup = next()?.parse().map_err(|_| "bad startup seconds")?,
            "--" => {
                cmd.extend(it.cloned());
                break;
            }
            other => return Err(format!("unknown shot option `{other}`")),
        }
    }
    if cmd.is_empty() {
        return Err("shot: need -- <binary> [args]".into());
    }

    let tmux = Tmux::start(&cmd, size.0, size.1)?;
    let result = (|| -> Result<(), String> {
        sleep(startup);
        for k in keys.split_whitespace() {
            tmux.key(k)?;
        }
        for step in &mouse {
            let parts: Vec<&str> = step.split_whitespace().collect();
            let [kind, x, y] = parts[..] else {
                return Err(format!("mouse step must be `kind X Y`, got `{step}`"));
            };
            tmux.mouse(
                kind,
                x.parse().map_err(|_| "bad mouse x")?,
                y.parse().map_err(|_| "bad mouse y")?,
            )?;
        }
        sleep(0.2);
        if !tmux.alive() {
            eprintln!("shot: the process died before the capture");
        }
        if text_only {
            print!("{}", tmux.capture(false)?);
        } else {
            let ansi = tmux.capture(true)?;
            render(&ansi, size.0, size.1, &out)?;
            println!("wrote {out}");
        }
        Ok(())
    })();

    let stderr = tmux.close();
    if !stderr.trim().is_empty() {
        eprintln!("--- stderr ---\n{}", tail(&stderr, 2000));
    }
    result
}

fn sleep(secs: f32) {
    std::thread::sleep(Duration::from_secs_f32(secs));
}

fn tail(s: &str, n: usize) -> &str {
    if s.len() <= n { s } else { &s[s.len() - n..] }
}

struct Tmux {
    socket: String,
    err_path: String,
}

impl Tmux {
    fn start(cmd: &[String], cols: u32, rows: u32) -> Result<Self, String> {
        let socket = format!("shot-{}", std::process::id());
        let err_path = format!("/tmp/shot-{}.err", std::process::id());
        let shell = format!(
            "{} 2>{}",
            cmd.iter().map(|c| quote(c)).collect::<Vec<_>>().join(" "),
            err_path
        );
        let me = Tmux { socket, err_path };

        // CI and agent shells often export NO_COLOR=1 or TERM=dumb. The tmux server, and so
        // the app under test, inherits that and crossterm would strip every colour.
        let status = me
            .cmd()
            .args(["new-session", "-d", "-s", "s", "-x"])
            .arg(cols.to_string())
            .arg("-y")
            .arg(rows.to_string())
            .arg(&shell)
            .env_remove("NO_COLOR")
            .env_remove("TMUX")
            .env_remove("TMUX_PANE")
            .env("TERM", "xterm-256color")
            .env("COLORTERM", "truecolor")
            .status()
            .map_err(|e| format!("shot: cannot run tmux: {e}"))?;
        if !status.success() {
            return Err("shot: tmux new-session failed".into());
        }
        let _ = me.cmd().args(["set", "-g", "status", "off"]).status();
        Ok(me)
    }

    fn cmd(&self) -> Command {
        let mut c = Command::new("tmux");
        c.args(["-L", &self.socket]);
        c
    }

    fn key(&self, k: &str) -> Result<(), String> {
        self.cmd()
            .args(["send-keys", "-t", "s", k])
            .status()
            .map_err(|e| format!("shot: send-keys: {e}"))?;
        // ESC followed immediately by another key is parsed as Alt+key.
        sleep(if k == "Escape" || k == "Esc" { 0.35 } else { 0.0 });
        sleep(0.25);
        Ok(())
    }

    fn mouse(&self, kind: &str, x: u32, y: u32) -> Result<(), String> {
        if kind == "click" {
            self.mouse("down", x, y)?;
            return self.mouse("up", x, y);
        }
        let (button, suffix) = match kind {
            "down" => (0, 'M'),
            "up" => (0, 'm'),
            "drag" => (32, 'M'),
            "move" => (35, 'M'),
            "wheelup" => (64, 'M'),
            "wheeldown" => (65, 'M'),
            other => return Err(format!("unknown mouse step `{other}`")),
        };
        let seq = format!("\x1b[<{button};{};{}{suffix}", x + 1, y + 1);
        self.cmd()
            .args(["send-keys", "-t", "s", "-l", &seq])
            .status()
            .map_err(|e| format!("shot: mouse: {e}"))?;
        sleep(0.15);
        Ok(())
    }

    fn capture(&self, ansi: bool) -> Result<String, String> {
        let mut c = self.cmd();
        c.args(["capture-pane", "-t", "s", "-p"]);
        if ansi {
            c.arg("-e");
        }
        let out = c.output().map_err(|e| format!("shot: capture-pane: {e}"))?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn alive(&self) -> bool {
        let out = self
            .cmd()
            .args(["list-panes", "-t", "s", "-F", "#{pane_dead}"])
            .output();
        matches!(out, Ok(o) if o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "0")
    }

    fn close(&self) -> String {
        let _ = self.cmd().arg("kill-server").output();
        let err = std::fs::read_to_string(&self.err_path).unwrap_or_default();
        let _ = std::fs::remove_file(&self.err_path);
        err
    }
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn load_font(candidates: &[&str]) -> Option<FontVec> {
    candidates
        .iter()
        .find_map(|p| std::fs::read(p).ok())
        .and_then(|bytes| FontVec::try_from_vec(bytes).ok())
}

/// xterm's 256-colour cube.
fn color256(n: u8) -> [u8; 3] {
    const BASE: [[u8; 3]; 16] = [
        [0, 0, 0],
        [205, 0, 0],
        [0, 205, 0],
        [205, 205, 0],
        [0, 0, 238],
        [205, 0, 205],
        [0, 205, 205],
        [229, 229, 229],
        [127, 127, 127],
        [255, 0, 0],
        [0, 255, 0],
        [255, 255, 0],
        [92, 92, 255],
        [255, 0, 255],
        [0, 255, 255],
        [255, 255, 255],
    ];
    match n {
        0..=15 => BASE[n as usize],
        16..=231 => {
            let n = n - 16;
            let step = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            [step(n / 36), step((n / 6) % 6), step(n % 6)]
        }
        _ => {
            let v = 8 + (n - 232) * 10;
            [v, v, v]
        }
    }
}

struct Pen {
    fg: [u8; 3],
    bg: [u8; 3],
    bold: bool,
    inverse: bool,
    underline: bool,
}

impl Pen {
    fn reset() -> Self {
        Pen { fg: DEFAULT_FG, bg: DEFAULT_BG, bold: false, inverse: false, underline: false }
    }

    fn apply(&mut self, params: &[u16]) {
        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => *self = Pen::reset(),
                1 => self.bold = true,
                4 => self.underline = true,
                7 => self.inverse = true,
                22 => self.bold = false,
                24 => self.underline = false,
                27 => self.inverse = false,
                39 => self.fg = DEFAULT_FG,
                49 => self.bg = DEFAULT_BG,
                p @ 30..=37 => self.fg = color256((p - 30) as u8),
                p @ 90..=97 => self.fg = color256((p - 90 + 8) as u8),
                p @ 40..=47 => self.bg = color256((p - 40) as u8),
                p @ 100..=107 => self.bg = color256((p - 100 + 8) as u8),
                p @ (38 | 48) if i + 1 < params.len() => {
                    let colour = match params[i + 1] {
                        2 if i + 4 < params.len() => {
                            let c = [params[i + 2] as u8, params[i + 3] as u8, params[i + 4] as u8];
                            i += 4;
                            Some(c)
                        }
                        5 if i + 2 < params.len() => {
                            let c = color256(params[i + 2] as u8);
                            i += 2;
                            Some(c)
                        }
                        _ => None,
                    };
                    if let Some(c) = colour {
                        if p == 38 { self.fg = c } else { self.bg = c }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

fn render(ansi: &str, cols: u32, rows: u32, out: &str) -> Result<(), String> {
    let regular = load_font(&FONT_CANDIDATES)
        .ok_or("shot: no monospace TTF found; install JetBrains Mono or DejaVu Sans Mono")?;
    let bold = load_font(&FONT_BOLD);
    // Box-drawing glyphs tile only if one advance is exactly one cell. Stretch the face
    // horizontally to CELL_W instead of leaving the font's natural advance, which is
    // narrower and turns a run of `─` into a dashed line.
    let natural = regular.as_scaled(PxScale::from(18.0));
    let advance = natural.h_advance(regular.glyph_id('M'));
    let scale = PxScale { x: 18.0 * CELL_W as f32 / advance, y: 18.0 };

    let mut img = image::RgbImage::from_pixel(cols * CELL_W, rows * CELL_H, image::Rgb(DEFAULT_BG));

    for (y, line) in ansi.split('\n').take(rows as usize).enumerate() {
        let y = y as u32;
        let mut pen = Pen::reset();
        let mut x = 0u32;
        let mut chars = line.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '\x1b' {
                if chars.peek() == Some(&'[') {
                    chars.next();
                    let mut raw = String::new();
                    for c in chars.by_ref() {
                        if c.is_ascii_alphabetic() {
                            if c == 'm' {
                                let params: Vec<u16> = if raw.is_empty() {
                                    vec![0]
                                } else {
                                    raw.split(';').map(|p| p.parse().unwrap_or(0)).collect()
                                };
                                pen.apply(&params);
                            }
                            break;
                        }
                        raw.push(c);
                    }
                }
                continue;
            }

            let w = c.width().unwrap_or(1).max(1) as u32;
            let (fg, bg) = if pen.inverse { (pen.bg, pen.fg) } else { (pen.fg, pen.bg) };
            if x < cols {
                fill_cells(&mut img, x, y, w, bg);
                if c != ' ' {
                    let font = if pen.bold { bold.as_ref().unwrap_or(&regular) } else { &regular };
                    draw_glyph(&mut img, font, scale, c, x, y, fg);
                }
                if pen.underline {
                    let uy = (y + 1) * CELL_H - 2;
                    for px in x * CELL_W..((x + w) * CELL_W).min(img.width()) {
                        img.put_pixel(px, uy.min(img.height() - 1), image::Rgb(fg));
                    }
                }
            }
            x += w;
        }
        // tmux trims trailing blank cells: extend the last background to the edge.
        if x < cols {
            fill_cells(&mut img, x, y, cols - x, pen.bg);
        }
    }

    img.save(out).map_err(|e| format!("shot: writing {out}: {e}"))
}

fn fill_cells(img: &mut image::RgbImage, x: u32, y: u32, w: u32, colour: [u8; 3]) {
    let (x0, y0) = (x * CELL_W, y * CELL_H);
    for py in y0..(y0 + CELL_H).min(img.height()) {
        for px in x0..(x0 + w * CELL_W).min(img.width()) {
            img.put_pixel(px, py, image::Rgb(colour));
        }
    }
}

fn draw_glyph(
    img: &mut image::RgbImage,
    font: &FontVec,
    scale: PxScale,
    c: char,
    cell_x: u32,
    cell_y: u32,
    fg: [u8; 3],
) {
    let scaled = font.as_scaled(scale);
    let glyph = font.glyph_id(c).with_scale(scale);
    let Some(outline) = font.outline_glyph(glyph) else { return };
    let bounds = outline.px_bounds();
    // Baseline placement inside the cell, matching the ascent of an 18px face in a 22px cell.
    let origin_x = (cell_x * CELL_W) as f32;
    let origin_y = (cell_y * CELL_H) as f32 + scaled.ascent();
    outline.draw(|gx, gy, coverage| {
        if coverage <= 0.01 {
            return;
        }
        let px = (origin_x + bounds.min.x + gx as f32).round();
        let py = (origin_y + bounds.min.y + gy as f32).round();
        if px < 0.0 || py < 0.0 {
            return;
        }
        let (px, py) = (px as u32, py as u32);
        if px >= img.width() || py >= img.height() {
            return;
        }
        let bg = img.get_pixel(px, py).0;
        let a = coverage.clamp(0.0, 1.0);
        let blend = |f: u8, b: u8| (f as f32 * a + b as f32 * (1.0 - a)).round() as u8;
        img.put_pixel(
            px,
            py,
            image::Rgb([blend(fg[0], bg[0]), blend(fg[1], bg[1]), blend(fg[2], bg[2])]),
        );
    });
}
