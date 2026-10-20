//! OpenGL ES 2.0 bajaruvchi: `Frame` ni chizadi. Faqat Android (`libGLESv2`).

use crate::batch::{Frame, Op, Vertex, ATLAS_SIZE};
use std::ffi::c_void;

#[link(name = "GLESv2")]
extern "C" {
    fn glCreateShader(t: u32) -> u32;
    fn glShaderSource(s: u32, n: i32, src: *const *const u8, len: *const i32);
    fn glCompileShader(s: u32);
    fn glGetShaderiv(s: u32, p: u32, out: *mut i32);
    fn glGetShaderInfoLog(s: u32, max: i32, len: *mut i32, log: *mut u8);
    fn glCreateProgram() -> u32;
    fn glAttachShader(p: u32, s: u32);
    fn glBindAttribLocation(p: u32, i: u32, name: *const u8);
    fn glLinkProgram(p: u32);
    fn glGetProgramiv(p: u32, pn: u32, out: *mut i32);
    fn glGetProgramInfoLog(p: u32, max: i32, len: *mut i32, log: *mut u8);
    fn glUseProgram(p: u32);
    fn glGetUniformLocation(p: u32, name: *const u8) -> i32;
    fn glUniform2f(l: i32, x: f32, y: f32);
    fn glUniform1i(l: i32, v: i32);
    fn glGenBuffers(n: i32, out: *mut u32);
    fn glBindBuffer(t: u32, b: u32);
    fn glBufferData(t: u32, size: isize, data: *const c_void, usage: u32);
    fn glEnableVertexAttribArray(i: u32);
    fn glVertexAttribPointer(i: u32, size: i32, t: u32, norm: u8, stride: i32, ptr: *const c_void);
    fn glGenTextures(n: i32, out: *mut u32);
    fn glBindTexture(t: u32, tex: u32);
    fn glTexImage2D(
        t: u32,
        lvl: i32,
        ifmt: i32,
        w: i32,
        h: i32,
        border: i32,
        fmt: u32,
        ty: u32,
        data: *const c_void,
    );
    fn glTexSubImage2D(
        t: u32,
        lvl: i32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        fmt: u32,
        ty: u32,
        data: *const c_void,
    );
    fn glTexParameteri(t: u32, p: u32, v: i32);
    fn glPixelStorei(p: u32, v: i32);
    fn glActiveTexture(t: u32);
    fn glViewport(x: i32, y: i32, w: i32, h: i32);
    fn glClearColor(r: f32, g: f32, b: f32, a: f32);
    fn glClear(m: u32);
    fn glEnable(c: u32);
    fn glDisable(c: u32);
    fn glScissor(x: i32, y: i32, w: i32, h: i32);
    fn glBlendFuncSeparate(a: u32, b: u32, c: u32, d: u32);
    fn glDrawArrays(mode: u32, first: i32, count: i32);
    fn glGetError() -> u32;
    fn glGenFramebuffers(n: i32, out: *mut u32);
    fn glBindFramebuffer(t: u32, f: u32);
    fn glFramebufferTexture2D(t: u32, att: u32, tt: u32, tex: u32, lvl: i32);
    fn glCheckFramebufferStatus(t: u32) -> u32;
    fn glUniform1f(l: i32, v: f32);
    fn glDeleteTextures(n: i32, t: *const u32);
    fn glDisableVertexAttribArray(i: u32);
}

const VS: &[u8] = b"attribute vec2 a_pos;
attribute vec2 a_uv;
attribute vec4 a_color;
attribute vec4 a_p;
attribute float a_q;
uniform vec2 u_scale;
varying vec2 v_uv;
varying vec4 v_color;
varying vec4 v_p;
varying float v_q;
void main() {
  gl_Position = vec4(a_pos.x * u_scale.x - 1.0, 1.0 - a_pos.y * u_scale.y, 0.0, 1.0);
  v_uv = a_uv; v_color = a_color; v_p = a_p; v_q = a_q;
}\0";

