use crate::{kitty, options::Color};
use std::{
    io::{self, BufWriter, IsTerminal, Write},
    time::{Duration, Instant},
};

unsafe extern "C" {
    fn gm_terminal_open() -> i32;
    fn gm_terminal_restore();
    fn gm_terminal_close();
    fn gm_terminal_stopped() -> i32;
    fn gm_terminal_stop();
    fn gm_terminal_write(bytes: *const u8, len: usize) -> isize;
    fn gm_terminal_wait_output(timeout_ms: i32, watch_input: i32) -> i32;
    fn gm_terminal_size(cols: *mut u16, rows: *mut u16, width: *mut u16, height: *mut u16) -> i32;
    fn gm_terminal_read(bytes: *mut u8, len: usize, timeout_ms: i32) -> i32;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
    pub width: u16,
    pub height: u16,
}

impl Size {
    pub fn get() -> io::Result<Self> {
        let mut size = Self {
            cols: 0,
            rows: 0,
            width: 0,
            height: 0,
        };
        if unsafe {
            gm_terminal_size(
                &mut size.cols,
                &mut size.rows,
                &mut size.width,
                &mut size.height,
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        if size.cols == 0 || size.rows == 0 {
            return Err(io::Error::other("terminal has no usable size"));
        }
        Ok(size)
    }

    pub fn pixels(self) -> (u32, u32) {
        if self.width != 0 && self.height != 0 {
            (self.width as u32, self.height as u32)
        } else {
            (u32::from(self.cols) * 8, u32::from(self.rows) * 16)
        }
    }
}

pub struct Session {
    pub output: BufWriter<Output>,
    number: u32,
}

impl Session {
    pub fn enter(number: u32) -> io::Result<Self> {
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            return Err(io::Error::other(
                "stdin and stdout must be a terminal; use --snapshot PATH for headless output",
            ));
        }
        if unsafe { gm_terminal_open() } < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut session = Self {
            output: BufWriter::with_capacity(64 * 1024, Output::default()),
            number,
        };
        session
            .output
            .write_all(b"\x1b[?1049h\x1b[2J\x1b[H\x1b[?25l")?;
        session.output.flush()?;
        Ok(session)
    }

    pub fn stopped(&self) -> bool {
        stopped()
    }

    pub fn probe(&mut self) -> io::Result<()> {
        let number = self.number;
        // Probe an actual frame edit: basic Kitty support alone is insufficient.
        for command in [
            format!("\x1b_Ga=t,I={number},f=24,s=1,v=1;AAAA\x1b\\"),
            format!("\x1b_Ga=f,I={number},r=1,f=24,s=1,v=1,X=1;AAAA\x1b\\"),
        ] {
            self.query(command.as_bytes(), |bytes| graphics_response(bytes, number))?
                .ok_or_else(|| io::Error::other(
                    "no Kitty animation response; use Ghostty 1.4+ or Kitty directly (see --force)",
                ))?
                .map_err(io::Error::other)?;
        }
        kitty::delete(&mut self.output, number)?;
        self.output.flush()
    }

    pub fn background_color(&mut self) -> io::Result<Option<Color>> {
        self.query(b"\x1b]11;?\x1b\\", background_response)
    }

    fn query<T>(
        &mut self,
        command: &[u8],
        parse: impl Fn(&[u8]) -> Option<T>,
    ) -> io::Result<Option<T>> {
        self.output.get_mut().query_input = Some(Vec::new());
        let result = (|| {
            self.output.write_all(command)?;
            self.output.flush()?;
            let deadline = Instant::now() + Duration::from_millis(800);
            while Instant::now() < deadline && !self.stopped() {
                let output = self.output.get_mut();
                let response = output.query_input.as_ref().expect("query is active");
                if let Some(result) = parse(response) {
                    return Ok(Some(result));
                }
                if response.len() >= 8192 {
                    break;
                }
                output.poll_input(Duration::from_millis(20))?;
            }
            Ok(None)
        })();
        self.output.get_mut().query_input = None;
        result
    }

    pub fn events(&mut self, timeout: Duration) -> io::Result<Vec<Action>> {
        let output = self.output.get_mut();
        if output.pending.is_empty() {
            output.poll_input(timeout.min(Duration::from_millis(20)))?;
        }
        Ok(output.pending.take())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Discard buffered frame data, including BufWriter's implicit drop-time flush.
        self.output.get_mut().disabled = true;
        unsafe {
            gm_terminal_restore();
        }
        let mut cleanup = b"\x18\x1b\\".to_vec();
        let _ = kitty::delete(&mut cleanup, self.number);
        cleanup.extend_from_slice(b"\x1b[?25h\x1b[?1049l");
        cleanup_output(&cleanup);
        unsafe {
            gm_terminal_close();
        }
    }
}

pub fn stopped() -> bool {
    unsafe { gm_terminal_stopped() != 0 }
}

#[derive(Default)]
pub struct Output {
    disabled: bool,
    input: Input,
    pending: PendingActions,
    query_input: Option<Vec<u8>>,
}

impl Output {
    fn poll_input(&mut self, timeout: Duration) -> io::Result<()> {
        let bytes = read(timeout)?;
        self.accept_input(&bytes, Instant::now());
        if self.pending.quit {
            unsafe {
                gm_terminal_stop();
            }
        }
        Ok(())
    }

