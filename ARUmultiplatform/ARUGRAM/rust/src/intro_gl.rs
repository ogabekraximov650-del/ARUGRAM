//! Telegram kirish (intro) sahnasi — DrKLO/Telegram `TMessagesProj/jni/intro/IntroRenderer.c` ning Rust'ga o'tkazilgan nusxasi.
//!
//! Telegram for Android: Copyright (c) Nikolai Kudashov va hissa qo'shuvchilar, GNU GPL v2 yoki undan keyingi. ARUGRAM ham GPLv2 ostida
//! (LICENSE, NOTICE). OpenGL ES 2 chaqiruvlari `libGLESv2` dan to'g'ridan-to'g'ri olinadi (qo'shimcha kutubxonasiz); Kotlin tomoni
//! (`IntroGlView`) EGL kontekst, teksturalarni yuklash va vaqtni beradi.
//!
//! Tuzilma asliga mos: `Shape`/`TexturedShape`, `mvp_matrix`, `t()` (Bezier egri chiziqlari), sahifa/yo'nalish holatlari (`current_page`,
//! `direct`) va har bir sahifa uchun `on_draw_frame`dagi bo'limlar. Hamma sonlar (o'lchamlar, vaqtlar, burchaklar) asldagidek.

#![allow(clippy::too_many_arguments, clippy::needless_range_loop, non_upper_case_globals)]

use std::f32::consts::PI;
use std::os::raw::{c_char, c_void};
use std::sync::Mutex;

// ───────────────────────── OpenGL ES 2 ─────────────────────────
#[allow(non_snake_case)]
mod gl {
    use std::os::raw::{c_char, c_void};
    pub const COLOR_BUFFER_BIT: u32 = 0x4000;
    pub const DEPTH_BUFFER_BIT: u32 = 0x100;
    pub const BLEND: u32 = 0x0BE2;
    pub const DEPTH_TEST: u32 = 0x0B71;
    pub const SRC_ALPHA: u32 = 0x0302;
    pub const ONE_MINUS_SRC_ALPHA: u32 = 0x0303;
    pub const ONE: u32 = 1;
    pub const GREATER: u32 = 0x0204;
    pub const LESS: u32 = 0x0201;
    pub const VERTEX_SHADER: u32 = 0x8B31;
    pub const FRAGMENT_SHADER: u32 = 0x8B30;
    pub const COMPILE_STATUS: u32 = 0x8B81;
    pub const LINK_STATUS: u32 = 0x8B82;
    pub const FLOAT: u32 = 0x1406;
    pub const TRIANGLE_FAN: u32 = 6;
    pub const TRIANGLE_STRIP: u32 = 5;
    pub const ARRAY_BUFFER: u32 = 0x8892;
    pub const STATIC_DRAW: u32 = 0x88E4;
    pub const TEXTURE0: u32 = 0x84C0;
    pub const TEXTURE_2D: u32 = 0x0DE1;

    #[link(name = "GLESv2")]
    extern "C" {
        pub fn glClearColor(r: f32, g: f32, b: f32, a: f32);
        pub fn glClear(mask: u32);
        pub fn glViewport(x: i32, y: i32, w: i32, h: i32);
        pub fn glEnable(cap: u32);
        pub fn glDisable(cap: u32);
        pub fn glBlendFunc(s: u32, d: u32);
        pub fn glColorMask(r: u8, g: u8, b: u8, a: u8);
        pub fn glDepthMask(f: u8);
        pub fn glDepthFunc(f: u32);
        pub fn glClearDepthf(d: f32);
        pub fn glCreateShader(t: u32) -> u32;
        pub fn glShaderSource(s: u32, count: i32, src: *const *const c_char, len: *const i32);
        pub fn glCompileShader(s: u32);
        pub fn glGetShaderiv(s: u32, pname: u32, out: *mut i32);
        pub fn glCreateProgram() -> u32;
        pub fn glAttachShader(p: u32, s: u32);
        pub fn glLinkProgram(p: u32);
        pub fn glGetProgramiv(p: u32, pname: u32, out: *mut i32);
        pub fn glUseProgram(p: u32);
        pub fn glGetAttribLocation(p: u32, name: *const c_char) -> i32;
        pub fn glGetUniformLocation(p: u32, name: *const c_char) -> i32;
        pub fn glUniformMatrix4fv(loc: i32, count: i32, transpose: u8, v: *const f32);
        pub fn glUniform1i(loc: i32, v: i32);
        pub fn glUniform1f(loc: i32, v: f32);
        pub fn glUniform4fv(loc: i32, count: i32, v: *const f32);
        pub fn glVertexAttribPointer(idx: u32, size: i32, ty: u32, norm: u8, stride: i32, ptr: *const c_void);
        pub fn glEnableVertexAttribArray(idx: u32);
        pub fn glDrawArrays(mode: u32, first: i32, count: i32);
        pub fn glActiveTexture(t: u32);
        pub fn glBindTexture(target: u32, tex: u32);
        pub fn glGenBuffers(n: i32, out: *mut u32);
        pub fn glBindBuffer(target: u32, buf: u32);
        pub fn glBufferData(target: u32, size: isize, data: *const c_void, usage: u32);
    }
}

// ───────────────────────── matematika (asldagidek: ustunlar bo'yicha) ─────────────────────────
type Mat = [[f32; 4]; 4];

fn identity() -> Mat {
    let mut m = [[0.0; 4]; 4];
    for i in 0..4 {
        m[i][i] = 1.0;
    }
    m
}

/// `mat4x4_mul(M, a, b)`: M[c][r] = sum_k a[k][r] * b[c][k]
fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut m = [[0.0f32; 4]; 4];
    for c in 0..4 {
        for r in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[k][r] * b[c][k];
            }
            m[c][r] = s;
        }
    }
    m
}

fn translate(x: f32, y: f32, z: f32) -> Mat {
    let mut m = identity();
    m[3][0] = x;
    m[3][1] = y;
    m[3][2] = z;
    m
}

fn scale_aniso(a: &Mat, x: f32, y: f32, z: f32) -> Mat {
    let mut m = *a;
    for i in 0..4 {
        m[0][i] = a[0][i] * x;
        m[1][i] = a[1][i] * y;
        m[2][i] = a[2][i] * z;
    }
    m
}