const FS: &[u8] = b"#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
uniform sampler2D u_tex;
uniform sampler2D u_img;
uniform sampler2D u_blur;
uniform sampler2D u_cap;
uniform vec2 u_view;
uniform float u_blur_ok;
varying vec2 v_uv;
varying vec4 v_color;
varying vec4 v_p;
varying float v_q;
void main() {
  if (v_p.x < -1.5) {
    vec4 c = texture2D(u_img, v_uv);
    gl_FragColor = vec4(c.rgb, c.a * v_color.a);
  } else if (v_p.x < 0.0) {
    gl_FragColor = vec4(v_color.rgb, v_color.a * texture2D(u_tex, v_uv).a);
  } else if (v_p.w < -3.5) {
    float d = length(v_uv);
    float w = v_p.y;
    float ring = clamp(0.5 - (abs(d - (v_p.x - w * 0.5)) - w * 0.5), 0.0, 1.0);
    float sweep = -v_p.w - 4.0;
    float rel = mod(atan(v_uv.y, v_uv.x) - v_p.z, 6.2831853);
    float ca = clamp(min(rel * d, (sweep - rel) * d) + 0.5, 0.0, 1.0);
    gl_FragColor = vec4(v_color.rgb, v_color.a * ring * ca);
  } else if (v_p.w < -2.5) {
    float d = length(v_uv) / v_p.x;
    float k = clamp(1.0 - d, 0.0, 1.0);
    gl_FragColor = vec4(v_color.rgb, v_color.a * k * k);
  } else if (v_p.w < -0.5) {
    vec2 q = abs(v_uv) - (v_p.xy - v_p.z);
    float sd = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - v_p.z;
    float cov = clamp(0.5 - sd, 0.0, 1.0);
    float rim = cov - clamp(0.5 - (sd + 1.5), 0.0, 1.0);
    vec2 dir = normalize(v_uv + vec2(0.001));
    float lit = 0.35 + 0.65 * max(dot(dir, vec2(-0.7071, -0.7071)), 0.0);
    float glow = clamp(1.0 + sd / 14.0, 0.0, 1.0) * cov;
    float sheen = clamp(0.5 - v_uv.y / (2.0 * v_p.y), 0.0, 1.0) * cov;
    float hl = rim * lit * 0.85 + glow * 0.10 * lit + sheen * 0.07;
    if (v_p.w < -1.5) {
      // Haqiqiy shisha: ostidagi xiralashgan tasvir + tint, to'yinganlik biroz oshiriladi.
      vec3 base = v_color.rgb;
      if (u_blur_ok > 0.5) {
        if (v_q > 1.001) {
          // Lupa: ostidagi (o'tkir) tasvir panel markazidan kattalashadi, chetida kuchliroq sinadi.
          vec2 pos = gl_FragCoord.xy;
          vec2 local_gl = vec2(v_uv.x, -v_uv.y);
          vec2 center_gl = pos - local_gl;
          float edge = clamp((sd + 16.0) / 16.0, 0.0, 1.0);
          float z = v_q + 0.55 * edge * edge;
          vec2 src = center_gl + local_gl / z;
          base = texture2D(u_cap, src / u_view).rgb;
          float l = dot(base, vec3(0.299, 0.587, 0.114));
          base = mix(vec3(l), base, 1.12);
          base = mix(base, v_color.rgb, v_color.a);
        } else {
          base = texture2D(u_blur, gl_FragCoord.xy / u_view).rgb;
          float l = dot(base, vec3(0.299, 0.587, 0.114));
          base = mix(vec3(l), base, 1.25);
          base = mix(base, v_color.rgb, v_color.a);
        }
      }
      float total = clamp(cov + hl, 0.0, 1.0);
      vec3 col = mix(base, vec3(1.0), clamp(hl / max(total, 0.001), 0.0, 1.0));
      gl_FragColor = vec4(col, total);
    } else {
      float a = v_color.a * cov;
      float total = clamp(a + hl, 0.0, 1.0);
      vec3 col = mix(v_color.rgb, vec3(1.0), clamp(hl / max(total, 0.001), 0.0, 1.0));
      gl_FragColor = vec4(col, total);
    }
  } else {
    vec2 q = abs(v_uv) - (v_p.xy - v_p.z);
    float sd = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - v_p.z;
    float cov = clamp(0.5 - sd, 0.0, 1.0);
    if (v_p.w > 0.0) cov -= clamp(0.5 - (sd + v_p.w), 0.0, 1.0);
    gl_FragColor = vec4(v_color.rgb, v_color.a * cov);
  }
}\0";

const BVS: &[u8] = b"attribute vec2 a_pos;
varying vec2 v_uv;
void main() { gl_Position = vec4(a_pos * 2.0 - 1.0, 0.0, 1.0); v_uv = a_pos; }\0";

