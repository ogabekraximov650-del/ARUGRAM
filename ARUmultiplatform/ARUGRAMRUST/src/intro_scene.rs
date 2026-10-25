//! UZ: Telegram tanishtiruv sahnasi (`IntroRenderer.c`, GPL) — Rust'ga ko'chirilgan; dasturiy `softgl` ustida ishlaydi.
//! EN: The Telegram intro scene (`IntroRenderer.c`, GPL) ported to Rust; runs on the software `softgl`.
//! RU: Сцена интро Telegram (`IntroRenderer.c`, GPL), перенесённая на Rust; работает на программном `softgl`.

use crate::intro_shapes::*;
use crate::softgl::*;
use std::f32::consts::PI;

#[cfg(unix)]
extern "C" {
    fn random() -> std::os::raw::c_long;
}
const RAND_MAX: f64 = 2147483647.0;

#[derive(Clone, Copy, PartialEq)]
pub enum Ty {
    Default,
    EaseIn,
    EaseOut,
    EaseInEaseOut,
    Linear,
    Sin,
    EaseOutBounce,
}

fn coeffs(ty: Ty) -> [f32; 4] {
    match ty {
        Ty::Default => [0.25, 0.1, 0.25, 1.0],
        Ty::EaseInEaseOut => [0.42, 0.0, 0.58, 1.0],
        Ty::EaseIn => [0.42, 0.0, 1.0, 1.0],
        Ty::EaseOut => [0.0, 0.0, 0.58, 1.0],
        Ty::EaseOutBounce => [0.0, 0.0, 0.0, 1.25],
        Ty::Linear => [0.0, 0.0, 1.0, 1.0],
        Ty::Sin => [0.0, 0.0, 0.0, 0.0],
    }
}

fn timing(x: f32, ty: Ty) -> f32 {
    if x == 0.0 || x == 1.0 {
        return x;
    }
    let c = coeffs(ty);
    let (c1x, c1y, c2x, c2y) = (c[0], c[1], c[2], c[3]);
    let cx = [0.0, 3.0 * c1x, -6.0 * c1x + 3.0 * c2x, 3.0 * c1x - 3.0 * c2x + 1.0];
    let cx = [cx[0], cx[1], cx[2], cx[3]];
    let cy = [0.0, 3.0 * c1y, -6.0 * c1y + 3.0 * c2y, 3.0 * c1y - 3.0 * c2y + 1.0];
    let mut t = x;
    for _ in 0..10 {
        let x2 = cx[0] + t * cx[1] + t * t * cx[2] + t * t * t * cx[3] - x;
        let d = cx[1] + 2.0 * t * cx[2] + 3.0 * t * t * cx[3];
        t -= x2 / d;
    }
    cy[0] + t * cy[1] + t * t * cy[2] + t * t * t * cy[3]
}

fn tf(time: f32, sv: f32, ev: f32, st: f32, dur: f32, ty: Ty) -> f32 {
    if time > st + dur {
        return ev;
    }
    let k = (dur + st).min((time - st).max(0.0)) / dur;
    if ty == Ty::Linear { sv + (ev - sv) * k } else { sv + (ev - sv) * timing(k, ty) }
}

fn id(a: i32, b: i32) -> f32 {
    (a / b) as f32
}

const R1: f32 = 58.5;
const R2: f32 = 70.0;
const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const STARS: usize = 80;
const DUR: f32 = 0.3;

pub struct Scene {
    pub gl: Gl,
    tex: Vec<Texture>,
    view: View,
    scale_factor: f32,
    bg: [f32; 4],
    // holat / state
    date: f32,
    date0: f32,
    ms0: f32,
    time: f32,
    time_local: f32,
    direct: i32,
    cur: i32,
    prev: i32,
    scroll_offset: f32,
    ms0_anim: f32,
    fps_anim: i32,
    last_stars_fps: i32,
    count_anim_fps: i32,
    ribbon_length: f32,
    stars_far: f32,
    speedo_scroll: f32,
    free_scroll: f32,
    private_scroll: f32,
    calc_speedo_sin: f32,
    cloud_scroll: f32,
    knot_delays: [f32; 4],
    // ic animatsiyalari
    pencil_start: f32,
    pencil_all_start: f32,
    pencil_all_end: f32,
    pencil_stage: i32,
    pencil_period: i32,
    bubble_dots_stage: i32,
    bubble_dots_end_period: i32,
    videocam_start: f32,
    videocam_next: f32,
    videocam_dur: f32,
    videocam_angle: f32,
    videocam_old: f32,
    cam_start: f32,
    cam_next: f32,
    cam_dur: f32,
    cam_angle: f32,
    cam_old: f32,
    q_shot: i32,
    camshot_start: f32,
    camshot_dur: f32,
    smile_t1: f32,
    smile_t2: f32,
    smile_blink: f32,
    smile_blink_one: i32,
    smile_stage: i32,
    pin_start: f32,
    pin_dur: f32,
    // matritsalar
    main_matrix: Mat4,
    stars_matrix: Mat4,
    ribbons_layer: Mat4,
    ic_matrix: Mat4,
    private_matrix: Mat4,
    // qatlamlar
    ribbon_layer: LayerParams,
    private_layer: LayerParams,
    ic_pin_l: LayerParams,
    ic_cam_l: LayerParams,
    ic_videocam_l: LayerParams,
    ic_smile_l: LayerParams,
    ic_bubble_l: LayerParams,
    ic_pencil_l: LayerParams,
    // shakllar
    mask1: Shape,
    cloud_extra: [Shape; 4],
    cloud_cover: Shape,
    cloud_bg: Shape,
    free_bg: Shape,
    powerful_bg: Shape,
    private_bg: Shape,
    ribbon: [Shape; 4],
    infinity: Shape,
    private_stroke: Shape,
    spiral: TShape,
    fast_body: TShape,
    fast_arrow_shadow: TShape,
    fast_arrow: TShape,
    knot: [TShape; 4],
    powerful_mask: TShape,
    powerful_infinity: TShape,
    powerful_infinity_white: TShape,
    telegram_sphere: TShape,
    telegram_plane: TShape,
    telegram_mask: TShape,
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
    private_door: TShape,
    private_screw: TShape,
}

// Tekstura indekslari / texture indices
pub const T_FAST_ARROW_SHADOW: usize = 0;
pub const T_FAST_ARROW: usize = 1;
pub const T_FAST_BODY: usize = 2;
pub const T_FAST_SPIRAL: usize = 3;
pub const T_BUBBLE_DOT: usize = 4;
pub const T_BUBBLE: usize = 5;
pub const T_CAM_LENS: usize = 6;
pub const T_CAM: usize = 7;
pub const T_PENCIL: usize = 8;
pub const T_PIN: usize = 9;
pub const T_SMILE_EYE: usize = 10;
pub const T_SMILE: usize = 11;
pub const T_VIDEOCAM: usize = 12;
pub const T_KNOT_DOWN: usize = 13;
pub const T_KNOT_UP: usize = 14;
pub const T_INF_WHITE: usize = 15;
pub const T_INF: usize = 16;
pub const T_PMASK: usize = 17;
pub const T_STAR: usize = 18;
pub const T_DOOR: usize = 19;
pub const T_SCREW: usize = 20;
pub const T_PLANE: usize = 21;
pub const T_SPHERE: usize = 22;
pub const T_TMASK: usize = 23;

fn mat_plain(w: i32, h: i32) -> Mat4 {
    let mut m = [[0.0; 4]; 4];
    m[0][0] = 1.0;
    m[1][1] = w as f32 / h as f32;
    m[2][2] = 1.0;
    m[3][3] = w as f32 / 2.0;
    m
}

fn mat_stars(w: i32, h: i32, y_off_abs: i32, scale_factor: f32) -> Mat4 {
    let mut m = [[0.0; 4]; 4];
    if h >= w {
        let k = w as f32 / h as f32;
        let q = 1.4;
        m[0][0] = 1.0 / q;
        m[1][1] = k / q;
        m[2][2] = 1.0;
        m[2][3] = -1.25;
        m[3][3] = w as f32 * k;
    } else {
        let k = h as f32 / w as f32;
        let q = 2.0;
        m[0][0] = 1.0 / q;
        m[1][1] = (1.0 / k) / q;
        m[2][2] = 1.0;
        m[2][3] = -1.25;
        m[3][3] = h as f32 * k;
    }
    translate_independed(&mut m, 0.0, -2.0 * y_off_abs as f32 / h as f32 + 4.0 * scale_factor / h as f32, 0.0);
    m
}

