#!/usr/bin/env python3
"""Exercise the real binary through a PTY with a small Kitty protocol peer.

Requires Linux, Python 3, and working headless EGL. Does not open a terminal GUI.
"""
import base64
from contextlib import contextmanager
import fcntl
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
import zlib

BINARY = Path(sys.argv[1] if len(sys.argv) > 1 else "target/release/matrix-kitty").resolve()


class Terminal:
    def __init__(self, *args, reject=False, respond=True, background_reply=None):
        self.master, self.slave = pty.openpty()
        self.resize(80, 24, 960, 540)
        self.saved = termios.tcgetattr(self.slave)
        self.saved_flags = fcntl.fcntl(self.slave, fcntl.F_GETFL)
        env = os.environ.copy()
        env.pop("TMUX", None)
        self.process = subprocess.Popen(
            [str(BINARY), "--seed", "42", "--width", "320", "--height", "180", *args],
            stdin=self.slave, stdout=self.slave, stderr=subprocess.PIPE, env=env,
        )
        self.respond = respond
        self.reject = reject
        self.wire = bytearray()
        self.pending = bytearray()
        self.upload = None
        self.images = {}
        self.presented = []
        self.max_frames = 0
        self.frame_edits = 0
        self.probes = 0
        self.background_queries = 0
        self.background_reply = background_reply

    def resize(self, cols, rows, width, height):
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, width, height))

    def send(self, data):
        os.write(self.master, data)

    def command(self, command):
        header, _, payload = command.partition(b";")
        keys = dict(item.split("=", 1) for item in header.decode().split(",") if "=" in item)
        action = keys.get("a", "t")
        number = keys.get("I")
        if payload or self.upload is not None:
            assert len(payload) <= 4096 and len(payload) % 4 == 0
            if self.upload is None:
                self.upload = (keys, bytearray())
            else:
                first = self.upload[0]
                if first.get("a") == "f":
                    assert action == "f", "frame continuation lost a=f"
                    assert keys.get("r") == first.get("r"), "frame continuation lost its edit target"
                    assert keys.get("X") == first.get("X"), "frame continuation lost replacement mode"
            self.upload[1].extend(payload)
            if keys.get("m", "0") == "1":
                return
            keys, encoded = self.upload
            self.upload = None
            action = keys.get("a", "t")
            number = keys.get("I")
            pixels = base64.b64decode(encoded, validate=True)
            if keys.get("o") == "z":
                pixels = zlib.decompress(pixels)
            width, height = int(keys["s"]), int(keys["v"])
            assert keys["f"] == "24"
            assert len(pixels) == width * height * 3
            if keys.get("q", "0") != "2":
                self.probes += 1
                if self.respond:
                    status = "EINVAL:animation unavailable" if self.reject and action == "f" else "OK"
                    self.send(f"\x1b_Gi=100,I={number};{status}\x1b\\".encode())
                return
            if action == "T":
                assert keys["C"] == "1"
                self.images[number] = (width, height, [pixels])
                self.presented.append((width, height, pixels))
            else:
                assert action == "f" and keys["X"] == "1"
                old_width, old_height, frames = self.images[number]
                assert (width, height) == (old_width, old_height)
                frame = int(keys.get("r", "0"))
                if frame == 0:
                    frames.append(pixels)
                else:
                    assert 1 <= frame <= len(frames)
                    frames[frame - 1] = pixels
                    self.frame_edits += 1
            self.max_frames = max(self.max_frames, len(self.images[number][2]))
            assert self.max_frames <= 2, "animation accumulates terminal frames"
        elif action == "a":
            width, height, frames = self.images[number]
            assert keys["s"] == "1"
            self.presented.append((width, height, frames[int(keys["c"]) - 1]))
        elif action == "d":
            assert keys["d"] == "N", "must delete only the owned image number"
            self.images.pop(number, None)

    def pump(self, duration=0.05):
        readable, _, _ = select.select([self.master], [], [], duration)
        if not readable:
            return
        data = os.read(self.master, 65536)
        self.wire.extend(data)
        self.pending.extend(data)
        while True:
            starts = [self.pending.find(prefix) for prefix in (b"\x1b_G", b"\x1b]")]
            start = min((position for position in starts if position >= 0), default=-1)
            cancel = self.pending.find(b"\x18")
            end = self.pending.find(b"\x1b\\", start) if start >= 0 else -1
            if cancel >= 0 and (start < 0 or cancel < start or end < 0 or cancel < end):
                self.upload = None
                del self.pending[:cancel + 1]
                continue
            if start < 0:
                self.pending[:] = self.pending[-2:]
                break
            if end < 0:
                del self.pending[:start]
                break
            osc = self.pending[start:start + 2] == b"\x1b]"
            command = bytes(self.pending[start + (2 if osc else 3):end])
            del self.pending[:end + 2]
            if osc:
                assert command == b"11;?", "unexpected terminal color mutation"
                self.background_queries += 1
                if self.background_reply is not None:
                    chunks = self.background_reply
                    if isinstance(chunks, bytes):
                        chunks = [chunks]
                    for chunk in chunks:
                        self.send(chunk)
                        time.sleep(0.01)
            else:
                self.command(command)

    def until(self, predicate, timeout=5):
        deadline = time.monotonic() + timeout
        while not predicate() and time.monotonic() < deadline:
            self.pump()
        assert predicate(), "timed out waiting for terminal state"

    def drain_for(self, duration):
        deadline = time.monotonic() + duration
        while time.monotonic() < deadline:
            self.pump(0.01)

    def finish(self, expected=0):
        self.until(lambda: self.process.poll() is not None)
        self.drain_for(0.03)
        errors = self.process.stderr.read().decode()
        assert self.process.returncode == expected, errors
        assert termios.tcgetattr(self.slave) == self.saved, "termios was not restored"
        assert fcntl.fcntl(self.slave, fcntl.F_GETFL) == self.saved_flags, "terminal descriptor flags changed"
        assert b"\x1b[?25h\x1b[?1049l" in self.wire, "cursor or main screen not restored"
        assert not self.images, "image storage was not released"
        return errors

    def close(self):
        if self.process.poll() is None:
            self.process.kill()
            self.process.wait()
        self.process.stderr.close()
        os.close(self.master)
        os.close(self.slave)