const BFS: &[u8] = b"precision mediump float;
uniform sampler2D u_tex;
uniform vec2 u_step;
uniform float u_down;
varying vec2 v_uv;
void main() {
  if (u_down < -0.5) {
    gl_FragColor = texture2D(u_tex, v_uv);
  } else if (u_down > 0.5) {
    vec4 c = texture2D(u_tex, v_uv + u_step * vec2(-1.0, -1.0)) + texture2D(u_tex, v_uv + u_step * vec2(1.0, -1.0))
           + texture2D(u_tex, v_uv + u_step * vec2(-1.0, 1.0)) + texture2D(u_tex, v_uv + u_step * vec2(1.0, 1.0));
    gl_FragColor = c * 0.25;
  } else {
    vec4 c = texture2D(u_tex, v_uv) * 0.2270270270;
    c += (texture2D(u_tex, v_uv + u_step * 1.3846153846) + texture2D(u_tex, v_uv - u_step * 1.3846153846)) * 0.3162162162;
    c += (texture2D(u_tex, v_uv + u_step * 3.2307692308) + texture2D(u_tex, v_uv - u_step * 3.2307692308)) * 0.0702702703;
    gl_FragColor = c;
  }
}\0";

/// Xiralashtirish (backdrop) uchun resurslar: ekran nusxasi va chorak o'lchamli ikkita tekstura.
struct BlurRes {
    program: u32,
    quad: u32,
    fbo: [u32; 2],
    /// Sahna (to'liq o'lcham) FBO: shisha panellar ostidagi tasvir shu teksturadan olinadi (ekranni nusxalash o'rniga).
    scene_fbo: u32,
    tex: [u32; 3], // 0: ekran nusxasi (to'liq o'lcham), 1 va 2: chorak o'lcham
    w: i32,
    h: i32,
    qw: i32,
    qh: i32,
    u_tex: i32,
    u_step: i32,
    u_down: i32,
}

pub struct GlRenderer {
    /// Katta rasmlar (poster) uchun alohida teksturalar.
    big: std::collections::HashMap<u32, u32>,
    /// Hozir 1-birlikka bog'langan: atlas (`None`) yoki alohida rasm.
    cur_img: Option<u32>,
    /// Sahna teksturasiga chizilmoqda: u’ni shu paytda namuna sifatida bog'lamaymiz (qayta aloqa halqasi bo'lmasin).
    scene_phase: bool,
    blur: Option<BlurRes>,
    blur_failed: bool,
    u_view: i32,
    u_blur: i32,
    u_blur_ok: i32,
    u_cap: i32,
    program: u32,
    vbo: u32,
    tex: u32,
    img_tex: u32,
    u_scale: i32,
}

unsafe fn shader(kind: u32, src: &[u8]) -> Result<u32, String> {
    let s = glCreateShader(kind);
    let p = src.as_ptr();
    glShaderSource(s, 1, &p, std::ptr::null());
    glCompileShader(s);
    let mut ok = 0;
    glGetShaderiv(s, 0x8B81, &mut ok);
    if ok == 0 {
        let mut buf = [0u8; 512];
        let mut n = 0;
        glGetShaderInfoLog(s, 512, &mut n, buf.as_mut_ptr());
        return Err(String::from_utf8_lossy(&buf[..n.max(0) as usize]).into_owned());
    }
    Ok(s)
}