impl Scene {
    /// `onSurfaceChanged` + `onSurfaceCreated` (+ `setBackgroundColor`). `bg` — fon rangi (0..1) / background (0..1).
    pub fn new(tex: Vec<Texture>, bg: [f32; 3], w_px: usize, h_px: usize, scale_factor: f32) -> Scene {
        let bgc = [bg[0], bg[1], bg[2], 1.0];
        let width = (w_px as f32 / scale_factor) as i32;
        let height = (h_px as f32 / scale_factor) as i32;
        let main_matrix = mat_plain(width, height);
        let stars_matrix = mat_stars(width, height, 0, scale_factor);
        let circle = |_: usize| create_circle(1.0, 64, BLACK);
        let mut ribbon: Vec<Shape> = Vec::new();
        let rl = default_layer_params();
        let _ = rl;
        for r in [0.0, 90.0, 180.0, 270.0] {
            let mut s = create_ribbon(86.5, WHITE);
            s.params.rotation = r;
            s.params.position.y = -9.0;
            ribbon.push(s);
        }
        let mut cloud_cover = create_rectangle((240.0, 100.0), bgc);
        cloud_cover.params.anchor.y = -50.0;
        let mut fast_arrow_shadow = create_textured_rectangle((82.0, 22.0), T_FAST_ARROW_SHADOW);
        fast_arrow_shadow.params.position.x = -1.0;
        fast_arrow_shadow.params.position.y = 2.0;
        let mut fast_arrow = create_textured_rectangle((82.0, 22.0), T_FAST_ARROW);
        fast_arrow.params.anchor.x = -19.0;
        fast_arrow_shadow.params.anchor.x = -19.0;
        let mut telegram_plane = create_textured_rectangle((82.0, 74.0), T_PLANE);
        telegram_plane.params.anchor = xyz(6.0, -5.0, 0.0);
        let mk_knot = |tx: usize, s: f32, ax: f32, ay: f32| {
            let mut k = create_textured_rectangle((s, s), tx);
            k.params.anchor.x = ax;
            k.params.anchor.y = ay;
            k
        };
        let knot = [
            mk_knot(T_KNOT_UP, id(138, 3), -23.0 + 10.0, 23.0 - 10.0),
            mk_knot(T_KNOT_UP, id(138, 3), -23.0 + 10.0, 23.0 - 10.0),
            mk_knot(T_KNOT_DOWN, id(150, 3), -100.0 / 4.0 + 20.0 / 2.0, -100.0 / 4.0 + 20.0 / 2.0),
            mk_knot(T_KNOT_DOWN, id(150, 3), -100.0 / 4.0 + 20.0 / 2.0, -100.0 / 4.0 + 20.0 / 2.0),
        ];
        let mut ic_pin_l = default_layer_params();
        ic_pin_l.anchor = xyz(0.0, id(50, 2), 0.0);
        let mut ic_pencil_l = default_layer_params();
        ic_pencil_l.anchor = xyz(id(-30, 2), id(30, 2), 0.0);
        let mut star = create_textured_rectangle((5.25, 5.25), T_STAR);
        star.params.is_star = true;
        let mut sc = Scene {
            gl: Gl::new(w_px, h_px),
            tex,
            view: View { width, height, y_offset: 0.0, y_offset_absolute: 0 },
            scale_factor,
            bg: bgc,
            date: 1.0,
            date0: 0.0,
            ms0: 0.0,
            time: 0.0,
            time_local: 0.0,
            direct: 0,
            cur: 0,
            prev: 0,
            scroll_offset: 0.0,
            ms0_anim: 0.0,
            fps_anim: 0,
            last_stars_fps: 0,
            count_anim_fps: 0,
            ribbon_length: 86.5,
            stars_far: 500.0,
            speedo_scroll: 0.0,
            free_scroll: 0.0,
            private_scroll: 0.0,
            calc_speedo_sin: 0.0,
            cloud_scroll: 0.0,
            knot_delays: [0.0; 4],
            pencil_start: 0.0,
            pencil_all_start: 0.0,
            pencil_all_end: 0.0,
            pencil_stage: 0,
            pencil_period: 0,
            bubble_dots_stage: 0,
            bubble_dots_end_period: 0,
            videocam_start: 0.0,
            videocam_next: 0.0,
            videocam_dur: 0.0,
            videocam_angle: 0.0,
            videocam_old: 0.0,
            cam_start: 0.0,
            cam_next: 0.0,
            cam_dur: 0.0,
            cam_angle: 0.0,
            cam_old: 0.0,
            q_shot: 0,
            camshot_start: 0.0,
            camshot_dur: 0.0,
            smile_t1: 0.0,
            smile_t2: 0.0,
            smile_blink: 0.0,
            smile_blink_one: 0,
            smile_stage: 0,
            pin_start: 0.0,
            pin_dur: 0.0,
            main_matrix,
            stars_matrix,
            ribbons_layer: identity(),
            ic_matrix: identity(),
            private_matrix: identity(),
            ribbon_layer: default_layer_params(),
            private_layer: default_layer_params(),
            ic_pin_l,
            ic_cam_l: default_layer_params(),
            ic_videocam_l: default_layer_params(),
            ic_smile_l: default_layer_params(),
            ic_bubble_l: default_layer_params(),
            ic_pencil_l,
            mask1: create_rounded_rectangle((60.0, 60.0), 0.0, 16, BLACK),
            cloud_extra: [circle(0), circle(1), circle(2), circle(3)],
            cloud_cover,
            cloud_bg: create_rectangle((320.0, 320.0), [42.0 / 255.0, 180.0 / 255.0, 247.0 / 255.0, 1.0]),
            free_bg: create_rectangle((320.0, 320.0), [246.0 / 255.0, 73.0 / 255.0, 55.0 / 255.0, 1.0]),
            powerful_bg: create_rectangle((200.0, 200.0), [47.0 / 255.0, 90.0 / 255.0, 131.0 / 255.0, 1.0]),
            private_bg: create_rectangle((240.0, 240.0), [200.0 / 255.0, 207.0 / 255.0, 212.0 / 255.0, 1.0]),
            ribbon: [ribbon[0].clone(), ribbon[1].clone(), ribbon[2].clone(), ribbon[3].clone()],
            infinity: create_infinity(11.7, 0.0, 32, WHITE),
            private_stroke: create_rounded_rectangle_stroked((122.0, 122.0), 21.0, 9.0, 16, WHITE),
            spiral: create_segmented_square(R1, d2r(35.0 + 1.0), d2r(35.0 + 1.0 - 10.0 + 180.0), T_FAST_SPIRAL),
            fast_body: create_textured_rectangle((148.0, 148.0), T_FAST_BODY),
            fast_arrow_shadow,
            fast_arrow,
            knot,
            powerful_mask: create_textured_rectangle((200.0, 200.0), T_PMASK),
            powerful_infinity: create_textured_rectangle((id(366, 3), id(180, 3)), T_INF),
            powerful_infinity_white: create_textured_rectangle((id(366, 3), id(180, 3)), T_INF_WHITE),
            telegram_sphere: create_textured_rectangle((150.0, 150.0), T_SPHERE),
            telegram_plane,
            telegram_mask: create_textured_rectangle((200.0, 150.0), T_TMASK),
            star,
            stars: Vec::new(),
            ic_bubble_dot: create_textured_rectangle((id(18, 3), id(18, 3)), T_BUBBLE_DOT),
            ic_bubble: create_textured_rectangle((id(102, 3), id(102, 3)), T_BUBBLE),
            ic_cam_lens: create_textured_rectangle((id(36, 3), id(36, 3)), T_CAM_LENS),
            ic_cam: create_textured_rectangle((id(108, 3), id(96, 3)), T_CAM),
            ic_pencil: create_textured_rectangle((id(86, 3), id(86, 3)), T_PENCIL),
            ic_pin: create_textured_rectangle((id(90, 3), id(120, 3)), T_PIN),
            ic_smile_eye: create_textured_rectangle((id(18, 3), id(18, 3)), T_SMILE_EYE),
            ic_smile: create_textured_rectangle((id(120, 3), id(120, 3)), T_SMILE),
            ic_videocam: create_textured_rectangle((id(144, 3), id(84, 3)), T_VIDEOCAM),
            private_door: create_textured_rectangle((id(408, 3), id(408, 3)), T_DOOR),
            private_screw: create_textured_rectangle((id(30, 3), id(30, 3)), T_SCREW),
        };
        sc.gl.clear = sc.bg;
        for i in 0..STARS {
            let mut p = default_params(Mode::Strip);
            p.position = sc.star_create_position(-(i as f32 * 1500.0) / STARS as f32);
            sc.stars.push(p);
        }
        sc
    }

    pub fn set_page(&mut self, page: i32) {
        if self.cur == page {
            return;
        }
        self.prev = self.cur;
        self.cur = page;
        self.direct = if self.cur > self.prev { 1 } else { 0 };
        self.date0 = self.date;
        self.time = 0.0;
    }
    pub fn set_scroll_offset(&mut self, o: f32) {
        self.scroll_offset = o;
    }
    pub fn set_date(&mut self, d: f32) {
        self.date = d;
    }

    fn frand(&self, from: f32, to: f32) -> f32 {
        #[cfg(unix)]
        let r = unsafe { random() } as f64 / RAND_MAX;
        #[cfg(not(unix))]
        let r = 0.5;
        (r * (to - from) as f64 + from as f64) as f32
    }
    fn irand(&self, from: i32, to: i32) -> i32 {
        #[cfg(unix)]
        let r = unsafe { random() } as f64 / RAND_MAX;
        #[cfg(not(unix))]
        let r = 0.5;
        (r * (to - from + 1) as f64 + from as f64) as i32
    }
    fn signrand(&self) -> i32 {
        self.irand(0, 1) * 2 - 1
    }

    fn star_create_position(&mut self, far: f32) -> Xyz {
        self.stars_far = 1500.0;
        let (a, b) = (self.signrand() as f32 * self.frand(100.0, 1000.0), 0.0);
        let y = self.signrand() as f32 * self.frand(100.0, 1000.0);
        let _ = b;
        xyz(a, y, far)
    }

    fn star_initial_position(&mut self, rand_z: i32, forward: i32) -> Xyz {
        self.stars_far = 1500.0;
        let mut z = 0.0;
        if forward == 1 {
            z = if rand_z == 0 { -self.stars_far } else { self.frand(0.0, -self.stars_far) };
        }
        let x = self.signrand() as f32 * self.frand(100.0, 1000.0);
        let y = self.signrand() as f32 * self.frand(100.0, 1000.0);
        xyz(x, y, z)
    }

    // ---- GL holat yordamchilari / GL state helpers ----
    fn normal_draw(&mut self) {
        self.gl.depth_test = false;
        self.gl.blend_one = false;
        self.gl.color_mask = true;
        self.gl.depth_mask = false;
    }
    fn mask_draw(&mut self) {
        self.gl.depth_test = true;
        self.gl.blend = false;
        self.gl.depth_mask = true;
        self.gl.color_mask = false;
        self.gl.depth_func = DepthFunc::Greater;
        self.gl.clear_depth = 0.0;
        self.gl.clear_depth_buffer();
    }
    fn normal_draw_through_mask(&mut self) {
        self.gl.color_mask = true;
        self.gl.depth_func = DepthFunc::Less;
        self.gl.depth_mask = false;
    }

    fn mat_layer(&self, params: &LayerParams, s: f32, r: f32) -> Mat4 {
        let a = self.main_matrix[1][1];
        let sc = self.main_matrix[3][3];
        let mut model = translate(-params.anchor.x / sc, params.anchor.y / sc * a, params.anchor.z / sc);
        let scaled = scale_aniso(&identity(), params.scale.x, params.scale.y, params.scale.z);
        model = mul(&scaled, &model);
        let rotate = rotate_z2(&identity(), -deg_to_rad(params.rotation));
        model = mul(&rotate, &model);
        translate_independed(&mut model, params.position.x / sc, -params.position.y / sc * a, params.position.z / sc);
        let mut m = mul(&model, &self.main_matrix);
        m[1][0] /= a;
        m[0][1] *= a;
        let mut scale_m = scale_aniso(&identity(), s, s, s);
        scale_m = rotate_z2(&scale_m, r);
        scale_m[1][0] /= a;
        scale_m[0][1] *= a;
        mul(&scale_m, &m)
    }
}

