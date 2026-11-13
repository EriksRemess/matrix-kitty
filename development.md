# Development

## Build

- Linux and Rust 1.100 or newer.
- A C compiler and `ar` for the small POSIX terminal shim.
- EGL 1.5, compatibility-profile OpenGL, and zlib development libraries.
  On Debian/Ubuntu the packages are `build-essential libegl-dev libgl-dev zlib1g-dev`.
- An EGL driver that supports device or surfaceless offscreen rendering.
  Mesa software rendering works when no GPU is available.

There are no Cargo dependencies or runtime font dependencies. The original
glyph atlas, including kana bitmaps, is compiled into the binary.

```sh
cargo build --release --offline
./target/release/matrix-kitty
```

## Rendering and animation

The renderer is adapted from `glmatrix-rs` 0.1.5 (base commit `bfc66ad`).
It keeps the original rain simulation and glyph atlas, renders into an offscreen
EGL buffer, and reads back pixels for terminal display. The C shim handles
POSIX terminal settings, input polling, window dimensions, and signals.

Terminal output uses a separate nonblocking descriptor, leaving the shell's
descriptor flags unchanged. Writes poll input and output while waiting for space,
so keyboard quit controls and termination signals can cancel a stalled frame.
Pause toggles and restart requests are coalesced for the main loop, while probe
replies remain available to the capability check. Shutdown restores terminal
settings before attempting display cleanup, which has a 100 ms deadline;
a terminal that remains unable to accept output may
not receive the cursor, image, and alternate-screen reset commands.

The default render limit is **960 × 540 at up to 30 fps**, scaled to fill the
terminal while preserving its pixel aspect ratio. `--width` and `--height`
set the maximum internal resolution; increasing them improves detail at the
cost of rendering and transfer bandwidth. If the terminal reports no pixel
dimensions, sizing assumes an 8 × 16 pixel cell. The simulation keeps the
original 30 ms timestep independently of the frame rate, with catch-up capped
after long stalls.

Rain starts after 600 simulation warm-up ticks. Use `--warmup 0` for the
original gradual entrance. `--seed N` gives reproducible rain, and `--frames N`
exits after N displayed frames. Pause and resize continue to work with these
options.

## Kitty protocol and compatibility

Each frame is an opaque RGB image, zlib-compressed and base64-encoded in payloads
of at most 4096 bytes. A root image is placed once with `a=T`. Later frames use
`a=f` and `a=a,c=…` to alternate between two reusable animation frame slots.
The process generates fresh rain continuously and controls frame presentation;
terminal storage does not grow with animation duration.

Frame continuation chunks repeat `a=f,r=…,X=1` along with `m` and `q`.
Kitty 0.48.2 reads the edit target and replacement mode from the final chunk;
omitting them appends frames instead of editing the two existing slots. This
compatibility workaround extends the minimal continuation headers described in
the [protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/#remote-client).

Startup probes both image loading and animation frame editing. Older terminals
with only static Kitty graphics support fail with a clear error. `--force` skips
that probe for terminals known to support animation but unable to return replies.
[Ghostty added animation support in 1.4.](https://ghostty.org/docs/install/release-notes/1-4-0)

This first port outputs SDR and uses keyboard controls. HDR, desktop decorations,
mouse controls, and terminal-multiplexer passthrough are not implemented. Run
directly in the terminal rather than inside tmux or screen.

## Additional options

`--mode` accepts `matrix`, `binary`, `decimal`, `hexadecimal`, and `dna`.
Original single-dash switches and `+fog`, `+waves`, `+rotate`, `+texture`, and
`+flip` are also accepted. Use `--no-fog`, `--no-waves`, `--no-rotate`,
`--no-texture`, `--flip`, or `--wireframe` to change the rendering.

`--fg COLOR` sets the glyph color, retaining lighter highlights and strip heads.
`--bg COLOR` sets the background, with fog and dim glyphs fading into that color.
Colors accept three or six hexadecimal digits, with an optional `#` prefix.
The default palette retains the original green glow on black. Colored backgrounds
use alpha blending so that dark glyphs also remain visible on light backgrounds.

`--transparent-bg` queries the terminal's default background once at startup
using [OSC 11](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html).
It accepts `rgb:R/G/B` replies with one to four hex digits per channel and either
BEL or ST termination. The query waits up to 800 ms for a valid reply, then falls
back to `--bg` (black by default). The queried color takes precedence over `--bg`
regardless of argument order. Keyboard controls remain active during the query,
and `--force` skips only the Kitty capability probe, not the color query.
Frames remain opaque RGB images: this option matches a terminal color, not its
opacity or wallpaper. Use explicit `--bg` for headless `--snapshot` output.

## Headless preview

Save an actual rendered frame as a binary PPM image:

```sh
cargo run --release --offline -- --seed 42 --snapshot /tmp/matrix.ppm
```

This needs EGL but no terminal graphics support or display server. Snapshot
dimensions are exactly `--width` × `--height`. Output reports the selected
OpenGL renderer. To explicitly select Mesa software rendering on Debian/Ubuntu:

```sh
__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json \
  LIBGL_ALWAYS_SOFTWARE=true \
  cargo run --release --offline -- --snapshot /tmp/matrix.ppm
```

Adjust the vendor JSON path on other distributions. Interactive mode uses the
same renderer selection and can be launched with the same environment variables.

## Validation

```sh
cargo fmt --check
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo build --release --offline
python3 scripts/smoke.py
```

The Python smoke test uses a real pseudo-terminal and a small Kitty protocol
peer. It decodes and decompresses image data, compares the first frame with an
independent snapshot, checks frame-slot reuse, exercises resize/pause/restart,
and verifies cleanup after keys, SIGTERM, rejection, and probe timeout. It also
checks quit controls after unfinished escape sequences and termination with a
full PTY output queue, using both signals and keyboard quit controls. It checks
restoration of terminal settings and descriptor flags, and preservation of pause
and resume through output stalls. It needs Python 3 and working headless EGL.
It does not establish visual playback or
performance in a real terminal emulator.

Color checks compare decoded terminal frames with explicit-color snapshots,
exercise OSC 11 replies, fallback, and cancellation, and check dark glyphs on a
light background in textured, solid, and wireframe modes.

When Kitty is installed, also run its actual graphics decoder against the binary:

```sh
kitty +runpy 'import runpy; runpy.run_path("scripts/kitty-smoke.py", run_name="__main__")'
```

This uses Kitty's internal test API (tested with 0.48.2) without opening a window.
It checks every selected frame's pixels, bounded frame storage, resize, and cleanup
for both single-chunk and chunked uploads. Set `MATRIX_KITTY_BINARY` to test another
binary. This validates Kitty's decoding and frame selection, not GUI presentation.