impl GlRenderer {
    /// GL konteksti joriy bo'lgan oqimda chaqiriladi.
    ///
    /// # Safety
    /// EGL konteksti shu oqimda joriy bo'lishi shart.
    pub unsafe fn new() -> Result<GlRenderer, String> {
        let vs = shader(0x8B31, VS)?;
        let fs = shader(0x8B30, FS)?;
        let program = glCreateProgram();
        glAttachShader(program, vs);
        glAttachShader(program, fs);
        for (i, n) in [&b"a_pos\0"[..], b"a_uv\0", b"a_color\0", b"a_p\0", b"a_q\0"]
            .iter()
            .enumerate()
        {
            glBindAttribLocation(program, i as u32, n.as_ptr());
        }
        glLinkProgram(program);
        let mut ok = 0;
        glGetProgramiv(program, 0x8B82, &mut ok);
        if ok == 0 {
            let mut buf = [0u8; 512];
            let mut n = 0;
            glGetProgramInfoLog(program, 512, &mut n, buf.as_mut_ptr());
            return Err(String::from_utf8_lossy(&buf[..n.max(0) as usize]).into_owned());
        }
        let u_scale = glGetUniformLocation(program, b"u_scale\0".as_ptr());
        let u_tex = glGetUniformLocation(program, b"u_tex\0".as_ptr());
        let u_img = glGetUniformLocation(program, b"u_img\0".as_ptr());
        let (mut vbo, mut tex, mut img_tex) = (0u32, 0u32, 0u32);
        glGenBuffers(1, &mut vbo);
        glGenTextures(1, &mut tex);
        glGenTextures(1, &mut img_tex);
        glActiveTexture(0x84C1);
        glBindTexture(0x0DE1, img_tex);
        glPixelStorei(0x0CF5, 1);
        glTexImage2D(
            0x0DE1,
            0,
            0x1908,
            ATLAS_SIZE as i32,
            ATLAS_SIZE as i32,
            0,
            0x1908,
            0x1401,
            std::ptr::null(),
        );
        for (p, v) in [
            (0x2801, 0x2601),
            (0x2800, 0x2601),
            (0x2802, 0x812F),
            (0x2803, 0x812F),
        ] {
            glTexParameteri(0x0DE1, p, v);
        }
        glActiveTexture(0x84C0);
        glBindTexture(0x0DE1, tex);
        glPixelStorei(0x0CF5, 1);
        let sz = ATLAS_SIZE as i32;
        glTexImage2D(
            0x0DE1,
            0,
            0x1906,
            sz,
            sz,
            0,
            0x1906,
            0x1401,
            std::ptr::null(),
        );
        for (p, v) in [
            (0x2801, 0x2600),
            (0x2800, 0x2600),
            (0x2802, 0x812F),
            (0x2803, 0x812F),
        ] {
            glTexParameteri(0x0DE1, p, v);
        }
        glUseProgram(program);
        glUniform1i(u_tex, 0);
        glUniform1i(u_img, 1);
        let err = glGetError();
        if err != 0 {
            return Err(format!("GL xato 0x{err:x}"));
        }
        Ok(GlRenderer {
            big: std::collections::HashMap::new(),
            cur_img: None,
            scene_phase: false,
            blur: None,
            blur_failed: false,
            u_view: glGetUniformLocation(program, b"u_view\0".as_ptr()),
            u_blur: glGetUniformLocation(program, b"u_blur\0".as_ptr()),
            u_blur_ok: glGetUniformLocation(program, b"u_blur_ok\0".as_ptr()),
            u_cap: glGetUniformLocation(program, b"u_cap\0".as_ptr()),
            program,
            vbo,
            tex,
            img_tex,
            u_scale,
        })
    }

    /// Asosiy dastur holatini tiklaydi (dastur, tekstura birliklari, uchlar ko'rsatkichlari).
    unsafe fn bind_main(&mut self, w: i32, h: i32) {
        glBindFramebuffer(0x8D40, 0);
        glViewport(0, 0, w, h);
        glEnable(0x0BE2);
        self.cur_img = None;
        self.bind_main_inner(w, h);
    }

    unsafe fn bind_main_inner(&self, w: i32, h: i32) {
        glUseProgram(self.program);
        glUniform2f(self.u_scale, 2.0 / w as f32, 2.0 / h as f32);
        glUniform2f(self.u_view, w as f32, h as f32);
        glUniform1i(self.u_blur, 2);
        glUniform1i(self.u_cap, 3);
        glActiveTexture(0x84C3);
        glBindTexture(
            0x0DE1,
            if self.scene_phase {
                self.tex
            } else {
                self.blur.as_ref().map_or(self.tex, |b| b.tex[0])
            },
        );
        glUniform1f(self.u_blur_ok, if self.blur.is_some() { 1.0 } else { 0.0 });
        glActiveTexture(0x84C2);
        glBindTexture(0x0DE1, self.blur.as_ref().map_or(self.tex, |b| b.tex[1]));
        glActiveTexture(0x84C1);
        glBindTexture(0x0DE1, self.img_tex);
        glActiveTexture(0x84C0);
        glBindTexture(0x0DE1, self.tex);
        glBindBuffer(0x8892, self.vbo);
        let stride = std::mem::size_of::<Vertex>() as i32;
        glEnableVertexAttribArray(0);
        glVertexAttribPointer(0, 2, 0x1406, 0, stride, 0 as *const c_void);
        glEnableVertexAttribArray(1);
        glVertexAttribPointer(1, 2, 0x1406, 0, stride, 8 as *const c_void);
        glEnableVertexAttribArray(2);
        glVertexAttribPointer(2, 4, 0x1401, 1, stride, 16 as *const c_void);
        glEnableVertexAttribArray(3);
        glVertexAttribPointer(3, 4, 0x1406, 0, stride, 20 as *const c_void);
        glEnableVertexAttribArray(4);
        glVertexAttribPointer(4, 1, 0x1406, 0, stride, 36 as *const c_void);
    }