macro_rules! ds {
    ($s:ident, $f:expr, $m:expr) => {{
        let m = $m;
        draw_shape(&mut $s.gl, $s.view.y_offset, &$f, &m)
    }};
}
macro_rules! dts {
    ($s:ident, $f:expr, $m:expr, $p:expr) => {{
        let m = $m;
        draw_textured_shape(&mut $s.gl, &$s.tex, &$s.view, &$f, &m, $p)
    }};
}

impl Scene {
    fn draw_stars(&mut self) {
        let time = self.time;
        let t = move |sv: f32, ev: f32, st: f32, dur: f32, ty: Ty| tf(time, sv, ev, st, dur, ty);
        let update = self.last_stars_fps != self.fps_anim;
        self.last_stars_fps = self.fps_anim;
        self.view.y_offset = 0.0;
        for i in 0..STARS {
            let stars_scroll_offset = self.scroll_offset.max(0.0) * 2.0;
            let transition_speed = if self.direct == 1 {
                let s = 5.0;
                s - t(0.0, s, 0.0, DUR + 1.0 + 0.8, Ty::Linear)
            } else {
                t(-4.0, 0.0, 0.0, DUR + 1.0, Ty::EaseOut)
            };
            let speed = stars_scroll_offset + transition_speed;
            if update {
                self.stars[i].position.z += speed;
            }
            if self.stars[i].position.z > 0.0 && speed > 0.0 {
                self.stars[i].position = self.star_initial_position(0, 1);
            }
            if self.stars[i].position.z < -1500.0 && speed < 0.0 {
                self.stars[i].position = self.star_initial_position(0, 0);
            }
            let inc = self.scroll_offset * 100.0;
            self.stars[i].position.z += inc;
            self.star.params.position = self.stars[i].position;
            let z = self.stars[i].position.z;
            let s = 1.0 + (-z) / self.stars_far * 5.0;
            self.star.params.scale = xyz(s, s, 1.0);
            let far = self.stars_far;
            let a = (1.0 - (-z) / far) * 10.0;
            self.star.params.alpha = a * a / 10.0;
            dts!(self, self.star, self.stars_matrix, Prog::Normal);
            self.stars[i].position.z -= inc;
        }
        self.view.y_offset = 0.0;
    }

    fn draw_safe(&mut self, alpha: f32, screw_alpha: f32) {
        let d = 53.0;
        self.private_screw.params.alpha = alpha * screw_alpha;
        self.private_screw.params.scale = xyz(1.0, 1.0, 1.0);
        for (x, y) in [(-d, -d), (d, -d), (-d, d), (d, d)] {
            self.private_screw.params.position = xyz(x, y, 0.0);
            dts!(self, self.private_screw, self.private_matrix, Prog::NormalOne);
        }
    }

    fn bubble_dots_sin(&self, a: f32) -> f32 {
        if a < PI * 2.0 * self.bubble_dots_end_period as f32 { a.sin() } else { 0.0 }
    }

    fn reset_ic(&mut self) {
        self.smile_t1 = self.time_local;
        self.pencil_start = 0.0;
        self.pencil_all_end = 0.0;
        self.cam_next = self.time_local;
        self.smile_stage = 0;
        self.smile_blink_one = 0;
        self.pencil_stage = 0;
        self.bubble_dots_end_period = 4;
        self.pencil_period = 1;
    }

    fn draw_ic(&mut self, ty: i32) {
        let time = self.time;
        let tl = self.time_local;
        let t = move |sv: f32, ev: f32, st: f32, dur: f32, k: Ty| tf(time, sv, ev, st, dur, k);
        let t_local = move |sv: f32, ev: f32, st: f32, dur: f32, k: Ty| -> f32 {
            if k == Ty::Sin {
                return sv + (ev - sv) * (((tl - st) / dur * PI).max(0.0).min(PI)).sin();
            }
            tf(tl, sv, ev, st, dur, k)
        };
        let begin_y = 250.0f32;
        let (rotation, begin_time_k, common_delay, bounce, color, light) = if ty == 0 {
            self.cloud_scroll = 0.0;
            (-d2r(self.free_scroll), 2.0f32, DUR * 0.5, 1.0f32, Prog::Red, Prog::LightRed)
        } else {
            (0.0, 2.5, 0.0, 1.0, Prog::Blue, Prog::LightBlue)
        };
        let ic_layer_alpha = if self.cur == 1 && self.direct == 0 { t(1.0, 0.0, 0.0, DUR * 0.25, Ty::EaseOut) } else { 1.0 };
        for s in [&mut self.ic_pin, &mut self.ic_cam, &mut self.ic_cam_lens, &mut self.ic_smile, &mut self.ic_smile_eye, &mut self.ic_videocam, &mut self.ic_bubble, &mut self.ic_bubble_dot, &mut self.ic_pencil] {
            s.params.alpha = ic_layer_alpha;
        }
        let bounce_y = |delay_k: f32, phase: f32| -> f32 {
            let arg = (time - (common_delay + DUR * delay_k * begin_time_k)).max(0.0) * 50.0;
            let value = begin_y * 2.71f32.powf(-0.055 * arg * 2.0) * (0.08 * arg - phase).cos() * 0.4;
            t(begin_y, 0.0, common_delay + DUR * delay_k * begin_time_k, DUR, Ty::EaseOut) - value * bounce
        };
        let cs = self.cloud_scroll;
        let skip = self.cur == 1 && self.direct == 0;

        // --- pin ---
        let (mut ic_pos, mut t_y);
        if ty == 0 {
            ic_pos = (id(-106, 2), id(61, 2));
            t_y = if skip { 0.0 } else { bounce_y(0.2, 0.0) };
        } else {
            ic_pos = (id(-162, 2) + 4.0, id(26, 2) + 20.0);
            t_y = bounce_y(0.2, PI / 8.0);
        }
        if tl > self.pin_start && tl > self.pin_start + self.pin_dur {
            self.pin_start = tl + DUR * self.frand(10.0, 20.0) * 2.0;
            self.pin_dur = DUR * self.frand(10.0, 20.0) * 2.0;
        }
        let pinasin = 0.0;
        self.ic_pin_l.position = xyz(ic_pos.0 + (tl * 5.0).cos() * 3.0 * pinasin + cs, ic_pos.1 + (tl * 5.0).sin() * 1.5 * pinasin + t_y, 0.0);
        self.ic_matrix = self.mat_layer(&self.ic_pin_l.clone(), 1.0, rotation);
        dts!(self, self.ic_pin, self.ic_matrix, color);

        // --- videocam (faqat 1-tur) ---
        if ty == 1 {
            self.ic_videocam_l.rotation = -30.0 + t_local(self.videocam_old, self.videocam_angle, self.videocam_start, self.videocam_dur, Ty::EaseOut);
            t_y = bounce_y(0.45, 0.0);
            if t_y <= 1.0 && tl > self.videocam_next {
                self.videocam_dur = DUR * self.frand(1.0, 1.5) * 1.5;
                self.videocam_old = self.videocam_angle;
                self.videocam_angle = 15.0 * self.irand(-1, 1) as f32;
                self.videocam_start = tl;
                self.videocam_next = tl + 1000000.0 + DUR * self.frand(5.0, 8.0);
            }
            self.ic_videocam_l.position = xyz(id(-68, 2) + cs, id(80, 2) + t_y, 0.0);
            self.ic_matrix = self.mat_layer(&self.ic_videocam_l.clone(), 1.0, rotation);
            dts!(self, self.ic_videocam, self.ic_matrix, color);
        }

        // --- cam ---
        if ty == 0 {
            ic_pos = (id(107, 2), id(78, 2));
            t_y = if skip { 0.0 } else { bounce_y(0.3, 0.0) };
        } else {
            ic_pos = (id(-28, 2), id(-20, 2) + 2.0);
            t_y = bounce_y(0.15, 0.0);
        }
        if t_y <= 1.0 && tl > self.cam_next {
            self.cam_dur = DUR * self.frand(1.0, 1.5);
            self.cam_old = self.cam_angle;
            self.cam_start = tl;
            self.cam_next = tl + 10000000.0;
            let r = self.irand(0, 1);
            if r == 0 || r == 1 {
                self.cam_angle = self.signrand() as f32 * 10.0;
            }
            self.q_shot = self.irand(1, 2);
            self.camshot_start = tl + DUR * 0.5;
            self.camshot_dur = DUR * 0.4;
        }
        self.ic_cam_l.rotation = 15.0 + t_local(self.cam_old, self.cam_angle, self.cam_start, self.cam_dur, Ty::EaseOut);
        self.ic_cam_l.position = xyz(ic_pos.0 + cs, ic_pos.1 + t_y, 0.0);
        self.ic_matrix = self.mat_layer(&self.ic_cam_l.clone(), 1.0, rotation);
        dts!(self, self.ic_cam, self.ic_matrix, color);
        let mut lens_scale = 1.0;
        if self.q_shot >= 0 && tl > self.camshot_start {
            lens_scale = t_local(1.0, 0.0, self.camshot_start, self.camshot_dur, Ty::Sin);
            if tl > self.camshot_start + self.camshot_dur {
                self.q_shot -= 1;
                self.camshot_start = tl + self.camshot_dur;
            }
        }
        self.ic_cam_lens.params.scale = xyz(lens_scale, lens_scale, 1.0);
        self.ic_cam_lens.params.position = xyz(0.0, 1.7, 0.0);
        dts!(self, self.ic_cam_lens, self.ic_matrix, color);

        // --- smile ---
        if ty == 0 {
            ic_pos = (id(70, 2), id(-116, 2));
            t_y = if skip { 0.0 } else { bounce_y(0.0, 0.0) };
        } else {
            ic_pos = (id(60, 2), id(50, 2));
            t_y = bounce_y(0.25, PI / 8.0);
        }
        let mut smile_laught = 0.0;
        let fade_dur = DUR * 2.0;
        let smile_dur = DUR * 2.0;
        if self.smile_stage == 0 {
            smile_laught = t_local(0.0, 1.0, self.smile_t1, fade_dur, Ty::Linear);
            if tl > smile_dur * 3.0 + self.smile_t1 {
                self.smile_stage = 1;
                self.smile_t2 = tl;
            }
        }
        if self.smile_stage == 1 {
            smile_laught = t_local(1.0, 0.0, self.smile_t2, fade_dur, Ty::Linear);
            if tl > smile_dur + self.smile_t2 {
                smile_laught = 0.0;
                self.smile_stage = 2;
                self.smile_blink_one = 1;
                self.smile_blink = tl + DUR;
            }
        }
        let mut y = 0.0;
        if self.smile_stage < 2 {
            y = (tl * PI * 10.0).sin() * 1.5 * smile_laught;
        }
        self.ic_smile_l.position = xyz(ic_pos.0 + cs, y + ic_pos.1 + t_y, 0.0);
        self.ic_matrix = self.mat_layer(&self.ic_smile_l.clone(), 1.0, rotation);
        dts!(self, self.ic_smile, self.ic_matrix, color);
        if tl > self.smile_blink + 0.1 {
            let mut blink_pause = self.frand(3.0, 6.0);
            if self.irand(0, 3) == 0 {
                blink_pause = 0.3;
            }
            if self.smile_blink_one == 1 {
                blink_pause = self.frand(3.0, 6.0);
            }
            self.smile_blink = tl + blink_pause;
            self.smile_blink_one = 0;
        }
        let stop_time = 5.0;
        let eye_scale = t_local(1.0, 0.0, self.smile_blink, 0.1, Ty::Sin);
        self.ic_smile_eye.params.scale = xyz(1.0, eye_scale, 1.0);
        if time > stop_time {
            self.ic_smile_eye.params.scale = xyz(1.0, 1.0, 1.0);
        }
        self.ic_smile_eye.params.position = xyz(-7.0, -4.5, 0.0);
        dts!(self, self.ic_smile_eye, self.ic_matrix, color);
        if self.smile_blink_one == 1 {
            self.ic_smile_eye.params.scale = xyz(1.0, 1.0, 1.0);
        }
        if time > stop_time {
            self.ic_smile_eye.params.scale = xyz(1.0, 1.0, 1.0);
        }
        self.ic_smile_eye.params.position = xyz(7.0, -4.5, 0.0);
        dts!(self, self.ic_smile_eye, self.ic_matrix, color);

        // --- bubble ---
        if ty == 0 {
            ic_pos = (id(-60, 2), id(110, 2));
            t_y = if skip { 0.0 } else { bounce_y(0.45, 0.0) };
        } else {
            ic_pos = (id(72, 2), id(-74, 2));
            t_y = bounce_y(0.0, 0.0);
        }
        self.ic_bubble_l.position = xyz(ic_pos.0 + cs, ic_pos.1 + t_y, 0.0);
        self.ic_matrix = self.mat_layer(&self.ic_bubble_l.clone(), 1.0, rotation);
        dts!(self, self.ic_bubble, self.ic_matrix, color);
        let dots = [(0.0f32, -80.5f32), (-PI * 2.0 / 3.0, 0.0), (-PI * 2.0 / 3.0 * 2.0, 80.5)];
        for (k, (ph, px)) in dots.into_iter().enumerate() {
            let mut scale = 0.7 + 0.2 * self.bubble_dots_sin(ph + time * 10.0);
            if k > 0 && self.bubble_dots_stage == 0 {
                scale = scale.max(0.7);
            }
            self.ic_bubble_dot.params.scale = xyz(scale, scale, scale);
            self.ic_bubble_dot.params.position = xyz(px, -9.0 / 2.0, 0.0);
            dts!(self, self.ic_bubble_dot, self.ic_matrix, light);
        }

        // --- pencil ---
        if ty == 0 {
            ic_pos = (id(-88, 2) - 15.0, id(-100, 2) + 13.0);
            t_y = if skip { 0.0 } else { bounce_y(0.1, 0.0) };
        } else {
            ic_pos = (id(152, 2) - 17.0, id(66, 2) + 14.0);
            t_y = bounce_y(0.35, 0.0);
        }
        let mut pencil_x = 0.0;
        if self.pencil_stage == 0 {
            self.ic_pencil_l.rotation = t_local(0.0, -5.0, self.pencil_all_start, DUR * 0.5, Ty::EaseOut);
            pencil_x = t_local(0.0, 14.0, self.pencil_start, 1.5 * 0.85, Ty::Linear);
            if tl > self.pencil_start + 1.5 * 0.85 {
                self.pencil_start = tl;
                self.pencil_stage = 1;
            }
        } else if self.pencil_stage == 1 {
            pencil_x = t_local(14.0, 0.0, self.pencil_start, 1.5 * 0.15, Ty::Linear);
            if tl > self.pencil_start + 1.5 * 0.15 {
                if self.pencil_period == 0 {
                    self.pencil_all_end = tl;
                    self.pencil_start = tl + DUR * 1.0;
                    self.pencil_stage = 2;
                } else {
                    self.pencil_period -= 1;
                    self.pencil_start = tl;
                    self.pencil_stage = 0;
                }
            }
        } else if self.pencil_stage == 2 {
            self.ic_pencil_l.rotation = t_local(-5.0, 0.0, self.pencil_all_end, DUR * 0.5, Ty::EaseOut);
            if tl > self.pencil_start {
                self.pencil_all_start = tl;
                self.pencil_start = tl;
                self.pencil_stage = 3;
            }
        }
        let pencil_v = if self.pencil_stage < 2 { (tl * 2.0 * PI * 4.0).sin() * 0.8 } else { 0.0 };
        self.ic_pencil_l.position = xyz(pencil_x + ic_pos.0 + cs, pencil_v + ic_pos.1 + t_y, 0.0);
        self.ic_matrix = self.mat_layer(&self.ic_pencil_l.clone(), 1.0, rotation);
        dts!(self, self.ic_pencil, self.ic_matrix, color);
    }
}