    fn accept_input(&mut self, bytes: &[u8], now: Instant) {
        // Retain query replies even when input is read during a blocked write.
        if let Some(response) = &mut self.query_input {
            response.extend(bytes.iter().take(8192 - response.len()));
        }
        for action in self.input.feed(bytes, now) {
            self.pending.push(action);
        }
    }
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        loop {
            if self.disabled || stopped() {
                // Write::write_all retries Interrupted, so cancellation needs another kind.
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "terminal output cancelled",
                ));
            }
            match write_output(bytes) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    wait_output(20, true)?;
                    self.poll_input(Duration::ZERO)?;
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
                result => return result,
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn write_output(bytes: &[u8]) -> io::Result<usize> {
    let count = unsafe { gm_terminal_write(bytes.as_ptr(), bytes.len()) };
    if count < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(count as usize)
    }
}

fn wait_output(timeout_ms: i32, watch_input: bool) -> io::Result<()> {
    if unsafe { gm_terminal_wait_output(timeout_ms, i32::from(watch_input)) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn cleanup_output(mut bytes: &[u8]) {
    let deadline = Instant::now() + Duration::from_millis(100);
    while !bytes.is_empty() && Instant::now() < deadline {
        match write_output(bytes) {
            Ok(0) => break,
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if wait_output(remaining.as_millis().min(20) as i32, false).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

fn read(timeout: Duration) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0; 1024];
    let count = unsafe {
        gm_terminal_read(
            bytes.as_mut_ptr(),
            bytes.len(),
            timeout.as_millis().clamp(1, 20) as i32,
        )
    };
    if count < 0 {
        return Err(io::Error::last_os_error());
    }
    bytes.truncate(count as usize);
    Ok(bytes)
}

fn graphics_response(bytes: &[u8], number: u32) -> Option<Result<(), String>> {
    let text = String::from_utf8_lossy(bytes);
    for response in text.split("\x1b_G").skip(1) {
        let Some((response, _)) = response.split_once("\x1b\\") else {
            continue;
        };
        let Some((header, status)) = response.split_once(';') else {
            continue;
        };
        if !header.split(',').any(|key| key == format!("I={number}")) {
            continue;
        }
        return Some(if status == "OK" {
            Ok(())
        } else {
            Err(format!(
                "terminal rejected Kitty animation: {}",
                status.escape_default()
            ))
        });
    }
    None
}

fn background_response(bytes: &[u8]) -> Option<Color> {
    // OSC replies may use either BEL or ST, with one to four hex digits per channel.
    let text = String::from_utf8_lossy(bytes);
    for response in text.split("\x1b]").skip(1) {
        let end = match (response.find('\x07'), response.find("\x1b\\")) {
            (Some(a), Some(b)) => a.min(b),
            (Some(end), None) | (None, Some(end)) => end,
            (None, None) => continue,
        };
        let Some(rgb) = response[..end].strip_prefix("11;rgb:") else {
            continue;
        };
        let channels: Vec<_> = rgb.split('/').collect();
        if channels.len() != 3
            || channels.iter().any(|part| {
                !(1..=4).contains(&part.len()) || !part.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        {
            continue;
        }
        return Some(Color(std::array::from_fn(|i| {
            let value = u32::from_str_radix(channels[i], 16).unwrap();
            let max = (1 << (4 * channels[i].len())) - 1;
            ((value * 255 + max / 2) / max) as u8
        })));
    }
    None
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Quit,
    Pause,
    Restart,
}

#[derive(Default)]
struct PendingActions {
    quit: bool,
    pause: bool,
    restart: bool,
}

impl PendingActions {
    fn push(&mut self, action: Action) {
        // Keep bounded state during a long stall; pause toggles cancel in pairs.
        match action {
            Action::Quit => self.quit = true,
            Action::Pause => self.pause = !self.pause,
            Action::Restart => self.restart = true,
        }
    }

    fn is_empty(&self) -> bool {
        !self.quit && !self.pause && !self.restart
    }

    fn take(&mut self) -> Vec<Action> {
        let pending = std::mem::take(self);
        if pending.quit {
            return vec![Action::Quit];
        }
        let mut actions = Vec::new();
        if pending.pause {
            actions.push(Action::Pause);
        }
        if pending.restart {
            actions.push(Action::Restart);
        }
        actions
    }
}

#[derive(Default)]
struct Input {
    escape: Option<Instant>,
    sequence: bool,
    string: bool,
    string_escape: bool,
    last_input: Option<Instant>,
}

impl Input {
    fn feed(&mut self, bytes: &[u8], now: Instant) -> Vec<Action> {
        if (self.string || self.sequence)
            && self
                .last_input
                .is_some_and(|last| now.duration_since(last) >= Duration::from_millis(250))
        {
            *self = Self::default();
        }
        let mut result = Vec::new();
        for &byte in bytes {
            self.last_input = Some(now);
            if matches!(byte, 3 | 4 | 26) {
                *self = Self::default();
                return vec![Action::Quit];
            }
            if byte == 24 {
                *self = Self::default();
                continue;
            }
            if byte == 27 {
                self.escape = Some(now);
                self.string_escape = self.string;
                self.sequence = false;
                continue;
            }
            if self.string {
                if (self.string_escape && byte == b'\\') || byte == 7 {
                    self.string = false;
                }
                self.string_escape = false;
                self.escape = None;
                continue;
            }
            if self.sequence {
                if (0x40..=0x7e).contains(&byte) {
                    self.sequence = false;
                }
                continue;
            }
            if self.escape.take().is_some() {
                match byte {
                    b'[' | b'O' => self.sequence = true,
                    b'_' | b'P' | b']' => self.string = true,
                    _ => result.push(Action::Quit),
                }
                continue;
            }
            match byte {
                b'q' | b'Q' => result.push(Action::Quit),
                b' ' => result.push(Action::Pause),
                b'r' | b'R' | 8 | 127 => result.push(Action::Restart),
                _ => (),
            }
        }
        if self
            .escape
            .is_some_and(|start| now.duration_since(start) >= Duration::from_millis(40))
        {
            self.escape = None;
            result.push(Action::Quit);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_query_accepts_complete_rgb_replies_with_both_terminators() {
        for response in [
            b"\x1b]11;rgb:1/8/f\x07".as_slice(),
            b"\x1b]11;rgb:11/88/ff\x1b\\",
            b"\x1b]11;rgb:111/888/fff\x07",
            b"\x1b]11;rgb:1111/8888/FFFF\x1b\\",
        ] {
            assert_eq!(background_response(response), Some(Color([17, 136, 255])));
            for end in 0..response.len() {
                assert_eq!(background_response(&response[..end]), None);
            }
        }
        assert_eq!(
            background_response(b"\x1b]11;rgb:8000/0000/ffff\x07"),
            Some(Color([128, 0, 255]))
        );
        for value in [
            "\x1b]10;rgb:ff/ff/ff\x07",
            "\x1b]11;rgb:gg/00/00\x07",
            "\x1b]11;rgb:00000/0/0\x07",
            "\x1b]11;rgb:0/0\x07",
            "\x1b]11;rgb:0/0/0/0\x07",
            "\x1b]11;rgb:/0/0\x07",
            "\x1b]11;rgb:+f/0/0\x07",
            "\x1b]11;rgb:é/0/0\x07",
        ] {
            assert_eq!(background_response(value.as_bytes()), None, "{value:?}");
        }
        assert_eq!(
            background_response(b"\x1b]10;rgb:0/0/0\x07\x1b]11;bad\x07\x1b]11;rgb:ab/cd/ef\x07"),
            Some(Color([171, 205, 239]))
        );
    }

    #[test]
    fn background_query_keeps_fragmented_replies_and_keyboard_controls() {
        let mut output = Output {
            query_input: Some(Vec::new()),
            ..Output::default()
        };
        let now = Instant::now();
        output.accept_input(b"\x1b]11;rgb:12", now);
        output.accept_input(b"/34/56\x1b", now);
        output.accept_input(b"\\ r", now);
        assert_eq!(
            background_response(output.query_input.as_ref().unwrap()),
            Some(Color([18, 52, 86]))
        );
        assert_eq!(output.pending.take(), vec![Action::Pause, Action::Restart]);
    }

    #[test]
    fn blocked_input_preserves_parser_state_and_pending_controls() {
        let mut output = Output::default();
        let now = Instant::now();
        output.accept_input(b"\x1b_GI=9;", now);
        output.accept_input(b"error q\x1b\\", now);
        assert!(output.pending.is_empty());
        output.accept_input(b" r", now);
        assert_eq!(output.pending.take(), vec![Action::Pause, Action::Restart]);
        output.accept_input(b"\x1b", now);
        output.accept_input(b"", now + Duration::from_millis(50));
        assert_eq!(output.pending.take(), vec![Action::Quit]);
    }

    #[test]
    fn pending_controls_coalesce_without_losing_pause_parity_or_quit() {
        let mut pending = PendingActions::default();
        for _ in 0..10_000 {
            pending.push(Action::Pause);
            pending.push(Action::Restart);
        }
        assert_eq!(pending.take(), vec![Action::Restart]);
        assert!(pending.is_empty());
        pending.push(Action::Quit);
        pending.push(Action::Pause);
        pending.push(Action::Restart);
        assert_eq!(pending.take(), vec![Action::Quit]);
    }

    #[test]
    fn probe_replies_read_during_writes_are_retained_and_bounded() {
        let mut output = Output {
            query_input: Some(Vec::new()),
            ..Output::default()
        };
        let now = Instant::now();
        output.accept_input(b"\x1b_Gi=100,I=9;", now);
        output.accept_input(b"OK\x1b\\ ", now);
        assert_eq!(
            graphics_response(output.query_input.as_ref().unwrap(), 9),
            Some(Ok(()))
        );
        assert_eq!(output.pending.take(), vec![Action::Pause]);
        output.accept_input(&[b'x'; 10_000], now);
        assert_eq!(output.query_input.as_ref().unwrap().len(), 8192);
    }

    #[test]
    fn probe_only_accepts_complete_matching_response() {
        assert_eq!(
            graphics_response(b"\x1b_Gi=3,I=9;OK\x1b\\", 9),
            Some(Ok(()))
        );
        assert_eq!(graphics_response(b"\x1b_Gi=3,I=8;OK\x1b\\", 9), None);
        assert_eq!(graphics_response(b"\x1b_Gi=3,I=9;OK\x1b", 9), None);
        assert!(
            graphics_response(b"\x1b_Gi=3,I=9;EINVAL:no animation\x1b\\", 9)
                .unwrap()
                .is_err()
        );
    }
    #[test]
    fn ignores_fragmented_terminal_sequences_and_handles_escape_key() {
        let mut input = Input::default();
        let now = Instant::now();
        assert!(input.feed(b"\x1b[", now).is_empty());
        assert!(input.feed(b"1;5A\x1b_GI=9;", now).is_empty());
        assert!(input.feed(b"error q\x1b\\", now).is_empty());
        assert_eq!(input.feed(b" r", now), vec![Action::Pause, Action::Restart]);
        assert!(input.feed(b"\x1b", now).is_empty());
        assert_eq!(
            input.feed(b"", now + Duration::from_millis(50)),
            vec![Action::Quit]
        );
    }

    #[test]
    fn control_keys_quit_from_every_escape_state() {
        let now = Instant::now();
        for prefix in [
            b"\x1b".as_slice(),
            b"\x1b[1;",
            b"\x1bO",
            b"\x1b]title",
            b"\x1b_GI=9;",
            b"\x1bPdata",
            b"\x1b]title\x1b",
        ] {
            for key in [3, 4, 26] {
                let mut input = Input::default();
                assert!(input.feed(prefix, now).is_empty());
                assert_eq!(
                    input.feed(&[key], now),
                    vec![Action::Quit],
                    "{prefix:?}, {key}"
                );
            }
        }
    }

    #[test]
    fn escape_quits_from_incomplete_strings_and_csi() {
        let now = Instant::now();
        for prefix in [b"\x1b[1;".as_slice(), b"\x1b]title", b"\x1b_GI=9;"] {
            let mut input = Input::default();
            assert!(input.feed(prefix, now).is_empty());
            assert!(input.feed(b"\x1b", now).is_empty());
            assert_eq!(
                input.feed(b"", now + Duration::from_millis(50)),
                vec![Action::Quit]
            );
        }
    }

    #[test]
    fn stale_escape_sequences_expire_and_can_cancels_them() {
        let now = Instant::now();
        for prefix in [
            b"\x1b[1;".as_slice(),
            b"\x1b]title",
            b"\x1b_GI=9;",
            b"\x18\x1b]title",
        ] {
            let mut input = Input::default();
            assert!(input.feed(prefix, now).is_empty());
            assert_eq!(
                input.feed(b"q", now + Duration::from_millis(300)),
                vec![Action::Quit]
            );
        }
        let mut input = Input::default();
        assert!(input.feed(b"\x1b]title", now).is_empty());
        assert_eq!(input.feed(b"\x18q", now), vec![Action::Quit]);
    }
}