    /// Xiralashtirish resurslarini (o'lcham o'zgarsa qaytadan) yaratadi. `false` — imkonsiz.
    unsafe fn ensure_blur(&mut self, w: i32, h: i32) -> bool {
        if self.blur_failed {
            return false;
        }
        if let Some(b) = &self.blur {
            if b.w == w && b.h == h {
                return true;
            }
        }
        let (qw, qh) = ((w / 4).max(1), (h / 4).max(1));
        let mk = |tw: i32, th: i32| -> u32 {
            let mut t = 0u32;
            glGenTextures(1, &mut t);
            glBindTexture(0x0DE1, t);
            glTexImage2D(
                0x0DE1,
                0,
                0x1908,
                tw,
                th,
                0,
                0x1908,
                0x1401,
                std::ptr::null(),
            );
            for (p, v) in [
                (0x2801, 0x2601),
                (0x2800, 0x2601),
                (0x2802, 0x812F),
                (0x2803, 0x812F),
            ] {
                glTexParameteri(0x0DE1, p, v);
            }
            t
        };
        let tex = [mk(w, h), mk(qw, qh), mk(qw, qh)];
        let mut fbo = [0u32; 2];
        glGenFramebuffers(2, fbo.as_mut_ptr());
        for i in 0..2 {
            glBindFramebuffer(0x8D40, fbo[i]);
            glFramebufferTexture2D(0x8D40, 0x8CE0, 0x0DE1, tex[i + 1], 0);
            if glCheckFramebufferStatus(0x8D40) != 0x8CD5 {
                glBindFramebuffer(0x8D40, 0);
                self.blur_failed = true;
                return false;
            }
        }
        let mut scene_fbo = 0u32;
        glGenFramebuffers(1, &mut scene_fbo);
        glBindFramebuffer(0x8D40, scene_fbo);
        glFramebufferTexture2D(0x8D40, 0x8CE0, 0x0DE1, tex[0], 0);
        if glCheckFramebufferStatus(0x8D40) != 0x8CD5 {
            glBindFramebuffer(0x8D40, 0);
            self.blur_failed = true;
            return false;
        }
        glBindFramebuffer(0x8D40, 0);
        let (program, quad) = match &self.blur {
            Some(b) => (b.program, b.quad),
            None => {
                let (Ok(vs), Ok(fs)) = (shader(0x8B31, BVS), shader(0x8B30, BFS)) else {
                    self.blur_failed = true;
                    return false;
                };
                let p = glCreateProgram();
                glAttachShader(p, vs);
                glAttachShader(p, fs);
                glBindAttribLocation(p, 0, b"a_pos\0".as_ptr());
                glLinkProgram(p);
                let mut ok = 0;
                glGetProgramiv(p, 0x8B82, &mut ok);
                if ok == 0 {
                    self.blur_failed = true;
                    return false;
                }
                let mut q = 0u32;
                glGenBuffers(1, &mut q);
                glBindBuffer(0x8892, q);
                let quad: [f32; 12] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0];
                glBufferData(0x8892, 48, quad.as_ptr() as *const c_void, 0x88E4);
                (p, q)
            }
        };
        self.blur = Some(BlurRes {
            program,
            quad,
            fbo,
            scene_fbo,
            tex,
            w,
            h,
            qw,
            qh,
            u_tex: glGetUniformLocation(program, b"u_tex\0".as_ptr()),
            u_step: glGetUniformLocation(program, b"u_step\0".as_ptr()),
            u_down: glGetUniformLocation(program, b"u_down\0".as_ptr()),
        });
        true
    }

    /// `region` (yuqori-chap boshlang'ich, piksel) ni xiralashtiradi: ekran nusxasi -> chorak o'lcham -> gorizontal -> vertikal.
    unsafe fn run_blur(&mut self, region: [i32; 4], blur: f32, w: i32, h: i32) {
        if !self.ensure_blur(w, h) {
            return;
        }
        let Some(b) = &self.blur else { return };
        let [x, y, rw, rh] = region;
        if rw <= 0 || rh <= 0 {
            return;
        }
        let gy = h - (y + rh);
        glDisable(0x0C11);
        glActiveTexture(0x84C0);
        glBindTexture(0x0DE1, b.tex[0]);
        glDisable(0x0BE2);
        glUseProgram(b.program);
        glBindBuffer(0x8892, b.quad);
        for i in 1..5 {
            glDisableVertexAttribArray(i);
        }
        glEnableVertexAttribArray(0);
        glVertexAttribPointer(0, 2, 0x1406, 0, 8, 0 as *const c_void);
        glUniform1i(b.u_tex, 0);
        glEnable(0x0C11);
        glScissor(
            (x / 4 - 2).max(0),
            (gy / 4 - 2).max(0),
            rw / 4 + 5,
            rh / 4 + 5,
        );
        let s = (blur / 16.0).clamp(0.4, 3.0);
        let passes: [(u32, u32, i32, i32, f32, (f32, f32), f32); 3] = [
            (
                b.fbo[0],
                b.tex[0],
                b.qw,
                b.qh,
                1.0,
                (1.0 / w as f32, 1.0 / h as f32),
                1.0,
            ),
            (
                b.fbo[1],
                b.tex[1],
                b.qw,
                b.qh,
                0.0,
                (s / b.qw as f32, 0.0),
                0.0,
            ),
            (
                b.fbo[0],
                b.tex[2],
                b.qw,
                b.qh,
                0.0,
                (0.0, s / b.qh as f32),
                0.0,
            ),
        ];
        for (fbo, tex, vw, vh, down, step, _) in
            passes.into_iter().take(if blur > 0.5 { 3 } else { 0 })
        {
            glBindFramebuffer(0x8D40, fbo);
            glViewport(0, 0, vw, vh);
            glBindTexture(0x0DE1, tex);
            glUniform1f(b.u_down, down);
            glUniform2f(b.u_step, step.0, step.1);
            glDrawArrays(4, 0, 6);
        }
        glDisable(0x0C11);
    }

    /// Kadrni joriy sirtga chizadi (almashtirish — chaqiruvchida).
    ///
    /// # Safety
    /// `new` bilan bir xil kontekst joriy bo'lishi shart.
    pub unsafe fn render(&mut self, frame: &Frame, w: i32, h: i32) {
        glBindFramebuffer(0x8D40, 0);
        glViewport(0, 0, w, h);
        glDisable(0x0C11);
        glClearColor(0.0, 0.0, 0.0, 1.0);
        glClear(0x4000);
        glEnable(0x0BE2);
        glBlendFuncSeparate(0x302, 0x303, 1, 0x303);
        glBindBuffer(0x8892, self.vbo);
        glBufferData(
            0x8892,
            (frame.verts.len() * std::mem::size_of::<Vertex>()) as isize,
            frame.verts.as_ptr() as *const c_void,
            0x88E0,
        );
        // Shisha (backdrop) bor kadr: birinchi `Blur` gacha bo'lgan hamma narsa sahna teksturasiga chiziladi,
        // keyin ekranga ko'chiriladi va shisha panellar shu tekstura ustidan (nusxa olmasdan) tanlaydi.
        let first_blur = frame.ops.iter().position(|o| matches!(o, Op::Blur { .. }));
        let mut start = 0;
        let mut scene = false;
        if let Some(k) = first_blur {
            if self.ensure_blur(w, h) {
                if let Some(b) = &self.blur {
                    glBindFramebuffer(0x8D40, b.scene_fbo);
                    glViewport(0, 0, w, h);
                    glClearColor(0.0, 0.0, 0.0, 1.0);
                    glClear(0x4000);
                }
                self.scene_phase = true;
                self.bind_main_fb(w, h);
                self.exec(&frame.ops[..k], w, h, false);
                self.scene_phase = false;
                self.present_scene(w, h);
                start = k;
                scene = true;
            }
        }
        if !scene {
            self.bind_main(w, h);
        }
        self.exec(&frame.ops[start..], w, h, scene);
    }

    /// `bind_main`, lekin joriy FBO'ni o'zgartirmaydi.
    unsafe fn bind_main_fb(&self, w: i32, h: i32) {
        glViewport(0, 0, w, h);
        glEnable(0x0BE2);
        glBlendFuncSeparate(0x302, 0x303, 1, 0x303);
        self.bind_main_inner(w, h);
    }

    /// Sahna teksturasini ekranga (asosiy framebuffer) chizadi.
    unsafe fn present_scene(&mut self, w: i32, h: i32) {
        let Some(b) = &self.blur else { return };
        glBindFramebuffer(0x8D40, 0);
        glViewport(0, 0, w, h);
        glDisable(0x0C11);
        glDisable(0x0BE2);
        glUseProgram(b.program);
        glBindBuffer(0x8892, b.quad);
        for i in 1..5 {
            glDisableVertexAttribArray(i);
        }
        glEnableVertexAttribArray(0);
        glVertexAttribPointer(0, 2, 0x1406, 0, 8, 0 as *const c_void);
        glActiveTexture(0x84C0);
        glBindTexture(0x0DE1, b.tex[0]);
        glUniform1i(b.u_tex, 0);
        glUniform1f(b.u_down, -1.0);
        glUniform2f(b.u_step, 0.0, 0.0);
        glDrawArrays(4, 0, 6);
        self.bind_main(w, h);
    }

    /// Operatsiyalarni bajaradi. `scene` — sahna teksturasi tayyor (shisha uchun xiralashtirish mumkin).
    unsafe fn exec(&mut self, ops: &[Op], w: i32, h: i32, scene: bool) {
        for op in ops {
            match op {
                Op::Upload { x, y, w, h, data } => {
                    glTexSubImage2D(
                        0x0DE1,
                        0,
                        *x as i32,
                        *y as i32,
                        *w as i32,
                        *h as i32,
                        0x1906,
                        0x1401,
                        data.as_ptr() as *const c_void,
                    );
                }
                Op::Blur { region, blur } => {
                    // Lupa (blur ~ 0) uchun xiralashtirish kerak emas: bo'sh o'tkazishlar va holatni qayta bog'lash tejaladi.
                    if scene && *blur > 0.5 {
                        self.run_blur(*region, *blur, w, h);
                        self.bind_main(w, h);
                    }
                }
                Op::ClearAtlas | Op::ClearImages => {}
                Op::UploadImage { x, y, w, h, data } => {
                    glActiveTexture(0x84C1);
                    glTexSubImage2D(
                        0x0DE1,
                        0,
                        *x as i32,
                        *y as i32,
                        *w as i32,
                        *h as i32,
                        0x1908,
                        0x1401,
                        data.as_ptr() as *const c_void,
                    );
                    glActiveTexture(0x84C0);
                }
                Op::NewImage { id, w, h, data } => {
                    let mut t = 0u32;
                    glGenTextures(1, &mut t);
                    glActiveTexture(0x84C1);
                    glBindTexture(0x0DE1, t);
                    glTexImage2D(
                        0x0DE1,
                        0,
                        0x1908,
                        *w as i32,
                        *h as i32,
                        0,
                        0x1908,
                        0x1401,
                        data.as_ptr() as *const c_void,
                    );
                    for (p, v) in [
                        (0x2801, 0x2601),
                        (0x2800, 0x2601),
                        (0x2802, 0x812F),
                        (0x2803, 0x812F),
                    ] {
                        glTexParameteri(0x0DE1, p, v);
                    }
                    // Atlasni qayta 1-birlikka bog'laymiz.
                    glBindTexture(0x0DE1, self.img_tex);
                    glActiveTexture(0x84C0);
                    self.big.insert(*id, t);
                }
                Op::FreeImage(id) => {
                    if let Some(t) = self.big.remove(id) {
                        glDeleteTextures(1, &t);
                    }
                }
                Op::Draw {
                    scissor,
                    first,
                    count,
                    image,
                } => {
                    match scissor {
                        Some([sx, sy, sw, sh]) => {
                            glEnable(0x0C11);
                            glScissor(*sx, h - (*sy + *sh), *sw, *sh);
                        }
                        None => glDisable(0x0C11),
                    }
                    // Alohida rasm teksturasi (poster) yoki atlas.
                    if *image != self.cur_img {
                        glActiveTexture(0x84C1);
                        let t = image
                            .and_then(|id| self.big.get(&id).copied())
                            .unwrap_or(self.img_tex);
                        glBindTexture(0x0DE1, t);
                        glActiveTexture(0x84C0);
                        self.cur_img = *image;
                    }
                    glDrawArrays(4, *first as i32, *count as i32);
                }
            }
        }
    }
}
