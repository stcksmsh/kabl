#!/usr/bin/env python3
"""Drives the real release kabl-ui with real X input (xdotool) from a small script, aiming at
the targets the app itself reports (KABL_HITS_FILE), and saves screenshots.

    Xvfb :97 -screen 0 1600x1000x24 &
    DISPLAY=:97 python3 docs/rack-migration/drive.py docs/rack-migration/scripts/tour.txt 1440x900 OUTDIR

Script lines (# comments):
    click KEY | rclick KEY | dclick KEY          press on a target's centre
    clickat X Y | rclickat X Y
    drag KEY KEY                                 press on one target, release on another
    dragby KEY DX DY [shift]                     press, move by (DX, DY), release
    pan BX BY KEY X Y                            drag bare rack at (BX, BY) so KEY lands at (X, Y)
    hold KEY DX DY | release                     press and move without releasing
    wheel KEY N [ctrl]                           N wheel notches (negative = down) over a target
    key COMBO                                    e.g. ctrl+z, Escape
    type TEXT
    at X Y                                       move the pointer
    nudge DX DY                                  move the pointer relative (button state unchanged)
    shot NAME                                    screenshot to OUTDIR/NAME.png
    save DIR                                     type DIR into the patch field and press Save
    sleep S
    wait T                                       until T s after the script's first line
    midi LINE                                    a line for the virtual controller (KABL_PLAYER)
    midikill | midistart                         unplug / plug the virtual controller back in

Environment: KABL_DRIVE_LOG=FILE logs each line with its wall-clock time; KABL_APP_LOG=FILE
keeps kabl-ui's stderr; KABL_ARGS adds
kabl-ui arguments; KABL_PLAYER=1 starts
target/release/examples/midi_player first (a virtual MIDI port, `kabl-player`). Without an ALSA
sequencer (a container) also set KABL_MIDI_PIPE=PATH and pass `--midi kabl-pipe`: the player
and kabl-ui then talk through that fifo (a test hook, see crates/ui/src/main.rs).
"""
import os
import subprocess
import sys
import time

script, size, out = sys.argv[1], sys.argv[2], sys.argv[3]
patch = sys.argv[4] if len(sys.argv) > 4 else "patches/reference"
os.makedirs(out, exist_ok=True)
# Display scale (WINIT_X11_SCALE_FACTOR): targets are in egui points, xdotool works in pixels.
scale = float(os.environ.get("WINIT_X11_SCALE_FACTOR", "1"))
hits_file = os.path.join(out, "hits.txt")
env = dict(os.environ, KABL_HITS_FILE=hits_file)
player = None


def start_player():
    p = subprocess.Popen(["./target/release/examples/midi_player"], stdin=subprocess.PIPE,
                         text=True)
    time.sleep(0.5)
    return p


if os.environ.get("KABL_PLAYER"):
    player = start_player()
extra = os.environ.get("KABL_ARGS", "").split()
app = subprocess.Popen(["./target/release/kabl-ui", "--patch", patch, "--size", size, *extra],
                       env=env, stdout=subprocess.DEVNULL,
                       stderr=open(os.environ["KABL_APP_LOG"], "w") if os.environ.get("KABL_APP_LOG")
                       else subprocess.DEVNULL)


def x(*args):
    subprocess.run(["xdotool", *map(str, args)], check=True)


def hits():
    out = {}
    for line in open(hits_file):
        k, *r = line.split()
        out[k] = tuple(map(float, r))
    return out


def centre(key):
    # Up to 10 s: a heavy patch can take a few seconds to draw its first frames.
    for _ in range(100):
        h = hits()
        if key in h:
            x0, y0, x1, y1 = h[key]
            return round((x0 + x1) / 2 * scale), round((y0 + y1) / 2 * scale)
        time.sleep(0.1)
    raise SystemExit(f"no target {key}")


def glide(tx, ty, steps=10):
    loc = subprocess.run(["xdotool", "getmouselocation", "--shell"], capture_output=True, text=True).stdout
    cur = dict(l.split("=") for l in loc.split())
    cx, cy = int(cur["X"]), int(cur["Y"])
    for i in range(1, steps + 1):
        x("mousemove", round(cx + (tx - cx) * i / steps), round(cy + (ty - cy) * i / steps))
        time.sleep(0.02)
    time.sleep(0.15)


