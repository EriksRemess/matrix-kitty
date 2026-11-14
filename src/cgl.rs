use crate::gl;
use std::{
    ffi::{CStr, c_char, c_int, c_uint, c_void},
    ptr,
};

const CGL_PFA_COLOR_SIZE: c_int = 8;
const CGL_PFA_ALPHA_SIZE: c_int = 11;
const CGL_PFA_ALLOW_OFFLINE_RENDERERS: c_int = 96;
const CGL_PFA_OPENGL_PROFILE: c_int = 99;
const CGL_PROFILE_VERSION_LEGACY: c_int = 0x1000;
const GL_FRAMEBUFFER: c_uint = 0x8D40;
const GL_RENDERBUFFER: c_uint = 0x8D41;
const GL_COLOR_ATTACHMENT0: c_uint = 0x8CE0;
const GL_FRAMEBUFFER_COMPLETE: c_uint = 0x8CD5;
const GL_RGBA8: c_uint = 0x8058;

type CGLContext = *mut c_void;
type CGLPixelFormat = *mut c_void;

unsafe extern "C" {
    fn CGLChoosePixelFormat(
        attributes: *const c_int,
        format: *mut CGLPixelFormat,
        count: *mut c_int,
    ) -> c_int;
    fn CGLDestroyPixelFormat(format: CGLPixelFormat) -> c_int;
    fn CGLCreateContext(
        format: CGLPixelFormat,
        share: CGLContext,
        context: *mut CGLContext,
    ) -> c_int;
    fn CGLDestroyContext(context: CGLContext) -> c_int;
    fn CGLSetCurrentContext(context: CGLContext) -> c_int;
    fn CGLErrorString(error: c_int) -> *const c_char;

    // The legacy profile exposes framebuffer objects through the EXT entry points.
    fn glGenFramebuffersEXT(count: c_int, framebuffers: *mut c_uint);
    fn glDeleteFramebuffersEXT(count: c_int, framebuffers: *const c_uint);
    fn glBindFramebufferEXT(target: c_uint, framebuffer: c_uint);
    fn glCheckFramebufferStatusEXT(target: c_uint) -> c_uint;
    fn glGenRenderbuffersEXT(count: c_int, renderbuffers: *mut c_uint);
    fn glDeleteRenderbuffersEXT(count: c_int, renderbuffers: *const c_uint);
    fn glBindRenderbufferEXT(target: c_uint, renderbuffer: c_uint);
    fn glRenderbufferStorageEXT(target: c_uint, format: c_uint, width: c_int, height: c_int);
    fn glFramebufferRenderbufferEXT(
        target: c_uint,
        attachment: c_uint,
        renderbuffer_target: c_uint,
        renderbuffer: c_uint,
    );
}

pub struct Context {
    context: CGLContext,
    framebuffer: c_uint,
    color: c_uint,
    pub width: u32,
    pub height: u32,
}

impl Context {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        // GLMatrix uses fixed-function OpenGL, which requires the legacy profile.
        // A framebuffer object supplies storage without a window or a display drawable.
        let attributes = [
            CGL_PFA_OPENGL_PROFILE,
            CGL_PROFILE_VERSION_LEGACY,
            CGL_PFA_COLOR_SIZE,
            24,
            CGL_PFA_ALPHA_SIZE,
            8,
            CGL_PFA_ALLOW_OFFLINE_RENDERERS,
            0,
        ];
        let mut format = ptr::null_mut();
        let mut count = 0;
        unsafe {
            check(
                CGLChoosePixelFormat(attributes.as_ptr(), &mut format, &mut count),
                "CGLChoosePixelFormat",
            )?;
            if format.is_null() {
                return Err("CGLChoosePixelFormat found no compatible OpenGL renderer".into());
            }
            let mut context = ptr::null_mut();
            let status = CGLCreateContext(format, ptr::null_mut(), &mut context);
            CGLDestroyPixelFormat(format);
            check(status, "CGLCreateContext")?;
            let mut result = Self {
                context,
                framebuffer: 0,
                color: 0,
                width: 0,
                height: 0,
            };
            check(CGLSetCurrentContext(context), "CGLSetCurrentContext")?;
            result.resize(width, height)?;
            Ok(result)
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
            return Err("render dimensions must be 1–4096 pixels".into());
        }
        if (self.width, self.height) == (width, height) {
            return Ok(());
        }
        unsafe {
            let mut framebuffer = 0;
            let mut color = 0;
            glGenFramebuffersEXT(1, &mut framebuffer);
            glGenRenderbuffersEXT(1, &mut color);
            glBindFramebufferEXT(GL_FRAMEBUFFER, framebuffer);
            glBindRenderbufferEXT(GL_RENDERBUFFER, color);
            glRenderbufferStorageEXT(GL_RENDERBUFFER, GL_RGBA8, width as c_int, height as c_int);
            glFramebufferRenderbufferEXT(
                GL_FRAMEBUFFER,
                GL_COLOR_ATTACHMENT0,
                GL_RENDERBUFFER,
                color,
            );
            let status = glCheckFramebufferStatusEXT(GL_FRAMEBUFFER);
            let error = gl::glGetError();
            glBindRenderbufferEXT(GL_RENDERBUFFER, 0);
            if status != GL_FRAMEBUFFER_COMPLETE || error != 0 {
                glBindFramebufferEXT(GL_FRAMEBUFFER, self.framebuffer);
                glDeleteFramebuffersEXT(1, &framebuffer);
                glDeleteRenderbuffersEXT(1, &color);
                return Err(format!(
                    "cannot create offscreen OpenGL framebuffer (status 0x{status:x}, error 0x{error:x})"
                ));
            }
            glDeleteFramebuffersEXT(1, &self.framebuffer);
            glDeleteRenderbuffersEXT(1, &self.color);
            self.framebuffer = framebuffer;
            self.color = color;
            self.width = width;
            self.height = height;
        }
        Ok(())
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            if CGLSetCurrentContext(self.context) == 0 {
                glDeleteFramebuffersEXT(1, &self.framebuffer);
                glDeleteRenderbuffersEXT(1, &self.color);
                CGLSetCurrentContext(ptr::null_mut());
            }
            CGLDestroyContext(self.context);
        }
    }
}

fn check(status: c_int, operation: &str) -> Result<(), String> {
    if status == 0 {
        return Ok(());
    }
    let message = unsafe {
        let message = CGLErrorString(status);
        if message.is_null() {
            "unknown error".into()
        } else {
            CStr::from_ptr(message).to_string_lossy()
        }
    };
    Err(format!("{operation} failed: {message} (CGL {status})"))
}