impl Scene {
    /// `onDrawFrame(deltaMs)`; natija — `to_rgba8` / result via `to_rgba8`.
    pub fn draw_frame(&mut self, delta_ms: i32) {
        self.time_local += delta_ms as f32 / 1000.0;
        if self.cur != self.prev {
            self.reset_ic();
            self.ms0_anim = self.date;
            self.fps_anim = 0;
            self.count_anim_fps = 1;
        }
        let knot_step = 0.075;
        if self.prev != self.cur {
            for i in 0..4 {
                self.knot_delays[i] = (0.65 + knot_step * i as f32) * DUR;
            }
            for _ in 0..10 {
                let j1 = self.irand(0, 3) as usize;
                let j2 = self.irand(0, 3) as usize;
                self.knot_delays.swap(j1.min(3), j2.min(3));
            }
            if self.cur == 2 {
                self.ic_pin_l.rotation = -15.0;
                self.ic_cam_l.rotation = 15.0;
                self.ic_smile_l.rotation = -15.0;
                self.ic_bubble_l.rotation = -15.0;
            }
            if self.cur == 5 {
                self.ic_pin_l.rotation = -15.0;
                self.ic_videocam_l.rotation = -30.0;
                self.ic_cam_l.rotation = 15.0;
                self.ic_smile_l.rotation = -15.0;
                self.ic_bubble_l.rotation = -15.0;
            }
        }
        self.fps_anim = (self.time_local / 0.016) as i32;
        if self.count_anim_fps == 1 && self.date - self.ms0_anim >= DUR {
            self.count_anim_fps = 0;
        }
        if self.date - self.ms0 >= 1.0 {
            self.ms0 = self.date;
        }
        self.time = self.date - self.date0;
        let time = self.time;
        let t = move |sv: f32, ev: f32, st: f32, dur: f32, ty: Ty| tf(time, sv, ev, st, dur, ty);
        // t_reversed(end, start, ...): start + (end - start) * ...
        let tr = move |ev: f32, sv: f32, st: f32, dur: f32, ty: Ty| tf(time, sv, ev, st, dur, ty).max(f32::NEG_INFINITY);
        let _ = &tr;
        let t_rev = move |ev: f32, sv: f32, st: f32, dur: f32, ty: Ty| -> f32 {
            if time > st + dur {
                return ev;
            }
            let k = (dur + st).min((time - st).max(0.0)) / dur;
            if ty == Ty::Linear { sv + (ev - sv) * k } else { sv + (ev - sv) * timing(k, ty) }
        };
        let (cur, direct) = (self.cur, self.direct);
        let back_k = 0.8f32;
        self.gl.clear = self.bg;
        self.gl.clear_color_buffer();

        // ---- 1-bosqich: oldindan chizish va private layer ----
        if cur == 0 {
            self.normal_draw();
            let mut alpha = 1.0;
            if direct == 0 {
                alpha = t(0.0, 1.0, 0.0, DUR, Ty::Linear);
                self.fast_body.params.alpha = 1.0;
                self.fast_body.params.scale = xyz(1.0, 1.0, 1.0);
                dts!(self, self.fast_body, self.main_matrix, Prog::Normal);
            }
            self.telegram_sphere.params.alpha = alpha;
            self.telegram_sphere.params.scale = xyz(1.0, 1.0, 1.0);
            self.telegram_plane.params.alpha = 1.0;
            let tt = (-PI * 125.0 / 180.0 + time * PI * 2.0 * 1.5).min(0.0);
            let (dx, dy) = (tt.sin() * 75.0, -tt.sin() * 60.0);
            self.telegram_plane.params.position = xyz(dx, dy, 0.0);
            let scale = (tt.cos() + 1.0) * 0.5;
            self.telegram_plane.params.scale = xyz(tt.cos() * scale, scale, 1.0);
            if tt < d2r(125.0) {
                self.gl.blend_one = true;
            }
        } else if cur == 1 {
            self.normal_draw();
            if direct == 1 {
                self.fast_body.params.scale = xyz(1.0, 1.0, 1.0);
                self.fast_body.params.alpha = 1.0;
                dts!(self, self.fast_body, self.main_matrix, Prog::Normal);
            } else {
                self.fast_body.params.alpha = t(0.0, 1.0, 0.0, DUR, Ty::Linear);
                let scale = t(0.95, 1.0, 0.0, DUR, Ty::EaseInEaseOut);
                self.fast_body.params.scale = xyz(scale, scale, 1.0);
                dts!(self, self.fast_body, self.main_matrix, Prog::Normal);
            }
        } else if cur == 2 {
            self.normal_draw();
            if direct == 1 {
                self.fast_body.params.alpha = t(1.0, 0.0, 0.0, DUR, Ty::Linear);
                let scale = t(1.0, 0.95, 0.0, DUR, Ty::EaseInEaseOut);
                self.fast_body.params.scale = xyz(scale, scale, 1.0);
                dts!(self, self.fast_body, self.main_matrix, Prog::Normal);
            }
        } else if cur == 4 {
            self.private_layer.rotation = if direct == 1 { self.private_scroll + t(-90.0, 0.0, 0.0, DUR, Ty::EaseOut) } else { self.private_scroll + t(90.0, 0.0, 0.0, DUR * back_k, Ty::EaseOut) };
            self.private_matrix = self.mat_layer(&self.private_layer.clone(), 1.0, 0.0);
        }

        // ---- niqob (depth) ----
        self.mask_draw();
        self.mask1.params.position.z = 1.0;
        for m in self.cloud_extra.iter_mut() {
            m.params.position.z = 1.0;
        }
        let (r1, r2) = (R1, R2);
        if cur == 0 {
            if direct == 0 {
                change_rounded_rectangle(&mut self.mask1, (r1 * 2.0, r1 * 2.0), r1);
                self.mask1.params.rotation = 0.0;
            }
        } else if cur == 1 {
            if direct == 1 {
                change_rounded_rectangle(&mut self.mask1, (r1 * 2.0, r1 * 2.0), r1);
                self.mask1.params.rotation = 0.0;
            } else {
                let size = t(r2 * 2.0, r1 * 2.0, 0.0, DUR, Ty::EaseInEaseOut);
                let round = t(30.0, r1, 0.0, DUR, Ty::EaseInEaseOut);
                change_rounded_rectangle(&mut self.mask1, (size, size), round);
                self.free_scroll = 0.0;
                self.mask1.params.rotation = t(180.0, 0.0, 0.0, DUR, Ty::EaseInEaseOut) + self.free_scroll;
            }
        } else if cur == 2 {
            if direct == 1 {
                let size = t(r1 * 2.0, r2 * 2.0, 0.0, DUR, Ty::EaseInEaseOut);
                let round = t(r1, 30.0, 0.0, DUR, Ty::EaseInEaseOut);
                change_rounded_rectangle(&mut self.mask1, (size, size), round);
                self.free_scroll = self.scroll_offset * 5.0;
                self.mask1.params.rotation = t(0.0, 180.0, 0.0, DUR, Ty::EaseInEaseOut) + self.free_scroll;
            } else {
                self.free_scroll = self.scroll_offset * 5.0;
                let r = 316.0 / 4.0;
                let size = t_rev(r2 * 2.0, r * 2.0, 0.0, DUR, Ty::EaseInEaseOut);
                let round = t_rev(30.0, 20.0, 0.0, DUR, Ty::EaseInEaseOut);
                change_rounded_rectangle(&mut self.mask1, (size, size), round);
                self.mask1.params.rotation = t_rev(180.0 + self.free_scroll, 180.0 + 90.0, 0.0, DUR, Ty::EaseInEaseOut);
            }
        } else if cur == 3 {
            if direct == 1 {
                let r = 316.0 / 4.0;
                let size = t(r2 * 2.0, r * 2.0, 0.0, DUR, Ty::EaseInEaseOut);
                let round = t(30.0, 20.0, 0.0, DUR, Ty::EaseInEaseOut);
                change_rounded_rectangle(&mut self.mask1, (size, size), round);
                self.mask1.params.rotation = t(180.0 + self.free_scroll, 180.0 + 90.0, 0.0, DUR, Ty::EaseInEaseOut);
            } else {
                let r = 316.0 / 4.0;
                let size = t_rev(r * 2.0, r2 * 2.0, 0.0, DUR, Ty::EaseOut);
                let round = t_rev(20.0, 30.0, 0.0, DUR, Ty::EaseOut);
                change_rounded_rectangle(&mut self.mask1, (size, size), round);
                self.mask1.params.rotation = t_rev(180.0 + 90.0, 180.0 + 90.0 + 90.0, 0.0, DUR, Ty::EaseOut);
                self.mask1.params.position = xyz(0.0, 0.0, self.mask1.params.position.z);
            }
        } else if cur == 4 {
            if direct == 1 {
                let r = 316.0 / 4.0;
                let size = t(r * 2.0, r2 * 2.0, 0.0, DUR, Ty::EaseOut);
                let round = t(20.0, 30.0, 0.0, DUR, Ty::EaseOut);
                change_rounded_rectangle(&mut self.mask1, (size, size), round);
                self.mask1.params.rotation = self.private_scroll + t(180.0 + 90.0, 180.0 + 90.0 + 90.0, 0.0, DUR, Ty::EaseOut);
                self.mask1.params.position = xyz(0.0, 0.0, self.mask1.params.position.z);
            } else {
                let k = 1.0 * back_k;
                let scale = t_rev(r2 * 2.0, 100.0, 0.0, DUR * k, Ty::EaseOut);
                change_rounded_rectangle(&mut self.mask1, (scale, scale), t_rev(30.0, 50.0, 0.0, DUR * k, Ty::EaseOut));
                self.mask1.params.position = xyz(t_rev(0.0, id(29, 2), 0.0, DUR * k, Ty::EaseOut), t_rev(0.0, id(-19, 2), 0.0, DUR * k, Ty::EaseOut), self.mask1.params.position.z);
                self.mask1.params.rotation = self.private_scroll + t_rev(180.0 + 90.0 + 90.0, 180.0 + 90.0 + 90.0 + 90.0, 0.0, DUR * k, Ty::EaseOut);
                // bulut qo'shimcha niqoblari / cloud extra masks
                let defs = [(1.0f32, id(-122, 2), id(54, 2) - 1.0, 33.0f32), (1.15, id(-84, 2), id(-29, 2), id(94, 4)), (1.3, id(128, 2), id(56, 2), id(124, 4)), (1.5, 0.0, 50.0, 64.0)];
                for (i, (kk, px, py, rad)) in defs.into_iter().enumerate() {
                    let k = kk * back_k;
                    let z = self.cloud_extra[i].params.position.z;
                    self.cloud_extra[i].params.position = xyz(t_rev(0.0, px, 0.0, DUR * k, Ty::EaseOut), t_rev(0.0, py, 0.0, DUR * k, Ty::EaseOut), z);
                    let s = t_rev(0.0, rad, 0.0, DUR * k, Ty::EaseOut);
                    self.cloud_extra[i].params.scale = xyz(s, s, 1.0);
                    ds!(self, self.cloud_extra[i], self.main_matrix);
                }
            }
        } else if cur == 5 {
            let k = 0.8;
            let scale = t(r2 * 2.0, 100.0, 0.0, DUR * k, Ty::EaseOut);
            change_rounded_rectangle(&mut self.mask1, (scale, scale), t(30.0, 50.0, 0.0, DUR * k, Ty::EaseOut));
            self.mask1.params.position = xyz(t(0.0, id(29, 2), 0.0, DUR * k, Ty::EaseOut), t(0.0, id(-19, 2), 0.0, DUR * k, Ty::EaseOut), self.mask1.params.position.z);
            self.mask1.params.rotation = t(180.0 + 90.0 + 90.0, 180.0 + 90.0 + 90.0 + 90.0, 0.0, DUR * k, Ty::EaseOut);
            let defs = [(1.0f32, id(-122, 2), id(54, 2) - 1.0, 33.0f32), (1.15, id(-84, 2), id(-29, 2), id(94, 4)), (1.3, id(128, 2), id(56, 2), id(124, 4)), (1.5, 0.0, 50.0, 64.0)];
            for (i, (kk, px, py, rad)) in defs.into_iter().enumerate() {
                let z = self.cloud_extra[i].params.position.z;
                self.cloud_extra[i].params.position = xyz(t(0.0, px, 0.0, DUR * kk, Ty::EaseOut), t(0.0, py, 0.0, DUR * kk, Ty::EaseOut), z);
                let s = t(0.0, rad, 0.0, DUR * kk, Ty::EaseOut);
                self.cloud_extra[i].params.scale = xyz(s, s, 1.0);
                ds!(self, self.cloud_extra[i], self.main_matrix);
            }
        }
        ds!(self, self.mask1, self.main_matrix);
        self.normal_draw_through_mask();
        self.draw_pages(cur, direct, back_k);
        self.prev = self.cur;
    }
}

