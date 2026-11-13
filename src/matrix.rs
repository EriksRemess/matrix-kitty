// Adapted from glmatrix-rs 0.1.5 (bfc66ad), based on GLMatrix by Jamie Zawinski.
// See LICENSE and NOTICE for attribution and permissions.
use crate::{
    gl, kana,
    options::{Color, Options},
};
use std::os::raw::{c_char, c_int, c_long};
use std::{
    ffi::CString,
    mem, process,
    time::{SystemTime, UNIX_EPOCH},
};

const CHAR_COLS: usize = 16;
const CHAR_ROWS: usize = 19;
const REAL_CHAR_ROWS: usize = CHAR_ROWS - 2;
const GRID_SIZE: usize = 70;
const GRID_DEPTH: f32 = 35.0;
const WAVE_SIZE: usize = 22;
const SPLASH_RATIO: f32 = 0.7;
const ORIGINAL_MATRIX_ENCODING: [i32; 26] = [
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170,
    171, 172, 173, 174, 175,
];
const KANA_GLYPH_START: usize = 176;
const MATRIX_ENCODING: [i32; 118] = matrix_encoding();

const fn matrix_encoding() -> [i32; 118] {
    let mut encoding = [0; 118];
    let mut i = 0;
    while i < ORIGINAL_MATRIX_ENCODING.len() {
        encoding[i] = ORIGINAL_MATRIX_ENCODING[i];
        i += 1;
    }
    let mut j = 0;
    while j < kana::PATTERNS.len() {
        encoding[i + j] = (KANA_GLYPH_START + j) as i32;
        j += 1;
    }
    encoding
}
const DECIMAL_ENCODING: [i32; 10] = [16, 17, 18, 19, 20, 21, 22, 23, 24, 25];
const HEX_ENCODING: [i32; 16] = [
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 33, 34, 35, 36, 37, 38,
];
const BINARY_ENCODING: [i32; 2] = [16, 17];
const DNA_ENCODING: [i32; 4] = [33, 35, 39, 52];

const NICE_VIEWS: [View; 16] = [
    View { x: 0.0, y: 0.0 },
    View { x: 0.0, y: -20.0 },
    View { x: 0.0, y: 20.0 },
    View { x: 25.0, y: 0.0 },
    View { x: -25.0, y: 0.0 },
    View { x: 25.0, y: 20.0 },
    View { x: -25.0, y: 20.0 },
    View { x: 25.0, y: -20.0 },
    View { x: -25.0, y: -20.0 },
    View { x: 10.0, y: 0.0 },
    View { x: -10.0, y: 0.0 },
    View { x: 0.0, y: 0.0 },
    View { x: 0.0, y: 0.0 },
    View { x: 0.0, y: 0.0 },
    View { x: 0.0, y: 0.0 },
    View { x: 0.0, y: 0.0 },
];

const CHAR_MAP: [u8; 256] = [
    96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96,
    96, 96, 96, 96, 96, 96, 96, 96, 0, 1, 2, 96, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,
    18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41,
    42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65,
    66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89,
    90, 91, 92, 93, 94, 95, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96,
    96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 97, 98, 99, 100, 101, 102, 103,
    104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122,
    123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141,
    142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 96,
    96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96,
    96, 96, 96, 96, 96, 96, 96,
];

#[derive(Clone, Copy)]
struct View {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy)]
pub enum GlyphMode {
    Matrix,
    Binary,
    Decimal,
    Hexadecimal,
    Dna,
}

