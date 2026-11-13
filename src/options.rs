use crate::matrix::GlyphMode;
use std::{ffi::OsString, path::PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color(pub [u8; 3]);

impl Color {
    pub fn parse(value: &str) -> Result<Self, String> {
        let hex = value.strip_prefix('#').unwrap_or(value);
        if !matches!(hex.len(), 3 | 6) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("invalid color {value:?}; use #RGB or #RRGGBB"));
        }
        let digits = hex.len() / 3;
        Ok(Self(std::array::from_fn(|i| {
            let value = u8::from_str_radix(&hex[i * digits..(i + 1) * digits], 16).unwrap();
            if digits == 1 { value * 17 } else { value }
        })))
    }

    pub fn components(self) -> [f32; 3] {
        self.0.map(|value| f32::from(value) / 255.0)
    }
}

#[derive(Clone)]
pub struct Options {
    pub speed: f32,
    pub density: f32,
    pub do_clock: bool,
    pub timefmt: String,
    pub do_fog: bool,
    pub do_waves: bool,
    pub do_rotate: bool,
    pub do_texture: bool,
    pub flip_texture: Option<bool>,
    pub wireframe: bool,
    pub mode: GlyphMode,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub seed: Option<u64>,
    pub warmup: u32,
    pub frames: Option<u64>,
    pub snapshot: Option<PathBuf>,
    pub force: bool,
    pub fg: Option<Color>,
    pub bg: Color,
    pub transparent_bg: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            speed: 1.0,
            density: 20.0,
            do_clock: false,
            timefmt: " %l%M%p ".into(),
            do_fog: true,
            do_waves: true,
            do_rotate: true,
            do_texture: true,
            flip_texture: Some(false),
            wireframe: false,
            mode: GlyphMode::Matrix,
            width: 960,
            height: 540,
            fps: 30,
            seed: None,
            warmup: 600,
            frames: None,
            snapshot: None,
            force: false,
            fg: None,
            bg: Color::default(),
            transparent_bg: false,
        }
    }
}

impl Options {
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Option<Self>, String> {
        let mut options = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let arg = arg.to_str().ok_or("options must be valid UTF-8")?;
            if arg == "--snapshot" {
                options.snapshot = Some(args.next().ok_or("--snapshot needs a path")?.into());
                continue;
            }
            let mut value = || -> Result<String, String> {
                args.next()
                    .ok_or_else(|| format!("{arg} needs a value"))?
                    .into_string()
                    .map_err(|_| format!("{arg} needs a UTF-8 value"))
            };
            match arg {
                "-h" | "--help" => return Ok(None),
                "-speed" | "--speed" => options.speed = number(arg, &value()?, 0.05, 20.0)?,
                "-density" | "--density" => options.density = number(arg, &value()?, 0.1, 1000.0)?,
                "-mode" | "--mode" => options.mode = GlyphMode::parse(&value()?)?,
                "-binary" | "--binary" => options.mode = GlyphMode::Binary,
                "-decimal" | "--decimal" => options.mode = GlyphMode::Decimal,
                "-hex" | "--hex" | "-hexadecimal" | "--hexadecimal" => {
                    options.mode = GlyphMode::Hexadecimal
                }
                "-dna" | "--dna" => options.mode = GlyphMode::Dna,
                "-clock" | "--clock" => options.do_clock = true,
                "+clock" | "--no-clock" => options.do_clock = false,
                "-timefmt" | "--timefmt" => {
                    options.timefmt = value()?;
                    if options.timefmt.contains('\0') {
                        return Err("time format contains NUL".into());
                    }
                }
                "-fog" | "--fog" => options.do_fog = true,
                "+fog" | "--no-fog" => options.do_fog = false,
                "-waves" | "--waves" => options.do_waves = true,
                "+waves" | "--no-waves" => options.do_waves = false,
                "-rotate" | "--rotate" => options.do_rotate = true,
                "+rotate" | "--no-rotate" => options.do_rotate = false,
                "-texture" | "--texture" => options.do_texture = true,
                "+texture" | "--no-texture" => options.do_texture = false,
                "-flip" | "--flip" => options.flip_texture = Some(true),
                "+flip" | "--no-flip" => options.flip_texture = Some(false),
                "-wireframe" | "--wireframe" => options.wireframe = true,
                "+wireframe" | "--no-wireframe" => options.wireframe = false,
                "-width" | "--width" => options.width = integer(arg, &value()?, 16, 4096)? as u32,
                "-height" | "--height" => {
                    options.height = integer(arg, &value()?, 16, 4096)? as u32
                }
                "--fps" => options.fps = integer(arg, &value()?, 1, 120)? as u32,
                "--warmup" => options.warmup = integer(arg, &value()?, 0, 10_000)? as u32,
                "--seed" => options.seed = Some(integer(arg, &value()?, 0, u64::MAX)?),
                "--frames" => options.frames = Some(integer(arg, &value()?, 1, u64::MAX)?),
                "--force" => options.force = true,
                "--fg" => options.fg = Some(Color::parse(&value()?)?),
                "--bg" => options.bg = Color::parse(&value()?)?,
                "--transparent-bg" => options.transparent_bg = true,
                _ => return Err(format!("unknown option {arg:?}; use --help")),
            }
        }
        if options.wireframe {
            options.do_texture = false;
        }
        if options.transparent_bg && options.snapshot.is_some() {
            return Err(
                "--transparent-bg requires interactive output; use --bg with --snapshot".into(),
            );
        }
        Ok(Some(options))
    }

    pub fn render_size(&self, width: u32, height: u32) -> (u32, u32) {
        let width = width.max(1);
        let height = height.max(1);
        let scale = (self.width as f64 / width as f64)
            .min(self.height as f64 / height as f64)
            .min(1.0);
        (
            (width as f64 * scale).round().max(1.0) as u32,
            (height as f64 * scale).round().max(1.0) as u32,
        )
    }
}