def check(name, test, *args, **kwargs):
    terminal = Terminal(*args, **kwargs)
    try:
        test(terminal)
        print(f"PASS {name}")
    finally:
        terminal.close()


def frames(t):
    t.finish()
    assert t.probes == 2 and len(t.presented) == 12 and t.max_frames == 2
    assert t.frame_edits == 10
    assert len({pixels for _, _, pixels in t.presented}) > 1
    width, height, pixels = t.presented[0]
    assert (width, height) == (320, 180)
    assert any(pixels)
    # An independently saved snapshot must match the decoded first Kitty frame.
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "frame.ppm"
        subprocess.run([str(BINARY), "--seed", "42", "--width", "320", "--height", "180",
                        "--snapshot", str(path)], check=True, capture_output=True)
        assert path.read_bytes() == b"P6\n320 180\n255\n" + pixels


def controls(t):
    t.until(lambda: len(t.presented) >= 3)
    t.send(b" ")
    t.drain_for(0.15)
    paused = len(t.presented)
    t.drain_for(0.2)
    assert len(t.presented) == paused, "paused animation still transmits"
    t.resize(60, 30, 900, 900)
    t.until(lambda: any(w == 180 and h == 180 for w, h, _ in t.presented))
    t.send(b"r ")
    t.until(lambda: len(t.presented) >= paused + 4)
    t.send(b"q")
    t.finish()


def shutdown(t, byte=None, sig=None):
    t.until(lambda: len(t.presented) >= 2)
    if sig is not None:
        t.process.send_signal(sig)
    else:
        t.send(byte)
    t.finish()


def reject(t):
    assert "rejected Kitty animation" in t.finish(expected=1)


def timeout(t):
    assert "no Kitty animation response" in t.finish(expected=1)


def force(t):
    t.finish()
    assert t.probes == 0 and len(t.presented) == 3


def unfinished_escape(t, prefix, key, delay=0):
    t.until(lambda: len(t.presented) >= 2)
    t.send(prefix)
    t.drain_for(delay)
    t.send(key)
    t.finish()


@contextmanager
def blocked_output(t):
    t.until(lambda: len(t.presented) >= 2)
    t.send(b" ")
    t.drain_for(0.15)
    filler = os.open(os.ttyname(t.slave), os.O_WRONLY | os.O_NONBLOCK | os.O_NOCTTY)
    try:
        # Fill the PTY without draining it, then make the application write a frame.
        written = 0
        while True:
            try:
                written += os.write(filler, b"." * 4096)
            except BlockingIOError:
                break
        assert written > 0
        assert fcntl.fcntl(t.slave, fcntl.F_GETFL) == t.saved_flags
        t.send(b" ")
        time.sleep(0.1)
        assert t.process.poll() is None
        yield
    finally:
        os.close(filler)


def stalled_output(t, key=None):
    with blocked_output(t):
        if key is None:
            t.process.send_signal(signal.SIGTERM)
        else:
            t.send(key)
        # Do not pump here: exit must not depend on the peer reading output.
        assert t.process.wait(timeout=2) == 0, t.process.stderr.read().decode()
        assert termios.tcgetattr(t.slave) == t.saved
        assert fcntl.fcntl(t.slave, fcntl.F_GETFL) == t.saved_flags
        assert not t.process.stderr.read(), "cancellation printed an error"


def stalled_controls(t):
    with blocked_output(t):
        t.send(b" r")
        time.sleep(0.1)
        t.drain_for(0.2)
        paused = len(t.presented)
        t.drain_for(0.2)
        assert len(t.presented) == paused, "pause received during a write was lost"
        t.send(b" ")
        t.until(lambda: len(t.presented) >= paused + 3)
        t.send(b"q")
        t.finish()