impl GlyphMode {
    fn glyphs(self) -> &'static [i32] {
        match self {
            GlyphMode::Matrix => &MATRIX_ENCODING,
            GlyphMode::Binary => &BINARY_ENCODING,
            GlyphMode::Decimal => &DECIMAL_ENCODING,
            GlyphMode::Hexadecimal => &HEX_ENCODING,
            GlyphMode::Dna => &DNA_ENCODING,
        }
    }

    fn flips_texture(self) -> bool {
        matches!(self, GlyphMode::Matrix)
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        if value.eq_ignore_ascii_case("matrix") {
            Ok(Self::Matrix)
        } else if value.eq_ignore_ascii_case("bin") || value.eq_ignore_ascii_case("binary") {
            Ok(Self::Binary)
        } else if value.eq_ignore_ascii_case("dec") || value.eq_ignore_ascii_case("decimal") {
            Ok(Self::Decimal)
        } else if value.eq_ignore_ascii_case("hex") || value.eq_ignore_ascii_case("hexadecimal") {
            Ok(Self::Hexadecimal)
        } else if value.eq_ignore_ascii_case("dna") {
            Ok(Self::Dna)
        } else {
            Err(format!(
                "mode must be matrix, binary, decimal, hexadecimal, or dna: got {value:?}"
            ))
        }
    }
}

struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: Option<u64>) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let pid = u64::from(process::id());
        Self {
            state: seed.unwrap_or(nanos ^ pid.rotate_left(32)).max(1),
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn usize(&mut self, max: usize) -> usize {
        if max == 0 {
            0
        } else {
            (self.next_u64() % max as u64) as usize
        }
    }

    fn one_in(&mut self, n: usize) -> bool {
        self.usize(n) == 0
    }

    fn frand(&mut self, max: f32) -> f32 {
        let unit = ((self.next_u64() >> 40) as f32) / ((1u64 << 24) as f32);
        unit * max
    }

    fn bellrand(&mut self, max: f32) -> f32 {
        (self.frand(max) + self.frand(max) + self.frand(max)) / 3.0
    }
}

#[derive(Clone)]
struct Strip {
    x: f32,
    y: f32,
    z: f32,
    dx: f32,
    dy: f32,
    dz: f32,
    erasing: bool,
    spinner_glyph: i32,
    spinner_y: f32,
    spinner_speed: f32,
    glyphs: [i32; GRID_SIZE],
    highlight: [bool; GRID_SIZE],
    spin_speed: i32,
    spin_tick: i32,
    wave_position: i32,
    wave_speed: i32,
    wave_tick: i32,
}

impl Default for Strip {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            dx: 0.0,
            dy: 0.0,
            dz: 0.0,
            erasing: false,
            spinner_glyph: 0,
            spinner_y: 0.0,
            spinner_speed: 0.0,
            glyphs: [0; GRID_SIZE],
            highlight: [false; GRID_SIZE],
            spin_speed: 1,
            spin_tick: 0,
            wave_position: 0,
            wave_speed: 1,
            wave_tick: 0,
        }
    }
}

pub struct Matrix {
    options: Options,
    rng: Rng,
    texture: gl::GLuint,
    nstrips: usize,
    strips: Vec<Strip>,
    glyph_map: &'static [i32],
    tex_char_width: f32,
    tex_char_height: f32,
    real_char_rows: i32,
    brightness_ramp: [f32; WAVE_SIZE],
    last_view: usize,
    target_view: usize,
    view_x: f32,
    view_y: f32,
    view_steps: i32,
    view_tick: i32,
    auto_tracking: bool,
    track_tick: i32,
}

impl Matrix {
    pub fn new(options: Options) -> Self {
        let glyph_map = options.mode.glyphs();
        let nstrips = ((options.density * 2.2) as usize).clamp(1, 2000);
        let mut matrix = Self {
            rng: Rng::new(options.seed),
            options,
            texture: 0,
            nstrips,
            strips: Vec::new(),
            glyph_map,
            tex_char_width: 0.0,
            tex_char_height: 0.0,
            real_char_rows: REAL_CHAR_ROWS as i32,
            brightness_ramp: std::array::from_fn(|i| {
                let mut j = (WAVE_SIZE - i) as f32 / (WAVE_SIZE - 1) as f32;
                j *= std::f32::consts::FRAC_PI_2;
                j = j.sin();
                0.2 + j * 0.8
            }),
            last_view: 0,
            target_view: 0,
            view_x: NICE_VIEWS[0].x,
            view_y: NICE_VIEWS[0].y,
            view_steps: 100,
            view_tick: 0,
            auto_tracking: false,
            track_tick: 0,
        };

        matrix.strips = Vec::from_fn(nstrips, |_| matrix.random_strip(true));

        matrix
    }

