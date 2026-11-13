// Adapted from glmatrix-rs 0.1.5 (bfc66ad), based on GLMatrix by Jamie Zawinski.
// See LICENSE and NOTICE for attribution and permissions.
use std::os::raw::{c_int, c_uint, c_void};

pub type GLenum = c_uint;
pub type GLbitfield = c_uint;
pub type GLint = c_int;
pub type GLsizei = c_int;
pub type GLuint = c_uint;
pub type GLfloat = f32;
pub type GLdouble = f64;

pub const GL_COLOR_BUFFER_BIT: GLbitfield = 0x0000_4000;
pub const GL_DEPTH_BUFFER_BIT: GLbitfield = 0x0000_0100;
pub const GL_LINES: GLenum = 0x0001;
pub const GL_LINE_LOOP: GLenum = 0x0002;
pub const GL_QUADS: GLenum = 0x0007;
pub const GL_TEXTURE_2D: GLenum = 0x0DE1;
pub const GL_BLEND: GLenum = 0x0BE2;
pub const GL_SRC_ALPHA: GLenum = 0x0302;
pub const GL_ONE_MINUS_SRC_ALPHA: GLenum = 0x0303;
pub const GL_ONE: GLenum = 1;
pub const GL_CULL_FACE: GLenum = 0x0B44;
pub const GL_DEPTH_TEST: GLenum = 0x0B71;
pub const GL_NORMALIZE: GLenum = 0x0BA1;
pub const GL_SMOOTH: GLenum = 0x1D01;
pub const GL_PROJECTION: GLenum = 0x1701;
pub const GL_MODELVIEW: GLenum = 0x1700;
pub const GL_RGBA: GLenum = 0x1908;
pub const GL_UNSIGNED_BYTE: GLenum = 0x1401;
pub const GL_UNPACK_ALIGNMENT: GLenum = 0x0CF5;
pub const GL_TEXTURE_MAG_FILTER: GLenum = 0x2800;
pub const GL_TEXTURE_MIN_FILTER: GLenum = 0x2801;
pub const GL_TEXTURE_WRAP_S: GLenum = 0x2802;
pub const GL_TEXTURE_WRAP_T: GLenum = 0x2803;
pub const GL_LINEAR: GLenum = 0x2601;
pub const GL_REPEAT: GLenum = 0x2901;
pub const GL_TEXTURE_ENV: GLenum = 0x2300;
pub const GL_TEXTURE_ENV_MODE: GLenum = 0x2200;
pub const GL_MODULATE: GLenum = 0x2100;

unsafe extern "C" {
    pub fn glBegin(mode: GLenum);
    pub fn glBindTexture(target: GLenum, texture: GLuint);
    pub fn glBlendFunc(sfactor: GLenum, dfactor: GLenum);
    pub fn glClear(mask: GLbitfield);
    pub fn glClearColor(red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat);
    pub fn glColor4f(red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat);
    pub fn glDeleteTextures(n: GLsizei, textures: *const GLuint);
    pub fn glDisable(cap: GLenum);
    pub fn glEnable(cap: GLenum);
    pub fn glEnd();
    pub fn glFrustum(
        left: GLdouble,
        right: GLdouble,
        bottom: GLdouble,
        top: GLdouble,
        near_val: GLdouble,
        far_val: GLdouble,
    );
    pub fn glGenTextures(n: GLsizei, textures: *mut GLuint);
    pub fn glLoadIdentity();
    pub fn glMatrixMode(mode: GLenum);
    pub fn glNormal3f(nx: GLfloat, ny: GLfloat, nz: GLfloat);
    pub fn glPixelStorei(pname: GLenum, param: GLint);
    pub fn glPopMatrix();
    pub fn glPushMatrix();
    pub fn glRotatef(angle: GLfloat, x: GLfloat, y: GLfloat, z: GLfloat);
    pub fn glShadeModel(mode: GLenum);
    pub fn glTexCoord2f(s: GLfloat, t: GLfloat);
    pub fn glTexEnvi(target: GLenum, pname: GLenum, param: GLint);
    pub fn glTexImage2D(
        target: GLenum,
        level: GLint,
        internalformat: GLint,
        width: GLsizei,
        height: GLsizei,
        border: GLint,
        format: GLenum,
        type_: GLenum,
        pixels: *const c_void,
    );
    pub fn glTexParameteri(target: GLenum, pname: GLenum, param: GLint);
    pub fn glTranslatef(x: GLfloat, y: GLfloat, z: GLfloat);
    pub fn glVertex3f(x: GLfloat, y: GLfloat, z: GLfloat);
    pub fn glViewport(x: GLint, y: GLint, width: GLsizei, height: GLsizei);
}

pub const GL_RGB: GLenum = 0x1907;
pub const GL_PACK_ALIGNMENT: GLenum = 0x0D05;
pub const GL_RENDERER: GLenum = 0x1F01;
unsafe extern "C" {
    pub fn glReadPixels(
        x: GLint,
        y: GLint,
        width: GLsizei,
        height: GLsizei,
        format: GLenum,
        kind: GLenum,
        pixels: *mut c_void,
    );
    pub fn glGetError() -> GLenum;
    pub fn glGetString(name: GLenum) -> *const u8;
}
