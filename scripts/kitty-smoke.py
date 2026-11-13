#!/usr/bin/env python3
"""Check the real binary's frame uploads with Kitty's installed graphics engine.

Run with: kitty +runpy 'import runpy; runpy.run_path("scripts/kitty-smoke.py", run_name="__main__")'
Uses Kitty's internal test API, tested with 0.48.2. No GUI or GPU context is opened.
"""
import os
from pathlib import Path
import sys
import tempfile

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import smoke
from kitty.fast_data_types import Screen

smoke.BINARY = Path(os.environ.get("MATRIX_KITTY_BINARY", "target/release/matrix-kitty")).resolve()


class KittyTerminal(smoke.Terminal):
    def __init__(self, *args):
        self.screen = Screen(None, 24, 80, 0, 12, 24)
        self.checked_frames = 0
        super().__init__(*args)

    def command(self, command):
        buffer = self.screen.test_create_write_buffer()
        self.screen.test_commit_write_buffer(b"\x1b_G" + command + b"\x1b\\", buffer)
        self.screen.test_parse_written_data()
        # The independent peer records the decoded pixels requested by the application.
        super().command(command)
        for number in self.images:
            image = self.screen.grman.image_for_client_number(int(number))
            assert image is not None, "Kitty lost the animation image"
            assert len(image["extra_frames"]) <= 1, "Kitty appended frames instead of editing them"
            if self.checked_frames < len(self.presented):
                width, height, expected = self.presented[-1]
                assert (image["width"], image["height"]) == (width, height)
                index = image["current_frame_index"]
                frame = image if index == 0 else image["extra_frames"][index - 1]
                assert frame["data"] == expected, "Kitty selected stale or corrupted frame pixels"
                self.checked_frames += 1

    def finish(self, expected=0):
        result = super().finish(expected)
        assert self.screen.grman.image_count == 0, "Kitty retained the deleted image"
        assert self.checked_frames == len(self.presented)
        return result


with tempfile.TemporaryDirectory(prefix="matrix-kitty-cache-") as directory:
    os.environ["KITTY_CACHE_DIRECTORY"] = directory
    for name, args in [
        ("chunked frame edits", []),
        ("single-chunk frame edits", ["--width", "32", "--height", "16"]),
    ]:
        terminal = KittyTerminal("--frames", "12", *args)
        try:
            terminal.finish()
            assert terminal.checked_frames == 12
            print(f"PASS Kitty engine: {name}, all 12 displayed frames match, bounded storage")
        finally:
            terminal.close()
    terminal = KittyTerminal()
    try:
        smoke.controls(terminal)
        print("PASS Kitty engine: pause, resize, restart, resume, and cleanup")
    finally:
        terminal.close()