    pub fn init_gl(&mut self) {
        unsafe {
            gl::glShadeModel(gl::GL_SMOOTH);
            gl::glDisable(gl::GL_DEPTH_TEST);
            gl::glDisable(gl::GL_CULL_FACE);
            gl::glEnable(gl::GL_NORMALIZE);
        }
        self.set_background(self.options.bg);

        if self.options.do_texture {
            self.load_texture();
        }
    }

    pub fn set_background(&mut self, color: Color) {
        self.options.bg = color;
        let [r, g, b] = color.components();
        unsafe {
            gl::glClearColor(r, g, b, 1.0);
        }
    }

    fn load_texture(&mut self) {
        let atlas = make_texture_atlas(
            self.options
                .flip_texture
                .unwrap_or_else(|| self.options.mode.flips_texture()),
        );
        self.real_char_rows = atlas.real_rows as i32;
        self.tex_char_width = atlas.cell as f32 / atlas.width as f32;
        self.tex_char_height = atlas.cell as f32 / atlas.height as f32;

        unsafe {
            gl::glGenTextures(1, &mut self.texture);
            gl::glPixelStorei(gl::GL_UNPACK_ALIGNMENT, 1);
            gl::glBindTexture(gl::GL_TEXTURE_2D, self.texture);
            gl::glTexImage2D(
                gl::GL_TEXTURE_2D,
                0,
                gl::GL_RGBA as gl::GLint,
                atlas.width as gl::GLsizei,
                atlas.height as gl::GLsizei,
                0,
                gl::GL_RGBA,
                gl::GL_UNSIGNED_BYTE,
                atlas.data.as_ptr().cast(),
            );
            gl::glTexParameteri(
                gl::GL_TEXTURE_2D,
                gl::GL_TEXTURE_MAG_FILTER,
                gl::GL_LINEAR as gl::GLint,
            );
            gl::glTexParameteri(
                gl::GL_TEXTURE_2D,
                gl::GL_TEXTURE_MIN_FILTER,
                gl::GL_LINEAR as gl::GLint,
            );
            gl::glTexParameteri(
                gl::GL_TEXTURE_2D,
                gl::GL_TEXTURE_WRAP_S,
                gl::GL_REPEAT as gl::GLint,
            );
            gl::glTexParameteri(
                gl::GL_TEXTURE_2D,
                gl::GL_TEXTURE_WRAP_T,
                gl::GL_REPEAT as gl::GLint,
            );
            gl::glTexEnvi(
                gl::GL_TEXTURE_ENV,
                gl::GL_TEXTURE_ENV_MODE,
                gl::GL_MODULATE as gl::GLint,
            );
        }
    }

    pub fn reshape(&self, width: u32, height: u32) {
        let mut viewport_height = height.clamp(1, i32::MAX as u32) as i32;
        let viewport_width = width.clamp(1, i32::MAX as u32) as i32;
        let mut y = 0;

        if i64::from(viewport_width) > i64::from(viewport_height) * 5 {
            viewport_height = (i64::from(viewport_width) * 9 / 16) as i32;
            y = -viewport_height / 2;
        }

        let aspect = viewport_width as f64 / viewport_height.max(1) as f64;
        let near = 1.0_f64;
        let far = 100.0_f64;
        let top = near * (80.0_f64.to_radians() / 2.0).tan();
        let right = top * aspect;

        unsafe {
            gl::glViewport(0, y, viewport_width, viewport_height);
            gl::glMatrixMode(gl::GL_PROJECTION);
            gl::glLoadIdentity();
            gl::glFrustum(-right, right, -top, top, near, far);
            gl::glMatrixMode(gl::GL_MODELVIEW);
            gl::glLoadIdentity();
            gl::glTranslatef(0.0, 0.0, -25.0);
            gl::glClear(gl::GL_COLOR_BUFFER_BIT);
        }
    }