fn rotate_z2(m: &Mat, angle: f32) -> Mat {
    let s = angle.sin();
    let c = angle.cos();
    let r: Mat = [[c, s, 0.0, 0.0], [-s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
    mul(m, &r)
}

fn translate_independed(m: &mut Mat, x: f32, y: f32, z: f32) {
    let mut tr = identity();
    for i in 0..4 {
        tr[3][i] += tr[0][i] * x + tr[1][i] * y + tr[2][i] * z;
    }
    let dup = *m;
    *m = mul(&tr, &dup);
}

fn mul_vec4(m: &Mat, v: [f32; 4]) -> [f32; 4] {
    let mut r = [0.0f32; 4];
    for j in 0..4 {
        for i in 0..4 {
            r[j] += m[i][j] * v[i];
        }
    }
    r
}

fn deg_to_rad(d: f32) -> f32 {
    d * PI / 180.0
}
fn d2r(a: f32) -> f32 {
    a * PI / 180.0
}
fn r2d(a: f32) -> f32 {
    a * 180.0 / PI
}

// ───────────────────────── vaqt egri chiziqlari ─────────────────────────
#[derive(Clone, Copy, PartialEq)]
enum Tm {
    Default = 0,
    EaseIn = 1,
    EaseOut = 2,
    EaseInEaseOut = 3,
    Linear = 4,
    Sin = 5,
    EaseOutBounce = 6,
}
const TIMING_NUM: usize = 7;

struct Timing {
    cx: [[f32; 4]; TIMING_NUM],
    cy: [[f32; 4]; TIMING_NUM],
}

impl Timing {
    fn new() -> Self {
        let mut c = [[0.0f32; 4]; TIMING_NUM];
        c[Tm::Default as usize] = [0.25, 0.1, 0.25, 1.0];
        c[Tm::EaseInEaseOut as usize] = [0.42, 0.0, 0.58, 1.0];
        c[Tm::EaseIn as usize] = [0.42, 0.0, 1.0, 1.0];
        c[Tm::EaseOut as usize] = [0.0, 0.0, 0.58, 1.0];
        c[Tm::EaseOutBounce as usize] = [0.0, 0.0, 0.0, 1.25];
        c[Tm::Linear as usize] = [0.0, 0.0, 1.0, 1.0];
        let (c0x, c0y, c3x, c3y) = (0.0f32, 0.0f32, 1.0f32, 1.0f32);
        let mut cx = [[0.0f32; 4]; TIMING_NUM];
        let mut cy = [[0.0f32; 4]; TIMING_NUM];
        for i in 0..TIMING_NUM {
            let (c1x, c1y, c2x, c2y) = (c[i][0], c[i][1], c[i][2], c[i][3]);
            cx[i] = [c0x, -3.0 * c0x + 3.0 * c1x, 3.0 * c0x - 6.0 * c1x + 3.0 * c2x, -c0x + 3.0 * c1x - 3.0 * c2x + c3x];
            cy[i] = [c0y, -3.0 * c0y + 3.0 * c1y, 3.0 * c0y - 6.0 * c1y + 3.0 * c2y, -c0y + 3.0 * c1y - 3.0 * c2y + c3y];
        }
        Timing { cx, cy }
    }

    fn eval(t: f32, c: &[f32; 4]) -> f32 {
        c[0] + t * c[1] + t * t * c[2] + t * t * t * c[3]
    }

    fn timing(&self, x: f32, ty: Tm) -> f32 {
        if x == 0.0 || x == 1.0 {
            return x;
        }
        let i = ty as usize;
        let mut t = x;
        for _ in 0..10 {
            let x2 = Self::eval(t, &self.cx[i]) - x;
            let c = &self.cx[i];
            let d = c[1] + 2.0 * t * c[2] + 3.0 * t * t * c[3];
            t -= x2 / d;
        }
        Self::eval(t, &self.cy[i])
    }
}

// ───────────────────────── tuzilmalar ─────────────────────────
#[derive(Clone, Copy)]
struct V3 {
    x: f32,
    y: f32,
    z: f32,
}
fn v3(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}

#[derive(Clone, Copy)]
struct Layer {
    anchor: V3,
    position: V3,
    rotation: f32,
    scale: V3,
}
fn default_layer() -> Layer {
    Layer { anchor: v3(0.0, 0.0, 0.0), position: v3(0.0, 0.0, 0.0), rotation: 0.0, scale: v3(1.0, 1.0, 1.0) }
}

#[derive(Clone, Copy)]
struct Params {
    anchor: V3,
    position: V3,
    rotation: f32,
    scale: V3,
    alpha: f32,
    // o'zgaruvchi parametrlar
    side_length: f32,
    start_angle: f32,
    end_angle: f32,
    angle: f32,
    size: (f32, f32),
    radius: f32,
    width: f32,
    // o'zgarmas
    round_count: i32,
    mode: u32,
    is_star: bool,
}
fn default_params() -> Params {
    Params {
        anchor: v3(0.0, 0.0, 0.0),
        position: v3(0.0, 0.0, 0.0),
        rotation: 0.0,
        scale: v3(1.0, 1.0, 1.0),
        alpha: 1.0,
        side_length: 0.0,
        start_angle: 0.0,
        end_angle: 0.0,
        angle: 0.0,
        size: (0.0, 0.0),
        radius: 0.0,
        width: 0.0,
        round_count: 0,
        mode: gl::TRIANGLE_FAN,
        is_star: false,
    }
}

/// Rangli shakl (nuqtalar xotirada, `glVertexAttribPointer` bilan beriladi).
#[derive(Clone)]
struct Shape {
    color: [f32; 4],
    data: Vec<[f32; 2]>,
    p: Params,
}

/// Teksturali shakl (x, y, u, v) — VBO.
#[derive(Clone)]
struct TShape {
    texture: u32,
    data: Vec<f32>,
    buffer: u32,
    num: i32,
    p: Params,
}

#[derive(Clone, Copy, PartialEq)]
enum Prog {
    Normal,
    NormalOne,
    Red,
    Blue,
    LightRed,
    LightBlue,
}

#[derive(Default, Clone, Copy)]
struct TexProg {
    program: u32,
    a_pos: u32,
    a_tex: u32,
    u_mvp: i32,
    u_unit: i32,
    u_alpha: i32,
}
#[derive(Default, Clone, Copy)]
struct ColProg {
    program: u32,
    a_pos: u32,
    u_mvp: i32,
    u_color: i32,
    u_alpha: i32,
}

fn cstr(s: &str) -> std::ffi::CString {
    std::ffi::CString::new(s).unwrap_or_default()
}

unsafe fn compile(ty: u32, src: &str, ok: &mut bool) -> u32 {
    let sh = gl::glCreateShader(ty);
    let p = src.as_ptr() as *const c_char;
    let len = src.len() as i32;
    gl::glShaderSource(sh, 1, &p, &len);
    gl::glCompileShader(sh);
    let mut st = 0;
    gl::glGetShaderiv(sh, gl::COMPILE_STATUS, &mut st);
    if st == 0 {
        *ok = false;
    }
    sh
}

unsafe fn build_program(vs: &str, fs: &str, ok: &mut bool) -> u32 {
    let v = compile(gl::VERTEX_SHADER, vs, ok);
    let f = compile(gl::FRAGMENT_SHADER, fs, ok);
    let p = gl::glCreateProgram();
    gl::glAttachShader(p, v);
    gl::glAttachShader(p, f);
    gl::glLinkProgram(p);
    let mut st = 0;
    gl::glGetProgramiv(p, gl::LINK_STATUS, &mut st);
    if st == 0 {
        *ok = false;
    }
    p
}

unsafe fn tex_prog(p: u32) -> TexProg {
    TexProg {
        program: p,
        a_pos: gl::glGetAttribLocation(p, cstr("a_Position").as_ptr()) as u32,
        a_tex: gl::glGetAttribLocation(p, cstr("a_TextureCoordinates").as_ptr()) as u32,
        u_mvp: gl::glGetUniformLocation(p, cstr("u_MvpMatrix").as_ptr()),
        u_unit: gl::glGetUniformLocation(p, cstr("u_TextureUnit").as_ptr()),
        u_alpha: gl::glGetUniformLocation(p, cstr("u_Alpha").as_ptr()),
    }
}

unsafe fn col_prog(p: u32) -> ColProg {
    ColProg {
        program: p,
        a_pos: gl::glGetAttribLocation(p, cstr("a_Position").as_ptr()) as u32,
        u_mvp: gl::glGetUniformLocation(p, cstr("u_MvpMatrix").as_ptr()),
        u_color: gl::glGetUniformLocation(p, cstr("u_Color").as_ptr()),
        u_alpha: gl::glGetUniformLocation(p, cstr("u_Alpha").as_ptr()),
    }
}

const VS_TEX: &str = "uniform mat4 u_MvpMatrix;attribute vec4 a_Position;attribute vec2 a_TextureCoordinates;varying vec2 v_TextureCoordinates;void main(){v_TextureCoordinates = a_TextureCoordinates;gl_Position = u_MvpMatrix * a_Position;}";
const FS_HEAD: &str = "precision lowp float;uniform sampler2D u_TextureUnit;varying vec2 v_TextureCoordinates;uniform float u_Alpha;void main(){";

fn tex_fs(body: &str) -> String {
    format!("{FS_HEAD}{body}}}")
}

fn rand01(seed: &mut u64) -> f32 {
    // xorshift64*
    *seed ^= *seed >> 12;
    *seed ^= *seed << 25;
    *seed ^= *seed >> 27;
    let v = seed.wrapping_mul(0x2545F4914F6CDD1D) >> 40;
    (v as f32) / ((1u64 << 24) as f32)
}

// ───────────────────────── shakl yaratuvchilari ─────────────────────────
fn gen_rounded_rectangle(size: (f32, f32), radius: f32, rc: i32) -> Vec<[f32; 2]> {
    let mut out = Vec::with_capacity((4 * (2 + rc) + 2) as usize);
    out.push([0.0, 0.0]);
    let k = PI / 2.0 / (rc as f32 + 1.0);
    let (w, h) = size;
    let centers = [(w / 2.0 - radius, h / 2.0 - radius), (-w / 2.0 + radius, h / 2.0 - radius), (-w / 2.0 + radius, -h / 2.0 + radius), (w / 2.0 - radius, -h / 2.0 + radius)];
    for n in 0..4 {
        let start = (rc + 1) * n;
        let end = rc + 1 + (rc + 1) * n;
        let (cx, cy) = centers[n as usize];
        for i in start..=end {
            out.push([cx + (i as f32 * k).cos() * radius, cy + (i as f32 * k).sin() * radius]);
        }
    }
    out.push([w / 2.0, h / 2.0 - radius]);
    out
}

fn gen_rounded_rectangle_stroked(size: (f32, f32), radius: f32, stroke: f32, rc: i32) -> Vec<[f32; 2]> {
    let mut out = Vec::new();
    let k = PI / 2.0 / (rc as f32 + 1.0);
    let inner = radius - stroke;
    let (w, h) = size;
    let centers = [(w / 2.0 - radius, h / 2.0 - radius), (-w / 2.0 + radius, h / 2.0 - radius), (-w / 2.0 + radius, -h / 2.0 + radius), (w / 2.0 - radius, -h / 2.0 + radius)];
    for n in 0..4 {
        let start = (rc + 1) * n;
        let end = rc + 1 + (rc + 1) * n;
        let (cx, cy) = centers[n as usize];
        for i in start..=end {
            let a = i as f32 * k;
            out.push([cx + a.cos() * radius, cy + a.sin() * radius]);
            out.push([cx + a.cos() * inner, cy + a.sin() * inner]);
        }
    }
    let (cx, cy) = centers[0];
    out.push([cx + radius, cy]);
    out.push([cx + inner, cy]);
    out
}

fn square_point(angle: f32, radius: f32) -> [f32; 2] {
    let h = PI / 2.0;
    if angle <= h * 0.5 || angle > h * 3.5 {
        [radius, radius * angle.sin() / angle.cos()]
    } else if angle <= h * 1.5 {
        [radius * angle.cos() / angle.sin(), radius]
    } else if angle <= h * 2.5 {
        [-radius, -radius * angle.sin() / angle.cos()]
    } else if angle <= h * 3.5 {
        [-radius * angle.cos() / angle.sin(), -radius]
    } else {
        [0.0, 0.0]
    }
}

fn square_texture_point(p: [f32; 2], side: f32) -> [f32; 2] {
    [-p[0] / side * 0.5 + 0.5, -p[1] / side * 0.5 + 0.5]
}

fn gen_segmented_square(side: f32, start: f32, end: f32) -> Vec<f32> {
    let mut out: Vec<f32> = Vec::with_capacity(28);
    let mut push = |p: [f32; 2]| {
        out.extend_from_slice(&p);
        out.extend_from_slice(&square_texture_point(p, side));
    };
    let da = d2r(-2.6 * 2.0);
    push([(start + end).sin() * 6.0, -(start + end).cos() * 6.0]);
    push(square_point(start + da, side));
    let mut q = 0;
    let mut i = start as i32;
    while (i as f32) < r2d(start + end + da).floor() {
        if (i + 45) % 90 == 0 && q < 4 {
            push(square_point(d2r(i as f32), side));
            q += 1;
        }
        i += 1;
    }
    push(square_point(start + end + da, side));
    for _ in 0..(4 - q) {
        push(square_point(start + end + da, side));
    }
    out
}

fn gen_ribbon(length: f32) -> Vec<[f32; 2]> {
    vec![[-(length - 5.5).max(0.0), -5.5], [0.0, -5.5], [-length.max(0.0), 5.5], [0.0, 5.5]]
}

fn gen_circle(radius: f32, vc: i32) -> Vec<[f32; 2]> {
    let mut out = vec![[0.0, 0.0]];
    for i in 0..=vc {
        let a = 2.0 * PI * (i as f32 / vc as f32);
        out.push([radius * a.cos(), radius * a.sin()]);
    }
    out
}

fn gen_infinity(width: f32, angle: f32, seg_count: i32) -> Vec<[f32; 2]> {
    let path: [[f32; 2]; 13] = [[53.0, 23.0], [49.0, 31.0], [39.0, 47.0], [22.0, 47.0], [6.0, 47.0], [0.0, 31.0], [0.0, 23.0], [0.0, 16.0], [5.0, 0.0], [23.0, 0.0], [39.0, 0.0], [48.0, 15.0], [52.0, 21.0]];
    let mut out = Vec::with_capacity(((seg_count + 1) * 2) as usize);
    for seg in 0..=seg_count {
        let tt = (seg as f32 / seg_count as f32) * angle;
        let q = 4.0f32;
        let tstep = 1.0 / q;
        let n = ((tt / tstep).floor() as i32).clamp(0, 3) as usize;
        let a = path[3 * n];
        let p1 = path[1 + 3 * n];
        let p2 = path[2 + 3 * n];
        let b = path[3 + 3 * n];
        let t = (tt - tstep * n as f32) * q;
        let nt = 1.0 - t;
        let p = [
            a[0] * nt * nt * nt + 3.0 * p1[0] * nt * nt * t + 3.0 * p2[0] * nt * t * t + b[0] * t * t * t,
            a[1] * nt * nt * nt + 3.0 * p1[1] * nt * nt * t + 3.0 * p2[1] * nt * t * t + b[1] * t * t * t,
        ];
        let tangent = [
            -3.0 * a[0] * nt * nt + 3.0 * p1[0] * (1.0 - 4.0 * t + 3.0 * t * t) + 3.0 * p2[0] * (2.0 * t - 3.0 * t * t) + 3.0 * b[0] * t * t,
            -3.0 * a[1] * nt * nt + 3.0 * p1[1] * (1.0 - 4.0 * t + 3.0 * t * t) + 3.0 * p2[1] * (2.0 * t - 3.0 * t * t) + 3.0 * b[1] * t * t,
        ];
        let tn = [-tangent[1], tangent[0]];
        let len = (tn[0] * tn[0] + tn[1] * tn[1]).sqrt();
        let k = 1.0 / len;
        let norm = [tn[0] * k, tn[1] * k];
        out.push([p[0] + norm[0] * width / 2.0, p[1] + norm[1] * width / 2.0]);
        out.push([p[0] - norm[0] * width / 2.0, p[1] - norm[1] * width / 2.0]);
    }
    out
}

fn rectangle_texture_point(p: [f32; 2], size: (f32, f32)) -> [f32; 2] {
    [1.0 - (-p[0] / size.0 + 0.5), p[1] / size.1 + 0.5]
}

fn new_shape(color: [f32; 4], data: Vec<[f32; 2]>, mode: u32) -> Shape {
    let mut p = default_params();
    p.mode = mode;
    Shape { color, data, p }
}

fn rounded_rectangle(size: (f32, f32), radius: f32, rc: i32, color: [f32; 4]) -> Shape {
    let mut s = new_shape(color, gen_rounded_rectangle(size, radius, rc), gl::TRIANGLE_FAN);
    s.p.round_count = rc;
    s.p.size = size;
    s.p.radius = radius;
    s
}

fn change_rounded_rectangle(s: &mut Shape, size: (f32, f32), radius: f32) {
    if s.p.size != size || s.p.radius != radius {
        s.p.size = size;
        s.p.radius = radius;
        s.data = gen_rounded_rectangle(size, radius, s.p.round_count);
    }
}

fn rounded_rectangle_stroked(size: (f32, f32), radius: f32, stroke: f32, rc: i32, color: [f32; 4]) -> Shape {
    let mut s = new_shape(color, gen_rounded_rectangle_stroked(size, radius, stroke, rc), gl::TRIANGLE_STRIP);
    s.p.round_count = rc;
    s.p.size = size;
    s.p.radius = radius;
    s.p.width = stroke;
    s
}

fn change_rounded_rectangle_stroked(s: &mut Shape, size: (f32, f32), radius: f32) {
    if s.p.size != size || s.p.radius != radius {
        s.p.size = size;
        s.p.radius = radius;
        s.data = gen_rounded_rectangle_stroked(size, radius, s.p.width, s.p.round_count);
    }
}

fn rectangle(size: (f32, f32), color: [f32; 4]) -> Shape {
    let (w, h) = size;
    new_shape(color, vec![[-w / 2.0, -h / 2.0], [w / 2.0, -h / 2.0], [-w / 2.0, h / 2.0], [w / 2.0, h / 2.0]], gl::TRIANGLE_STRIP)
}

fn ribbon(length: f32, color: [f32; 4]) -> Shape {
    let mut s = new_shape(color, gen_ribbon(length), gl::TRIANGLE_STRIP);
    s.p.side_length = length;
    s
}

fn change_ribbon(s: &mut Shape, length: f32) {
    if s.p.side_length != length {
        s.p.side_length = length;
        s.data = gen_ribbon(length);
    }
}

fn circle(radius: f32, vc: i32, color: [f32; 4]) -> Shape {
    let mut s = new_shape(color, gen_circle(radius, vc), gl::TRIANGLE_FAN);
    s.p.round_count = vc;
    s
}

fn infinity_shape(width: f32, angle: f32, seg: i32, color: [f32; 4]) -> Shape {
    let mut s = new_shape(color, gen_infinity(width, angle, seg), gl::TRIANGLE_STRIP);
    s.p.round_count = seg;
    s.p.width = width;
    s.p.angle = angle;
    s
}

fn change_infinity(s: &mut Shape, angle: f32) {
    if s.p.angle != angle {
        s.p.angle = angle;
        s.data = gen_infinity(s.p.width, angle, s.p.round_count);
    }
}

unsafe fn upload(data: &[f32]) -> u32 {
    let mut b = 0u32;
    gl::glGenBuffers(1, &mut b);
    gl::glBindBuffer(gl::ARRAY_BUFFER, b);
    gl::glBufferData(gl::ARRAY_BUFFER, (data.len() * 4) as isize, data.as_ptr() as *const c_void, gl::STATIC_DRAW);
    gl::glBindBuffer(gl::ARRAY_BUFFER, 0);
    b
}

fn textured_rectangle(size: (f32, f32), texture: u32) -> TShape {
    let (w, h) = size;
    let pts = [[-w / 2.0, -h / 2.0], [w / 2.0, -h / 2.0], [-w / 2.0, h / 2.0], [w / 2.0, h / 2.0]];
    let mut data = Vec::with_capacity(16);
    for p in pts {
        data.extend_from_slice(&p);
        data.extend_from_slice(&rectangle_texture_point(p, size));
    }
    let buffer = unsafe { upload(&data) };
    let mut p = default_params();
    p.mode = gl::TRIANGLE_STRIP;
    TShape { texture, data, buffer, num: 4, p }
}

fn segmented_square(side: f32, start: f32, end: f32, texture: u32) -> TShape {
    let data = gen_segmented_square(side, start, end);
    let mut p = default_params();
    p.mode = gl::TRIANGLE_FAN;
    TShape { texture, data, buffer: 0, num: 7, p }
}

fn change_segmented_square(s: &mut TShape, side: f32, start: f32, end: f32) {
    if s.p.side_length != side || s.p.start_angle != start || s.p.end_angle != end {
        s.p.side_length = side;
        s.p.start_angle = start;
        s.p.end_angle = end;
        s.data = gen_segmented_square(side, start, end);
    }
}

// ───────────────────────── sahna holati ─────────────────────────
const DC: f32 = 0.3; // duration_const
const R1: f32 = 58.5;
const R2: f32 = 70.0;
const STARS_COUNT: usize = 80;

struct Intro {
    ok: bool,
    tm: Timing,
    seed: u64,
    tp: TexProg,
    tp_one: TexProg,
    tp_red: TexProg,
    tp_blue: TexProg,
    tp_lred: TexProg,
    tp_lblue: TexProg,
    cp: ColProg,
    tex: [u32; 24],

    spiral: TShape,
    mask1: Shape,
    cem: [Shape; 4],
    cloud_cover: Shape,
    free_bg: Shape,
    fast_body: TShape,
    fast_arrow_shadow: TShape,
    fast_arrow: TShape,
    knots: [TShape; 4],
    ribbons: [Shape; 4],
    ribbon_layer: Layer,
    powerful_bg: Shape,
    powerful_mask: TShape,
    powerful_infinity: TShape,
    powerful_infinity_white: TShape,
    private_bg: Shape,
    telegram_sphere: TShape,
    telegram_plane: TShape,
    telegram_mask: TShape,
    cloud_bg: Shape,
    star: TShape,
    stars: Vec<Params>,
    ic_bubble_dot: TShape,
    ic_bubble: TShape,
    ic_cam_lens: TShape,
    ic_cam: TShape,
    ic_pencil: TShape,
    ic_pin: TShape,
    ic_smile_eye: TShape,
    ic_smile: TShape,
    ic_videocam: TShape,
    ic_pin_layer: Layer,
    ic_cam_layer: Layer,
    ic_videocam_layer: Layer,
    ic_smile_layer: Layer,
    ic_bubble_layer: Layer,
    ic_pencil_layer: Layer,
    infinity: Shape,
    private_door: TShape,
    private_screw: TShape,
    private_stroke: Shape,
    private_layer: Layer,

    main_matrix: Mat,
    stars_matrix: Mat,
    ribbons_layer: Mat,
    ic_matrix: Mat,
    private_matrix: Mat,

    width: i32,
    height: i32,
    scale_factor: f32,
    y_offset_absolute: i32,
    y_offset: f32,
    offset_y: f32,
    bg: [f32; 4],
    date: f32,
    date0: f32,
    time: f32,
    time_local: f32,
    current_page: i32,
    prev_page: i32,
    direct: i32,
    scroll_offset: f32,
    knot_delays: [f32; 4],
    ribbon_length: f32,
    stars_far: i32,
    calculated_speedometer_sin: f32,
    ms0: f32,
    ms0_anim: f32,
    fps_anim: i32,
    last_stars_update_fps: i32,
    count_anim_fps: i32,
    speedometer_scroll_offset: f32,
    free_scroll_offset: f32,
    private_scroll_offset: f32,
    anim_pencil_start_time: f32,
    anim_pencil_start_all_time: f32,
    anim_pencil_start_all_end_time: f32,
    anim_pencil_stage: i32,
    anim_bubble_dots_stage: i32,
    anim_bubble_dots_end_period: i32,
    anim_videocam_start_time: f32,
    anim_videocam_next_time: f32,
    anim_videocam_duration: f32,
    anim_videocam_angle: f32,
    anim_videocam_old_angle: f32,
    anim_cam_start_time: f32,
    anim_cam_next_time: f32,
    anim_cam_duration: f32,
    anim_cam_angle: f32,
    anim_cam_old_angle: f32,
    anim_cam_position: (f32, f32),
    anim_cam_old_position: (f32, f32),
    q_shot: i32,
    anim_camshot_start_time: f32,
    anim_camshot_duration: f32,
    anim_smile_start_time1: f32,
    anim_smile_start_time2: f32,
    anim_smile_blink_start_time: f32,
    anim_smile_blink_one: i32,
    anim_smile_stage: i32,
    anim_pin_start_time: f32,
    anim_pin_duration: f32,
    anim_pencil_period: i32,
    cloud_scroll_offset: f32,
}

static STATE: Mutex<Option<Box<Intro>>> = Mutex::new(None);
static TEXTURES: Mutex<[u32; 24]> = Mutex::new([0; 24]);
static PENDING: Mutex<Pending> = Mutex::new(Pending { page: 0, scroll: 0.0, date: 1.0, bg: [1.0; 4], size: None });

struct Pending {
    page: i32,
    scroll: f32,
    date: f32,
    bg: [f32; 4],
    size: Option<(i32, i32, f32, i32)>,
}

const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

impl Intro {
    // ── tasodif ──
    fn frand(&mut self, from: f32, to: f32) -> f32 {
        rand01(&mut self.seed) * (to - from) + from
    }
    fn irand(&mut self, from: i32, to: i32) -> i32 {
        (rand01(&mut self.seed) * ((to - from + 1) as f32) + from as f32) as i32
    }
    fn signrand(&mut self) -> i32 {
        self.irand(0, 1) * 2 - 1
    }

    // ── vaqt funksiyalari ──
    fn t(&self, sv: f32, ev: f32, st: f32, dur: f32, ty: Tm) -> f32 {
        if self.time > st + dur {
            return ev;
        }
        let x = (dur + st).min((self.time - st).max(0.0));
        if ty == Tm::Linear {
            return sv + (ev - sv) * x / dur;
        }
        sv + (ev - sv) * self.tm.timing(x / dur, ty)
    }
    fn t_reversed(&self, ev: f32, sv: f32, st: f32, dur: f32, ty: Tm) -> f32 {
        // C'da t_reversed(end_value, start_value, ...) — parametrlar tartibi shunday, formula t() bilan bir xil.
        self.t(sv, ev, st, dur, ty)
    }
    fn t_local(&self, sv: f32, ev: f32, st: f32, dur: f32, ty: Tm) -> f32 {
        if ty == Tm::Sin {
            return sv + (ev - sv) * ((self.time_local - st) / dur * PI).max(0.0).min(PI).sin();
        }
        if self.time_local > st + dur {
            return ev;
        }
        let x = (dur + st).min((self.time_local - st).max(0.0));
        if ty == Tm::Linear {
            return sv + (ev - sv) * x / dur;
        }
        sv + (ev - sv) * self.tm.timing(x / dur, ty)
    }

    // ── chizish ──
    fn mvp(&self, p: &Params, vp: &Mat) -> Mat {
        let mut model = translate(-p.anchor.x, -p.anchor.y, p.anchor.z);
        let scaled = scale_aniso(&identity(), p.scale.x, -p.scale.y, p.scale.z);
        model = mul(&scaled, &model);
        let rot = rotate_z2(&identity(), deg_to_rad(-p.rotation));
        model = mul(&rot, &model);
        translate_independed(&mut model, p.position.x, -p.position.y, p.position.z);
        let mm = mul(&identity(), vp);
        let mut out = mul(&mm, &model);
        translate_independed(&mut out, 0.0, -self.y_offset / vp[3][3], 0.0);
        out
    }

    fn visible(p: &Params) -> bool {
        p.alpha > 0.0 && p.scale.x.abs() > 0.0 && p.scale.y.abs() > 0.0 && p.scale.z.abs() > 0.0
    }

    fn draw_shape(&self, s: &Shape, vp: &Mat) {
        if !Self::visible(&s.p) {
            return;
        }
        let m = self.mvp(&s.p, vp);
        unsafe {
            gl::glUseProgram(self.cp.program);
            gl::glUniformMatrix4fv(self.cp.u_mvp, 1, 0, m.as_ptr() as *const f32);
            gl::glUniform4fv(self.cp.u_color, 1, s.color.as_ptr());
            gl::glUniform1f(self.cp.u_alpha, s.p.alpha);
            gl::glBindBuffer(gl::ARRAY_BUFFER, 0);
            gl::glVertexAttribPointer(self.cp.a_pos, 2, gl::FLOAT, 0, 8, s.data.as_ptr() as *const c_void);
            gl::glEnableVertexAttribArray(self.cp.a_pos);
            gl::glDrawArrays(s.p.mode, 0, s.data.len() as i32);
        }
    }

    fn draw_ts(&self, s: &TShape, vp: &Mat, prog: Prog) {
        if !Self::visible(&s.p) {
            return;
        }
        let m = self.mvp(&s.p, vp);
        if s.p.is_star {
            let pos = mul_vec4(&m, [0.0, 0.0, 0.0, 1.0]);
            let ndc = [pos[0] / pos[3], pos[1] / pos[3]];
            let wx = ndc[0] * self.width as f32;
            let wy = -ndc[1] * self.height as f32;
            let d = 160.0;
            if wx.abs() > d || wy > (self.y_offset_absolute * 2) as f32 + d || wy < (self.y_offset_absolute * 2) as f32 - d {
                return;
            }
        }
        let tp = match prog {
            Prog::Red => &self.tp_red,
            Prog::Blue => &self.tp_blue,
            Prog::LightRed => &self.tp_lred,
            Prog::LightBlue => &self.tp_lblue,
            Prog::NormalOne => &self.tp_one,
            Prog::Normal => &self.tp,
        };
        unsafe {
            gl::glUseProgram(tp.program);
            gl::glActiveTexture(gl::TEXTURE0);
            gl::glBindTexture(gl::TEXTURE_2D, s.texture);
            gl::glUniformMatrix4fv(tp.u_mvp, 1, 0, m.as_ptr() as *const f32);
            gl::glUniform1i(tp.u_unit, 0);
            gl::glUniform1f(tp.u_alpha, s.p.alpha);
            if s.buffer != 0 {
                gl::glBindBuffer(gl::ARRAY_BUFFER, s.buffer);
                gl::glVertexAttribPointer(tp.a_pos, 2, gl::FLOAT, 0, 16, std::ptr::null());
                gl::glVertexAttribPointer(tp.a_tex, 2, gl::FLOAT, 0, 16, 8 as *const c_void);
            } else {
                gl::glBindBuffer(gl::ARRAY_BUFFER, 0);
                gl::glVertexAttribPointer(tp.a_pos, 2, gl::FLOAT, 0, 16, s.data.as_ptr() as *const c_void);
                gl::glVertexAttribPointer(tp.a_tex, 2, gl::FLOAT, 0, 16, s.data.as_ptr().wrapping_add(2) as *const c_void);
            }
            gl::glEnableVertexAttribArray(tp.a_pos);
            gl::glEnableVertexAttribArray(tp.a_tex);
            gl::glDrawArrays(s.p.mode, 0, s.num);
            gl::glBindBuffer(gl::ARRAY_BUFFER, 0);
        }
    }

    fn rgl_normal_draw(&self) {
        unsafe {
            gl::glDisable(gl::DEPTH_TEST);
            gl::glBlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::glColorMask(1, 1, 1, 1);
            gl::glDepthMask(0);
        }
    }
    fn rgl_mask_draw(&self) {
        unsafe {
            gl::glEnable(gl::DEPTH_TEST);
            gl::glDisable(gl::BLEND);
            gl::glDepthMask(1);
            gl::glColorMask(0, 0, 0, 0);
            gl::glDepthFunc(gl::GREATER);
            gl::glClearDepthf(0.0);
            gl::glClear(gl::DEPTH_BUFFER_BIT);
        }
    }
    fn rgl_normal_draw_through_mask(&self) {
        unsafe {
            gl::glColorMask(1, 1, 1, 1);
            gl::glDepthFunc(gl::LESS);
            gl::glDepthMask(0);
        }
    }
    fn blend(&self, s: u32, d: u32) {
        unsafe { gl::glBlendFunc(s, d) }
    }
    fn blend_on(&self) {
        unsafe { gl::glEnable(gl::BLEND) }
    }
    fn blend_off(&self) {
        unsafe { gl::glDisable(gl::BLEND) }
    }

    fn mat_layer(&self, lp: &Layer, s: f32, r: f32) -> Mat {
        let mm = &self.main_matrix;
        let a = mm[1][1];
        let sc = mm[3][3];
        let mut model = translate(-lp.anchor.x / sc, lp.anchor.y / sc * a, lp.anchor.z / sc);
        let scaled = scale_aniso(&identity(), lp.scale.x, lp.scale.y, lp.scale.z);
        model = mul(&scaled, &model);
        let rot = rotate_z2(&identity(), -deg_to_rad(lp.rotation));
        model = mul(&rot, &model);
        translate_independed(&mut model, lp.position.x / sc, -lp.position.y / sc * a, lp.position.z / sc);
        let mut m = mul(&model, mm);
        m[1][0] /= a;
        m[0][1] *= a;
        let mut scale_m = scale_aniso(&identity(), s, s, s);
        scale_m = rotate_z2(&scale_m, r);
        scale_m[1][0] /= a;
        scale_m[0][1] *= a;
        mul(&scale_m, &m)
    }
}

// ───────────────────────── yaratish (onSurfaceCreated) ─────────────────────────
impl Intro {
    fn create(tex: [u32; 24], bg: [f32; 4]) -> Box<Intro> {
        let mut ok = true;
        let (tp, tp_one, tp_red, tp_blue, tp_lred, tp_lblue, cp);
        unsafe {
            let cvs = "uniform mat4 u_MvpMatrix;attribute vec4 a_Position;void main(){gl_Position = u_MvpMatrix * a_Position;}";
            let cfs = "precision lowp float;uniform vec4 u_Color;uniform float u_Alpha;void main() {gl_FragColor = u_Color;gl_FragColor.w*=u_Alpha;}";
            cp = col_prog(build_program(cvs, cfs, &mut ok));
            tp = tex_prog(build_program(VS_TEX, &tex_fs("gl_FragColor = texture2D(u_TextureUnit, v_TextureCoordinates);gl_FragColor.w *= u_Alpha;"), &mut ok));
            tp_blue = tex_prog(build_program(VS_TEX, &tex_fs("gl_FragColor = texture2D(u_TextureUnit, v_TextureCoordinates);float p = u_Alpha*gl_FragColor.w;gl_FragColor = vec4(0.0,0.6,0.898,p);"), &mut ok));
            tp_red = tex_prog(build_program(VS_TEX, &tex_fs("gl_FragColor = texture2D(u_TextureUnit, v_TextureCoordinates);float p = gl_FragColor.w*u_Alpha;gl_FragColor = vec4(210./255.,57./255.,41./255.,p);"), &mut ok));
            tp_lred = tex_prog(build_program(VS_TEX, &tex_fs("gl_FragColor = texture2D(u_TextureUnit, v_TextureCoordinates);float p = u_Alpha*gl_FragColor.w;gl_FragColor = vec4(246./255., 73./255., 55./255., p);"), &mut ok));
            tp_lblue = tex_prog(build_program(VS_TEX, &tex_fs("gl_FragColor = texture2D(u_TextureUnit, v_TextureCoordinates);float p = u_Alpha*gl_FragColor.w;gl_FragColor = vec4(42./255.,180./255.,247./255.,p);"), &mut ok));
            tp_one = tex_prog(build_program(VS_TEX, &tex_fs("gl_FragColor = texture2D(u_TextureUnit, v_TextureCoordinates);gl_FragColor *= u_Alpha;"), &mut ok));
        }
        let ribbon_length = 86.5f32;
        let mut seed = 0x9E3779B97F4A7C15u64 ^ 0xDEADBEEF;
        let mut star = textured_rectangle((5.25, 5.25), tex[18]);
        star.p.is_star = true;
        let mut stars = Vec::with_capacity(STARS_COUNT);
        for i in 0..STARS_COUNT {
            let mut p = default_params();
            // star_create_position(far): minR=100, maxR=1000, z = far
            let sx = if rand01(&mut seed) < 0.5 { -1.0 } else { 1.0 };
            let sy = if rand01(&mut seed) < 0.5 { -1.0 } else { 1.0 };
            let x = sx * (rand01(&mut seed) * 900.0 + 100.0);
            let y = sy * (rand01(&mut seed) * 900.0 + 100.0);
            p.position = v3(x, y, -(i as f32 * 1500.0) / STARS_COUNT as f32);
            stars.push(p);
        }
        let mut knots = [
            textured_rectangle((138.0 / 3.0, 138.0 / 3.0), tex[14]),
            textured_rectangle((138.0 / 3.0, 138.0 / 3.0), tex[14]),
            textured_rectangle((150.0 / 3.0, 150.0 / 3.0), tex[13]),
            textured_rectangle((150.0 / 3.0, 150.0 / 3.0), tex[13]),
        ];
        knots[0].p.anchor.x = -23.0 + 10.0;
        knots[0].p.anchor.y = 23.0 - 10.0;
        knots[1].p.anchor.x = -23.0 + 10.0;
        knots[1].p.anchor.y = 23.0 - 10.0;
        knots[2].p.anchor.x = -100.0 / 4.0 + 20.0 / 2.0;
        knots[2].p.anchor.y = -100.0 / 4.0 + 20.0 / 2.0;
        knots[3].p.anchor.x = -100.0 / 4.0 + 20.0 / 2.0;
        knots[3].p.anchor.y = -100.0 / 4.0 + 20.0 / 2.0;

        let ribbon_layer = default_layer();
        let mut ribbons = [ribbon(ribbon_length, WHITE), ribbon(ribbon_length, WHITE), ribbon(ribbon_length, WHITE), ribbon(ribbon_length, WHITE)];
        ribbons[1].p.rotation = 90.0;
        ribbons[2].p.rotation = 180.0;
        ribbons[3].p.rotation = 270.0;
        for r in ribbons.iter_mut() {
            r.p.position.y = -9.0;
        }

        let mut telegram_plane = textured_rectangle((82.0, 74.0), tex[21]);
        telegram_plane.p.anchor = v3(6.0, -5.0, 0.0);
        let mut fast_arrow_shadow = textured_rectangle((164.0 / 2.0, 44.0 / 2.0), tex[0]);
        fast_arrow_shadow.p.position.x = -1.0;
        fast_arrow_shadow.p.position.y = 2.0;
        let mut fast_arrow = textured_rectangle((164.0 / 2.0, 44.0 / 2.0), tex[1]);
        fast_arrow.p.anchor.x = -19.0;
        fast_arrow_shadow.p.anchor.x = -19.0;

        let ang = 180.0;
        let spiral = segmented_square(R1, d2r(35.0 + 1.0), d2r(35.0 + 1.0 - 10.0 + ang), tex[3]);

        let mut cloud_cover = rectangle((240.0, 100.0), bg);
        cloud_cover.p.anchor.y = -50.0;
        let mut ic_pin_layer = default_layer();
        let mut ic_pencil_layer = default_layer();
        ic_pin_layer.anchor = v3(0.0, 50.0 / 2.0, 0.0);
        ic_pencil_layer.anchor = v3(-30.0 / 2.0, 30.0 / 2.0, 0.0);
        let mut private_stroke = rounded_rectangle_stroked((244.0 / 2.0, 244.0 / 2.0), 21.0, 9.0, 16, WHITE);
        private_stroke.p.mode = gl::TRIANGLE_STRIP;

        Box::new(Intro {
            ok,
            tm: Timing::new(),
            seed,
            tp,
            tp_one,
            tp_red,
            tp_blue,
            tp_lred,
            tp_lblue,
            cp,
            tex,
            spiral,
            mask1: rounded_rectangle((60.0, 60.0), 0.0, 16, BLACK),
            cem: [circle(1.0, 64, BLACK), circle(1.0, 64, BLACK), circle(1.0, 64, BLACK), circle(1.0, 64, BLACK)],
            cloud_cover,
            free_bg: rectangle((160.0 * 2.0, 160.0 * 2.0), [246.0 / 255.0, 73.0 / 255.0, 55.0 / 255.0, 1.0]),
            fast_body: textured_rectangle((148.0, 148.0), tex[2]),
            fast_arrow_shadow,
            fast_arrow,
            knots,
            ribbons,
            ribbon_layer,
            powerful_bg: rectangle((200.0, 200.0), [47.0 / 255.0, 90.0 / 255.0, 131.0 / 255.0, 1.0]),
            powerful_mask: textured_rectangle((200.0, 200.0), tex[17]),
            powerful_infinity: textured_rectangle((366.0 / 3.0, 180.0 / 3.0), tex[16]),
            powerful_infinity_white: textured_rectangle((366.0 / 3.0, 180.0 / 3.0), tex[15]),
            private_bg: rectangle((240.0, 240.0), [200.0 / 255.0, 207.0 / 255.0, 212.0 / 255.0, 1.0]),
            telegram_sphere: textured_rectangle((150.0, 150.0), tex[22]),
            telegram_plane,
            telegram_mask: textured_rectangle((200.0, 150.0), tex[23]),
            cloud_bg: rectangle((160.0 * 2.0, 160.0 * 2.0), [42.0 / 255.0, 180.0 / 255.0, 247.0 / 255.0, 1.0]),
            star,
            stars,
            ic_bubble_dot: textured_rectangle((18.0 / 3.0, 18.0 / 3.0), tex[4]),
            ic_bubble: textured_rectangle((102.0 / 3.0, 102.0 / 3.0), tex[5]),
            ic_cam_lens: textured_rectangle((36.0 / 3.0, 36.0 / 3.0), tex[6]),
            ic_cam: textured_rectangle((108.0 / 3.0, 96.0 / 3.0), tex[7]),
            ic_pencil: textured_rectangle((86.0 / 3.0, 86.0 / 3.0), tex[8]),
            ic_pin: textured_rectangle((90.0 / 3.0, 120.0 / 3.0), tex[9]),
            ic_smile_eye: textured_rectangle((18.0 / 3.0, 18.0 / 3.0), tex[10]),
            ic_smile: textured_rectangle((120.0 / 3.0, 120.0 / 3.0), tex[11]),
            ic_videocam: textured_rectangle((144.0 / 3.0, 84.0 / 3.0), tex[12]),
            ic_pin_layer,
            ic_cam_layer: default_layer(),
            ic_videocam_layer: default_layer(),
            ic_smile_layer: default_layer(),
            ic_bubble_layer: default_layer(),
            ic_pencil_layer,
            infinity: infinity_shape(11.7, 0.0, 32, WHITE),
            private_door: textured_rectangle((408.0 / 3.0, 408.0 / 3.0), tex[19]),
            private_screw: textured_rectangle((30.0 / 3.0, 30.0 / 3.0), tex[20]),
            private_stroke,
            private_layer: default_layer(),
            main_matrix: identity(),
            stars_matrix: identity(),
            ribbons_layer: identity(),
            ic_matrix: identity(),
            private_matrix: identity(),
            width: 0,
            height: 0,
            scale_factor: 1.0,
            y_offset_absolute: 0,
            y_offset: 0.0,
            offset_y: 0.0,
            bg,
            date: 1.0,
            date0: 0.0,
            time: 0.0,
            time_local: 0.0,
            current_page: 0,
            prev_page: 0,
            direct: 0,
            scroll_offset: 0.0,
            knot_delays: [0.0; 4],
            ribbon_length,
            stars_far: 500,
            calculated_speedometer_sin: 0.0,
            ms0: 0.0,
            ms0_anim: 0.0,
            fps_anim: 0,
            last_stars_update_fps: 0,
            count_anim_fps: 0,
            speedometer_scroll_offset: 0.0,
            free_scroll_offset: 0.0,
            private_scroll_offset: 0.0,
            anim_pencil_start_time: 0.0,
            anim_pencil_start_all_time: 0.0,
            anim_pencil_start_all_end_time: 0.0,
            anim_pencil_stage: 0,
            anim_bubble_dots_stage: 0,
            anim_bubble_dots_end_period: 0,
            anim_videocam_start_time: 0.0,
            anim_videocam_next_time: 0.0,
            anim_videocam_duration: 0.0,
            anim_videocam_angle: 0.0,
            anim_videocam_old_angle: 0.0,
            anim_cam_start_time: 0.0,
            anim_cam_next_time: 0.0,
            anim_cam_duration: 0.0,
            anim_cam_angle: 0.0,
            anim_cam_old_angle: 0.0,
            anim_cam_position: (0.0, 0.0),
            anim_cam_old_position: (0.0, 0.0),
            q_shot: 0,
            anim_camshot_start_time: 0.0,
            anim_camshot_duration: 0.0,
            anim_smile_start_time1: 0.0,
            anim_smile_start_time2: 0.0,
            anim_smile_blink_start_time: 0.0,
            anim_smile_blink_one: 0,
            anim_smile_stage: 0,
            anim_pin_start_time: 0.0,
            anim_pin_duration: 0.0,
            anim_pencil_period: 0,
            cloud_scroll_offset: 0.0,
        })
    }

    fn on_surface_changed(&mut self, wpx: i32, hpx: i32, scale: f32, a1: i32) {
        unsafe { gl::glViewport(0, 0, wpx, hpx) };
        self.width = (wpx as f32 / scale) as i32;
        self.height = (hpx as f32 / scale) as i32;
        self.scale_factor = scale;
        let (w, h) = (self.width, self.height);
        // mat4x4_plain
        let mut m = [[0.0f32; 4]; 4];
        m[0][0] = 1.0;
        m[1][1] = w as f32 / h as f32;
        m[2][2] = 1.0;
        m[3][3] = w as f32 / 2.0;
        self.main_matrix = m;
        self.offset_y = a1 as f32 * self.main_matrix[1][1];
        self.y_offset = self.offset_y;
        self.y_offset_absolute = a1;
        // mat4x4_stars
        let mut s = [[0.0f32; 4]; 4];
        if h >= w {
            let k = w as f32 / h as f32;
            let q = 1.4;
            s[0][0] = 1.0 / q;
            s[1][1] = k / q;
            s[2][2] = 1.0;
            s[2][3] = -1.25;
            s[3][3] = w as f32 * k;
        } else {
            let k = h as f32 / w as f32;
            let q = 2.0;
            s[0][0] = 1.0 / q;
            s[1][1] = (1.0 / k) / q;
            s[2][2] = 1.0;
            s[2][3] = -1.25;
            s[3][3] = h as f32 * k;
        }
        translate_independed(&mut s, 0.0, -2.0 * self.y_offset_absolute as f32 / h as f32 + 4.0 * self.scale_factor / h as f32, 0.0);
        self.stars_matrix = s;
    }

    fn set_background(&mut self, bg: [f32; 4]) {
        self.bg = bg;
        self.cloud_cover = rectangle((240.0, 100.0), bg);
        self.cloud_cover.p.anchor.y = -50.0;
        let was = self.powerful_mask.p;
        let tex = self.tex;
        self.powerful_mask = textured_rectangle((200.0, 200.0), tex[17]);
        self.powerful_mask.p = was;
        self.telegram_mask = textured_rectangle((200.0, 150.0), tex[23]);
    }

    // ── yulduzlar ──
    fn star_initial_position(&mut self, rand_z: i32, forward: i32) -> V3 {
        self.stars_far = 1500;
        let sx = self.signrand() as f32;
        let sy = self.signrand() as f32;
        let x = sx * self.frand(100.0, 1000.0);
        let y = sy * self.frand(100.0, 1000.0);
        let mut z = 0.0;
        if forward == 1 {
            z = if rand_z == 0 { -(self.stars_far as f32) } else { self.frand(0.0, -(self.stars_far as f32)) };
        }
        v3(x, y, z)
    }

    fn draw_stars(&mut self) {
        let update = self.last_stars_update_fps != self.fps_anim;
        self.last_stars_update_fps = self.fps_anim;
        self.y_offset = -100.0 * (self.width as f32 / self.height as f32) * 0.0;
        for i in 0..STARS_COUNT {
            let stars_scroll_offset = self.scroll_offset.max(0.0) * 2.0;
            let transition_speed = if self.direct == 1 {
                let s = 5.0;
                s - self.t(0.0, s, 0.0, DC + 1.0 + 0.8, Tm::Linear)
            } else {
                self.t(-4.0, 0.0, 0.0, DC + 1.0, Tm::EaseOut)
            };
            let speed = stars_scroll_offset + transition_speed;
            if update {
                self.stars[i].position.z += speed;
            }
            if self.stars[i].position.z > 0.0 && speed > 0.0 {
                let p = self.star_initial_position(0, 1);
                self.stars[i].position = p;
            }
            if self.stars[i].position.z < -1500.0 && speed < 0.0 {
                let p = self.star_initial_position(0, 0);
                self.stars[i].position = p;
            }
            let inc = self.scroll_offset * 100.0;
            self.stars[i].position.z += inc;
            self.star.p.position = self.stars[i].position;
            let s = 1.0 + (-self.stars[i].position.z) / self.stars_far as f32 * 5.0;
            self.star.p.scale = v3(s, s, 1.0);
            let far = self.stars_far as f32;
            let a = (1.0 - (-self.stars[i].position.z) / far) * 10.0;
            self.star.p.alpha = a * a / 10.0;
            let (star, sm) = (self.star.clone(), self.stars_matrix);
            self.draw_ts(&star, &sm, Prog::Normal);
            self.stars[i].position.z -= inc;
        }
        self.y_offset = self.offset_y;
    }
}

// ───────────────────────── belgilar (bulut / sovg'a ustidagi) ─────────────────────────
impl Intro {
    fn reset_ic(&mut self) {
        self.anim_smile_start_time1 = self.time_local;
        self.anim_pencil_start_time = 0.0;
        self.anim_pencil_start_all_end_time = 0.0;
        self.anim_cam_next_time = self.time_local;
        self.anim_smile_stage = 0;
        self.anim_smile_blink_one = 0;
        self.anim_pencil_stage = 0;
        self.anim_bubble_dots_end_period = 4;
        self.anim_pencil_period = 1;
    }

    fn drop_y(&self, delay: f32, phase: f32) -> f32 {
        let t_y = self.t(250.0, 0.0, delay, DC, Tm::EaseOut);
        let arg = (self.time - delay).max(0.0) * 50.0;
        let value = 250.0 * 2.71f32.powf(-0.055 * arg * 2.0) * (0.08 * arg - phase).cos() * 0.4;
        t_y - value
    }

    fn bubble_dots_sin(&self, a: f32) -> f32 {
        if a < PI * 2.0 * self.anim_bubble_dots_end_period as f32 {
            a.sin()
        } else {
            0.0
        }
    }

    fn draw_ic(&mut self, ty: i32) {
        let begin_time_k: f32;
        let common_delay: f32;
        let rotation: f32;
        let (color, light): (Prog, Prog);
        if ty == 0 {
            begin_time_k = 2.0;
            common_delay = DC * 0.5;
            rotation = -d2r(self.free_scroll_offset);
            self.cloud_scroll_offset = 0.0;
            color = Prog::Red;
            light = Prog::LightRed;
        } else {
            rotation = 0.0;
            begin_time_k = 2.5;
            common_delay = 0.0;
            color = Prog::Blue;
            light = Prog::LightBlue;
        }
        let page1_back = self.current_page == 1 && self.direct == 0;
        let ic_layer_alpha = if page1_back { self.t(1.0, 0.0, 0.0, DC * 0.25, Tm::EaseOut) } else { 1.0 };
        for s in [&mut self.ic_pin, &mut self.ic_cam, &mut self.ic_cam_lens, &mut self.ic_smile, &mut self.ic_smile_eye, &mut self.ic_videocam, &mut self.ic_bubble, &mut self.ic_bubble_dot, &mut self.ic_pencil] {
            s.p.alpha = ic_layer_alpha;
        }
        let pi8 = PI / 8.0;

        // ── pin ──
        let (pin_pos, mut t_y);
        if ty == 0 {
            pin_pos = (-53.0f32, 30.0f32);
            t_y = if page1_back { 0.0 } else { self.drop_y(common_delay + DC * 0.2 * begin_time_k, 0.0) };
        } else {
            pin_pos = (-77.0, 33.0);
            t_y = self.drop_y(common_delay + DC * 0.2 * begin_time_k, pi8);
        }
        if self.time_local > self.anim_pin_start_time && self.time_local > self.anim_pin_start_time + self.anim_pin_duration {
            self.anim_pin_start_time = self.time_local + DC * self.frand(10.0, 20.0) * 2.0;
            self.anim_pin_duration = DC * self.frand(10.0, 20.0) * 2.0;
        }
        let pinasin = 0.0f32;
        self.ic_pin_layer.position = v3(
            pin_pos.0 + (self.time_local * 5.0).cos() * 3.0 * pinasin + self.cloud_scroll_offset,
            pin_pos.1 + (self.time_local * 5.0).sin() * 1.5 * pinasin + t_y,
            0.0,
        );
        self.ic_matrix = self.mat_layer(&self.ic_pin_layer, 1.0, rotation);
        let (s, m) = (self.ic_pin.clone(), self.ic_matrix);
        self.draw_ts(&s, &m, color);

        // ── videokamera (faqat bulut) ──
        if ty == 1 {
            self.ic_videocam_layer.rotation = -30.0
                + self.t_local(self.anim_videocam_old_angle, self.anim_videocam_angle, self.anim_videocam_start_time, self.anim_videocam_duration, Tm::EaseOut);
            t_y = self.drop_y(common_delay + DC * 0.45 * begin_time_k, 0.0);
            if t_y <= 1.0 && self.time_local > self.anim_videocam_next_time {
                self.anim_videocam_duration = DC * self.frand(1.0, 1.5) * 1.5;
                self.anim_videocam_old_angle = self.anim_videocam_angle;
                self.anim_videocam_angle = 15.0 * self.irand(-1, 1) as f32;
                self.anim_videocam_start_time = self.time_local;
                self.anim_videocam_next_time = self.time_local + 1_000_000.0 + DC * self.frand(5.0, 8.0);
            }
            self.ic_videocam_layer.position = v3(-34.0 + self.cloud_scroll_offset, 40.0 + t_y, 0.0);
            self.ic_matrix = self.mat_layer(&self.ic_videocam_layer, 1.0, rotation);
            let (s, m) = (self.ic_videocam.clone(), self.ic_matrix);
            self.draw_ts(&s, &m, color);
        }

        // ── kamera ──
        let cam_pos;
        if ty == 0 {
            cam_pos = (53.0f32, 39.0f32);
            t_y = if page1_back { 0.0 } else { self.drop_y(common_delay + DC * 0.3 * begin_time_k, 0.0) };
        } else {
            cam_pos = (-14.0, -8.0);
            t_y = self.drop_y(common_delay + DC * 0.15 * begin_time_k, 0.0);
        }
        if t_y <= 1.0 && self.time_local > self.anim_cam_next_time {
            self.anim_cam_duration = DC * self.frand(1.0, 1.5);
            self.anim_cam_old_angle = self.anim_cam_angle;
            self.anim_cam_old_position = self.anim_cam_position;
            self.anim_cam_start_time = self.time_local;
            self.anim_cam_next_time = self.time_local + 10_000_000.0;
            let r = self.irand(0, 1);
            let sg = self.signrand() as f32;
            if r == 0 {
                self.anim_cam_position = (-8.0 + 4.0, 0.0);
                self.anim_cam_angle = sg * 10.0;
            } else if r == 1 {
                self.anim_cam_position = (4.0, -5.0);
                self.anim_cam_angle = sg * 10.0;
            }
            self.q_shot = self.irand(1, 2);
            self.anim_camshot_start_time = self.time_local + DC * 0.5;
            self.anim_camshot_duration = DC * 0.4;
        }
        self.ic_cam_layer.rotation =
            15.0 + self.t_local(self.anim_cam_old_angle, self.anim_cam_angle, self.anim_cam_start_time, self.anim_cam_duration, Tm::EaseOut);
        self.ic_cam_layer.position = v3(cam_pos.0 + self.cloud_scroll_offset, cam_pos.1 + t_y, 0.0);
        self.ic_matrix = self.mat_layer(&self.ic_cam_layer, 1.0, rotation);
        let (s, m) = (self.ic_cam.clone(), self.ic_matrix);
        self.draw_ts(&s, &m, color);

        let mut lens_scale = 1.0;
        if self.q_shot >= 0 && self.time_local > self.anim_camshot_start_time {
            lens_scale = self.t_local(1.0, 0.0, self.anim_camshot_start_time, self.anim_camshot_duration, Tm::Sin);
            if self.time_local > self.anim_camshot_start_time + self.anim_camshot_duration {
                self.q_shot -= 1;
                self.anim_camshot_start_time = self.time_local + self.anim_camshot_duration;
            }
        }
        self.ic_cam_lens.p.scale = v3(lens_scale, lens_scale, 1.0);
        self.ic_cam_lens.p.position = v3(0.0, 1.7, 0.0);
        let s = self.ic_cam_lens.clone();
        self.draw_ts(&s, &m, color);

        // ── tabassum ──
        let smile_pos;
        if ty == 0 {
            smile_pos = (35.0f32, -58.0f32);
            t_y = if page1_back { 0.0 } else { self.drop_y(common_delay + DC * 0.0 * begin_time_k, 0.0) };
        } else {
            smile_pos = (30.0, 25.0);
            t_y = self.drop_y(common_delay + DC * 0.25 * begin_time_k, pi8);
        }
        let mut smile_laught = 0.0;
        let fade = DC * 2.0;
        let dur = DC * 2.0;
        if self.anim_smile_stage == 0 {
            smile_laught = self.t_local(0.0, 1.0, self.anim_smile_start_time1, fade, Tm::Linear);
            if self.time_local > dur * 3.0 + self.anim_smile_start_time1 {
                self.anim_smile_stage = 1;
                self.anim_smile_start_time2 = self.time_local;
            }
        }
        if self.anim_smile_stage == 1 {
            smile_laught = self.t_local(1.0, 0.0, self.anim_smile_start_time2, fade, Tm::Linear);
            if self.time_local > dur + self.anim_smile_start_time2 {
                smile_laught = 0.0;
                self.anim_smile_stage = 2;
                self.anim_smile_blink_one = 1;
                self.anim_smile_blink_start_time = self.time_local + DC;
            }
        }
        let mut y = 0.0;
        if self.anim_smile_stage < 2 {
            y = (self.time_local * PI * 10.0).sin() * 1.5 * smile_laught;
        }
        self.ic_smile_layer.position = v3(smile_pos.0 + self.cloud_scroll_offset, y + smile_pos.1 + t_y, 0.0);
        self.ic_matrix = self.mat_layer(&self.ic_smile_layer, 1.0, rotation);
        let (s, m) = (self.ic_smile.clone(), self.ic_matrix);
        self.draw_ts(&s, &m, color);

        if self.time_local > self.anim_smile_blink_start_time + 0.1 {
            let mut blink_pause = self.frand(3.0, 6.0);
            if self.irand(0, 3) == 0 {
                blink_pause = 0.3;
            }
            if self.anim_smile_blink_one == 1 {
                blink_pause = self.frand(3.0, 6.0);
            }
            self.anim_smile_blink_start_time = self.time_local + blink_pause;
            self.anim_smile_blink_one = 0;
        }
        let stop_time = 5.0;
        let eye_scale = self.t_local(1.0, 0.0, self.anim_smile_blink_start_time, 0.1, Tm::Sin);
        self.ic_smile_eye.p.scale = v3(1.0, eye_scale, 1.0);
        if self.time > stop_time {
            self.ic_smile_eye.p.scale = v3(1.0, 1.0, 1.0);
        }
        self.ic_smile_eye.p.position = v3(-7.0, -4.5, 0.0);
        let s = self.ic_smile_eye.clone();
        self.draw_ts(&s, &m, color);
        if self.anim_smile_blink_one == 1 {
            self.ic_smile_eye.p.scale = v3(1.0, 1.0, 1.0);
        }
        if self.time > stop_time {
            self.ic_smile_eye.p.scale = v3(1.0, 1.0, 1.0);
        }
        self.ic_smile_eye.p.position = v3(7.0, -4.5, 0.0);
        let s = self.ic_smile_eye.clone();
        self.draw_ts(&s, &m, color);

        // ── pufak ──
        let bub_pos;
        if ty == 0 {
            bub_pos = (-30.0f32, 55.0f32);
            t_y = if page1_back { 0.0 } else { self.drop_y(common_delay + DC * 0.45 * begin_time_k, 0.0) };
        } else {
            bub_pos = (36.0, -37.0);
            t_y = self.drop_y(common_delay + DC * 0.0 * begin_time_k, 0.0);
        }
        self.ic_bubble_layer.position = v3(bub_pos.0 + self.cloud_scroll_offset, bub_pos.1 + t_y, 0.0);
        self.ic_matrix = self.mat_layer(&self.ic_bubble_layer, 1.0, rotation);
        let (s, m) = (self.ic_bubble.clone(), self.ic_matrix);
        self.draw_ts(&s, &m, color);

        let time = self.time;
        let mut scale = 0.7 + 0.2 * self.bubble_dots_sin(time * 10.0);
        self.ic_bubble_dot.p.scale = v3(scale, scale, scale);
        self.ic_bubble_dot.p.position = v3(0.0 - 80.5, -9.0 / 2.0, 0.0);
        let s = self.ic_bubble_dot.clone();
        self.draw_ts(&s, &m, light);

        scale = 0.7 + 0.2 * self.bubble_dots_sin(-PI * 2.0 / 3.0 + time * 10.0);
        if self.anim_bubble_dots_stage == 0 {
            scale = scale.max(0.7);
        }
        self.ic_bubble_dot.p.scale = v3(scale, scale, scale);
        self.ic_bubble_dot.p.position = v3(0.0, -9.0 / 2.0, 0.0);
        let s = self.ic_bubble_dot.clone();
        self.draw_ts(&s, &m, light);

        scale = 0.7 + 0.2 * self.bubble_dots_sin(-PI * 2.0 / 3.0 * 2.0 + time * 10.0);
        if self.anim_bubble_dots_stage == 0 {
            scale = scale.max(0.7);
        }
        self.ic_bubble_dot.p.scale = v3(scale, scale, scale);
        self.ic_bubble_dot.p.position = v3(0.0 + 80.5, -9.0 / 2.0, 0.0);
        let s = self.ic_bubble_dot.clone();
        self.draw_ts(&s, &m, light);

        // ── qalam ──
        let pen_pos;
        if ty == 0 {
            pen_pos = (-59.0f32, -37.0f32);
            t_y = if page1_back { 0.0 } else { self.drop_y(common_delay + DC * 0.1 * begin_time_k, 0.0) };
        } else {
            pen_pos = (59.0, 47.0);
            t_y = self.drop_y(common_delay + DC * 0.35 * begin_time_k, 0.0);
        }
        let mut pencil_x = 0.0;
        if self.anim_pencil_stage == 0 {
            self.ic_pencil_layer.rotation = self.t_local(0.0, -5.0, self.anim_pencil_start_all_time, DC * 0.5, Tm::EaseOut);
            pencil_x = self.t_local(0.0, 14.0, self.anim_pencil_start_time, 1.5 * 0.85, Tm::Linear);
            if self.time_local > self.anim_pencil_start_time + 1.5 * 0.85 {
                self.anim_pencil_start_time = self.time_local;
                self.anim_pencil_stage = 1;
            }
        } else if self.anim_pencil_stage == 1 {
            pencil_x = self.t_local(14.0, 0.0, self.anim_pencil_start_time, 1.5 * 0.15, Tm::Linear);
            if self.time_local > self.anim_pencil_start_time + 1.5 * 0.15 {
                if self.anim_pencil_period == 0 {
                    self.anim_pencil_start_all_end_time = self.time_local;
                    self.anim_pencil_start_time = self.time_local + DC;
                    self.anim_pencil_stage = 2;
                } else {
                    self.anim_pencil_period -= 1;
                    self.anim_pencil_start_time = self.time_local;
                    self.anim_pencil_stage = 0;
                }
            }
        } else if self.anim_pencil_stage == 2 {
            self.ic_pencil_layer.rotation = self.t_local(-5.0, 0.0, self.anim_pencil_start_all_end_time, DC * 0.5, Tm::EaseOut);
            if self.time_local > self.anim_pencil_start_time {
                self.anim_pencil_start_all_time = self.time_local;
                self.anim_pencil_start_time = self.time_local;
                self.anim_pencil_stage = 3;
            }
        }
        let pencil_v = if self.anim_pencil_stage < 2 { (self.time_local * 2.0 * PI * 4.0).sin() * 0.8 } else { 0.0 };
        self.ic_pencil_layer.position = v3(pencil_x + pen_pos.0 + self.cloud_scroll_offset, pencil_v + pen_pos.1 + t_y, 0.0);
        self.ic_matrix = self.mat_layer(&self.ic_pencil_layer, 1.0, rotation);
        let (s, m) = (self.ic_pencil.clone(), self.ic_matrix);
        self.draw_ts(&s, &m, color);
    }

    fn draw_safe(&mut self, alpha: f32, screw_alpha: f32) {
        let d = 53.0;
        self.private_screw.p.alpha = alpha * screw_alpha;
        for (x, y) in [(-d, -d), (d, -d), (-d, d), (d, d)] {
            self.private_screw.p.scale = v3(1.0, 1.0, 1.0);
            self.private_screw.p.position = v3(x, y, 0.0);
            let (s, m) = (self.private_screw.clone(), self.private_matrix);
            self.draw_ts(&s, &m, Prog::NormalOne);
        }
    }
}

// ───────────────────────── asosiy kadr (onDrawFrame) ─────────────────────────
impl Intro {
    fn cloud_masks(&mut self, reversed: bool, k0: f32) {
        // 4 ta bulut doirasi: (kechikish koeffitsienti, oxirgi nuqta, radius)
        let specs: [(f32, (f32, f32), f32); 4] = [
            (1.0, (-122.0 / 2.0, 54.0 / 2.0 - 1.0), 33.0),
            (1.15, (-84.0 / 2.0, -14.0), 23.0),
            (1.3, (128.0 / 2.0, 56.0 / 2.0), 31.0),
            (1.5, (0.0, 50.0), 64.0),
        ];
        for (i, (k, end, r)) in specs.iter().enumerate() {
            let kk = k * k0;
            let (px, py, sc) = if reversed {
                (
                    self.t_reversed(0.0, end.0, 0.0, DC * kk, Tm::EaseOut),
                    self.t_reversed(0.0, end.1, 0.0, DC * kk, Tm::EaseOut),
                    self.t_reversed(0.0, *r, 0.0, DC * kk, Tm::EaseOut),
                )
            } else {
                (self.t(0.0, end.0, 0.0, DC * kk, Tm::EaseOut), self.t(0.0, end.1, 0.0, DC * kk, Tm::EaseOut), self.t(0.0, *r, 0.0, DC * kk, Tm::EaseOut))
            };
            let z = self.cem[i].p.position.z;
            self.cem[i].p.position = v3(px, py, z);
            self.cem[i].p.scale = v3(sc, sc, 1.0);
            let (s, m) = (self.cem[i].clone(), self.main_matrix);
            self.draw_shape(&s, &m);
        }
    }

    /// Maskaga "chizilayotgan" cheksizlik (change_infinity + ikki marta, ikkinchisi teskari masshtabda).
    fn infinity_mask(&mut self, progress: f32, rot1: f32) {
        change_infinity(&mut self.infinity, progress);
        self.infinity.p.rotation = rot1;
        self.infinity.p.position.z = 1.0;
        self.infinity.p.position.y = -6.0;
        self.infinity.p.anchor = v3(52.75, 23.5, 0.0);
        let sc = 1.025;
        self.infinity.p.scale = v3(sc, sc, 1.0);
        self.draw_shape(&self.infinity, &self.main_matrix);
        self.infinity.p.scale = v3(-sc, -sc, 1.0);
        self.draw_shape(&self.infinity, &self.main_matrix);
    }

    fn on_draw_frame(&mut self, delta_ms: i32) {
        unsafe {
            gl::glClearColor(self.bg[0], self.bg[1], self.bg[2], self.bg[3]);
            gl::glClear(gl::COLOR_BUFFER_BIT);
        }
        self.time_local += delta_ms as f32 / 1000.0;

        if self.current_page != self.prev_page {
            self.reset_ic();
            self.ms0_anim = self.date;
            self.fps_anim = 0;
            self.count_anim_fps = 1;
        }
        let knot_delay_step = 0.075;
        if self.prev_page != self.current_page {
            for i in 0..4 {
                self.knot_delays[i] = (0.65 + knot_delay_step * i as f32) * DC;
            }
            for _ in 0..10 {
                let j1 = self.irand(0, 3).clamp(0, 3) as usize;
                let j2 = self.irand(0, 3).clamp(0, 3) as usize;
                self.knot_delays.swap(j1, j2);
            }
            if self.current_page == 2 {
                self.ic_pin_layer.rotation = -15.0;
                self.ic_cam_layer.rotation = 15.0;
                self.ic_smile_layer.rotation = -15.0;
                self.ic_bubble_layer.rotation = -15.0;
            }
            if self.current_page == 5 {
                self.ic_pin_layer.rotation = -15.0;
                self.ic_videocam_layer.rotation = -30.0;
                self.ic_cam_layer.rotation = 15.0;
                self.ic_smile_layer.rotation = -15.0;
                self.ic_bubble_layer.rotation = -15.0;
            }
        }
        self.fps_anim = (self.time_local / 0.016) as i32;
        if self.count_anim_fps == 1 && self.date - self.ms0_anim >= DC {
            self.count_anim_fps = 0;
        }
        if self.date - self.ms0 >= 1.0 {
            self.ms0 = self.date;
        }
        self.time = self.date - self.date0;
        let private_back_k = 0.8;
        let mm = self.main_matrix;
        let page = self.current_page;
        let dir = self.direct;

        // ═══ 1-qism: asosiy rasm va niqob o'lchamlari ═══
        if page == 0 {
            self.rgl_normal_draw();
            let mut alpha = 1.0;
            if dir == 0 {
                alpha = self.t(0.0, 1.0, 0.0, DC, Tm::Linear);
                self.fast_body.p.alpha = 1.0;
                self.fast_body.p.scale = v3(1.0, 1.0, 1.0);
                self.draw_ts(&self.fast_body, &mm, Prog::Normal);
            }
            self.telegram_sphere.p.alpha = alpha;
            self.telegram_sphere.p.scale = v3(1.0, 1.0, 1.0);
            self.telegram_plane.p.alpha = 1.0;
            let tt = (-PI * 125.0 / 180.0 + self.time * PI * 2.0 * 1.5).min(0.0);
            let dx = tt.sin() * 75.0;
            let dy = -tt.sin() * 60.0;
            self.telegram_plane.p.position = v3(dx, dy, 0.0);
            let sc = (tt.cos() + 1.0) * 0.5;
            self.telegram_plane.p.scale = v3(tt.cos() * sc, sc, 1.0);
            if tt < d2r(125.0) {
                self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
            }
        } else if page == 1 {
            self.rgl_normal_draw();
            if dir == 1 {
                self.fast_body.p.scale = v3(1.0, 1.0, 1.0);
                self.fast_body.p.alpha = 1.0;
                self.draw_ts(&self.fast_body, &mm, Prog::Normal);
            } else {
                self.fast_body.p.alpha = self.t(0.0, 1.0, 0.0, DC, Tm::Linear);
                let sc = self.t(0.95, 1.0, 0.0, DC, Tm::EaseInEaseOut);
                self.fast_body.p.scale = v3(sc, sc, 1.0);
                self.draw_ts(&self.fast_body, &mm, Prog::Normal);
            }
        } else if page == 2 {
            self.rgl_normal_draw();
            if dir == 1 {
                self.fast_body.p.alpha = self.t(1.0, 0.0, 0.0, DC, Tm::Linear);
                let sc = self.t(1.0, 0.95, 0.0, DC, Tm::EaseInEaseOut);
                self.fast_body.p.scale = v3(sc, sc, 1.0);
                self.draw_ts(&self.fast_body, &mm, Prog::Normal);
            }
        } else if page == 4 {
            self.private_layer.rotation = if dir == 1 {
                self.private_scroll_offset + self.t(-90.0, 0.0, 0.0, DC, Tm::EaseOut)
            } else {
                self.private_scroll_offset + self.t(90.0, 0.0, 0.0, DC * private_back_k, Tm::EaseOut)
            };
            self.private_matrix = self.mat_layer(&self.private_layer, 1.0, 0.0);
        }

        self.rgl_mask_draw();
        let z1 = 1.0;
        self.mask1.p.position.z = z1;
        for c in self.cem.iter_mut() {
            c.p.position.z = z1;
        }
        let r = 316.0 / 4.0;
        match page {
            0 => {
                if dir == 0 {
                    change_rounded_rectangle(&mut self.mask1, (R1 * 2.0, R1 * 2.0), R1);
                    self.mask1.p.rotation = 0.0;
                }
            }
            1 => {
                if dir == 1 {
                    change_rounded_rectangle(&mut self.mask1, (R1 * 2.0, R1 * 2.0), R1);
                    self.mask1.p.rotation = 0.0;
                } else {
                    let size = self.t(R2 * 2.0, R1 * 2.0, 0.0, DC, Tm::EaseInEaseOut);
                    let round = self.t(30.0, R1, 0.0, DC, Tm::EaseInEaseOut);
                    change_rounded_rectangle(&mut self.mask1, (size, size), round);
                    self.free_scroll_offset = 0.0;
                    self.mask1.p.rotation = self.t(180.0, 0.0, 0.0, DC, Tm::EaseInEaseOut) + self.free_scroll_offset;
                }
            }
            2 => {
                if dir == 1 {
                    let size = self.t(R1 * 2.0, R2 * 2.0, 0.0, DC, Tm::EaseInEaseOut);
                    let round = self.t(R1, 30.0, 0.0, DC, Tm::EaseInEaseOut);
                    change_rounded_rectangle(&mut self.mask1, (size, size), round);
                    self.free_scroll_offset = self.scroll_offset * 5.0;
                    self.mask1.p.rotation = self.t(0.0, 180.0, 0.0, DC, Tm::EaseInEaseOut) + self.free_scroll_offset;
                } else {
                    self.free_scroll_offset = self.scroll_offset * 5.0;
                    let size = self.t_reversed(R2 * 2.0, r * 2.0, 0.0, DC, Tm::EaseInEaseOut);
                    let round = self.t_reversed(30.0, 20.0, 0.0, DC, Tm::EaseInEaseOut);
                    change_rounded_rectangle(&mut self.mask1, (size, size), round);
                    self.mask1.p.rotation = self.t_reversed(180.0 + self.free_scroll_offset, 180.0 + 90.0, 0.0, DC, Tm::EaseInEaseOut);
                }
            }
            3 => {
                if dir == 1 {
                    let size = self.t(R2 * 2.0, r * 2.0, 0.0, DC, Tm::EaseInEaseOut);
                    let round = self.t(30.0, 20.0, 0.0, DC, Tm::EaseInEaseOut);
                    change_rounded_rectangle(&mut self.mask1, (size, size), round);
                    self.mask1.p.rotation = self.t(180.0 + self.free_scroll_offset, 180.0 + 90.0, 0.0, DC, Tm::EaseInEaseOut);
                } else {
                    let size = self.t_reversed(r * 2.0, R2 * 2.0, 0.0, DC, Tm::EaseOut);
                    let round = self.t_reversed(20.0, 30.0, 0.0, DC, Tm::EaseOut);
                    change_rounded_rectangle(&mut self.mask1, (size, size), round);
                    self.mask1.p.rotation = self.t_reversed(180.0 + 90.0, 180.0 + 90.0 + 90.0, 0.0, DC, Tm::EaseOut);
                    let z = self.mask1.p.position.z;
                    self.mask1.p.position = v3(0.0, 0.0, z);
                }
            }
            4 => {
                if dir == 1 {
                    let size = self.t(r * 2.0, R2 * 2.0, 0.0, DC, Tm::EaseOut);
                    let round = self.t(20.0, 30.0, 0.0, DC, Tm::EaseOut);
                    change_rounded_rectangle(&mut self.mask1, (size, size), round);
                    self.mask1.p.rotation = self.private_scroll_offset + self.t(180.0 + 90.0, 180.0 + 90.0 + 90.0, 0.0, DC, Tm::EaseOut);
                    let z = self.mask1.p.position.z;
                    self.mask1.p.position = v3(0.0, 0.0, z);
                } else {
                    let k = 1.0 * private_back_k;
                    let sc = self.t_reversed(R2 * 2.0, 100.0, 0.0, DC * k, Tm::EaseOut);
                    let rr = self.t_reversed(30.0, 50.0, 0.0, DC * k, Tm::EaseOut);
                    change_rounded_rectangle(&mut self.mask1, (sc, sc), rr);
                    let z = self.mask1.p.position.z;
                    // C'da `29 / 2` va `-19 / 2` butun bo'linma: 14 va -9
                    self.mask1.p.position = v3(self.t_reversed(0.0, 14.0, 0.0, DC * k, Tm::EaseOut), self.t_reversed(0.0, -9.0, 0.0, DC * k, Tm::EaseOut), z);
                    self.mask1.p.rotation = self.private_scroll_offset + self.t_reversed(180.0 + 90.0 + 90.0, 180.0 + 90.0 + 90.0 + 90.0, 0.0, DC * k, Tm::EaseOut);
                    self.cloud_masks(true, private_back_k);
                }
            }
            5 => {
                let k = 0.8;
                let sc = self.t(R2 * 2.0, 100.0, 0.0, DC * k, Tm::EaseOut);
                let rr = self.t(30.0, 50.0, 0.0, DC * k, Tm::EaseOut);
                change_rounded_rectangle(&mut self.mask1, (sc, sc), rr);
                let z = self.mask1.p.position.z;
                self.mask1.p.position = v3(self.t(0.0, 14.0, 0.0, DC * k, Tm::EaseOut), self.t(0.0, -9.0, 0.0, DC * k, Tm::EaseOut), z);
                self.mask1.p.rotation = self.t(180.0 + 90.0 + 90.0, 180.0 + 90.0 + 90.0 + 90.0, 0.0, DC * k, Tm::EaseOut);
                self.cloud_masks(false, 1.0);
            }
            _ => {}
        }
        let (mask, mm2) = (self.mask1.clone(), self.main_matrix);
        self.draw_shape(&mask, &mm2);

        // ═══ 2-qism: maska orqali chiziladigan sahna ═══
        let (rr, seg, ang) = (30.0f32, 15.0f32, 180.0f32);
        self.rgl_normal_draw_through_mask();
        match page {
            0 => {
                if dir == 0 {
                    self.blend_on();
                    self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                    let end = d2r(self.speedometer_scroll_offset + self.calculated_speedometer_sin + self.t(-seg + ang, 0.0, 0.0, DC, Tm::EaseOut));
                    change_segmented_square(&mut self.spiral, R1, d2r(rr + seg), end);
                    self.spiral.p.scale = v3(1.0, 1.0, 1.0);
                    self.spiral.p.rotation = self.t(180.0, 0.0, 0.0, DC, Tm::EaseOut);
                    self.spiral.p.alpha = self.t(1.0, 0.0, 0.0, DC, Tm::Linear);
                    self.draw_ts(&self.spiral, &mm, Prog::NormalOne);
                    let a = self.t(1.0, 0.0, 0.0, DC, Tm::Linear);
                    self.fast_arrow.p.alpha = a;
                    self.fast_arrow_shadow.p.alpha = a;
                    let rot = self.t(rr, rr - 180.0 - 160.0, 0.0, DC, Tm::EaseOut) + self.speedometer_scroll_offset + self.calculated_speedometer_sin;
                    self.fast_arrow.p.rotation = rot;
                    self.fast_arrow_shadow.p.rotation = rot;
                    self.draw_ts(&self.fast_arrow_shadow, &mm, Prog::NormalOne);
                    self.draw_ts(&self.fast_arrow, &mm, Prog::NormalOne);
                }
            }
            1 => {
                self.blend_on();
                self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                self.speedometer_scroll_offset = self.scroll_offset * 25.0;
                self.calculated_speedometer_sin = 0.0;
                if dir == 1 {
                    let arg = self.time * 50.0;
                    let value = 180.0 - 180.0 * 2.71f32.powf(-0.055 * arg * 2.0) * (0.08 * arg * 3.0).cos();
                    let ta = self.t(0.0, 180.0, 0.0, DC, Tm::EaseOut);
                    change_segmented_square(&mut self.spiral, R1, d2r(rr + seg), d2r(-seg + value + self.speedometer_scroll_offset));
                    self.spiral.p.scale = v3(1.0, 1.0, 1.0);
                    self.spiral.p.rotation = ta;
                    self.spiral.p.alpha = self.t(0.0, 1.0, 0.0, DC, Tm::Linear);
                    self.draw_ts(&self.spiral, &mm, Prog::NormalOne);
                    let a = self.t(0.0, 1.0, 0.0, DC, Tm::Linear);
                    self.fast_arrow.p.alpha = a;
                    self.fast_arrow_shadow.p.alpha = a;
                    let rot = -330.0 + value + ta + self.speedometer_scroll_offset;
                    self.fast_arrow.p.rotation = rot;
                    self.fast_arrow_shadow.p.rotation = rot;
                    self.draw_ts(&self.fast_arrow_shadow, &mm, Prog::NormalOne);
                    self.draw_ts(&self.fast_arrow, &mm, Prog::NormalOne);
                } else {
                    self.spiral.p.alpha = 1.0;
                    self.fast_arrow.p.alpha = 1.0;
                    self.fast_arrow_shadow.p.alpha = 1.0;
                    let arg = self.time * 50.0;
                    let dangle = 90.0;
                    let mut value = 180.0 - 90.0 - (180.0 - 90.0) * 2.71f32.powf(-0.055 * arg * 2.0) * (0.08 * arg * 3.0).cos();
                    value *= -1.0;
                    let end = d2r(self.speedometer_scroll_offset + value + self.calculated_speedometer_sin + self.t(360.0, 360.0 - dangle - seg, 0.0, DC, Tm::EaseInEaseOut));
                    change_segmented_square(&mut self.spiral, R1, d2r(rr + seg), end);
                    let sc = self.t(1.18, 1.0, 0.0, DC, Tm::EaseInEaseOut);
                    self.spiral.p.scale = v3(sc, sc, 1.0);
                    self.spiral.p.rotation = self.t(360.0, 180.0, 0.0, DC, Tm::EaseInEaseOut);
                    self.draw_ts(&self.spiral, &mm, Prog::Normal);
                    let rot = self.speedometer_scroll_offset + value + self.calculated_speedometer_sin + self.t(rr + 360.0 + 6.0, rr + 360.0 - 180.0 - dangle, 0.0, DC, Tm::EaseInEaseOut);
                    self.fast_arrow.p.rotation = rot;
                    self.fast_arrow_shadow.p.rotation = rot;
                    self.draw_ts(&self.fast_arrow_shadow, &mm, Prog::Normal);
                    self.draw_ts(&self.fast_arrow, &mm, Prog::Normal);
                    self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                    self.free_bg.p.alpha = self.t(1.0, 0.0, 0.0, DC, Tm::Linear);
                    self.draw_shape(&self.free_bg, &mm);
                    self.draw_ic(0);
                }
            }
            2 => {
                self.blend_on();
                self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                if dir == 1 {
                    self.spiral.p.alpha = 1.0;
                    self.fast_arrow.p.alpha = 1.0;
                    self.fast_arrow_shadow.p.alpha = 1.0;
                    let end = d2r(self.t(-seg + ang, 360.0, 0.0, DC, Tm::EaseInEaseOut));
                    change_segmented_square(&mut self.spiral, R1, d2r(rr + seg + self.speedometer_scroll_offset), end);
                    let sc = self.t(1.0, 1.18, 0.0, DC, Tm::EaseInEaseOut);
                    self.spiral.p.scale = v3(sc, sc, 1.0);
                    self.spiral.p.rotation = self.t(180.0, 360.0, 0.0, DC, Tm::EaseInEaseOut);
                    self.draw_ts(&self.spiral, &mm, Prog::Normal);
                    let rot = self.speedometer_scroll_offset + self.t(rr, rr + 360.0 + 6.0, 0.0, DC, Tm::EaseInEaseOut);
                    self.fast_arrow.p.rotation = rot;
                    self.fast_arrow_shadow.p.rotation = rot;
                    self.draw_ts(&self.fast_arrow_shadow, &mm, Prog::Normal);
                    self.draw_ts(&self.fast_arrow, &mm, Prog::Normal);
                    self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                    self.free_bg.p.alpha = self.t(0.0, 1.0, 0.0, DC, Tm::Linear);
                    self.draw_shape(&self.free_bg, &mm);
                    self.draw_ic(0);
                } else {
                    self.blend_off();
                    self.free_bg.p.alpha = 1.0;
                    self.draw_shape(&self.free_bg, &mm);
                    self.blend_on();
                    self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                    self.draw_ic(0);
                    self.powerful_bg.p.alpha = self.t_reversed(0.0, 1.0, 0.0, DC, Tm::Linear);
                    self.draw_shape(&self.powerful_bg, &mm);
                }
                self.ribbons[0].p.rotation = 0.0;
                self.ribbons[1].p.rotation = 90.0;
                self.ribbons[2].p.rotation = 180.0;
                self.ribbons[3].p.rotation = 270.0;
            }
            3 => {
                if dir == 1 {
                    self.blend_off();
                    self.free_bg.p.alpha = 1.0;
                    self.draw_shape(&self.free_bg, &mm);
                    self.blend_on();
                    self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                    self.powerful_bg.p.alpha = self.t(0.0, 1.0, 0.0, DC, Tm::Linear);
                    self.draw_shape(&self.powerful_bg, &mm);
                    self.draw_stars();
                } else {
                    self.blend_off();
                    self.private_bg.p.alpha = 1.0;
                    self.draw_shape(&self.private_bg, &mm);
                    let a = self.t(0.0, 1.0, 0.0, DC, Tm::EaseOut);
                    self.blend_on();
                    self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                    self.powerful_bg.p.rotation = 0.0;
                    self.powerful_bg.p.alpha = a;
                    self.draw_shape(&self.powerful_bg, &mm);
                    self.draw_stars();
                }
            }
            4 => {
                if dir == 1 {
                    self.blend_off();
                    self.powerful_bg.p.alpha = 1.0;
                    self.draw_shape(&self.powerful_bg, &mm);
                    let a = self.t(0.0, 1.0, 0.0, DC, Tm::EaseOut);
                    self.blend_on();
                    self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                    self.private_bg.p.rotation = self.t(45.0, 0.0, 0.0, DC, Tm::EaseOut);
                    self.private_bg.p.alpha = a;
                    self.draw_shape(&self.private_bg, &mm);
                } else {
                    self.blend_off();
                    self.cloud_bg.p.alpha = 1.0;
                    self.draw_shape(&self.cloud_bg, &mm);
                    let a = self.t(0.0, 1.0, 0.0, DC * private_back_k, Tm::EaseOut);
                    self.blend_on();
                    self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                    self.private_bg.p.alpha = a;
                    self.draw_shape(&self.private_bg, &mm);
                }
            }
            5 => {
                self.blend_off();
                self.private_bg.p.alpha = 1.0;
                self.draw_shape(&self.private_bg, &mm);
                let a = self.t(0.0, 1.0, 0.0, DC, Tm::EaseOut);
                self.blend_on();
                self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                self.cloud_bg.p.alpha = a;
                self.draw_shape(&self.cloud_bg, &mm);
                self.cloud_scroll_offset = if self.scroll_offset > 0.0 { -self.scroll_offset * 40.0 } else { -self.scroll_offset * 15.0 };
                self.draw_ic(1);
            }
            _ => {}
        }
        self.on_draw_frame_top(private_back_k);
        self.prev_page = self.current_page;
    }
}

// ───────────────────────── 3-qism: ustki qatlam (sfera, lentalar, cheksizlik, eshik, bulut) ─────────────────────────
impl Intro {
    fn on_draw_frame_top(&mut self, private_back_k: f32) {
        let mm = self.main_matrix;
        let page = self.current_page;
        let dir = self.direct;
        match page {
            0 => {
                self.rgl_normal_draw();
                if dir == 0 {
                    self.telegram_sphere.p.alpha = self.t(0.0, 1.0, 0.0, DC * 0.8, Tm::Linear);
                    self.telegram_sphere.p.scale = v3(1.0, 1.0, 1.0);
                    self.draw_ts(&self.telegram_sphere, &mm, Prog::Normal);
                    let tt = (-PI * 125.0 / 180.0 + self.time * PI * 2.0 * 1.5).min(0.0);
                    let dx = self.t(-75.0, 0.0, 0.0, 0.15, Tm::EaseIn);
                    let dy = self.t(75.0, 0.0, 0.0, 0.15, Tm::EaseIn);
                    self.telegram_plane.p.position = v3(dx, dy, 0.0);
                    let sc = self.t(0.1, 1.0, 0.03, 0.15, Tm::EaseOut);
                    self.telegram_plane.p.scale = v3(sc, sc, 1.0);
                    if tt < d2r(125.0) {
                        self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                        self.draw_ts(&self.telegram_plane, &mm, Prog::NormalOne);
                        self.draw_ts(&self.telegram_mask, &mm, Prog::Normal);
                    }
                }
            }
            1 => {
                self.rgl_normal_draw();
                if dir == 1 {
                    self.telegram_sphere.p.alpha = self.t(1.0, 0.0, 0.0, DC, Tm::Linear);
                    self.draw_ts(&self.telegram_sphere, &mm, Prog::Normal);
                    let tt = self.time * PI * 2.0 * 1.5;
                    let dx = self.t(0.0, 75.0, 0.0, 0.15, Tm::EaseOut);
                    let dy = self.t(0.0, -75.0, 0.0, 0.15, Tm::EaseOut);
                    self.telegram_plane.p.position = v3(dx, dy, 0.0);
                    let sc = self.t(1.0, 0.1, 0.03, 0.15, Tm::EaseOut);
                    self.telegram_plane.p.scale = v3(sc, sc, 1.0);
                    if tt < d2r(125.0) {
                        self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                        self.draw_ts(&self.telegram_plane, &mm, Prog::NormalOne);
                        self.draw_ts(&self.telegram_mask, &mm, Prog::Normal);
                    }
                }
            }
            2 => {
                self.rgl_normal_draw();
                let dribbon = 87.0;
                if dir == 1 {
                    self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                    self.ribbon_layer.rotation = self.scroll_offset * 5.0 + self.t(180.0, 360.0, 0.0, DC, Tm::EaseInEaseOut);
                    self.ribbons_layer = self.mat_layer(&self.ribbon_layer, 1.0, 0.0);
                    let dur = DC * 0.5;
                    let rl = self.ribbons_layer;
                    // tugunlar
                    let kd = self.knot_delays;
                    self.knots[0].p.position = v3(5.0, -5.0 - 9.0, 0.0);
                    let sc = self.t(0.0, 1.0, kd[0], dur, Tm::EaseOut);
                    self.knots[0].p.scale = v3(sc, sc, 1.0);
                    self.draw_ts(&self.knots[0], &rl, Prog::NormalOne);
                    self.knots[1].p.position = v3(-5.0, -5.0 - 9.0, 0.0);
                    let sc = self.t(0.0, 1.0, kd[1], dur, Tm::EaseOut);
                    self.knots[1].p.scale = v3(-sc, sc, 1.0);
                    self.draw_ts(&self.knots[1], &rl, Prog::NormalOne);
                    self.knots[2].p.position = v3(-5.0, 5.0 - 9.0, 0.0);
                    let sc = self.t(0.0, 1.0, kd[2], dur, Tm::EaseOut);
                    self.knots[2].p.scale = v3(-sc, sc, 1.0);
                    self.draw_ts(&self.knots[2], &rl, Prog::NormalOne);
                    self.knots[2].p.position = v3(5.0, 5.0 - 9.0, 0.0);
                    let sc = self.t(0.0, 1.0, kd[3], dur, Tm::EaseOut);
                    self.knots[2].p.scale = v3(sc, sc, 1.0);
                    self.draw_ts(&self.knots[2], &rl, Prog::NormalOne);

                    self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                    let a = self.t(0.0, 1.0, 0.0, dur, Tm::EaseInEaseOut);
                    for i in 0..4 {
                        self.ribbons[i].p.alpha = a;
                    }
                    let ribbon_k = if self.time > DC { 1.0 } else { 0.0 };
                    let rlen = self.ribbon_length;
                    let fso = self.free_scroll_offset;
                    change_ribbon(&mut self.ribbons[0], rlen - 8.0 * ribbon_k - fso / 5.0 * (30.0 - 8.0 * ribbon_k));
                    self.ribbons[0].p.position.x = self.scroll_offset * 30.0 * 0.0 + self.t(-dribbon, 0.0, 0.0, DC, Tm::EaseInEaseOut);
                    self.draw_shape(&self.ribbons[0], &rl);
                    change_ribbon(&mut self.ribbons[1], rlen - 10.0 * ribbon_k - fso / 5.0 * (22.0 - 10.0 * ribbon_k));
                    self.ribbons[1].p.position.y = self.scroll_offset * 15.0 + self.t(-9.0 - dribbon, -9.0, 0.0, DC, Tm::EaseInEaseOut);
                    self.draw_shape(&self.ribbons[1], &rl);
                    self.ribbons[2].p.position.x = self.t(dribbon, 0.0, 0.0, DC, Tm::EaseInEaseOut);
                    self.draw_shape(&self.ribbons[2], &rl);
                    self.ribbons[3].p.position.y = self.t(-9.0 + dribbon, -9.0, 0.0, DC, Tm::EaseInEaseOut);
                    self.draw_shape(&self.ribbons[3], &rl);
                    self.ribbon_layer.anchor.y = 0.0;
                    self.ribbon_layer.position.y = 0.0;
                    for i in 0..4 {
                        change_ribbon(&mut self.ribbons[i], rlen);
                    }
                } else {
                    self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                    let sc = self.t(1.0, 2.0, 0.0, DC, Tm::EaseIn);
                    self.powerful_mask.p.scale = v3(sc, sc, 1.0);
                    self.draw_ts(&self.powerful_mask, &mm, Prog::NormalOne);
                    self.ribbon_layer.rotation = self.free_scroll_offset + self.t_reversed(360.0, 360.0 + (45.0 + 30.0), 0.0, DC, Tm::EaseOut);
                    self.ribbon_layer.position.y = self.t_reversed(0.0, -8.0, 0.0, DC * 0.8, Tm::EaseOut);
                    self.ribbon_layer.anchor.y = self.t_reversed(0.0, -9.0, 0.0, DC * 0.8, Tm::EaseOut);
                    self.ribbons_layer = self.mat_layer(&self.ribbon_layer, 1.0, 0.0);
                    let rl = self.ribbons_layer;
                    let dur = DC * 0.5;
                    let kd = self.knot_delays;
                    // C'da `11 / 2` butun bo'linma = 5
                    self.knots[0].p.position = v3(5.0, -5.0 - 9.0, 0.0);
                    let sc = self.t(0.0, 1.0, kd[0], dur, Tm::EaseOut);
                    self.knots[0].p.scale = v3(sc, sc, 1.0);
                    self.draw_ts(&self.knots[0], &rl, Prog::NormalOne);
                    self.knots[1].p.position = v3(-5.0, -5.0 - 9.0, 0.0);
                    let sc = self.t(0.0, 1.0, kd[1], dur, Tm::EaseOut);
                    self.knots[1].p.scale = v3(-sc, sc, 1.0);
                    self.draw_ts(&self.knots[1], &rl, Prog::NormalOne);
                    self.knots[2].p.position = v3(-5.0, 5.0 - 9.0, 0.0);
                    let sc = self.t(0.0, 1.0, kd[2], dur, Tm::EaseOut);
                    self.knots[2].p.scale = v3(-sc, sc, 1.0);
                    self.draw_ts(&self.knots[2], &rl, Prog::NormalOne);
                    self.knots[2].p.position = v3(5.0, 5.0 - 9.0, 0.0);
                    let sc = self.t(0.0, 1.0, kd[3], dur, Tm::EaseOut);
                    self.knots[2].p.scale = v3(sc, sc, 1.0);
                    self.draw_ts(&self.knots[2], &rl, Prog::NormalOne);

                    let a1 = -25.0;
                    self.ribbons[0].p.rotation = self.t_reversed(0.0, a1, 0.0, DC, Tm::EaseOut);
                    self.ribbons[2].p.rotation = self.t_reversed(180.0, 180.0 + a1, 0.0, DC, Tm::EaseOut);
                    let a2 = 0.0;
                    self.ribbons[1].p.rotation = self.t_reversed(90.0, 90.0 + a2, 0.0, DC, Tm::EaseOut);
                    self.ribbons[3].p.rotation = self.t_reversed(270.0, 270.0 + a2, 0.0, DC, Tm::EaseOut);
                    let k = 0.9;
                    let al = self.t_reversed(1.0, 0.0, DC * 0.5, DC * 0.1, Tm::Linear);
                    self.ribbons[1].p.alpha = al;
                    self.ribbons[3].p.alpha = al;
                    let rlen = self.ribbon_length;
                    let fso = self.free_scroll_offset;
                    let l0 = self.t_reversed(rlen - 8.0 * 0.0, 0.0, 0.0, DC * 0.9, Tm::Linear) - fso / 5.0 * (30.0 - 8.0 * 0.0);
                    change_ribbon(&mut self.ribbons[0], l0);
                    self.ribbons[0].p.position.x = 0.0;
                    self.draw_shape(&self.ribbons[0], &rl);
                    let l1 = self.t_reversed(rlen - 10.0 * 0.0, 0.0, 0.0, DC * k, Tm::Linear) - fso / 5.0 * (22.0 - 10.0 * 0.0);
                    change_ribbon(&mut self.ribbons[1], l1);
                    self.ribbons[1].p.position.y = self.scroll_offset * 15.0 + -9.0;
                    self.draw_shape(&self.ribbons[1], &rl);
                    let l2 = self.t_reversed(rlen, 0.0, 0.0, DC * 0.9, Tm::Linear);
                    change_ribbon(&mut self.ribbons[2], l2);
                    self.draw_shape(&self.ribbons[2], &rl);
                    let l3 = self.t_reversed(rlen, 0.0, DC * 0.6 * 0.0, DC * k, Tm::Linear);
                    change_ribbon(&mut self.ribbons[3], l3);
                    self.draw_shape(&self.ribbons[3], &rl);

                    let inf_dur_k = 1.3;
                    self.rgl_mask_draw();
                    let prog = self.t_reversed(0.0, 0.99, 0.0, DC * inf_dur_k, Tm::EaseOut);
                    let rot1 = self.t(0.0, -50.0, DC * 0.5, DC * 0.8, Tm::EaseOut);
                    let rot2 = self.t(0.0, -30.0, DC * 0.8, DC, Tm::EaseOut);
                    self.infinity_mask(prog, rot1);
                    self.rgl_normal_draw_through_mask();
                    self.blend_on();
                    self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                    self.powerful_infinity_white.p.rotation = rot1 + rot2;
                    self.powerful_infinity_white.p.alpha = 1.0;
                    self.powerful_infinity_white.p.position.y = -6.0;
                    self.draw_ts(&self.powerful_infinity_white, &mm, Prog::NormalOne);
                }
            }
            3 => {
                if dir == 1 {
                    self.ribbons[0].p.position.x = 0.0;
                    self.ribbons[1].p.position.y = -9.0;
                    self.ribbons[2].p.position.x = 0.0;
                    self.ribbons[3].p.position.y = -9.0;
                    self.rgl_normal_draw();
                    self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                    let sc = self.t(2.0, 1.0, 0.0, DC, Tm::EaseOut);
                    self.powerful_mask.p.scale = v3(sc, sc, 1.0);
                    self.draw_ts(&self.powerful_mask, &mm, Prog::NormalOne);
                    self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                    self.ribbon_layer.rotation = self.free_scroll_offset + self.t(360.0, 360.0 + (45.0 + 30.0), 0.0, DC * 0.8, Tm::EaseOut);
                    self.ribbon_layer.position.y = self.t(0.0, -8.0, 0.0, DC * 0.8, Tm::EaseOut);
                    self.ribbon_layer.anchor.y = self.t(0.0, -9.0, 0.0, DC * 0.8, Tm::EaseOut);
                    self.ribbons_layer = self.mat_layer(&self.ribbon_layer, 1.0, 0.0);
                    let rl = self.ribbons_layer;
                    let a1 = -25.0;
                    self.ribbons[0].p.rotation = self.t(0.0, a1, 0.0, DC, Tm::EaseOut);
                    self.ribbons[2].p.rotation = self.t(180.0, 180.0 + a1, 0.0, DC, Tm::EaseOut);
                    let a2 = 0.0;
                    self.ribbons[1].p.rotation = self.t(90.0, 90.0 + a2, 0.0, DC, Tm::EaseOut);
                    self.ribbons[3].p.rotation = self.t(270.0, 270.0 + a2, 0.0, DC, Tm::EaseOut);
                    let k = 0.5;
                    let al = self.t(1.0, 0.0, DC * k * 0.5, DC * k * 0.1, Tm::Linear);
                    self.ribbons[1].p.alpha = al;
                    self.ribbons[3].p.alpha = al;
                    let ribbon_k = if self.time > DC { 1.0 } else { 0.0 };
                    let rlen = self.ribbon_length;
                    let fso = self.free_scroll_offset;
                    let l0 = self.t(rlen - 8.0 * ribbon_k - fso / 5.0 * (30.0 - 8.0 * ribbon_k), 0.0, 0.0, DC * 0.9, Tm::Linear);
                    change_ribbon(&mut self.ribbons[0], l0);
                    self.draw_shape(&self.ribbons[0], &rl);
                    let l1 = self.t(rlen - 10.0 * ribbon_k - fso / 5.0 * (22.0 - 10.0 * ribbon_k), 0.0, 0.0, DC * k, Tm::Linear);
                    change_ribbon(&mut self.ribbons[1], l1);
                    self.draw_shape(&self.ribbons[1], &rl);
                    let l2 = self.t(rlen, 0.0, 0.0, DC * 0.9, Tm::Linear);
                    change_ribbon(&mut self.ribbons[2], l2);
                    self.draw_shape(&self.ribbons[2], &rl);
                    let l3 = self.t(rlen, 0.0, 0.0, DC * k, Tm::Linear);
                    change_ribbon(&mut self.ribbons[3], l3);
                    self.draw_shape(&self.ribbons[3], &rl);
                    self.infinity_or_final(1.1);
                } else {
                    self.rgl_normal_draw();
                    self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                    let sc = self.t(2.0, 1.0, 0.0, DC, Tm::EaseOut);
                    self.powerful_mask.p.scale = v3(sc, sc, 1.0);
                    self.draw_ts(&self.powerful_mask, &mm, Prog::NormalOne);
                    self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                    let sc = self.t(1.0, 2.0, 0.0, DC, Tm::EaseOut);
                    self.private_stroke.p.scale = v3(sc, sc, 1.0);
                    self.private_stroke.p.rotation = self.t(0.0, -90.0, 0.0, DC, Tm::EaseOut);
                    self.private_stroke.p.alpha = self.t(1.0, 0.0, 0.0, DC, Tm::Linear);
                    self.private_stroke.p.position = v3(0.0, self.t(0.0, -6.0, 0.0, DC, Tm::EaseOut), 0.0);
                    let sc = self.t_reversed(63.0 * 2.0, 63.0 * 2.0, 0.0, DC, Tm::EaseOut);
                    change_rounded_rectangle_stroked(&mut self.private_stroke, (sc, sc), sc / 2.0);
                    self.draw_shape(&self.private_stroke, &mm);
                    self.infinity_or_final(1.1);
                }
            }
            4 => {
                self.private_stroke.p.scale = v3(1.0, 1.0, 1.0);
                self.private_scroll_offset = self.scroll_offset * 5.0;
                self.rgl_normal_draw();
                self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                let sc = self.t(1.0, 2.0, 0.0, DC, Tm::EaseOut);
                if sc < 1.5 {
                    self.powerful_mask.p.scale = v3(sc, sc, 1.0);
                }
                self.private_layer.rotation = if dir == 1 {
                    self.private_scroll_offset + self.t(-90.0, 0.0, 0.0, DC, Tm::EaseOut)
                } else {
                    self.private_scroll_offset + self.t(90.0, 0.0, 0.0, DC * private_back_k, Tm::EaseOut)
                };
                self.private_matrix = self.mat_layer(&self.private_layer, 1.0, 0.0);
                if dir == 1 {
                    self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                    self.powerful_infinity.p.position.y = -6.0;
                    self.powerful_infinity.p.alpha = self.t(1.0, 0.0, 0.0, DC * 0.25, Tm::EaseIn);
                    self.draw_ts(&self.powerful_infinity, &mm, Prog::NormalOne);
                }
                self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                if dir == 1 {
                    self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                    let sc = self.t(0.5, 1.0, 0.0, DC, Tm::EaseOut);
                    self.private_door.p.scale = v3(sc, sc, 1.0);
                    self.private_door.p.alpha = self.t(0.0, 1.0, 0.0, DC, Tm::EaseOut);
                    self.draw_ts(&self.private_door, &mm, Prog::NormalOne);
                    self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                    self.private_stroke.p.rotation = self.private_scroll_offset;
                    self.private_stroke.p.alpha = 1.0;
                    self.private_stroke.p.position = v3(0.0, 0.0, 0.0);
                    let sc = self.t(63.0, 63.0 * 2.0, 0.0, DC, Tm::EaseOut);
                    change_rounded_rectangle_stroked(&mut self.private_stroke, (sc, sc), sc / 2.0);
                    self.draw_shape(&self.private_stroke, &mm);
                    self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                    let sa = self.t(0.0, 1.0, 0.0, DC, Tm::Linear);
                    self.draw_safe(1.0, sa);
                } else {
                    self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                    let sc = self.t(0.5, 1.0, 0.0, DC * private_back_k, Tm::EaseOut);
                    self.private_door.p.scale = v3(sc, sc, 1.0);
                    self.private_door.p.alpha = self.t(0.0, 1.0, 0.0, DC * private_back_k, Tm::EaseOut);
                    self.draw_ts(&self.private_door, &mm, Prog::NormalOne);
                    self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                    self.private_stroke.p.rotation = self.private_scroll_offset;
                    self.private_stroke.p.alpha = self.t(0.0, 1.0, 0.0, DC * 0.25, Tm::Linear);
                    self.private_stroke.p.position = v3(0.0, 0.0, 0.0);
                    let sc = self.t(63.0, 63.0 * 2.0, 0.0, DC * private_back_k, Tm::EaseOut);
                    change_rounded_rectangle_stroked(&mut self.private_stroke, (sc, sc), sc / 2.0);
                    self.draw_shape(&self.private_stroke, &mm);
                    self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                    if self.time < DC * 0.4 {
                        self.cloud_cover.p.position.y = self.t_reversed(118.0 / 2.0 + 50.0, 118.0 / 2.0, DC * 0.8 * private_back_k, DC * private_back_k, Tm::EaseOut);
                        self.draw_shape(&self.cloud_cover, &mm);
                    }
                    let a = self.t(0.0, 1.0, DC * private_back_k * 0.0, DC * private_back_k, Tm::Linear);
                    let sa = self.t(0.0, 1.0, 0.0, DC, Tm::Linear);
                    self.draw_safe(a, sa);
                }
            }
            5 => {
                let private_fade_k = 0.5;
                self.rgl_normal_draw();
                self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                self.private_door.p.scale = v3(1.0, 1.0, 1.0);
                self.private_door.p.alpha = self.t(1.0, 0.0, 0.0, DC * private_fade_k * 0.5, Tm::EaseOut);
                self.draw_ts(&self.private_door, &mm, Prog::NormalOne);
                self.blend(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                self.private_stroke.p.rotation = self.private_scroll_offset;
                self.private_stroke.p.alpha = self.t(1.0, 0.0, 0.0, DC * private_fade_k * 0.5, Tm::EaseOut);
                let sc = self.t(244.0 / 2.0, R2 * 2.0, 0.0, DC, Tm::EaseOut);
                change_rounded_rectangle_stroked(&mut self.private_stroke, (sc, sc), sc / 2.0);
                self.draw_shape(&self.private_stroke, &mm);
                self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
                self.private_layer.rotation = self.private_scroll_offset;
                let s = self.t(1.0, 0.9, 0.0, DC * private_fade_k, Tm::EaseOut);
                self.private_matrix = self.mat_layer(&self.private_layer, s, 0.0);
                self.cloud_cover.p.position.y = self.t(118.0 / 2.0 + 50.0, 118.0 / 2.0, 0.0, DC, Tm::EaseOut);
                self.draw_shape(&self.cloud_cover, &mm);
            }
            _ => {}
        }
    }

    /// 3-sahifadagi cheksizlik: chizilayotgan paytda maska + oq tekstura, tugagach tayyor tekstura.
    fn infinity_or_final(&mut self, inf_dur_k: f32) {
        let mm = self.main_matrix;
        if self.time < DC * inf_dur_k - 0.025 {
            self.rgl_mask_draw();
            let prog = self.t(0.0, 0.99, 0.0, DC * inf_dur_k, Tm::Linear);
            self.infinity_mask(prog, 0.0);
            self.rgl_normal_draw_through_mask();
            self.blend_on();
            self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
            self.powerful_infinity_white.p.rotation = 0.0;
            self.powerful_infinity_white.p.alpha = 1.0;
            self.powerful_infinity_white.p.position.y = -6.0;
            self.draw_ts(&self.powerful_infinity_white, &mm, Prog::NormalOne);
        } else {
            self.rgl_normal_draw();
            self.blend(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
            self.powerful_infinity.p.position.y = -6.0;
            self.powerful_infinity.p.alpha = 1.0;
            self.draw_ts(&self.powerful_infinity, &mm, Prog::NormalOne);
        }
    }
}

// ───────────────────────── tashqi interfeys (JNA) ─────────────────────────
fn with<R>(f: impl FnOnce(&mut Intro) -> R) -> Option<R> {
    let mut g = STATE.lock().ok()?;
    g.as_mut().map(|s| f(s))
}

/// 24 ta tekstura raqami (DrKLO `textures[]` tartibida; `Intro.kt` ga qarang).
#[no_mangle]
pub extern "C" fn rust_intro_set_textures(ids: *const u32, n: i32) {
    if ids.is_null() || n < 24 {
        return;
    }
    let s = unsafe { std::slice::from_raw_parts(ids, 24) };
    if let Ok(mut t) = TEXTURES.lock() {
        t.copy_from_slice(s);
    }
}

/// GL kontekstida chaqiriladi: shaderlar va shakllarni yaratadi. 1 — hammasi tayyor, 0 — shader xatosi (Kotlin zaxira rasmga o'tadi).
#[no_mangle]
pub extern "C" fn rust_intro_surface_created() -> i32 {
    let tex = TEXTURES.lock().map(|t| *t).unwrap_or([0; 24]);
    let bg = PENDING.lock().map(|p| p.bg).unwrap_or([1.0; 4]);
    let mut intro = Intro::create(tex, bg);
    let ok = intro.ok;
    if let Ok(p) = PENDING.lock() {
        intro.current_page = 0;
        if let Some((w, h, sc, a1)) = p.size {
            intro.on_surface_changed(w, h, sc, a1);
        }
    }
    if let Ok(mut g) = STATE.lock() {
        *g = Some(intro);
    }
    ok as i32
}

#[no_mangle]
pub extern "C" fn rust_intro_surface_changed(w: i32, h: i32, scale: f32, a1: i32) {
    if let Ok(mut p) = PENDING.lock() {
        p.size = Some((w, h, scale, a1));
    }
    with(|s| s.on_surface_changed(w, h, scale, a1));
}

#[no_mangle]
pub extern "C" fn rust_intro_set_background(r: f32, g: f32, b: f32, a: f32) {
    if let Ok(mut p) = PENDING.lock() {
        p.bg = [r, g, b, a];
    }
    with(|s| s.set_background([r, g, b, a]));
}

#[no_mangle]
pub extern "C" fn rust_intro_set_page(page: i32) {
    with(|s| {
        if s.current_page != page {
            s.prev_page = s.current_page;
            s.current_page = page;
            s.direct = if s.current_page > s.prev_page { 1 } else { 0 };
            s.date0 = s.date;
            s.time = 0.0;
        }
    });
}

#[no_mangle]
pub extern "C" fn rust_intro_set_scroll_offset(v: f32) {
    with(|s| s.scroll_offset = v);
}

#[no_mangle]
pub extern "C" fn rust_intro_set_date(v: f32) {
    with(|s| s.date = v);
}

#[no_mangle]
pub extern "C" fn rust_intro_draw_frame(delta_ms: i32) {
    with(|s| s.on_draw_frame(delta_ms));
}

#[no_mangle]
pub extern "C" fn rust_intro_destroy() {
    if let Ok(mut g) = STATE.lock() {
        *g = None;
    }
}

#[allow(dead_code)]
fn _unused(_: *const c_char) {}
