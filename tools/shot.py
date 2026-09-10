#!/usr/bin/env python3
"""Headless TUI screenshots: run a binary in a private tmux server, drive it with keys and
mouse, capture with colours and rasterize to PNG (or print plain text).

Examples
  tools/shot.py -- ./target/debug/showcase --page controls
  tools/shot.py -k "Tab Tab Enter" -o /tmp/a.png -- ./target/debug/showcase --page inputs
  tools/shot.py -k "Down Down" --mouse "click 40 12" --text -- ./target/debug/showcase
  tools/shot.py --script 'Tab;shot /tmp/1.png;Enter;wait 0.5;shot /tmp/2.png' -- ./target/debug/showcase

Key names are tmux send-keys names (Tab, BTab, Enter, Escape, Up, Down, Left, Right, Space,
PageUp, PageDown, Home, End, F1.., C-p (ctrl), M-1 (alt), plain characters). `Escape` is
always sent alone with a pause (crossterm would otherwise read ESC+key as Alt+key).

Mouse steps (SGR sequences): `click X Y`, `down X Y`, `up X Y`, `drag X Y`, `move X Y`,
`wheelup X Y`, `wheeldown X Y` — 0-based cell coordinates.
"""
import argparse
import itertools
import os
import re
import subprocess
import sys
import time

FONT_CANDIDATES = [
    "/usr/share/fonts/TTF/JetBrainsMonoNerdFontMono-Regular.ttf",
    "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
]
FONT_BOLD = [
    "/usr/share/fonts/TTF/JetBrainsMonoNerdFontMono-Bold.ttf",
    "/usr/share/fonts/TTF/JetBrainsMono-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono-Bold.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf",
]


_COUNTER = itertools.count()


class Tmux:
    def __init__(self, cmd, cols, rows):
        self.sock = f"shot-{os.getpid()}-{next(_COUNTER)}"
        self.t = ["tmux", "-L", self.sock]
        shell = " ".join(_q(c) for c in cmd) + " 2>/tmp/shot-%d.err" % os.getpid()
        # Agent shells often export NO_COLOR=1 / TERM=dumb: crossterm would then strip every
        # colour. The tmux server (and thus the app) inherits this environment, so scrub it.
        env = {k: v for k, v in os.environ.items() if k not in ("NO_COLOR", "TMUX", "TMUX_PANE")}
        env["TERM"] = "xterm-256color"
        env["COLORTERM"] = "truecolor"
        subprocess.run([*self.t, "new-session", "-d", "-s", "s", "-x", str(cols), "-y", str(rows), shell], check=True, env=env)
        subprocess.run([*self.t, "set", "-g", "status", "off"], check=False)
        self.cols, self.rows = cols, rows

    def keys(self, *ks, wait=0.25):
        for k in ks:
            subprocess.run([*self.t, "send-keys", "-t", "s", k], check=True)
            if k in ("Escape", "Esc"):
                time.sleep(0.35)
        time.sleep(wait)

    def mouse(self, kind, x, y, wait=0.15):
        codes = {"down": (0, "M"), "up": (0, "m"), "drag": (32, "M"), "move": (35, "M"), "wheelup": (64, "M"), "wheeldown": (65, "M")}
        if kind == "click":
            self.mouse("down", x, y, 0.05)
            self.mouse("up", x, y, wait)
            return
        b, suffix = codes[kind]
        seq = f"\x1b[<{b};{x + 1};{y + 1}{suffix}"
        subprocess.run([*self.t, "send-keys", "-t", "s", "-l", seq], check=True)
        time.sleep(wait)

    def capture(self, ansi=True):
        args = [*self.t, "capture-pane", "-t", "s", "-p"] + (["-e"] if ansi else [])
        return subprocess.run(args, capture_output=True, text=True, check=True).stdout

    def alive(self):
        r = subprocess.run([*self.t, "list-panes", "-t", "s", "-F", "#{pane_dead}"], capture_output=True, text=True)
        return r.returncode == 0 and r.stdout.strip() == "0"

    def close(self):
        subprocess.run([*self.t, "kill-server"], capture_output=True)
        err = f"/tmp/shot-{os.getpid()}.err"
        out = open(err).read() if os.path.exists(err) else ""
        if os.path.exists(err):
            os.remove(err)
        return out


def _q(s):
    return "'" + s.replace("'", "'\\''") + "'"