fn number(name: &str, value: &str, min: f32, max: f32) -> Result<f32, String> {
    let parsed = value
        .parse::<f32>()
        .map_err(|_| format!("{name} needs a number"))?;
    if !parsed.is_finite() || !(min..=max).contains(&parsed) {
        return Err(format!("{name} must be between {min} and {max}"));
    }
    Ok(parsed)
}

fn integer(name: &str, value: &str, min: u64, max: u64) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{name} needs an integer"))?;
    if !(min..=max).contains(&parsed) {
        return Err(format!("{name} must be between {min} and {max}"));
    }
    Ok(parsed)
}

pub const HELP: &str = "matrix-kitty — GLMatrix in your terminal

Usage: matrix-kitty [options]
  --speed N          Animation speed (0.05–20; default 1)
  --density N        Rain density (0.1–1000; default 20)
  --mode NAME        matrix, binary, decimal, hexadecimal, dna
  --fg COLOR         Glyph color (#RGB or #RRGGBB; default Matrix green)
  --bg COLOR         Background color (#RGB or #RRGGBB; default black)
  --transparent-bg   Match terminal background; --bg is the fallback
  --clock           Embed the local time in rain strips
  --timefmt FORMAT   strftime format (default \" %l%M%p \")
  --no-fog          Disable depth fog
  --no-waves        Disable brightness waves
  --no-rotate       Disable camera movement
  --flip            Mirror glyphs
  --wireframe       Draw outlines
  --no-texture      Draw solid blocks
  --width N         Maximum render width (16–4096; default 960)
  --height N        Maximum render height (16–4096; default 540)
  --fps N           Frame rate limit (1–120; default 30)
  --seed N          Reproducible random seed
  --warmup N        Initial simulation ticks (0–10000; default 600)
  --frames N        Exit after displaying N frames
  --snapshot PATH   Save one PPM frame without using the terminal
  --force           Skip the terminal animation capability check
  -h, --help        Show this help

Original single-dash options and +fog/+waves/+rotate/+texture/+flip work too.
Controls: q / Esc / Ctrl-C quit; Space pauses; Backspace / r restarts rain.
Requires Linux, EGL/OpenGL, zlib, and a terminal with Kitty animation support.
";

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<Option<Options>, String> {
        Options::parse(args.iter().map(OsString::from))
    }
    #[test]
    fn rejects_invalid_or_unbounded_work() {
        for args in [
            ["--speed", "NaN"],
            ["--speed", "inf"],
            ["--density", "-1"],
            ["--fps", "0"],
            ["--width", "4097"],
            ["--frames", "0"],
            ["--mode", "bad"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
        assert!(parse(&["--width"]).is_err());
    }
    #[test]
    fn retains_original_switches() {
        let options = parse(&["-binary", "+fog", "-clock", "--wireframe"])
            .unwrap()
            .unwrap();
        assert!(matches!(options.mode, GlyphMode::Binary));
        assert!(!options.do_fog && options.do_clock && !options.do_texture);
    }
    #[test]
    fn parses_colors_and_terminal_background_fallback() {
        assert_eq!(Color::parse("#aB3").unwrap(), Color([170, 187, 51]));
        assert_eq!(Color::parse("01aBfe").unwrap(), Color([1, 171, 254]));
        let options = parse(&["--fg", "#f80", "--transparent-bg", "--bg", "123456"])
            .unwrap()
            .unwrap();
        assert_eq!(options.fg, Some(Color([255, 136, 0])));
        assert_eq!(options.bg, Color([18, 52, 86]));
        assert!(options.transparent_bg);
        assert_eq!(Options::default().fg, None);
        assert_eq!(Options::default().bg, Color([0, 0, 0]));
        for value in [
            "",
            "#",
            "#1234",
            "12345",
            "#12345678",
            "red",
            "##fff",
            "ééé",
            " 123",
            "gggggg",
        ] {
            assert!(Color::parse(value).is_err(), "{value:?}");
        }
        assert!(parse(&["--fg"]).is_err());
        assert!(parse(&["--bg", "bad-color"]).is_err());
        assert!(parse(&["--snapshot", "frame.ppm", "--transparent-bg"]).is_err());
    }
    #[test]
    fn sizing_preserves_aspect_and_limits_bandwidth() {
        let options = Options::default();
        assert_eq!(options.render_size(1920, 1080), (960, 540));
        assert_eq!(options.render_size(1200, 1200), (540, 540));
        assert_eq!(options.render_size(400, 200), (400, 200));
        assert_eq!(options.render_size(0, 0), (1, 1));
    }
}