    pub fn erase(&mut self) {
        for strip in &mut self.strips {
            strip.dy = -(0.6 + self.rng.frand(0.6)) * self.options.speed;
        }
    }

    pub fn tick(&mut self) {
        for index in 0..self.nstrips {
            self.tick_strip(index);
        }
        self.auto_track();
    }

    fn random_strip(&mut self, initial_erasing: bool) -> Strip {
        let mut strip = Strip {
            x: self.rng.frand(GRID_SIZE as f32) - GRID_SIZE as f32 / 2.0,
            y: GRID_SIZE as f32 / 2.0 + self.rng.bellrand(0.5),
            z: GRID_DEPTH * 0.2 - self.rng.frand(GRID_DEPTH * 0.7),
            dx: 0.0,
            dy: 0.0,
            dz: self.rng.bellrand(0.02) * self.options.speed,
            spinner_y: 0.0,
            spinner_speed: self.rng.bellrand(0.3) * self.options.speed,
            spin_speed: self.rng.bellrand(2.0 / self.options.speed) as i32 + 1,
            wave_speed: self.rng.bellrand(3.0 / self.options.speed) as i32 + 1,
            ..Strip::default()
        };

        let mut time_displayed = false;
        let mut i = 0;
        while i < GRID_SIZE {
            if self.options.do_clock
                && !time_displayed
                && i < GRID_SIZE - 5
                && self.rng.one_in((GRID_SIZE - 5) * 5)
            {
                let text = current_time_text(&self.options.timefmt);
                for b in text.bytes() {
                    if i >= GRID_SIZE {
                        break;
                    }
                    strip.glyphs[i] = i32::from(CHAR_MAP[b as usize]) + 1;
                    strip.highlight[i] = true;
                    i += 1;
                }
                time_displayed = true;
            } else {
                let draw = self.rng.usize(7) != 0;
                let spin = draw && self.rng.one_in(20);
                let mut glyph = if draw {
                    random_visible_glyph(&mut self.rng, self.glyph_map)
                } else {
                    0
                };
                if spin {
                    glyph = -glyph;
                }
                strip.glyphs[i] = glyph;
                strip.highlight[i] = false;
                i += 1;
            }
        }

        strip.spinner_glyph = -random_visible_glyph(&mut self.rng, self.glyph_map);

        if initial_erasing {
            strip.erasing = true;
            strip.spinner_y = self.rng.frand(GRID_SIZE as f32);
            strip.glyphs = [0; GRID_SIZE];
            strip.highlight = [false; GRID_SIZE];
        }

        strip
    }

    fn tick_strip(&mut self, index: usize) {
        if self.strips[index].dy < 0.0 {
            let strip = &mut self.strips[index];
            strip.y += strip.dy;
            if strip.y < -(GRID_SIZE as f32) {
                self.strips[index] = self.random_strip(false);
            }
            return;
        }

        let mut reset = false;
        {
            let strip = &mut self.strips[index];
            strip.x += strip.dx;
            strip.y += strip.dy;
            strip.z += strip.dz;
            if strip.z > GRID_DEPTH * SPLASH_RATIO {
                reset = true;
            }
        }
        if reset {
            self.strips[index] = self.random_strip(false);
            return;
        }

        {
            let strip = &mut self.strips[index];
            strip.spinner_y += strip.spinner_speed;
            if strip.spinner_y >= GRID_SIZE as f32 {
                if strip.erasing {
                    reset = true;
                } else {
                    strip.erasing = true;
                    strip.spinner_y = 0.0;
                    strip.spinner_speed /= 2.0;
                }
            }
        }
        if reset {
            self.strips[index] = self.random_strip(false);
            return;
        }

        let glyph_map = self.glyph_map;
        let rng = &mut self.rng;
        let strip = &mut self.strips[index];

        strip.spin_tick += 1;
        if strip.spin_tick > strip.spin_speed {
            strip.spin_tick = 0;
            strip.spinner_glyph = -random_visible_glyph(rng, glyph_map);
            for glyph in &mut strip.glyphs {
                if *glyph < 0 {
                    *glyph = -random_visible_glyph(rng, glyph_map);
                    if rng.one_in(800) {
                        *glyph = -*glyph;
                    }
                }
            }
        }

        strip.wave_tick += 1;
        if strip.wave_tick > strip.wave_speed {
            strip.wave_tick = 0;
            strip.wave_position += 1;
            if strip.wave_position >= WAVE_SIZE as i32 {
                strip.wave_position = 0;
            }
        }
    }