def snapshot(*args):
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "frame.ppm"
        subprocess.run([str(BINARY), "--seed", "42", "--width", "320", "--height", "180",
                        *args, "--snapshot", str(path)], check=True, capture_output=True)
        return path.read_bytes().split(b"\n", 3)[3]


def colored_frame(t, background, queried=True, probes=2):
    t.finish()
    assert t.background_queries == int(queried) and t.probes == probes
    assert len(t.presented) == 1
    pixels = t.presented[0][2]
    assert pixels == snapshot("--fg", "#f00", "--bg", background)
    colors = list(zip(pixels[::3], pixels[1::3], pixels[2::3]))
    bg = tuple(bytes.fromhex(background.removeprefix("#")))
    assert bg in colors, "background color is missing from rendered frame"
    assert any(r > g + 20 and r > b + 20 for r, g, b in colors), "red glyphs are missing"


def quit_color_query(t):
    t.until(lambda: t.background_queries == 1)
    t.send(b"\x03")
    assert not t.finish()
    assert not t.presented, "rendered despite quitting during the color query"


if __name__ == "__main__":
    check("RGB, compression, chunking, two frame slots, snapshot parity", frames, "--frames", "12")
    check("pause, resize, restart, resume, quit", controls)
    check("Ctrl-C restores terminal", lambda t: shutdown(t, byte=b"\x03"))
    check("Escape restores terminal", lambda t: shutdown(t, byte=b"\x1b"))
    check("SIGTERM restores terminal", lambda t: shutdown(t, sig=signal.SIGTERM))
    check("unsupported animation restores terminal", reject, reject=True)
    check("probe timeout restores terminal", timeout, respond=False)
    check("--force skips probing", force, "--force", "--frames", "3", respond=False)
    check("Ctrl-C escapes unfinished OSC", lambda t: unfinished_escape(t, b"\x1b]title", b"\x03"))
    check("Ctrl-D escapes unfinished CSI", lambda t: unfinished_escape(t, b"\x1b[1;", b"\x04"))
    check("Escape escapes unfinished APC", lambda t: unfinished_escape(t, b"\x1b_GI=9;", b"\x1b"))
    check("quit works after an escape string expires", lambda t: unfinished_escape(t, b"\x1b]title", b"q", 0.3))
    check("SIGTERM restores terminal while output is blocked", stalled_output)
    for name, key in [("Ctrl-C", b"\x03"), ("q", b"q"), ("Escape", b"\x1b"),
                      ("Ctrl-D", b"\x04"), ("Ctrl-Z", b"\x1a"),
                      ("Ctrl-C after unfinished OSC", b"\x1b]title\x03")]:
        check(f"{name} restores terminal while output is blocked", lambda t, key=key: stalled_output(t, key))
    check("pause and resume survive blocked output", stalled_controls)
    check("explicit foreground and light background", lambda t: colored_frame(t, "#ffffff", queried=False),
          "--fg", "#f00", "--bg", "#fff", "--frames", "1")
    check("terminal background overrides explicit fallback", lambda t: colored_frame(t, "#18304a"),
          "--fg", "#f00", "--transparent-bg", "--bg", "#fff", "--frames", "1",
          background_reply=b"\x1b]11;rgb:1818/3030/4a4a\x07")
    check("fragmented background reply with ST", lambda t: colored_frame(t, "#18304a"),
          "--fg", "#f00", "--transparent-bg", "--frames", "1",
          background_reply=[b"\x1b]10;rgb:0/0/0\x07\x1b]11;rgb:18", b"/30/4a\x1b", b"\\"])
    check("background timeout uses explicit fallback", lambda t: colored_frame(t, "#112233"),
          "--fg", "#f00", "--bg", "#112233", "--transparent-bg", "--frames", "1")
    check("malformed background reply uses default black", lambda t: colored_frame(t, "#000000"),
          "--fg", "#f00", "--transparent-bg", "--frames", "1",
          background_reply=b"\x1b]11;rgb:gg/00/00\x07")
    check("--force still queries the background", lambda t: colored_frame(t, "#ffffff", probes=0),
          "--force", "--fg", "#f00", "--transparent-bg", "--frames", "1", respond=False,
          background_reply=b"\x1b]11;rgb:f/f/f\x1b\\")
    check("quit while waiting for a background reply", quit_color_query, "--transparent-bg")
    for mode in [[], ["--no-texture"], ["--wireframe"]]:
        pixels = snapshot("--fg", "#000", "--bg", "#fff", *mode)
        colors = list(zip(pixels[::3], pixels[1::3], pixels[2::3]))
        assert (255, 255, 255) in colors
        assert all(r == g == b for r, g, b in colors), "foreground tint was not applied"
        assert any(r < 128 for r, _, _ in colors), "dark glyphs vanished on a light background"
        print(f"PASS black foreground on white background ({mode or ['textured']})")