impl Scene {
    fn draw_pages(&mut self, cur: i32, direct: i32, back_k: f32) {
        let time = self.time;
        let t = move |sv: f32, ev: f32, st: f32, dur: f32, ty: Ty| tf(time, sv, ev, st, dur, ty);
        let t_rev = move |ev: f32, sv: f32, st: f32, dur: f32, ty: Ty| -> f32 {
            if time > st + dur {
                return ev;
            }
            let k = (dur + st).min((time - st).max(0.0)) / dur;
            if ty == Ty::Linear { sv + (ev - sv) * k } else { sv + (ev - sv) * timing(k, ty) }
        };
        let (r1, r2) = (R1, R2);
        let (rr, seg, ang) = (30.0f32, 15.0f32, 180.0f32);
        let mm = self.main_matrix;
        if cur == 0 {
            if direct == 0 {
                self.gl.blend = true;
                self.gl.blend_one = true;
                let a = d2r(self.speedo_scroll + self.calc_speedo_sin + t(-seg + ang, 0.0, 0.0, DUR, Ty::EaseOut));
                change_segmented_square(&mut self.spiral, r1, d2r(rr + seg), a);
                self.spiral.params.scale = xyz(1.0, 1.0, 1.0);
                self.spiral.params.rotation = t(180.0, 0.0, 0.0, DUR, Ty::EaseOut);
                self.spiral.params.alpha = t(1.0, 0.0, 0.0, DUR, Ty::Linear);
                dts!(self, self.spiral, mm, Prog::NormalOne);
                let al = t(1.0, 0.0, 0.0, DUR, Ty::Linear);
                self.fast_arrow.params.alpha = al;
                self.fast_arrow_shadow.params.alpha = al;
                let rot = t(rr, rr - 180.0 - 160.0, 0.0, DUR, Ty::EaseOut) + self.speedo_scroll + self.calc_speedo_sin;
                self.fast_arrow.params.rotation = rot;
                self.fast_arrow_shadow.params.rotation = rot;
                dts!(self, self.fast_arrow_shadow, mm, Prog::NormalOne);
                dts!(self, self.fast_arrow, mm, Prog::NormalOne);
            }
        } else if cur == 1 {
            self.gl.blend = true;
            self.gl.blend_one = true;
            self.speedo_scroll = self.scroll_offset * 25.0;
            self.calc_speedo_sin = 0.0;
            if direct == 1 {
                let arg = time * 50.0;
                let value = 180.0 - 180.0 * 2.71f32.powf(-0.055 * arg * 2.0) * (0.08 * arg * 3.0).cos();
                let ta = t(0.0, 180.0, 0.0, DUR, Ty::EaseOut);
                change_segmented_square(&mut self.spiral, r1, d2r(rr + seg), d2r(-seg + value + self.speedo_scroll));
                self.spiral.params.scale = xyz(1.0, 1.0, 1.0);
                self.spiral.params.rotation = ta;
                self.spiral.params.alpha = t(0.0, 1.0, 0.0, DUR, Ty::Linear);
                dts!(self, self.spiral, mm, Prog::NormalOne);
                let al = t(0.0, 1.0, 0.0, DUR, Ty::Linear);
                self.fast_arrow.params.alpha = al;
                self.fast_arrow_shadow.params.alpha = al;
                let rot = -330.0 + value + ta + self.speedo_scroll;
                self.fast_arrow.params.rotation = rot;
                self.fast_arrow_shadow.params.rotation = rot;
                dts!(self, self.fast_arrow_shadow, mm, Prog::NormalOne);
                dts!(self, self.fast_arrow, mm, Prog::NormalOne);
            } else {
                self.spiral.params.alpha = 1.0;
                self.fast_arrow.params.alpha = 1.0;
                self.fast_arrow_shadow.params.alpha = 1.0;
                let arg = time * 50.0;
                let dangle = 90.0;
                let mut value = 180.0 - 90.0 - (180.0 - 90.0) * 2.71f32.powf(-0.055 * arg * 2.0) * (0.08 * arg * 3.0).cos();
                value *= -1.0;
                change_segmented_square(&mut self.spiral, r1, d2r(rr + seg), d2r(self.speedo_scroll + value + self.calc_speedo_sin + t(360.0, 360.0 - dangle - seg, 0.0, DUR, Ty::EaseInEaseOut)));
                let scale = t(1.18, 1.0, 0.0, DUR, Ty::EaseInEaseOut);
                self.spiral.params.scale = xyz(scale, scale, 1.0);
                self.spiral.params.rotation = t(360.0, 180.0, 0.0, DUR, Ty::EaseInEaseOut);
                dts!(self, self.spiral, mm, Prog::Normal);
                let rot = self.speedo_scroll + value + self.calc_speedo_sin + t(rr + 360.0 + 6.0, rr + 360.0 - 180.0 - dangle, 0.0, DUR, Ty::EaseInEaseOut);
                self.fast_arrow.params.rotation = rot;
                self.fast_arrow_shadow.params.rotation = rot;
                dts!(self, self.fast_arrow_shadow, mm, Prog::Normal);
                dts!(self, self.fast_arrow, mm, Prog::Normal);
                self.gl.blend_one = false;
                self.free_bg.params.alpha = t(1.0, 0.0, 0.0, DUR, Ty::Linear);
                ds!(self, self.free_bg, mm);
                self.draw_ic(0);
            }
        } else if cur == 2 {
            self.gl.blend = true;
            self.gl.blend_one = true;
            if direct == 1 {
                self.spiral.params.alpha = 1.0;
                self.fast_arrow.params.alpha = 1.0;
                self.fast_arrow_shadow.params.alpha = 1.0;
                change_segmented_square(&mut self.spiral, r1, d2r(rr + seg + self.speedo_scroll), d2r(t(-seg + ang, 360.0, 0.0, DUR, Ty::EaseInEaseOut)));
                let scale = t(1.0, 1.18, 0.0, DUR, Ty::EaseInEaseOut);
                self.spiral.params.scale = xyz(scale, scale, 1.0);
                self.spiral.params.rotation = t(180.0, 360.0, 0.0, DUR, Ty::EaseInEaseOut);
                dts!(self, self.spiral, mm, Prog::Normal);
                let rot = self.speedo_scroll + t(rr, rr + 360.0 + 6.0, 0.0, DUR, Ty::EaseInEaseOut);
                self.fast_arrow.params.rotation = rot;
                self.fast_arrow_shadow.params.rotation = rot;
                dts!(self, self.fast_arrow_shadow, mm, Prog::Normal);
                dts!(self, self.fast_arrow, mm, Prog::Normal);
                self.gl.blend_one = false;
                self.free_bg.params.alpha = t(0.0, 1.0, 0.0, DUR, Ty::Linear);
                ds!(self, self.free_bg, mm);
                self.draw_ic(0);
            } else {
                self.gl.blend = false;
                self.free_bg.params.alpha = 1.0;
                ds!(self, self.free_bg, mm);
                self.gl.blend = true;
                self.gl.blend_one = false;
                self.draw_ic(0);
                self.powerful_bg.params.alpha = t_rev(0.0, 1.0, 0.0, DUR, Ty::Linear);
                ds!(self, self.powerful_bg, mm);
            }
            self.ribbon[0].params.rotation = 0.0;
            self.ribbon[1].params.rotation = 90.0;
            self.ribbon[2].params.rotation = 180.0;
            self.ribbon[3].params.rotation = 270.0;
        } else if cur == 3 {
            if direct == 1 {
                self.gl.blend = false;
                self.free_bg.params.alpha = 1.0;
                ds!(self, self.free_bg, mm);
                self.gl.blend = true;
                self.gl.blend_one = false;
                self.powerful_bg.params.alpha = t(0.0, 1.0, 0.0, DUR, Ty::Linear);
                ds!(self, self.powerful_bg, mm);
                self.draw_stars();
            } else {
                self.gl.blend = false;
                self.private_bg.params.alpha = 1.0;
                ds!(self, self.private_bg, mm);
                let a = t(0.0, 1.0, 0.0, DUR, Ty::EaseOut);
                self.gl.blend = true;
                self.gl.blend_one = false;
                self.powerful_bg.params.rotation = 0.0;
                self.powerful_bg.params.alpha = a;
                ds!(self, self.powerful_bg, mm);
                self.draw_stars();
            }
        } else if cur == 4 {
            if direct == 1 {
                self.gl.blend = false;
                self.powerful_bg.params.alpha = 1.0;
                ds!(self, self.powerful_bg, mm);
                let a = t(0.0, 1.0, 0.0, DUR, Ty::EaseOut);
                self.gl.blend = true;
                self.gl.blend_one = false;
                self.private_bg.params.rotation = t(45.0, 0.0, 0.0, DUR, Ty::EaseOut);
                self.private_bg.params.alpha = a;
                ds!(self, self.private_bg, mm);
            } else {
                self.gl.blend = false;
                self.cloud_bg.params.alpha = 1.0;
                ds!(self, self.cloud_bg, mm);
                let a = t(0.0, 1.0, 0.0, DUR * back_k, Ty::EaseOut);
                self.gl.blend = true;
                self.gl.blend_one = false;
                self.private_bg.params.alpha = a;
                ds!(self, self.private_bg, mm);
            }
        } else if cur == 5 {
            self.gl.blend = false;
            self.private_bg.params.alpha = 1.0;
            ds!(self, self.private_bg, mm);
            let a = t(0.0, 1.0, 0.0, DUR, Ty::EaseOut);
            self.gl.blend = true;
            self.gl.blend_one = false;
            self.cloud_bg.params.alpha = a;
            ds!(self, self.cloud_bg, mm);
            self.cloud_scroll = if self.scroll_offset > 0.0 { -self.scroll_offset * 40.0 } else { -self.scroll_offset * 15.0 };
            self.draw_ic(1);
        }
        self.draw_top(cur, direct, back_k);
    }
}

