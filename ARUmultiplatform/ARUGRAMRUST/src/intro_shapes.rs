//! UZ: `IntroRenderer.c` (Telegram, GPL) shakllari: parametrlar, to'rtburchak/doira/lenta/cheksizlik generatorlari va chizish yordamchilari.
//! EN: The shapes of `IntroRenderer.c` (Telegram, GPL): parameters, rectangle/circle/ribbon/infinity generators and draw helpers.
//! RU: Фигуры `IntroRenderer.c` (Telegram, GPL): параметры, генераторы прямоугольников/кругов/лент/бесконечности и помощники отрисовки.

use crate::softgl::*;
use std::f32::consts::PI;

#[derive(Clone, Copy, Default)]
pub struct Xyz {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub fn xyz(x: f32, y: f32, z: f32) -> Xyz {
    Xyz { x, y, z }
}

#[derive(Clone, Copy)]
pub struct LayerParams {
    pub anchor: Xyz,
    pub position: Xyz,
    pub rotation: f32,
    pub scale: Xyz,
}

pub fn default_layer_params() -> LayerParams {
    LayerParams { anchor: xyz(0.0, 0.0, 0.0), position: xyz(0.0, 0.0, 0.0), rotation: 0.0, scale: xyz(1.0, 1.0, 1.0) }
}

#[derive(Clone)]
pub struct Params {
    pub anchor: Xyz,
    pub position: Xyz,
    pub rotation: f32,
    pub scale: Xyz,
    pub alpha: f32,
    pub side_length: f32,
    pub start_angle: f32,
    pub end_angle: f32,
    pub angle: f32,
    pub size: (f32, f32),
    pub radius: f32,
    pub width: f32,
    pub mode: Mode,
    pub round_count: i32,
    pub is_star: bool,
}

pub fn default_params(mode: Mode) -> Params {
    Params {
        anchor: xyz(0.0, 0.0, 0.0),
        position: xyz(0.0, 0.0, 0.0),
        rotation: 0.0,
        scale: xyz(1.0, 1.0, 1.0),
        alpha: 1.0,
        side_length: 0.0,
        start_angle: 0.0,
        end_angle: 0.0,
        angle: 0.0,
        size: (0.0, 0.0),
        radius: 0.0,
        width: 0.0,
        mode,
        round_count: 0,
        is_star: false,
    }
}

/// Rangli shakl (faqat o'rinlar) / A colored shape (positions only).
#[derive(Clone)]
pub struct Shape {
    pub color: [f32; 4],
    pub data: Vec<[f32; 4]>,
    pub params: Params,
}

/// Teksturali shakl / A textured shape.
#[derive(Clone)]
pub struct TShape {
    pub texture: usize,
    pub data: Vec<[f32; 4]>,
    pub params: Params,
}

fn pt(x: f32, y: f32) -> [f32; 4] {
    [x, y, 0.0, 0.0]
}

pub fn d2r(a: f32) -> f32 {
    a * PI / 180.0
}
pub fn r2d(a: f32) -> f32 {
    a * 180.0 / PI
}

fn gen_rounded_rectangle(size: (f32, f32), radius: f32, rc: i32) -> Vec<[f32; 4]> {
    let mut out = vec![pt(0.0, 0.0)];
    let k = PI / 2.0 / (rc + 1) as f32;
    let (w, h) = size;
    let mut n = 0;
    let corners = [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
    for (sx, sy) in corners {
        let from = if n == 0 { (rc + 2) * n } else { (rc + 1) * n };
        let to = rc + 1 + (rc + 1) * n;
        for i in from..=to {
            let a = i as f32 * k;
            out.push(pt(sx * (w / 2.0 - radius) + a.cos() * radius, sy * (h / 2.0 - radius) + a.sin() * radius));
        }
        n += 1;
    }
    out.push(pt(w / 2.0, h / 2.0 - radius));
    out
}

pub fn create_rounded_rectangle(size: (f32, f32), radius: f32, rc: i32, color: [f32; 4]) -> Shape {
    let mut p = default_params(Mode::Fan);
    p.round_count = rc;
    p.size = size;
    p.radius = radius;
    Shape { color, data: gen_rounded_rectangle(size, radius, rc), params: p }
}

pub fn change_rounded_rectangle(s: &mut Shape, size: (f32, f32), radius: f32) {
    if s.params.size != size || s.params.radius != radius {
        s.params.size = size;
        s.params.radius = radius;
        s.data = gen_rounded_rectangle(size, radius, s.params.round_count);
    }
}

fn gen_rounded_rectangle_stroked(size: (f32, f32), radius: f32, stroke: f32, rc: i32) -> Vec<[f32; 4]> {
    let mut out = Vec::new();
    let k = PI / 2.0 / (rc + 1) as f32;
    let inner = radius - stroke;
    let (w, h) = size;
    let corners = [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
    let mut n = 0;
    for (sx, sy) in corners {
        let from = if n == 0 { (rc + 2) * n } else { (rc + 1) * n };
        let to = rc + 1 + (rc + 1) * n;
        for i in from..=to {
            let a = i as f32 * k;
            let (cx, cy) = (sx * (w / 2.0 - radius), sy * (h / 2.0 - radius));
            out.push(pt(cx + a.cos() * radius, cy + a.sin() * radius));
            out.push(pt(cx + a.cos() * inner, cy + a.sin() * inner));
        }
        n += 1;
    }
    let (cx, cy) = (w / 2.0 - radius, h / 2.0 - radius);
    out.push(pt(cx + radius, cy));
    out.push(pt(cx + inner, cy));
    out
}

pub fn create_rounded_rectangle_stroked(size: (f32, f32), radius: f32, stroke: f32, rc: i32, color: [f32; 4]) -> Shape {
    let mut p = default_params(Mode::Strip);
    p.round_count = rc;
    p.size = size;
    p.radius = radius;
    p.width = stroke;
    Shape { color, data: gen_rounded_rectangle_stroked(size, radius, stroke, rc), params: p }
}

pub fn change_rounded_rectangle_stroked(s: &mut Shape, size: (f32, f32), radius: f32) {
    if s.params.size != size || s.params.radius != radius {
        s.params.size = size;
        s.params.radius = radius;
        s.data = gen_rounded_rectangle_stroked(size, radius, s.params.width, s.params.round_count);
    }
}

fn square_point(angle: f32, radius: f32) -> (f32, f32) {
    let h = PI / 2.0;
    if angle <= h * 0.5 || angle > h * 3.5 {
        (radius, radius * angle.sin() / angle.cos())
    } else if angle <= h * 1.5 {
        (radius * angle.cos() / angle.sin(), radius)
    } else if angle <= h * 2.5 {
        (-radius, -radius * angle.sin() / angle.cos())
    } else if angle <= h * 3.5 {
        (-radius * angle.cos() / angle.sin(), -radius)
    } else {
        (0.0, 0.0)
    }
}

fn square_texture_point(p: (f32, f32), side: f32) -> (f32, f32) {
    (-p.0 / side * 0.5 + 0.5, -p.1 / side * 0.5 + 0.5)
}

fn gen_segmented_square(side: f32, start: f32, end: f32) -> Vec<[f32; 4]> {
    let mut out: Vec<[f32; 4]> = Vec::new();
    let radius = side;
    let push = |out: &mut Vec<[f32; 4]>, p: (f32, f32)| {
        let t = square_texture_point(p, side);
        out.push([p.0, p.1, t.0, t.1]);
    };
    let k = 1.0;
    let da = d2r(-2.6 * 2.0) * k;
    push(&mut out, ((start + end).sin() * 6.0 * k, -(start + end).cos() * 6.0 * k));
    push(&mut out, square_point(start + da, radius));
    let mut q = 0;
    let mut i = start as i32;
    while (i as f32) < r2d(start + end + da).floor() {
        if (i + 45) % 90 == 0 {
            push(&mut out, square_point(d2r(i as f32), radius));
            q += 1;
        }
        i += 1;
    }
    push(&mut out, square_point(start + end + da, radius));
    for _ in 0..(4 - q) {
        push(&mut out, square_point(start + end + da, radius));
    }
    out
}

pub fn create_segmented_square(side: f32, start: f32, end: f32, texture: usize) -> TShape {
    TShape { texture, data: gen_segmented_square(side, start, end), params: default_params(Mode::Fan) }
}

pub fn change_segmented_square(s: &mut TShape, side: f32, start: f32, end: f32) {
    if s.params.side_length != side || s.params.start_angle != start || s.params.end_angle != end {
        s.params.side_length = side;
        s.params.start_angle = start;
        s.params.end_angle = end;
        s.data = gen_segmented_square(side, start, end);
    }
}

pub fn create_rectangle(size: (f32, f32), color: [f32; 4]) -> Shape {
    let (w, h) = size;
    Shape { color, data: vec![pt(-w / 2.0, -h / 2.0), pt(w / 2.0, -h / 2.0), pt(-w / 2.0, h / 2.0), pt(w / 2.0, h / 2.0)], params: default_params(Mode::Strip) }
}

pub fn create_textured_rectangle(size: (f32, f32), texture: usize) -> TShape {
    let (w, h) = size;
    let v = |x: f32, y: f32| [x, y, 1.0 - (-x / w + 0.5), y / h + 0.5];
    TShape { texture, data: vec![v(-w / 2.0, -h / 2.0), v(w / 2.0, -h / 2.0), v(-w / 2.0, h / 2.0), v(w / 2.0, h / 2.0)], params: default_params(Mode::Strip) }
}

fn gen_ribbon(length: f32) -> Vec<[f32; 4]> {
    vec![pt(-(length - 5.5).max(0.0), -5.5), pt(0.0, -5.5), pt(-length.max(0.0), 5.5), pt(0.0, 5.5)]
}

pub fn create_ribbon(length: f32, color: [f32; 4]) -> Shape {
    let mut p = default_params(Mode::Strip);
    p.side_length = length;
    Shape { color, data: gen_ribbon(length), params: p }
}

pub fn change_ribbon(s: &mut Shape, length: f32) {
    if s.params.side_length != length {
        s.params.side_length = length;
        s.data = gen_ribbon(length);
    }
}

pub fn create_circle(radius: f32, vertex_count: i32, color: [f32; 4]) -> Shape {
    let mut data = vec![pt(0.0, 0.0)];
    for i in 0..=vertex_count {
        let a = 2.0 * PI * (i as f32 / vertex_count as f32);
        data.push(pt(radius * a.cos(), radius * a.sin()));
    }
    let mut p = default_params(Mode::Fan);
    p.round_count = vertex_count;
    Shape { color, data, params: p }
}

fn gen_infinity(width: f32, angle: f32, segments: i32) -> Vec<[f32; 4]> {
    let path: [(f32, f32); 13] = [(53.0, 23.0), (49.0, 31.0), (39.0, 47.0), (22.0, 47.0), (6.0, 47.0), (0.0, 31.0), (0.0, 23.0), (0.0, 16.0), (5.0, 0.0), (23.0, 0.0), (39.0, 0.0), (48.0, 15.0), (52.0, 21.0)];
    let mut out = Vec::new();
    for seg in 0..=segments {
        let tt = (seg as f32 / segments as f32) * angle;
        let q = 4.0f32;
        let tstep = 1.0 / q;
        let n = (tt / tstep).floor() as usize;
        let n = n.min(3);
        let (a, p1, p2, b) = (path[3 * n], path[1 + 3 * n], path[2 + 3 * n], path[3 + 3 * n]);
        let t = (tt - tstep * n as f32) * q;
        let nt = 1.0 - t;
        let px = a.0 * nt * nt * nt + 3.0 * p1.0 * nt * nt * t + 3.0 * p2.0 * nt * t * t + b.0 * t * t * t;
        let py = a.1 * nt * nt * nt + 3.0 * p1.1 * nt * nt * t + 3.0 * p2.1 * nt * t * t + b.1 * t * t * t;
        let tx = -3.0 * a.0 * nt * nt + 3.0 * p1.0 * (1.0 - 4.0 * t + 3.0 * t * t) + 3.0 * p2.0 * (2.0 * t - 3.0 * t * t) + 3.0 * b.0 * t * t;
        let ty = -3.0 * a.1 * nt * nt + 3.0 * p1.1 * (1.0 - 4.0 * t + 3.0 * t * t) + 3.0 * p2.1 * (2.0 * t - 3.0 * t * t) + 3.0 * b.1 * t * t;
        let (nx, ny) = (-ty, tx);
        let l = (nx * nx + ny * ny).sqrt();
        let k = 1.0 / l;
        let (nx, ny) = (nx * k, ny * k);
        out.push(pt(px + nx * width / 2.0, py + ny * width / 2.0));
        out.push(pt(px - nx * width / 2.0, py - ny * width / 2.0));
    }
    out
}

pub fn create_infinity(width: f32, angle: f32, segments: i32, color: [f32; 4]) -> Shape {
    let mut p = default_params(Mode::Strip);
    p.round_count = segments;
    p.width = width;
    p.angle = angle;
    Shape { color, data: gen_infinity(width, angle, segments), params: p }
}

pub fn change_infinity(s: &mut Shape, angle: f32) {
    if s.params.angle != angle {
        s.params.angle = angle;
        s.data = gen_infinity(s.params.width, angle, s.params.round_count);
    }
}

/// `mvp_matrix` (y_offset — global `y_offset`).
pub fn mvp_matrix(p: &Params, vp: &Mat4, y_offset: f32) -> Mat4 {
    let mut model = translate(-p.anchor.x, -p.anchor.y, p.anchor.z);
    let scaled = scale_aniso(&identity(), p.scale.x, -p.scale.y, p.scale.z);
    model = mul(&scaled, &model);
    let rotate = rotate_z2(&identity(), deg_to_rad(-p.rotation));
    model = mul(&rotate, &model);
    translate_independed(&mut model, p.position.x, -p.position.y, p.position.z);
    let mm = mul(&identity(), vp);
    let mut m = mul(&mm, &model);
    translate_independed(&mut m, 0.0, -y_offset / vp[3][3], 0.0);
    m
}

pub fn draw_shape(gl: &mut Gl, y_offset: f32, s: &Shape, vp: &Mat4) {
    let p = &s.params;
    if p.alpha > 0.0 && p.scale.x.abs() > 0.0 && p.scale.y.abs() > 0.0 && p.scale.z.abs() > 0.0 {
        let m = mvp_matrix(p, vp, y_offset);
        let color = if p.rotation == 10.0 { [0.0, 1.0, 0.0, 1.0] } else { s.color };
        gl.draw(p.mode, &s.data, &m, &Shade::Color(color, p.alpha));
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Prog {
    Normal,
    NormalOne,
    Red,
    Blue,
    LightRed,
    LightBlue,
}

pub struct View {
    pub width: i32,
    pub height: i32,
    pub y_offset: f32,
    pub y_offset_absolute: i32,
}

pub fn draw_textured_shape(gl: &mut Gl, textures: &[Texture], view: &View, s: &TShape, vp: &Mat4, prog: Prog) {
    let p = &s.params;
    if !(p.alpha > 0.0 && p.scale.x.abs() > 0.0 && p.scale.y.abs() > 0.0 && p.scale.z.abs() > 0.0) {
        return;
    }
    let m = mvp_matrix(p, vp, view.y_offset);
    if p.is_star {
        let pos = mul_vec4(&m, [0.0, 0.0, 0.0, 1.0]);
        let (nx, ny) = (pos[0] / pos[3], pos[1] / pos[3]);
        let (wx, wy) = (nx * view.width as f32, -ny * view.height as f32);
        let d = 160.0;
        let ya = view.y_offset_absolute as f32 * 2.0;
        if wx.abs() > d || wy > ya + d || wy < ya - d {
            return;
        }
    }
    let kind = match prog {
        Prog::Red => TexKind::Const([210.0 / 255.0, 57.0 / 255.0, 41.0 / 255.0]),
        Prog::Blue => TexKind::Const([0.0, 0.6, 0.898]),
        Prog::LightRed => TexKind::Const([246.0 / 255.0, 73.0 / 255.0, 55.0 / 255.0]),
        Prog::LightBlue => TexKind::Const([42.0 / 255.0, 180.0 / 255.0, 247.0 / 255.0]),
        Prog::NormalOne => TexKind::One,
        Prog::Normal => TexKind::Normal,
    };
    gl.draw(p.mode, &s.data, &m, &Shade::Tex(&textures[s.texture], p.alpha, kind));
}
