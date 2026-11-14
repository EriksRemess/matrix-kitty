use crate::gl;
use std::ffi::CStr;

#[cfg_attr(target_os = "macos", path = "cgl.rs")]
#[cfg_attr(not(target_os = "macos"), path = "egl.rs")]
mod platform;
pub use platform::Context;

impl Context {
    pub fn renderer(&self) -> String {
        unsafe {
            let name = gl::glGetString(gl::GL_RENDERER);
            if name.is_null() {
                "unknown".into()
            } else {
                CStr::from_ptr(name.cast()).to_string_lossy().into_owned()
            }
        }
    }

    pub fn read_rgb(&self, pixels: &mut Vec<u8>) -> Result<(), String> {
        let stride = self.width as usize * 3;
        pixels.resize(stride * self.height as usize, 0);
        unsafe {
            gl::glPixelStorei(gl::GL_PACK_ALIGNMENT, 1);
            gl::glReadPixels(
                0,
                0,
                self.width as i32,
                self.height as i32,
                gl::GL_RGB,
                gl::GL_UNSIGNED_BYTE,
                pixels.as_mut_ptr().cast(),
            );
            let error = gl::glGetError();
            if error != 0 {
                return Err(format!("OpenGL rendering/readback failed: 0x{error:x}"));
            }
        }
        flip_rows(pixels, stride);
        Ok(())
    }
}

fn flip_rows(pixels: &mut [u8], stride: usize) {
    let height = pixels.len() / stride;
    let (top, bottom) = pixels.split_at_mut(height.div_ceil(2) * stride);
    for (a, b) in top
        .chunks_exact_mut(stride)
        .zip(bottom.rchunks_exact_mut(stride))
    {
        a.swap_with_slice(b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readback_orientation_handles_odd_and_even_heights() {
        for (mut pixels, expected) in [
            (vec![1, 2, 3, 4, 5, 6], vec![5, 6, 3, 4, 1, 2]),
            (vec![1, 2, 3, 4], vec![3, 4, 1, 2]),
            (vec![1, 2], vec![1, 2]),
        ] {
            flip_rows(&mut pixels, 2);
            assert_eq!(pixels, expected);
        }
    }
}
