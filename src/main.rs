mod egl;
mod gl;
mod kana;
mod kitty;
mod matrix;
mod options;
mod terminal;

use options::Options;
use std::{
    error::Error,
    fs::File,
    io::{self, IsTerminal, Write},
    process::ExitCode,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use terminal::{Action, Size};

fn run() -> Result<(), Box<dyn Error>> {
    let Some(options) = Options::parse(std::env::args_os().skip(1))? else {
        print!("{}", options::HELP);
        return Ok(());
    };
    let mut size = if options.snapshot.is_none() {
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            return Err(
                "stdin and stdout must be a terminal; use --snapshot PATH for headless output"
                    .into(),
            );
        }
        if std::env::var_os("TMUX").is_some() && !options.force {
            return Err(
                "run directly in Ghostty or Kitty; tmux passthrough is not implemented".into(),
            );
        }
        Size::get()?
    } else {
        Size {
            cols: 1,
            rows: 1,
            width: options.width as u16,
            height: options.height as u16,
        }
    };
    let (width, height) = size.pixels();
    let (width, height) = options.render_size(width, height);
    // Declaration order keeps the GL context alive until Matrix releases its texture.
    let mut context = egl::Context::new(width, height)?;
    let mut matrix = matrix::Matrix::new(options.clone());
    matrix.init_gl();
    matrix.reshape(width, height);
    for _ in 0..options.warmup {
        matrix.tick();
    }
    let mut pixels = Vec::new();

    if let Some(path) = &options.snapshot {
        matrix.draw();
        context.read_rgb(&mut pixels)?;
        let mut file = io::BufWriter::new(File::create(path)?);
        write!(file, "P6\n{width} {height}\n255\n")?;
        file.write_all(&pixels)?;
        file.flush()?;
        eprintln!(
            "Saved {} ({width}×{height}; {})",
            path.display(),
            context.renderer()
        );
        return Ok(());
    }

    let number = image_number();
    let mut session = terminal::Session::enter(number)?;
    if !options.force {
        session.probe()?;
    }
    if options.transparent_bg
        && let Some(color) = session.background_color()?
    {
        matrix.set_background(color);
    }
    let mut animation = kitty::Animation::new(number);
    let frame_interval = Duration::from_secs_f64(1.0 / f64::from(options.fps));
    let tick_interval = Duration::from_millis(30);
    let mut next_tick = Instant::now() + tick_interval;
    let mut next_frame = Instant::now();
    let mut next_resize = Instant::now();
    let mut paused = false;
    let mut dirty = true;
    let mut frames = 0_u64;
    while !session.stopped() {
        let now = Instant::now();
        if now >= next_resize {
            if let Ok(new_size) = Size::get()
                && new_size != size
            {
                let (width, height) = new_size.pixels();
                let (width, height) = options.render_size(width, height);
                context.resize(width, height)?;
                matrix.reshape(width, height);
                animation.reset(&mut session.output)?;
                size = new_size;
                dirty = true;
            }
            next_resize = now + Duration::from_millis(200);
        }
        if paused {
            next_tick = now + tick_interval;
        } else {
            // Preserve the original 30 ms simulation step independently of terminal throughput.
            for _ in 0..8 {
                if now < next_tick {
                    break;
                }
                matrix.tick();
                next_tick += tick_interval;
                dirty = true;
            }
            if now >= next_tick {
                next_tick = now + tick_interval;
            }
        }
        if dirty && now >= next_frame {
            matrix.draw();
            context.read_rgb(&mut pixels)?;
            animation.present(
                &mut session.output,
                &pixels,
                (context.width, context.height),
                (size.cols, size.rows),
            )?;
            frames += 1;
            if options.frames.is_some_and(|limit| frames >= limit) {
                break;
            }
            dirty = false;
            next_frame = now + frame_interval;
        }
        let deadline = if paused {
            next_resize
        } else if dirty {
            next_tick.min(next_frame)
        } else {
            next_tick
        };
        let wait = deadline.saturating_duration_since(Instant::now());
        for action in session.events(wait)? {
            match action {
                Action::Quit => return Ok(()),
                Action::Pause => {
                    paused = !paused;
                    next_tick = Instant::now() + tick_interval;
                }
                Action::Restart => {
                    matrix.erase();
                    dirty = true;
                }
            }
        }
    }
    Ok(())
}

fn image_number() -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos ^ std::process::id().rotate_left(16)).max(1)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) if terminal::stopped() => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("matrix-kitty: {error}");
            ExitCode::FAILURE
        }
    }
}