def press_move(ax, ay, bx, by):
    glide(ax, ay)
    x("mousedown", 1)
    time.sleep(0.1)
    for i in range(1, 13):
        x("mousemove", round(ax + (bx - ax) * i / 12), round(ay + (by - ay) * i / 12))
        time.sleep(0.03)
    time.sleep(0.2)


try:
    time.sleep(3)
    wid = subprocess.run(["xdotool", "search", "--name", "^kabl$"], capture_output=True, text=True).stdout.split()[0]
    x("windowfocus", "--sync", wid)
    w, h = (round(int(v) * scale) for v in size.split("x"))
    log = open(os.environ["KABL_DRIVE_LOG"], "w") if os.environ.get("KABL_DRIVE_LOG") else None
    start = time.time()
    for raw in open(script):
        line = raw.split("#")[0].strip()
        if not line:
            continue
        if log:
            log.write(f"{time.time():.3f} {line}\n")
            log.flush()
        cmd, *a = line.split()
        if cmd in ("click", "rclick", "dclick"):
            glide(*centre(a[0]))
            x("click", *(["--repeat", "2", "--delay", "80"] if cmd == "dclick" else []), 3 if cmd == "rclick" else 1)
        elif cmd in ("clickat", "rclickat"):
            glide(int(a[0]), int(a[1]))
            x("click", 3 if cmd == "rclickat" else 1)
        elif cmd == "drag":
            press_move(*centre(a[0]), *centre(a[1]))
            x("mouseup", 1)
        elif cmd == "dragby":
            ax, ay = centre(a[0])
            if a[3:] == ["shift"]:
                x("keydown", "shift")
            press_move(ax, ay, ax + round(int(a[1]) * scale), ay + round(int(a[2]) * scale))
            x("mouseup", 1)
            if a[3:] == ["shift"]:
                x("keyup", "shift")
        elif cmd == "pan":
            bx, by, kx, ky = int(a[0]), int(a[1]), *centre(a[2])
            press_move(bx, by, bx + int(a[3]) - kx, by + int(a[4]) - ky)
            x("mouseup", 1)
        elif cmd == "hold":
            ax, ay = centre(a[0])
            press_move(ax, ay, ax + int(a[1]), ay + int(a[2]))
        elif cmd == "release":
            x("mouseup", 1)
        elif cmd == "wheel":
            glide(*centre(a[0]))
            n = int(a[1])
            if a[2:] == ["ctrl"]:
                x("keydown", "ctrl")
            for _ in range(abs(n)):
                x("click", 4 if n > 0 else 5)
                time.sleep(0.05)
            if a[2:] == ["ctrl"]:
                x("keyup", "ctrl")
        elif cmd == "key":
            x("key", a[0])
        elif cmd == "type":
            x("type", "--delay", "5", " ".join(a))
        elif cmd == "nudge":
            loc = subprocess.run(["xdotool", "getmouselocation", "--shell"], capture_output=True, text=True).stdout
            cur = dict(l.split("=") for l in loc.split())
            glide(int(cur["X"]) + int(a[0]), int(cur["Y"]) + int(a[1]))
        elif cmd == "at":
            glide(int(a[0]), int(a[1]))
        elif cmd == "shot":
            time.sleep(0.4)
            subprocess.run(["import", "-window", "root", "-crop", f"{w}x{h}+0+0",
                            os.path.join(out, a[0] + ".png")], check=True)
        elif cmd == "save":
            glide(*centre("patch-path"))
            x("click", 1)
            x("key", "ctrl+a")
            x("type", "--delay", "5", os.path.abspath(a[0]))
            x("key", "Return")
            glide(*centre("save"))
            x("click", 1)
            time.sleep(0.5)
        elif cmd == "sleep":
            time.sleep(float(a[0]))
        elif cmd == "wait":
            # Absolute: until T s after the script started (no settle time after it).
            time.sleep(max(0.0, start + float(a[0]) - time.time()))
            continue
        elif cmd == "midikill":
            player.kill()
            player.wait()
            # The fifo stand-in (KABL_MIDI_PIPE) is "unplugged" when it is gone.
            if os.environ.get("KABL_MIDI_PIPE"):
                try:
                    os.remove(os.environ["KABL_MIDI_PIPE"])
                except FileNotFoundError:
                    pass
        elif cmd == "midistart":
            player = start_player()
        elif cmd == "midi":
            player.stdin.write(" ".join(a) + "\n")
            player.stdin.flush()
            continue
        else:
            raise SystemExit(f"unknown command {cmd}")
        time.sleep(0.3)
finally:
    app.terminate()
    if player:
        player.terminate()
