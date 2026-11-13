# matrix-kitty

3D Matrix rain in your terminal, based on
[glmatrix-rs](https://github.com/EriksRemess/glmatrix-rs). Includes falling glyphs,
depth fog, brightness waves, and a rotating camera.

Requires Linux and **Ghostty 1.4+ or Kitty**. Run directly in the terminal;
tmux and screen passthrough are not supported.

## Install

```sh
cargo install matrix-kitty
```

See [development.md](development.md) for build prerequisites and development
instructions.

Then run:

```sh
matrix-kitty
```

## Controls

| Key | Action |
| --- | --- |
| `q`, Escape, Ctrl-C, Ctrl-D | Quit |
| Space | Pause/resume |
| `r`, Backspace | Drop the rain and restart |
| Ctrl-Z | Quit (job suspension is not supported) |

The animation resizes with the terminal.

## Options

```sh
matrix-kitty --mode hexadecimal --density 35 --speed 1.5
matrix-kitty --clock --timefmt ' %H:%M ' --no-rotate
matrix-kitty --fg '#ffb454' --bg '#181825'
matrix-kitty --fg '#89dceb' --transparent-bg
matrix-kitty --help
```

Modes: `matrix`, `binary`, `decimal`, `hexadecimal`, and `dna`.
Use `--width`, `--height`, and `--fps` to adjust detail and frame rate;
the defaults are a maximum of 960 × 540 pixels and 30 fps.

`--fg` and `--bg` accept hex colors (`#RGB` or `#RRGGBB`).
`--transparent-bg` reads the terminal's background color at startup and matches
it; if no color is returned, it uses `--bg` (black by default). This matches the
color rather than making the image translucent.

## License

Based on Jamie Zawinski's XScreenSaver GLMatrix via `glmatrix-rs`.
See [LICENSE](LICENSE) and [NOTICE](NOTICE) for attribution and the bundled
Noto-derived glyph license.
