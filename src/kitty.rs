use std::{
    io::{self, Write},
    os::raw::{c_int, c_ulong},
};

unsafe extern "C" {
    fn compressBound(source_len: c_ulong) -> c_ulong;
    fn compress2(
        dest: *mut u8,
        dest_len: *mut c_ulong,
        source: *const u8,
        source_len: c_ulong,
        level: c_int,
    ) -> c_int;
}

pub struct Animation {
    number: u32,
    current: u8,
    second_created: bool,
    compressed: Vec<u8>,
}

impl Animation {
    pub fn new(number: u32) -> Self {
        Self {
            number,
            current: 0,
            second_created: false,
            compressed: Vec::new(),
        }
    }

    pub fn reset(&mut self, output: &mut impl Write) -> io::Result<()> {
        delete(output, self.number)?;
        self.current = 0;
        self.second_created = false;
        Ok(())
    }

    pub fn present(
        &mut self,
        output: &mut impl Write,
        pixels: &[u8],
        (width, height): (u32, u32),
        (cols, rows): (u16, u16),
    ) -> io::Result<()> {
        if width == 0
            || height == 0
            || width > 4096
            || height > 4096
            || cols == 0
            || rows == 0
            || pixels.len() as u64 != u64::from(width) * u64::from(height) * 3
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid RGB canvas or placement size",
            ));
        }
        let bound = unsafe { compressBound(pixels.len() as c_ulong) };
        self.compressed.resize(bound as usize, 0);
        let mut length = bound;
        let result = unsafe {
            compress2(
                self.compressed.as_mut_ptr(),
                &mut length,
                pixels.as_ptr(),
                pixels.len() as c_ulong,
                1,
            )
        };
        if result != 0 {
            return Err(io::Error::other(format!(
                "zlib compression failed: {result}"
            )));
        }
        self.compressed.truncate(length as usize);
        let number = self.number;
        if self.current == 0 {
            write!(output, "\x1b[H")?;
            transmit(
                output,
                &format!("a=T,I={number},f=24,o=z,s={width},v={height},c={cols},r={rows},C=1,q=2"),
                "q=2",
                &self.compressed,
            )?;
            self.current = 1;
        } else {
            let next = if self.current == 1 { 2 } else { 1 };
            let target = if next == 2 && !self.second_created {
                0
            } else {
                next
            };
            transmit(
                output,
                &format!("a=f,I={number},f=24,o=z,s={width},v={height},r={target},X=1,q=2"),
                &format!("a=f,I={number},q=2"),
                &self.compressed,
            )?;
            write!(output, "\x1b_Ga=a,I={number},s=1,c={next},q=2\x1b\\")?;
            self.current = next;
            self.second_created = true;
        }
        output.flush()
    }
}

pub fn delete(output: &mut impl Write, number: u32) -> io::Result<()> {
    write!(output, "\x1b_Ga=d,d=N,I={number},q=2\x1b\\")
}

fn transmit(
    output: &mut impl Write,
    first: &str,
    continuation: &str,
    data: &[u8],
) -> io::Result<()> {
    // 3072 input bytes encode to the protocol's maximum 4096-byte payload.
    let mut encoded = [0; 4096];
    for (index, chunk) in data.chunks(3072).enumerate() {
        let size = base64(chunk, &mut encoded);
        let more = u8::from((index + 1) * 3072 < data.len());
        let header = if index == 0 { first } else { continuation };
        write!(output, "\x1b_G{header},m={more};")?;
        output.write_all(&encoded[..size])?;
        output.write_all(b"\x1b\\")?;
    }
    Ok(())
}

fn base64(input: &[u8], output: &mut [u8]) -> usize {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut position = 0;
    for chunk in input.chunks(3) {
        let a = chunk[0] as usize;
        let b = chunk.get(1).copied().unwrap_or(0) as usize;
        let c = chunk.get(2).copied().unwrap_or(0) as usize;
        output[position] = TABLE[a >> 2];
        output[position + 1] = TABLE[((a & 3) << 4) | (b >> 4)];
        output[position + 2] = if chunk.len() > 1 {
            TABLE[((b & 15) << 2) | (c >> 6)]
        } else {
            b'='
        };
        output[position + 3] = if chunk.len() > 2 { TABLE[c & 63] } else { b'=' };
        position += 4;
    }
    position
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn base64_matches_rfc4648_vectors() {
        for (plain, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            let mut output = [0; 16];
            let length = base64(plain.as_bytes(), &mut output);
            assert_eq!(&output[..length], expected.as_bytes());
        }
    }
    #[test]
    fn animation_chunks_retain_action_and_obey_payload_limit() {
        let mut output = Vec::new();
        transmit(&mut output, "a=f,I=9,r=2,q=2", "a=f,I=9,q=2", &[42; 7000]).unwrap();
        let text = String::from_utf8(output).unwrap();
        let chunks: Vec<_> = text.split("\x1b\\").filter(|s| !s.is_empty()).collect();
        assert_eq!(chunks.len(), 3);
        for (index, chunk) in chunks.iter().enumerate() {
            let (header, payload) = chunk.split_once(';').unwrap();
            assert!(header.starts_with("\x1b_Ga=f,I=9,"));
            assert!(payload.len() <= 4096 && payload.len() % 4 == 0);
            assert!(header.ends_with(if index == 2 { "m=0" } else { "m=1" }));
        }
    }
    #[test]
    fn unbounded_animation_reuses_two_frames_and_resets_on_resize() {
        let mut animation = Animation::new(9);
        let mut output = Vec::new();
        for _ in 0..12 {
            animation
                .present(&mut output, &[0; 12], (2, 2), (10, 5))
                .unwrap();
        }
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text.matches("a=T,").count(), 1);
        assert_eq!(text.matches("r=0,X=1").count(), 1);
        assert_eq!(text.matches("r=1,X=1").count(), 5);
        assert_eq!(text.matches("r=2,X=1").count(), 5);
        assert_eq!(text.matches("a=a,I=9,s=1,c=").count(), 11);
        let mut output = Vec::new();
        animation.reset(&mut output).unwrap();
        animation
            .present(&mut output, &[0; 18], (3, 2), (12, 6))
            .unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.starts_with("\x1b_Ga=d,d=N,I=9,q=2\x1b\\"));
        assert!(text.contains("a=T,I=9,f=24,o=z,s=3,v=2,c=12,r=6,C=1"));
    }
    #[test]
    fn invalid_canvas_does_not_write_partial_commands() {
        let mut output = Vec::new();
        assert!(
            Animation::new(9)
                .present(&mut output, &[0; 3], (2, 2), (10, 5))
                .is_err()
        );
        assert!(output.is_empty());
    }
}