impl Scene {
    fn draw_knots(&mut self, pos: [(f32, f32); 4], flip: [bool; 4], t: impl Fn(f32, f32, f32, f32, Ty) -> f32, dur: f32, layer: Mat4) {
        // C kodi: free_knot3 ikki marta chiziladi (3 va 4-chi tugun uchun) / the C code draws free_knot3 twice (for knots 3 and 4).
        let idx = [0usize, 1, 2, 2];
        for k in 0..4 {
            let i = idx[k];
            self.knot[i].params.position = xyz(pos[k].0, pos[k].1, 0.0);
            let s = t(0.0, 1.0, self.knot_delays[k], dur, Ty::EaseOut);
            self.knot[i].params.scale = if flip[k] { xyz(-s, s, 1.0) } else { xyz(s, s, 1.0) };
            dts!(self, self.knot[i], layer, Prog::NormalOne);
        }
    }

    fn infinity_mask_and_white(&mut self, rot: f32, infinity_ang: f32, white_rot: f32) {
        self.mask_draw();
        change_infinity(&mut self.infinity, infinity_ang);
        self.infinity.params.rotation = rot;
        self.infinity.params.position.z = 1.0;
        self.infinity.params.position.y = -6.0;
        self.infinity.params.anchor = xyz(52.75, 23.5, 0.0);
        let s = 1.025;
        self.infinity.params.scale = xyz(s, s, 1.0);
        let mm = self.main_matrix;
        ds!(self, self.infinity, mm);
        self.infinity.params.scale = xyz(-s, -s, 1.0);
        ds!(self, self.infinity, mm);
        self.normal_draw_through_mask();
        self.gl.blend = true;
        self.gl.blend_one = true;
        self.powerful_infinity_white.params.rotation = white_rot;
        self.powerful_infinity_white.params.alpha = 1.0;
        self.powerful_infinity_white.params.position.y = -6.0;
        dts!(self, self.powerful_infinity_white, mm, Prog::NormalOne);
    }