def render(ansi_text, cols, rows, out_path):
    try:
        from PIL import Image, ImageDraw, ImageFont
        import unicodedata
    except ImportError:
        sys.exit("PIL not installed: pip install pillow (or use --text)")
    font = next((ImageFont.truetype(p, 18) for p in FONT_CANDIDATES if os.path.exists(p)), ImageFont.load_default())
    fontb = next((ImageFont.truetype(p, 18) for p in FONT_BOLD if os.path.exists(p)), font)
    cw, ch = 11, 22
    img = Image.new("RGB", (cols * cw, rows * ch), (0, 0, 0))
    d = ImageDraw.Draw(img)

    def c256(n):
        if n < 16:
            base = [(0, 0, 0), (205, 0, 0), (0, 205, 0), (205, 205, 0), (0, 0, 238), (205, 0, 205), (0, 205, 205), (229, 229, 229),
                    (127, 127, 127), (255, 0, 0), (0, 255, 0), (255, 255, 0), (92, 92, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255)]
            return base[n]
        if n < 232:
            n -= 16
            r, g, b = n // 36, (n // 6) % 6, n % 6
            return tuple(0 if v == 0 else 55 + v * 40 for v in (r, g, b))
        v = 8 + (n - 232) * 10
        return (v, v, v)

    def cwidth(ch_):
        if unicodedata.combining(ch_):
            return 0
        return 2 if unicodedata.east_asian_width(ch_) in ("W", "F") else 1

    lines = ansi_text.split("\n")
    for y, line in enumerate(lines[:rows]):
        fg, bg, boldf, inv, ul = (229, 229, 229), (0, 0, 0), False, False, False
        x = 0
        i = 0
        while i < len(line):
            m = re.match(r"\x1b\[([0-9;]*)m", line[i:])
            if m:
                params = [int(p) if p else 0 for p in m.group(1).split(";")] or [0]
                j = 0
                while j < len(params):
                    p = params[j]
                    if p == 0:
                        fg, bg, boldf, inv, ul = (229, 229, 229), (0, 0, 0), False, False, False
                    elif p == 1:
                        boldf = True
                    elif p == 4:
                        ul = True
                    elif p == 7:
                        inv = True
                    elif p == 22:
                        boldf = False
                    elif p == 24:
                        ul = False
                    elif p == 27:
                        inv = False
                    elif p == 39:
                        fg = (229, 229, 229)
                    elif p == 49:
                        bg = (0, 0, 0)
                    elif 30 <= p <= 37:
                        fg = c256(p - 30)
                    elif 90 <= p <= 97:
                        fg = c256(p - 90 + 8)
                    elif 40 <= p <= 47:
                        bg = c256(p - 40)
                    elif 100 <= p <= 107:
                        bg = c256(p - 100 + 8)
                    elif p in (38, 48) and j + 1 < len(params):
                        if params[j + 1] == 2 and j + 4 < len(params):
                            col = tuple(params[j + 2:j + 5])
                            j += 4
                        elif params[j + 1] == 5 and j + 2 < len(params):
                            col = c256(params[j + 2])
                            j += 2
                        else:
                            col = None
                        if col is not None:
                            if p == 38:
                                fg = col
                            else:
                                bg = col
                    j += 1
                i += m.end()
                continue
            if line[i] == "\x1b":
                i += 1
                continue
            ch_ = line[i]
            w = cwidth(ch_)
            f, b = (bg, fg) if inv else (fg, bg)
            if x < cols:
                d.rectangle([x * cw, y * ch, (x + max(w, 1)) * cw - 1, (y + 1) * ch - 1], fill=b)
                if ch_ != " ":
                    d.text((x * cw, y * ch), ch_, font=fontb if boldf else font, fill=f)
                if ul:
                    d.line([x * cw, (y + 1) * ch - 2, (x + max(w, 1)) * cw - 1, (y + 1) * ch - 2], fill=f)
            x += w
            i += 1
        # tmux trims trailing blank cells: extend the last background to the edge
        if x < cols:
            d.rectangle([x * cw, y * ch, cols * cw - 1, (y + 1) * ch - 1], fill=bg)
    img.save(out_path)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("-s", "--size", default="130x42", help="COLSxROWS (default 130x42)")
    ap.add_argument("-k", "--keys", default="", help="space-separated tmux key names sent before the shot")
    ap.add_argument("-m", "--mouse", action="append", default=[], help="mouse step, e.g. 'click 40 12' (repeatable)")
    ap.add_argument("--script", default="", help="';'-separated steps: keys, 'mouse click X Y', 'wait S', 'shot PATH', 'text'")
    ap.add_argument("-o", "--out", default="/tmp/shot.png")
    ap.add_argument("--text", action="store_true", help="print plain-text capture instead of writing a PNG")
    ap.add_argument("--startup", type=float, default=0.8, help="seconds to wait after launch")
    ap.add_argument("cmd", nargs=argparse.REMAINDER, help="-- binary and args")
    a = ap.parse_args()
    cmd = a.cmd[1:] if a.cmd and a.cmd[0] == "--" else a.cmd
    if not cmd:
        ap.error("need -- <binary> [args]")
    cols, rows = (int(v) for v in a.size.lower().split("x"))
    tm = Tmux(cmd, cols, rows)
    try:
        time.sleep(a.startup)
        if a.keys:
            tm.keys(*a.keys.split())
        for step in a.mouse:
            kind, x, y = step.split()
            tm.mouse(kind, int(x), int(y))
        if a.script:
            for step in a.script.split(";"):
                step = step.strip()
                if not step:
                    continue
                parts = step.split()
                if parts[0] == "wait":
                    time.sleep(float(parts[1]))
                elif parts[0] == "shot":
                    render(tm.capture(), cols, rows, parts[1])
                    print("wrote", parts[1])
                elif parts[0] == "text":
                    print(tm.capture(ansi=False))
                elif parts[0] == "mouse":
                    tm.mouse(parts[1], int(parts[2]), int(parts[3]))
                else:
                    tm.keys(*parts)
        time.sleep(0.2)
        if not tm.alive():
            print("PROCESS DIED", file=sys.stderr)
        if a.text:
            print(tm.capture(ansi=False))
        elif not a.script:
            render(tm.capture(), cols, rows, a.out)
            print("wrote", a.out)
    finally:
        err = tm.close()
        if err.strip():
            print("--- stderr ---\n" + err[-2000:], file=sys.stderr)


if __name__ == "__main__":
    main()
