use crate::gl;
use std::{ffi::CStr, ptr};

const EGL_PLATFORM_SURFACELESS_MESA: EGLenum = 0x31DD;
const EGL_PLATFORM_DEVICE_EXT: EGLenum = 0x313F;
const EGL_PBUFFER_BIT: EGLint = 1;
const EGL_WIDTH: EGLint = 0x3057;
const EGL_HEIGHT: EGLint = 0x3056;

unsafe extern "C" {
    fn eglGetPlatformDisplay(
        platform: EGLenum,
        display: *mut c_void,
        attributes: *const isize,
    ) -> EGLDisplay;
    fn eglGetProcAddress(name: *const std::ffi::c_char) -> *const c_void;
    fn eglCreatePbufferSurface(
        display: EGLDisplay,
        config: EGLConfig,
        attributes: *const EGLint,
    ) -> EGLSurface;
}

pub struct Context {
    display: EGLDisplay,
    config: EGLConfig,
    context: EGLContext,
    surface: EGLSurface,
    pub width: u32,
    pub height: u32,
}

impl Context {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let mut displays = Vec::new();
        unsafe {
            let query = eglGetProcAddress(c"eglQueryDevicesEXT".as_ptr());
            if !query.is_null() {
                type QueryDevices =
                    unsafe extern "C" fn(EGLint, *mut *mut c_void, *mut EGLint) -> EGLBoolean;
                let query: QueryDevices = std::mem::transmute(query);
                let mut devices = [ptr::null_mut(); 16];
                let mut count = 0;
                if query(devices.len() as i32, devices.as_mut_ptr(), &mut count) != 0 {
                    for &device in devices.iter().take(count.max(0) as usize) {
                        displays.push(eglGetPlatformDisplay(
                            EGL_PLATFORM_DEVICE_EXT,
                            device,
                            ptr::null(),
                        ));
                    }
                }
            }
            displays.push(eglGetPlatformDisplay(
                EGL_PLATFORM_SURFACELESS_MESA,
                ptr::null_mut(),
                ptr::null(),
            ));
        }
        let mut last_error = "no headless EGL displays found".to_string();
        for display in displays {
            if display.is_null() {
                continue;
            }
            match Self::on_display(display, width, height) {
                Ok(context) => return Ok(context),
                Err(error) => last_error = error,
            }
        }
        Err(format!(
            "cannot create offscreen OpenGL: {last_error}; see README for Mesa software rendering"
        ))
    }

    fn on_display(display: EGLDisplay, width: u32, height: u32) -> Result<Self, String> {
        unsafe {
            if eglInitialize(display, ptr::null_mut(), ptr::null_mut()) == 0 {
                return Err(error("eglInitialize"));
            }
        }
        let mut result = Self {
            display,
            config: ptr::null_mut(),
            context: ptr::null_mut(),
            surface: ptr::null_mut(),
            width: 0,
            height: 0,
        };
        unsafe {
            if eglBindAPI(EGL_OPENGL_API) == 0 {
                return Err(error("eglBindAPI"));
            }
            let attributes = [
                EGL_SURFACE_TYPE,
                EGL_PBUFFER_BIT,
                EGL_RENDERABLE_TYPE,
                EGL_OPENGL_BIT,
                EGL_RED_SIZE,
                8,
                EGL_GREEN_SIZE,
                8,
                EGL_BLUE_SIZE,
                8,
                EGL_ALPHA_SIZE,
                8,
                EGL_NONE,
            ];
            let mut count = 0;
            if eglChooseConfig(
                display,
                attributes.as_ptr(),
                &mut result.config,
                1,
                &mut count,
            ) == 0
                || count == 0
            {
                return Err(error("eglChooseConfig"));
            }
            // No version attributes requests a compatibility context for the original GLMatrix renderer.
            result.context =
                eglCreateContext(display, result.config, ptr::null_mut(), [EGL_NONE].as_ptr());
            if result.context.is_null() {
                return Err(error("eglCreateContext"));
            }
        }
        result.resize(width, height)?;
        Ok(result)
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
            return Err("render dimensions must be 1–4096 pixels".into());
        }
        if (self.width, self.height) == (width, height) {
            return Ok(());
        }
        unsafe {
            let attributes = [EGL_WIDTH, width as i32, EGL_HEIGHT, height as i32, EGL_NONE];
            let surface = eglCreatePbufferSurface(self.display, self.config, attributes.as_ptr());
            if surface.is_null() {
                return Err(error("eglCreatePbufferSurface"));
            }
            if eglMakeCurrent(self.display, surface, surface, self.context) == 0 {
                let message = error("eglMakeCurrent");
                eglDestroySurface(self.display, surface);
                return Err(message);
            }
            if !self.surface.is_null() {
                eglDestroySurface(self.display, self.surface);
            }
            self.surface = surface;
            self.width = width;
            self.height = height;
        }
        Ok(())
    }

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

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            eglMakeCurrent(
                self.display,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            if !self.surface.is_null() {
                eglDestroySurface(self.display, self.surface);
            }
            if !self.context.is_null() {
                eglDestroyContext(self.display, self.context);
            }
            eglTerminate(self.display);
        }
    }
}

fn error(operation: &str) -> String {
    format!("{operation} failed (EGL 0x{:x})", unsafe { eglGetError() })
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
use std::os::raw::{c_int, c_uint, c_void};

pub type EGLBoolean = c_uint;
pub type EGLenum = c_uint;
pub type EGLint = c_int;
pub type EGLDisplay = *mut c_void;
pub type EGLConfig = *mut c_void;
pub type EGLSurface = *mut c_void;
pub type EGLContext = *mut c_void;

pub const EGL_NONE: EGLint = 0x3038;
pub const EGL_OPENGL_BIT: EGLint = 0x0008;
pub const EGL_SURFACE_TYPE: EGLint = 0x3033;
pub const EGL_RENDERABLE_TYPE: EGLint = 0x3040;
pub const EGL_RED_SIZE: EGLint = 0x3024;
pub const EGL_GREEN_SIZE: EGLint = 0x3023;
pub const EGL_BLUE_SIZE: EGLint = 0x3022;
pub const EGL_ALPHA_SIZE: EGLint = 0x3021;
pub const EGL_OPENGL_API: EGLenum = 0x30A2;

unsafe extern "C" {
    pub fn eglInitialize(dpy: EGLDisplay, major: *mut EGLint, minor: *mut EGLint) -> EGLBoolean;
    pub fn eglTerminate(dpy: EGLDisplay) -> EGLBoolean;
    pub fn eglBindAPI(api: EGLenum) -> EGLBoolean;
    pub fn eglChooseConfig(
        dpy: EGLDisplay,
        attrib_list: *const EGLint,
        configs: *mut EGLConfig,
        config_size: EGLint,
        num_config: *mut EGLint,
    ) -> EGLBoolean;
    pub fn eglCreateContext(
        dpy: EGLDisplay,
        config: EGLConfig,
        share_context: EGLContext,
        attrib_list: *const EGLint,
    ) -> EGLContext;
    pub fn eglDestroyContext(dpy: EGLDisplay, ctx: EGLContext) -> EGLBoolean;
    pub fn eglDestroySurface(dpy: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
    pub fn eglMakeCurrent(
        dpy: EGLDisplay,
        draw: EGLSurface,
        read: EGLSurface,
        ctx: EGLContext,
    ) -> EGLBoolean;
    pub fn eglGetError() -> EGLint;
}