    fn draw_top(&mut self, cur: i32, direct: i32, back_k: f32) {
        let time = self.time;
        let t = move |sv: f32, ev: f32, st: f32, dur: f32, ty: Ty| tf(time, sv, ev, st, dur, ty);
        let t_rev = move |ev: f32, sv: f32, st: f32, dur: f32, ty: Ty| -> f32 {
            if time > st + dur {
                return ev;
            }
            let k = (dur + st).min((time - st).max(0.0)) / dur;
            if ty == Ty::Linear { sv + (ev - sv) * k } else { sv + (ev - sv) * timing(k, ty) }
        };
        let mm = self.main_matrix;
        let r2 = R2;
        if cur == 0 {
            self.normal_draw();
            if direct == 0 {
                self.telegram_sphere.params.alpha = t(0.0, 1.0, 0.0, DUR * 0.8, Ty::Linear);
                self.telegram_sphere.params.scale = xyz(1.0, 1.0, 1.0);
                dts!(self, self.telegram_sphere, mm, Prog::Normal);
                let tt = (-PI * 125.0 / 180.0 + time * PI * 2.0 * 1.5).min(0.0);
                let dx = t(-75.0, 0.0, 0.0, 0.15, Ty::EaseIn);
                let dy = t(75.0, 0.0, 0.0, 0.15, Ty::EaseIn);
                self.telegram_plane.params.position = xyz(dx, dy, 0.0);
                let scale = t(0.1, 1.0, 0.03, 0.15, Ty::EaseOut);
                self.telegram_plane.params.scale = xyz(scale, scale, 1.0);
                if tt < d2r(125.0) {
                    self.gl.blend_one = true;
                    dts!(self, self.telegram_plane, mm, Prog::NormalOne);
                    dts!(self, self.telegram_mask, mm, Prog::Normal);
                }
            }
        } else if cur == 1 {
            self.normal_draw();
            if direct == 1 {
                self.telegram_sphere.params.alpha = t(1.0, 0.0, 0.0, DUR, Ty::Linear);
                dts!(self, self.telegram_sphere, mm, Prog::Normal);
                let tt = time * PI * 2.0 * 1.5;
                let dx = t(0.0, 75.0, 0.0, 0.15, Ty::EaseOut);
                let dy = t(0.0, -75.0, 0.0, 0.15, Ty::EaseOut);
                self.telegram_plane.params.position = xyz(dx, dy, 0.0);
                let scale = t(1.0, 0.1, 0.03, 0.15, Ty::EaseOut);
                self.telegram_plane.params.scale = xyz(scale, scale, 1.0);
                if tt < d2r(125.0) {
                    self.gl.blend_one = true;
                    dts!(self, self.telegram_plane, mm, Prog::NormalOne);
                    dts!(self, self.telegram_mask, mm, Prog::Normal);
                }
            }
        } else if cur == 2 {
            self.normal_draw();
            let dribbon = 87.0;
            if direct == 1 {
                self.gl.blend_one = true;
                self.ribbon_layer.rotation = self.scroll_offset * 5.0 + t(180.0, 360.0, 0.0, DUR, Ty::EaseInEaseOut);
                self.ribbons_layer = self.mat_layer(&self.ribbon_layer.clone(), 1.0, 0.0);
                let dur = DUR * 0.5;
                let rl = self.ribbons_layer;
                self.draw_knots([(5.0, -5.0 - 9.0), (-5.0, -5.0 - 9.0), (-5.0, 5.0 - 9.0), (5.0, 5.0 - 9.0)], [false, true, true, false], t, dur, rl);
                self.gl.blend_one = false;
                let a = t(0.0, 1.0, 0.0, dur, Ty::EaseInEaseOut);
                for r in self.ribbon.iter_mut() {
                    r.params.alpha = a;
                }
                let ribbon_k = if time > DUR { 1.0 } else { 0.0 };
                let rlen = self.ribbon_length;
                let fs = self.free_scroll;
                change_ribbon(&mut self.ribbon[0], rlen - 8.0 * ribbon_k - fs / 5.0 * (30.0 - 8.0 * ribbon_k));
                self.ribbon[0].params.position.x = self.scroll_offset * 30.0 * 0.0 + t(-dribbon, 0.0, 0.0, DUR, Ty::EaseInEaseOut);
                ds!(self, self.ribbon[0], rl);
                change_ribbon(&mut self.ribbon[1], rlen - 10.0 * ribbon_k - fs / 5.0 * (22.0 - 10.0 * ribbon_k));
                self.ribbon[1].params.position.y = self.scroll_offset * 15.0 + t(-9.0 - dribbon, -9.0, 0.0, DUR, Ty::EaseInEaseOut);
                ds!(self, self.ribbon[1], rl);
                self.ribbon[2].params.position.x = t(dribbon, 0.0, 0.0, DUR, Ty::EaseInEaseOut);
                ds!(self, self.ribbon[2], rl);
                self.ribbon[3].params.position.y = t(-9.0 + dribbon, -9.0, 0.0, DUR, Ty::EaseInEaseOut);
                ds!(self, self.ribbon[3], rl);
                self.ribbon_layer.anchor.y = 0.0;
                self.ribbon_layer.position.y = 0.0;
                for r in self.ribbon.iter_mut() {
                    change_ribbon(r, rlen);
                }
            } else {
                self.gl.blend_one = true;
                let scale = t(1.0, 2.0, 0.0, DUR, Ty::EaseIn);
                self.powerful_mask.params.scale = xyz(scale, scale, 1.0);
                dts!(self, self.powerful_mask, mm, Prog::NormalOne);
                self.ribbon_layer.rotation = self.free_scroll + t_rev(360.0, 360.0 + (45.0 + 30.0), 0.0, DUR, Ty::EaseOut);
                self.ribbon_layer.position.y = t_rev(0.0, -8.0, 0.0, DUR * 0.8, Ty::EaseOut);
                self.ribbon_layer.anchor.y = t_rev(0.0, -9.0, 0.0, DUR * 0.8, Ty::EaseOut);
                self.ribbons_layer = self.mat_layer(&self.ribbon_layer.clone(), 1.0, 0.0);
                let rl = self.ribbons_layer;
                let dur = DUR * 0.5;
                self.draw_knots([(id(11, 2), id(-11, 2) - 9.0), (id(-11, 2), id(-11, 2) - 9.0), (id(-11, 2), id(11, 2) - 9.0), (id(11, 2), id(11, 2) - 9.0)], [false, true, true, false], t, dur, rl);
                let a1 = -25.0;
                self.ribbon[0].params.rotation = t_rev(0.0, a1, 0.0, DUR, Ty::EaseOut);
                self.ribbon[2].params.rotation = t_rev(180.0, 180.0 + a1, 0.0, DUR, Ty::EaseOut);
                let a2 = 0.0;
                self.ribbon[1].params.rotation = t_rev(90.0, 90.0 + a2, 0.0, DUR, Ty::EaseOut);
                self.ribbon[3].params.rotation = t_rev(270.0, 270.0 + a2, 0.0, DUR, Ty::EaseOut);
                let k = 0.9;
                let a = t_rev(1.0, 0.0, DUR * 0.5, DUR * 0.1, Ty::Linear);
                self.ribbon[1].params.alpha = a;
                self.ribbon[3].params.alpha = a;
                let ribbon_k = 0.0;
                let rlen = self.ribbon_length;
                let fs = self.free_scroll;
                let l0 = t_rev(rlen - 8.0 * ribbon_k, 0.0, 0.0, DUR * 0.9, Ty::Linear) - fs / 5.0 * (30.0 - 8.0 * ribbon_k);
                change_ribbon(&mut self.ribbon[0], l0);
                self.ribbon[0].params.position.x = 0.0;
                ds!(self, self.ribbon[0], rl);
                let l1 = t_rev(rlen - 10.0 * ribbon_k, 0.0, 0.0, DUR * k, Ty::Linear) - fs / 5.0 * (22.0 - 10.0 * ribbon_k);
                change_ribbon(&mut self.ribbon[1], l1);
                self.ribbon[1].params.position.y = self.scroll_offset * 15.0 + -9.0;
                ds!(self, self.ribbon[1], rl);
                let l2 = t_rev(rlen, 0.0, 0.0, DUR * 0.9, Ty::Linear);
                change_ribbon(&mut self.ribbon[2], l2);
                ds!(self, self.ribbon[2], rl);
                let l3 = t_rev(rlen, 0.0, DUR * 0.6 * 0.0, DUR * k, Ty::Linear);
                change_ribbon(&mut self.ribbon[3], l3);
                ds!(self, self.ribbon[3], rl);
                let inf_dur_k = 1.3;
                let ang = t_rev(0.0, 0.99, 0.0, DUR * inf_dur_k, Ty::EaseOut);
                let rot1 = t(0.0, -50.0, DUR * 0.5, DUR * 0.8, Ty::EaseOut);
                let rot2 = t(0.0, -30.0, DUR * 0.8, DUR, Ty::EaseOut);
                self.infinity_mask_and_white(rot1, ang, rot1 + rot2);
            }
        } else if cur == 3 {
            if direct == 1 {
                self.ribbon[0].params.position.x = 0.0;
                self.ribbon[1].params.position.y = -9.0;
                self.ribbon[2].params.position.x = 0.0;
                self.ribbon[3].params.position.y = -9.0;
                self.normal_draw();
                self.gl.blend_one = true;
                let scale = t(2.0, 1.0, 0.0, DUR, Ty::EaseOut);
                self.powerful_mask.params.scale = xyz(scale, scale, 1.0);
                dts!(self, self.powerful_mask, mm, Prog::NormalOne);
                self.gl.blend_one = false;
                self.ribbon_layer.rotation = self.free_scroll + t(360.0, 360.0 + (45.0 + 30.0), 0.0, DUR * 0.8, Ty::EaseOut);
                self.ribbon_layer.position.y = t(0.0, -8.0, 0.0, DUR * 0.8, Ty::EaseOut);
                self.ribbon_layer.anchor.y = t(0.0, -9.0, 0.0, DUR * 0.8, Ty::EaseOut);
                self.ribbons_layer = self.mat_layer(&self.ribbon_layer.clone(), 1.0, 0.0);
                let rl = self.ribbons_layer;
                let a1 = -25.0;
                self.ribbon[0].params.rotation = t(0.0, a1, 0.0, DUR, Ty::EaseOut);
                self.ribbon[2].params.rotation = t(180.0, 180.0 + a1, 0.0, DUR, Ty::EaseOut);
                let a2 = 0.0;
                self.ribbon[1].params.rotation = t(90.0, 90.0 + a2, 0.0, DUR, Ty::EaseOut);
                self.ribbon[3].params.rotation = t(270.0, 270.0 + a2, 0.0, DUR, Ty::EaseOut);
                let k = 0.5;
                let a = t(1.0, 0.0, DUR * k * 0.5, DUR * k * 0.1, Ty::Linear);
                self.ribbon[1].params.alpha = a;
                self.ribbon[3].params.alpha = a;
                let ribbon_k = if time > DUR { 1.0 } else { 0.0 };
                let rlen = self.ribbon_length;
                let fs = self.free_scroll;
                let l0 = t(rlen - 8.0 * ribbon_k - fs / 5.0 * (30.0 - 8.0 * ribbon_k), 0.0, 0.0, DUR * 0.9, Ty::Linear);
                change_ribbon(&mut self.ribbon[0], l0);
                ds!(self, self.ribbon[0], rl);
                let l1 = t(rlen - 10.0 * ribbon_k - fs / 5.0 * (22.0 - 10.0 * ribbon_k), 0.0, 0.0, DUR * k, Ty::Linear);
                change_ribbon(&mut self.ribbon[1], l1);
                ds!(self, self.ribbon[1], rl);
                let l2 = t(rlen, 0.0, 0.0, DUR * 0.9, Ty::Linear);
                change_ribbon(&mut self.ribbon[2], l2);
                ds!(self, self.ribbon[2], rl);
                let l3 = t(rlen, 0.0, 0.0, DUR * k, Ty::Linear);
                change_ribbon(&mut self.ribbon[3], l3);
                ds!(self, self.ribbon[3], rl);
                let inf_dur_k = 1.1;
                if time < DUR * inf_dur_k - 0.025 {
                    let ang = t(0.0, 0.99, 0.0, DUR * inf_dur_k, Ty::Linear);
                    self.infinity_mask_and_white(0.0, ang, 0.0);
                } else {
                    self.normal_draw();
                    self.gl.blend_one = true;
                    self.powerful_infinity.params.position.y = -6.0;
                    self.powerful_infinity.params.alpha = 1.0;
                    dts!(self, self.powerful_infinity, mm, Prog::NormalOne);
                }
            } else {
                self.normal_draw();
                self.gl.blend_one = true;
                let scale = t(2.0, 1.0, 0.0, DUR, Ty::EaseOut);
                self.powerful_mask.params.scale = xyz(scale, scale, 1.0);
                dts!(self, self.powerful_mask, mm, Prog::NormalOne);
                self.gl.blend_one = false;
                let scale = t(1.0, 2.0, 0.0, DUR, Ty::EaseOut);
                self.private_stroke.params.scale = xyz(scale, scale, 1.0);
                self.private_stroke.params.rotation = t(0.0, -90.0, 0.0, DUR, Ty::EaseOut);
                self.private_stroke.params.alpha = t(1.0, 0.0, 0.0, DUR, Ty::Linear);
                self.private_stroke.params.position = xyz(0.0, t(0.0, -6.0, 0.0, DUR, Ty::EaseOut), 0.0);
                let scale = t_rev(63.0 * 2.0, 63.0 * 2.0, 0.0, DUR, Ty::EaseOut);
                change_rounded_rectangle_stroked(&mut self.private_stroke, (scale, scale), scale / 2.0);
                ds!(self, self.private_stroke, mm);
                let inf_dur_k = 1.1;
                if time < DUR * inf_dur_k - 0.025 {
                    let ang = t(0.0, 0.99, 0.0, DUR * inf_dur_k, Ty::Linear);
                    self.infinity_mask_and_white(0.0, ang, 0.0);
                } else {
                    self.normal_draw();
                    self.gl.blend_one = true;
                    self.powerful_infinity.params.position.y = -6.0;
                    self.powerful_infinity.params.alpha = 1.0;
                    dts!(self, self.powerful_infinity, mm, Prog::NormalOne);
                }
            }
        } else if cur == 4 {
            self.private_stroke.params.scale = xyz(1.0, 1.0, 1.0);
            self.private_scroll = self.scroll_offset * 5.0;
            self.normal_draw();
            self.gl.blend_one = true;
            let scale = t(1.0, 2.0, 0.0, DUR, Ty::EaseOut);
            if scale < 1.5 {
                self.powerful_mask.params.scale = xyz(scale, scale, 1.0);
            }
            self.private_layer.rotation = if direct == 1 { self.private_scroll + t(-90.0, 0.0, 0.0, DUR, Ty::EaseOut) } else { self.private_scroll + t(90.0, 0.0, 0.0, DUR * back_k, Ty::EaseOut) };
            self.private_matrix = self.mat_layer(&self.private_layer.clone(), 1.0, 0.0);
            if direct == 1 {
                self.gl.blend_one = true;
                self.powerful_infinity.params.position.y = -6.0;
                self.powerful_infinity.params.alpha = t(1.0, 0.0, 0.0, DUR * 0.25, Ty::EaseIn);
                dts!(self, self.powerful_infinity, mm, Prog::NormalOne);
            }
            self.gl.blend_one = false;
            if direct == 1 {
                self.gl.blend_one = true;
                let scale = t(0.5, 1.0, 0.0, DUR, Ty::EaseOut);
                self.private_door.params.scale = xyz(scale, scale, 1.0);
                self.private_door.params.alpha = t(0.0, 1.0, 0.0, DUR, Ty::EaseOut);
                dts!(self, self.private_door, mm, Prog::NormalOne);
                self.gl.blend_one = false;
                self.private_stroke.params.rotation = self.private_scroll;
                self.private_stroke.params.alpha = 1.0;
                self.private_stroke.params.position = xyz(0.0, 0.0, 0.0);
                let scale = t(63.0, 63.0 * 2.0, 0.0, DUR, Ty::EaseOut);
                change_rounded_rectangle_stroked(&mut self.private_stroke, (scale, scale), scale / 2.0);
                ds!(self, self.private_stroke, mm);
                self.gl.blend_one = true;
                let al = t(0.0, 1.0, 0.0, DUR, Ty::Linear);
                self.draw_safe(1.0, al);
            } else {
                self.gl.blend_one = true;
                let scale = t(0.5, 1.0, 0.0, DUR * back_k, Ty::EaseOut);
                self.private_door.params.scale = xyz(scale, scale, 1.0);
                self.private_door.params.alpha = t(0.0, 1.0, 0.0, DUR * back_k, Ty::EaseOut);
                dts!(self, self.private_door, mm, Prog::NormalOne);
                self.gl.blend_one = false;
                self.private_stroke.params.rotation = self.private_scroll;
                self.private_stroke.params.alpha = t(0.0, 1.0, 0.0, DUR * 0.25, Ty::Linear);
                self.private_stroke.params.position = xyz(0.0, 0.0, 0.0);
                let scale = t(63.0, 63.0 * 2.0, 0.0, DUR * back_k, Ty::EaseOut);
                change_rounded_rectangle_stroked(&mut self.private_stroke, (scale, scale), scale / 2.0);
                ds!(self, self.private_stroke, mm);
                self.gl.blend_one = true;
                if time < DUR * 0.4 {
                    self.cloud_cover.params.position.y = t_rev(id(118, 2) + 50.0, id(118, 2), DUR * 0.8 * back_k, DUR * back_k, Ty::EaseOut);
                    ds!(self, self.cloud_cover, mm);
                }
                let a = t(0.0, 1.0, DUR * back_k * 0.0, DUR * back_k, Ty::Linear);
                let sa = t(0.0, 1.0, 0.0, DUR, Ty::Linear);
                self.draw_safe(a, sa);
            }
        } else if cur == 5 {
            let private_fade_k = 0.5;
            self.normal_draw();
            self.gl.blend_one = true;
            self.private_door.params.scale = xyz(1.0, 1.0, 1.0);
            self.private_door.params.alpha = t(1.0, 0.0, 0.0, DUR * private_fade_k * 0.5, Ty::EaseOut);
            dts!(self, self.private_door, mm, Prog::NormalOne);
            self.gl.blend_one = false;
            self.private_stroke.params.rotation = self.private_scroll;
            self.private_stroke.params.alpha = t(1.0, 0.0, 0.0, DUR * private_fade_k * 0.5, Ty::EaseOut);
            let scale = t(id(244, 2), r2 * 2.0, 0.0, DUR, Ty::EaseOut);
            change_rounded_rectangle_stroked(&mut self.private_stroke, (scale, scale), scale / 2.0);
            ds!(self, self.private_stroke, mm);
            self.gl.blend_one = true;
            self.private_layer.rotation = self.private_scroll;
            let s = t(1.0, 0.9, 0.0, DUR * private_fade_k, Ty::EaseOut);
            self.private_matrix = self.mat_layer(&self.private_layer.clone(), s, 0.0);
            self.cloud_cover.params.position.y = t(id(118, 2) + 50.0, id(118, 2), 0.0, DUR, Ty::EaseOut);
            ds!(self, self.cloud_cover, mm);
        }
    }

    /// RGBA8 (yuqoridan pastga) / RGBA8 top-down.
    pub fn to_rgba8(&self) -> Vec<u8> {
        self.gl.to_rgba8()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn har_sahifa_chiziladi() {
        let bg = [0.094, 0.094, 0.098];
        let mut sc = Scene::new(crate::intro_assets::load(bg, 1.5), bg, 300, 225, 1.5);
        for page in 0..6 {
            sc.set_page(page);
            for i in 0..40 {
                sc.set_date(1.0 + (page as f32 * 40.0 + i as f32) * 0.016);
                sc.draw_frame(16);
            }
            let px = sc.to_rgba8();
            let bgb = (bg[0] * 255.0 + 0.5) as u8;
            // kadrda fondan boshqa piksellar bor / the frame has non-background pixels
            assert!(px.chunks(4).any(|p| p[0].abs_diff(bgb) > 20), "sahifa {page} bo'sh");
        }
    }
}