    pub fn draw(&self) {
        unsafe {
            gl::glClear(gl::GL_COLOR_BUFFER_BIT | gl::GL_DEPTH_BUFFER_BIT);
            gl::glPushMatrix();
        }

        if self.options.do_texture {
            unsafe {
                gl::glEnable(gl::GL_TEXTURE_2D);
                gl::glEnable(gl::GL_BLEND);
                // Preserve the original glow on black; blend into colored backgrounds.
                let destination = if self.options.bg == Color::default() {
                    gl::GL_ONE
                } else {
                    gl::GL_ONE_MINUS_SRC_ALPHA
                };
                gl::glBlendFunc(gl::GL_SRC_ALPHA, destination);
                gl::glBindTexture(gl::GL_TEXTURE_2D, self.texture);
            }
        } else {
            unsafe {
                gl::glDisable(gl::GL_TEXTURE_2D);
                gl::glDisable(gl::GL_BLEND);
            }
        }

        if self.options.do_rotate {
            unsafe {
                gl::glRotatef(self.view_x, 1.0, 0.0, 0.0);
                gl::glRotatef(self.view_y, 0.0, 1.0, 0.0);
            }
        }

        let mut order: Vec<usize> = (0..self.nstrips).collect();
        order.sort_by(|a, b| {
            self.strips[*a]
                .z
                .partial_cmp(&self.strips[*b].z)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut polygon_count = 0_u64;
        for index in order {
            self.draw_strip(&self.strips[index], &mut polygon_count);
        }

        unsafe {
            gl::glPopMatrix();
        }
    }

    fn draw_strip(&self, strip: &Strip, polygon_count: &mut u64) {
        for i in 0..GRID_SIZE {
            let glyph = strip.glyphs[i];
            let mut below = strip.spinner_y >= i as f32;
            if strip.erasing {
                below.toggle();
            }

            if glyph != 0 && below {
                let brightness = if self.options.do_waves {
                    let phase = (i + (GRID_SIZE - strip.wave_position as usize)) % WAVE_SIZE;
                    let j = (WAVE_SIZE - phase) % WAVE_SIZE;
                    self.brightness_ramp[j]
                } else {
                    1.0
                };

                self.draw_glyph(
                    glyph,
                    strip.highlight[i],
                    strip.x,
                    strip.y - i as f32,
                    strip.z,
                    brightness,
                    polygon_count,
                );
            }
        }

        if !strip.erasing {
            self.draw_glyph(
                strip.spinner_glyph,
                false,
                strip.x,
                strip.y - strip.spinner_y,
                strip.z,
                1.0,
                polygon_count,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_glyph(
        &self,
        glyph: i32,
        highlight: bool,
        mut x: f32,
        mut y: f32,
        z: f32,
        mut brightness: f32,
        polygon_count: &mut u64,
    ) {
        if glyph == 0 {
            return;
        }

        let spinner = glyph < 0;
        let glyph = glyph.abs();
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut size = 1.0;

        if spinner {
            brightness *= 1.5;
        }

        if !self.options.do_texture {
            size = 0.8;
            x += 0.1;
            y += 0.1;
        } else {
            let cell = glyph - 1;
            let ccx = cell % CHAR_COLS as i32;
            let ccy = cell / CHAR_COLS as i32;
            cx = ccx as f32 * self.tex_char_width;
            cy = (self.real_char_rows - ccy - 1) as f32 * self.tex_char_height;
        }

        if self.options.do_fog {
            let mut depth = z / GRID_DEPTH + 0.5;
            depth = 0.2 + depth * 0.8;
            brightness *= depth;
        }

        if highlight {
            brightness *= 2.0;
        }

        let [r, g, b] = if let Some(color) = self.options.fg {
            let highlight = if spinner {
                0.7
            } else if highlight {
                0.35
            } else {
                0.0
            };
            color
                .components()
                .map(|value| value + (1.0 - value) * highlight)
        } else if spinner {
            [0.72, 1.0, 0.74]
        } else if highlight {
            [0.38, 1.0, 0.44]
        } else {
            [0.04, 0.92, 0.16]
        };

        let mut alpha = brightness;
        if z > GRID_DEPTH / 2.0 {
            let ratio = (z - GRID_DEPTH / 2.0) / (GRID_DEPTH * SPLASH_RATIO - GRID_DEPTH / 2.0);
            let i = ((ratio * WAVE_SIZE as f32) as usize).min(WAVE_SIZE - 1);
            alpha *= self.brightness_ramp[i];
        }

        unsafe {
            if self.options.do_texture {
                gl::glColor4f(r, g, b, alpha);
            } else {
                let brightness = alpha.clamp(0.0, 1.0);
                let [br, bg, bb] = self.options.bg.components();
                gl::glColor4f(
                    br + (r - br) * brightness,
                    bg + (g - bg) * brightness,
                    bb + (b - bb) * brightness,
                    1.0,
                );
            }
            gl::glBegin(if self.options.wireframe {
                gl::GL_LINE_LOOP
            } else {
                gl::GL_QUADS
            });
            gl::glNormal3f(0.0, 0.0, 1.0);
            gl::glTexCoord2f(cx, cy);
            gl::glVertex3f(x, y, z);
            gl::glTexCoord2f(cx + self.tex_char_width, cy);
            gl::glVertex3f(x + size, y, z);
            gl::glTexCoord2f(cx + self.tex_char_width, cy + self.tex_char_height);
            gl::glVertex3f(x + size, y + size, z);
            gl::glTexCoord2f(cx, cy + self.tex_char_height);
            gl::glVertex3f(x, y + size, z);
            gl::glEnd();

            if self.options.wireframe && spinner {
                gl::glBegin(gl::GL_LINES);
                gl::glVertex3f(x, y, z);
                gl::glVertex3f(x + size, y + size, z);
                gl::glVertex3f(x, y + size, z);
                gl::glVertex3f(x + size, y, z);
                gl::glEnd();
            }
        }

        *polygon_count += 1;
    }

    fn auto_track(&mut self) {
        if !self.options.do_rotate {
            return;
        }

        if !self.auto_tracking {
            self.track_tick += 1;
            if self.track_tick < (20.0 / self.options.speed) as i32 {
                return;
            }
            self.track_tick = 0;
            if self.rng.one_in(20) {
                self.auto_tracking = true;
            } else {
                return;
            }
        }

        let origin = NICE_VIEWS[self.last_view];
        let target = NICE_VIEWS[self.target_view];
        let t =
            (std::f32::consts::FRAC_PI_2 * self.view_tick as f32 / self.view_steps as f32).sin();

        self.view_x = origin.x + (target.x - origin.x) * t;
        self.view_y = origin.y + (target.y - origin.y) * t;
        self.view_tick += 1;

        if self.view_tick >= self.view_steps {
            self.view_tick = 0;
            self.view_steps = ((350.0 / self.options.speed) as i32).max(1);
            self.last_view = self.target_view;
            self.target_view = self.rng.usize(NICE_VIEWS.len() - 1) + 1;
            self.auto_tracking = false;
        }
    }
}

impl Drop for Matrix {
    fn drop(&mut self) {
        if self.texture != 0 {
            unsafe {
                gl::glDeleteTextures(1, &self.texture);
            }
        }
    }
}

fn random_visible_glyph(rng: &mut Rng, glyph_map: &[i32]) -> i32 {
    let glyphs = if glyph_map.len() == MATRIX_ENCODING.len() {
        match rng.usize(3) {
            0 => &glyph_map[..10],
            1 => &glyph_map[10..ORIGINAL_MATRIX_ENCODING.len()],
            _ => &glyph_map[ORIGINAL_MATRIX_ENCODING.len()..],
        }
    } else {
        glyph_map
    };
    glyphs[rng.usize(glyphs.len())] + 1
}

struct TextureAtlas {
    width: usize,
    height: usize,
    cell: usize,
    real_rows: usize,
    data: Vec<u8>,
}

fn make_texture_atlas(flip: bool) -> TextureAtlas {
    let cell = 32;
    let width = CHAR_COLS * cell;
    let height = (CHAR_ROWS * cell).next_power_of_two();
    let mut atlas = TextureAtlas {
        width,
        height,
        cell,
        real_rows: REAL_CHAR_ROWS,
        data: vec![0; width * height * 4],
    };

    for glyph in 0..(CHAR_COLS * REAL_CHAR_ROWS) {
        draw_atlas_glyph(&mut atlas, glyph, flip);
    }

    atlas
}

fn draw_atlas_glyph(atlas: &mut TextureAtlas, glyph: usize, flip: bool) {
    let col = glyph % CHAR_COLS;
    let row = glyph / CHAR_COLS;
    let base_x = col * atlas.cell;
    let base_y = (atlas.real_rows - row - 1) * atlas.cell;

    if let Some(pattern) = glyph
        .checked_sub(KANA_GLYPH_START)
        .and_then(|index| kana::PATTERNS.get(index))
    {
        for (y, bits) in pattern.iter().enumerate() {
            for x in 0..16 {
                if bits & (1 << (15 - x)) != 0 {
                    draw_cell_rect(atlas, base_x, base_y, x * 2, (15 - y) * 2, 2, 2, 255, flip);
                }
            }
        }
        return;
    }

    if let Some(ch) = ascii_char_for_glyph(glyph) {
        if ch == ' ' {
            return;
        }
        if let Some(pattern) = font_pattern(ch) {
            draw_font_pattern(atlas, base_x, base_y, pattern, flip);
            return;
        }
    }

    draw_procedural_symbol(atlas, base_x, base_y, glyph, flip);
}

fn ascii_char_for_glyph(glyph: usize) -> Option<char> {
    if glyph < 96 {
        Some((glyph as u8 + b' ') as char)
    } else {
        None
    }
}

fn draw_font_pattern(
    atlas: &mut TextureAtlas,
    base_x: usize,
    base_y: usize,
    pattern: [u8; 7],
    flip: bool,
) {
    let scale = 4;
    let margin_x = 6;
    let margin_y = 2;
    for (row, bits) in pattern.iter().enumerate() {
        for col in 0..5 {
            if bits & (1 << (4 - col)) != 0 {
                let x = margin_x + col * scale;
                let y = atlas.cell - margin_y - (row + 1) * scale;
                draw_cell_rect(atlas, base_x, base_y, x, y, scale, scale, 230, flip);
                draw_cell_rect(
                    atlas,
                    base_x,
                    base_y,
                    x + 1,
                    y + 1,
                    scale.saturating_sub(2),
                    scale.saturating_sub(2),
                    255,
                    flip,
                );
            }
        }
    }
}

fn draw_procedural_symbol(
    atlas: &mut TextureAtlas,
    base_x: usize,
    base_y: usize,
    glyph: usize,
    flip: bool,
) {
    let mut seed = (glyph as u64 + 1)
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .rotate_left((glyph % 31) as u32);
    let scale = 4;
    let margin_x = 6;
    let margin_y = 2;

    for row in 0..7 {
        for col in 0..5 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let edge_bias = col == 0 || col == 4 || row == 0 || row == 6;
            let draw = if edge_bias {
                seed & 0b11 == 0
            } else {
                seed & 0b111 != 0
            };
            if draw {
                let x = margin_x + col * scale;
                let y = atlas.cell - margin_y - (row + 1) * scale;
                let alpha = 170 + (seed as u8 & 0x55);
                draw_cell_rect(atlas, base_x, base_y, x, y, scale, scale, alpha, flip);
            }
        }
    }

    let bar_x = margin_x + ((glyph * 3) % 5) * scale;
    draw_cell_rect(
        atlas,
        base_x,
        base_y,
        bar_x,
        margin_y + 2,
        2,
        atlas.cell - margin_y * 2 - 4,
        190,
        flip,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_cell_rect(
    atlas: &mut TextureAtlas,
    base_x: usize,
    base_y: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    alpha: u8,
    flip: bool,
) {
    if w == 0 || h == 0 {
        return;
    }

    for yy in y..(y + h).min(atlas.cell) {
        for xx in x..(x + w).min(atlas.cell) {
            let local_x = if flip { atlas.cell - xx - 1 } else { xx };
            put_atlas_pixel(atlas, base_x + local_x, base_y + yy, alpha);
        }
    }
}

fn put_atlas_pixel(atlas: &mut TextureAtlas, x: usize, y: usize, alpha: u8) {
    if x >= atlas.width || y >= atlas.height {
        return;
    }
    let index = (y * atlas.width + x) * 4;
    let current = atlas.data[index + 3];
    let alpha = alpha.max(current);
    atlas.data[index] = 255;
    atlas.data[index + 1] = 255;
    atlas.data[index + 2] = 255;
    atlas.data[index + 3] = alpha;
}

fn font_pattern(ch: char) -> Option<[u8; 7]> {
    let pattern = match ch {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10011, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' => [
            0b00001, 0b00001, 0b00001, 0b00001, 0b10001, 0b10001, 0b01110,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        ':' => [
            0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000,
        ],
        '+' => [
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ],
        _ => {
            if ch.is_ascii_lowercase() {
                return font_pattern(ch.to_ascii_uppercase());
            }
            return None;
        }
    };

    Some(pattern)
}

fn current_time_text(format: &str) -> String {
    let Ok(format) = CString::new(format) else {
        return String::new();
    };

    let mut now: ctime::TimeT = 0;
    unsafe { ctime::time(&mut now) };

    let mut tm = mem::MaybeUninit::<ctime::Tm>::zeroed();
    if unsafe { ctime::localtime_r(&now, tm.as_mut_ptr()) }.is_null() {
        return String::new();
    }
    let tm = unsafe { tm.assume_init() };

    let mut buf = [0_u8; 128];
    let len = unsafe { ctime::strftime(buf.as_mut_ptr().cast(), buf.len(), format.as_ptr(), &tm) };
    String::from_utf8_lossy(&buf[..len]).into_owned()
}

mod ctime {
    use super::{c_char, c_int, c_long};

    pub type TimeT = c_long;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Tm {
        pub tm_sec: c_int,
        pub tm_min: c_int,
        pub tm_hour: c_int,
        pub tm_mday: c_int,
        pub tm_mon: c_int,
        pub tm_year: c_int,
        pub tm_wday: c_int,
        pub tm_yday: c_int,
        pub tm_isdst: c_int,
        pub tm_gmtoff: c_long,
        pub tm_zone: *const c_char,
    }

    unsafe extern "C" {
        pub fn time(tloc: *mut TimeT) -> TimeT;
        pub fn localtime_r(timep: *const TimeT, result: *mut Tm) -> *mut Tm;
        pub fn strftime(s: *mut c_char, max: usize, format: *const c_char, tm: *const Tm) -> usize;
    }
}
