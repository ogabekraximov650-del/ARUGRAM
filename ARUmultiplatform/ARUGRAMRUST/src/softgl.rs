//! UZ: Telegram tanishtiruv chizuvchisi (`IntroRenderer.c`, GPL) uchun kichik dasturiy "GL": 4x4 matritsalar, uchburchaklar, chuqurlik buferi, aralashtirish (blend) va teksturalar. Faqat shu chizuvchi ishlatadigan qismi.
//! EN: A tiny software "GL" for the Telegram intro renderer (`IntroRenderer.c`, GPL): 4x4 matrices, triangles, a depth buffer, blending and textures. Only the subset that renderer uses.
//! RU: Небольшой программный «GL» для отрисовщика интро Telegram (`IntroRenderer.c`, GPL): матрицы 4x4, треугольники, буфер глубины, смешивание и текстуры. Только подмножество, нужное этому отрисовщику.

pub type Mat4 = [[f32; 4]; 4];

pub fn identity() -> Mat4 {
    let mut m = [[0.0; 4]; 4];
    for (i, row) in m.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    m
}

/// `M[c][r] = Σ a[k][r] * b[c][k]` (ustun-asosli / column-major, C kodidagidek / as in the C code).
pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut m = [[0.0; 4]; 4];
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

pub fn mul_vec4(m: &Mat4, v: [f32; 4]) -> [f32; 4] {
    let mut r = [0.0; 4];
    for j in 0..4 {
        for i in 0..4 {
            r[j] += m[i][j] * v[i];
        }
    }
    r
}

pub fn translate(x: f32, y: f32, z: f32) -> Mat4 {
    let mut t = identity();
    t[3][0] = x;
    t[3][1] = y;
    t[3][2] = z;
    t
}

pub fn scale_aniso(a: &Mat4, x: f32, y: f32, z: f32) -> Mat4 {
    let mut m = *a;
    for j in 0..4 {
        m[0][j] = a[0][j] * x;
        m[1][j] = a[1][j] * y;
        m[2][j] = a[2][j] * z;
    }
    m
}

/// `mat4x4_rotate_Z2(Q, M, angle)`: Q = M * R.
pub fn rotate_z2(m: &Mat4, angle: f32) -> Mat4 {
    let (s, c) = angle.sin_cos();
    let r: Mat4 = [[c, s, 0.0, 0.0], [-s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
    mul(m, &r)
}

pub fn translate_in_place(m: &mut Mat4, x: f32, y: f32, z: f32) {
    for i in 0..4 {
        m[3][i] += m[0][i] * x + m[1][i] * y + m[2][i] * z;
    }
}

/// `mat4x4_translate_independed`: m = T * m.
pub fn translate_independed(m: &mut Mat4, x: f32, y: f32, z: f32) {
    let mut tr = identity();
    translate_in_place(&mut tr, x, y, z);
    *m = mul(&tr, m);
}

pub fn deg_to_rad(d: f32) -> f32 {
    d * std::f32::consts::PI / 180.0
}

/// Tekstura (premultiplied RGBA, 0..1) / A texture (premultiplied RGBA, 0..1) / Текстура (premultiplied RGBA, 0..1).
pub struct Texture {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[f32; 4]>,
}

impl Texture {
    /// To'g'ri (straight) alfali RGBA8 dan; `tint` berilsa rang almashtiriladi (alfa saqlanadi) / from straight-alpha RGBA8; with `tint` the color is replaced (alpha kept).
    pub fn from_rgba8(w: usize, h: usize, rgba: &[u8], tint: Option<[f32; 3]>) -> Texture {
        let mut px = Vec::with_capacity(w * h);
        for i in 0..w * h {
            let a = rgba[i * 4 + 3] as f32 / 255.0;
            let (r, g, b) = match tint {
                Some(t) => (t[0], t[1], t[2]),
                None => (rgba[i * 4] as f32 / 255.0, rgba[i * 4 + 1] as f32 / 255.0, rgba[i * 4 + 2] as f32 / 255.0),
            };
            px.push([r * a, g * a, b * a, a]);
        }
        Texture { w, h, px }
    }

    /// Bilinear, CLAMP_TO_EDGE (GL_LINEAR).
    #[inline]
    pub fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let fx = u * self.w as f32 - 0.5;
        let fy = v * self.h as f32 - 0.5;
        let (x0f, y0f) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0f, fy - y0f);
        let (x0, y0) = (x0f as i32, y0f as i32);
        let (w, h) = (self.w as i32, self.h as i32);
        let (xa, xb) = (x0.clamp(0, w - 1) as usize, (x0 + 1).clamp(0, w - 1) as usize);
        let (ya, yb) = (y0.clamp(0, h - 1) as usize * self.w, (y0 + 1).clamp(0, h - 1) as usize * self.w);
        let (a, b, c, d) = (self.px[ya + xa], self.px[ya + xb], self.px[yb + xa], self.px[yb + xb]);
        let (w00, w10, w01, w11) = ((1.0 - tx) * (1.0 - ty), tx * (1.0 - ty), (1.0 - tx) * ty, tx * ty);
        [
            a[0] * w00 + b[0] * w10 + c[0] * w01 + d[0] * w11,
            a[1] * w00 + b[1] * w10 + c[1] * w01 + d[1] * w11,
            a[2] * w00 + b[2] * w10 + c[2] * w01 + d[2] * w11,
            a[3] * w00 + b[3] * w10 + c[3] * w01 + d[3] * w11,
        ]
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Fan,
    Strip,
}

#[derive(Clone, Copy)]
pub enum TexKind {
    /// `gl_FragColor = tex; w *= alpha`
    Normal,
    /// `gl_FragColor = tex * alpha`
    One,
    /// `vec4(rgb, alpha * tex.w)`
    Const([f32; 3]),
}

pub enum Shade<'a> {
    Color([f32; 4], f32),
    Tex(&'a Texture, f32, TexKind),
}

#[derive(Clone, Copy, PartialEq)]
pub enum DepthFunc {
    Less,
    Greater,
}

/// GL holati (kadrdan kadrga saqlanadi) / GL state (persists across frames) / Состояние GL (сохраняется между кадрами).
pub struct Gl {
    pub w: usize,
    pub h: usize,
    pub color: Vec<[f32; 4]>,
    pub depth: Vec<f32>,
    pub depth_test: bool,
    pub depth_func: DepthFunc,
    pub depth_mask: bool,
    pub color_mask: bool,
    pub blend: bool,
    /// `true`: (ONE, ONE_MINUS_SRC_ALPHA), `false`: (SRC_ALPHA, ONE_MINUS_SRC_ALPHA).
    pub blend_one: bool,
    pub clear: [f32; 4],
    pub clear_depth: f32,
}

#[derive(Clone, Copy)]
struct Clip {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
    u: f32,
    v: f32,
}

impl Gl {
    pub fn new(w: usize, h: usize) -> Gl {
        Gl {
            w,
            h,
            color: vec![[0.0; 4]; w * h],
            depth: vec![1.0; w * h],
            depth_test: false,
            depth_func: DepthFunc::Less,
            depth_mask: true,
            color_mask: true,
            blend: false,
            blend_one: true,
            clear: [0.0, 0.0, 0.0, 0.0],
            clear_depth: 1.0,
        }
    }

    pub fn clear_color_buffer(&mut self) {
        let c = self.clear;
        self.color.iter_mut().for_each(|p| *p = c);
    }

    pub fn clear_depth_buffer(&mut self) {
        let d = self.clear_depth;
        self.depth.iter_mut().for_each(|p| *p = d);
    }

    /// `glDrawArrays`: `verts` — (x, y, u, v) / (x, y, u, v) / (x, y, u, v).
    pub fn draw(&mut self, mode: Mode, verts: &[[f32; 4]], mvp: &Mat4, shade: &Shade) {
        let clip: Vec<Clip> = verts
            .iter()
            .map(|p| {
                let c = mul_vec4(mvp, [p[0], p[1], 0.0, 1.0]);
                Clip { x: c[0], y: c[1], z: c[2], w: c[3], u: p[2], v: p[3] }
            })
            .collect();
        match mode {
            Mode::Fan => {
                for i in 1..clip.len().saturating_sub(1) {
                    self.tri([clip[0], clip[i], clip[i + 1]], shade);
                }
            }
            Mode::Strip => {
                for i in 0..clip.len().saturating_sub(2) {
                    self.tri([clip[i], clip[i + 1], clip[i + 2]], shade);
                }
            }
        }
    }

    fn tri(&mut self, v: [Clip; 3], shade: &Shade) {
        if v.iter().any(|c| c.w <= 1e-6) {
            return;
        }
        let (fw, fh) = (self.w as f32, self.h as f32);
        let mut p = [(0.0f32, 0.0f32); 3];
        let mut z = [0.0f32; 3];
        let mut iw = [0.0f32; 3];
        for i in 0..3 {
            iw[i] = 1.0 / v[i].w;
            p[i] = ((v[i].x * iw[i] * 0.5 + 0.5) * fw, (v[i].y * iw[i] * 0.5 + 0.5) * fh);
            z[i] = v[i].z * iw[i] * 0.5 + 0.5;
        }
        let area = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[1].1 - p[0].1) * (p[2].0 - p[0].0);
        if area == 0.0 || !area.is_finite() {
            return;
        }
        // CCW (y yuqoriga) ga keltiramiz / normalize to CCW (y up).
        let (a, b, c) = if area > 0.0 { (0, 1, 2) } else { (0, 2, 1) };
        let (pa, pb, pc) = (p[a], p[b], p[c]);
        let area = ((pb.0 - pa.0) * (pc.1 - pa.1) - (pb.1 - pa.1) * (pc.0 - pa.0)).abs();
        let min_x = pa.0.min(pb.0).min(pc.0).floor().max(0.0) as usize;
        let max_x = (pa.0.max(pb.0).max(pc.0).ceil().min(fw)) as usize;
        let min_y = pa.1.min(pb.1).min(pc.1).floor().max(0.0) as usize;
        let max_y = (pa.1.max(pb.1).max(pc.1).ceil().min(fh)) as usize;
        if min_x >= max_x || min_y >= max_y {
            return;
        }
        let top_left = |s: (f32, f32), e: (f32, f32)| e.1 > s.1 || (e.1 == s.1 && e.0 < s.0);
        let tl = [top_left(pb, pc), top_left(pc, pa), top_left(pa, pb)];
        // chetlar: e_i(x, y) = A_i * x + B_i * y + C_i
        let ea = [-(pc.1 - pb.1), -(pa.1 - pc.1), -(pb.1 - pa.1)];
        let eb = [pc.0 - pb.0, pa.0 - pc.0, pb.0 - pa.0];
        let ec = [-ea[0] * pb.0 - eb[0] * pb.1, -ea[1] * pc.0 - eb[1] * pc.1, -ea[2] * pa.0 - eb[2] * pa.1];
        let inv_area = 1.0 / area;
        let za = [z[a], z[b], z[c]];
        let iwa = [iw[a], iw[b], iw[c]];
        let (depth_test, depth_func, depth_mask, color_mask, blend, blend_one) = (self.depth_test, self.depth_func, self.depth_mask, self.color_mask, self.blend, self.blend_one);
        if !color_mask && !(depth_test && depth_mask) {
            return;
        }
        let w = self.w;
        // piksel rangini hisoblash (uchburchakka bog'liq qism) / per-pixel shading
        let (tex, tex_alpha, tex_kind, col) = match shade {
            Shade::Color(c, al) => (None, 0.0, TexKind::Normal, [c[0], c[1], c[2], c[3] * al]),
            Shade::Tex(t, al, k) => (Some(*t), *al, *k, [0.0; 4]),
        };
        let (ua, ub, uc) = ([v[a].u * iwa[0], v[b].u * iwa[1], v[c].u * iwa[2]], [v[a].v * iwa[0], v[b].v * iwa[1], v[c].v * iwa[2]], iwa);
        for py in min_y..max_y {
            let qy = py as f32 + 0.5;
            let qx0 = min_x as f32 + 0.5;
            let mut e = [ea[0] * qx0 + eb[0] * qy + ec[0], ea[1] * qx0 + eb[1] * qy + ec[1], ea[2] * qx0 + eb[2] * qy + ec[2]];
            let row = py * w;
            for px in min_x..max_x {
                let inside = (e[0] > 0.0 || (e[0] == 0.0 && tl[0])) && (e[1] > 0.0 || (e[1] == 0.0 && tl[1])) && (e[2] > 0.0 || (e[2] == 0.0 && tl[2]));
                if inside {
                    let (l0, l1, l2) = (e[0] * inv_area, e[1] * inv_area, e[2] * inv_area);
                    let idx = row + px;
                    let depth = l0 * za[0] + l1 * za[1] + l2 * za[2];
                    let pass = !depth_test
                        || match depth_func {
                            DepthFunc::Less => depth < self.depth[idx],
                            DepthFunc::Greater => depth > self.depth[idx],
                        };
                    if pass {
                        if color_mask {
                            let src = match tex {
                                None => col,
                                Some(t) => {
                                    let s = l0 * uc[0] + l1 * uc[1] + l2 * uc[2];
                                    let u = (l0 * ua[0] + l1 * ua[1] + l2 * ua[2]) / s;
                                    let vv = (l0 * ub[0] + l1 * ub[1] + l2 * ub[2]) / s;
                                    let tx = t.sample(u, vv);
                                    match tex_kind {
                                        TexKind::Normal => [tx[0], tx[1], tx[2], tx[3] * tex_alpha],
                                        TexKind::One => [tx[0] * tex_alpha, tx[1] * tex_alpha, tx[2] * tex_alpha, tx[3] * tex_alpha],
                                        TexKind::Const(c) => [c[0], c[1], c[2], tex_alpha * tx[3]],
                                    }
                                }
                            };
                            let dst = &mut self.color[idx];
                            if blend {
                                let sf = if blend_one { 1.0 } else { src[3] };
                                let df = 1.0 - src[3];
                                for i in 0..4 {
                                    dst[i] = (src[i] * sf + dst[i] * df).clamp(0.0, 1.0);
                                }
                            } else {
                                for i in 0..4 {
                                    dst[i] = src[i].clamp(0.0, 1.0);
                                }
                            }
                        }
                        if depth_test && depth_mask {
                            self.depth[idx] = depth;
                        }
                    }
                }
                e[0] += ea[0];
                e[1] += ea[1];
                e[2] += ea[2];
            }
        }
    }

    /// RGBA8 (pastdan yuqoriga emas — yuqoridan pastga), alfa 255 / RGBA8 (top-down), alpha 255.
    pub fn to_rgba8(&self) -> Vec<u8> {
        let mut out = vec![255u8; self.w * self.h * 4];
        for y in 0..self.h {
            let sy = self.h - 1 - y;
            for x in 0..self.w {
                let c = self.color[sy * self.w + x];
                let o = (y * self.w + x) * 4;
                out[o] = (c[0] * 255.0 + 0.5) as u8;
                out[o + 1] = (c[1] * 255.0 + 0.5) as u8;
                out[o + 2] = (c[2] * 255.0 + 0.5) as u8;
            }
        }
        out
    }
}
