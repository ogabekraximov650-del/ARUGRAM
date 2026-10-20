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
}

const VS: &[u8] = b"attribute vec2 a_pos;
attribute vec2 a_uv;
attribute vec4 a_color;
attribute vec4 a_p;
uniform vec2 u_scale;
varying vec2 v_uv;
varying vec4 v_color;
varying vec4 v_p;
void main() {
  gl_Position = vec4(a_pos.x * u_scale.x - 1.0, 1.0 - a_pos.y * u_scale.y, 0.0, 1.0);
  v_uv = a_uv; v_color = a_color; v_p = a_p;
}\0";

const FS: &[u8] = b"#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
uniform sampler2D u_tex;
uniform sampler2D u_img;
varying vec2 v_uv;
varying vec4 v_color;
varying vec4 v_p;
void main() {
  if (v_p.x < -1.5) {
    vec4 c = texture2D(u_img, v_uv);
    gl_FragColor = vec4(c.rgb, c.a * v_color.a);
  } else if (v_p.x < 0.0) {
    gl_FragColor = vec4(v_color.rgb, v_color.a * texture2D(u_tex, v_uv).a);
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
    float a = v_color.a * cov;
    float total = clamp(a + hl, 0.0, 1.0);
    vec3 col = mix(v_color.rgb, vec3(1.0), clamp(hl / max(total, 0.001), 0.0, 1.0));
    gl_FragColor = vec4(col, total);
  } else {
    vec2 q = abs(v_uv) - (v_p.xy - v_p.z);
    float sd = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - v_p.z;
    float cov = clamp(0.5 - sd, 0.0, 1.0);
    if (v_p.w > 0.0) cov -= clamp(0.5 - (sd + v_p.w), 0.0, 1.0);
    gl_FragColor = vec4(v_color.rgb, v_color.a * cov);
  }
}\0";

pub struct GlRenderer {
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
        for (i, n) in [&b"a_pos\0"[..], b"a_uv\0", b"a_color\0", b"a_p\0"]
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
            program,
            vbo,
            tex,
            img_tex,
            u_scale,
        })
    }

    /// Kadrni joriy sirtga chizadi (almashtirish — chaqiruvchida).
    ///
    /// # Safety
    /// `new` bilan bir xil kontekst joriy bo'lishi shart.
    pub unsafe fn render(&mut self, frame: &Frame, w: i32, h: i32) {
        glViewport(0, 0, w, h);
        glDisable(0x0C11);
        glClearColor(0.0, 0.0, 0.0, 1.0);
        glClear(0x4000);
        glEnable(0x0BE2);
        glBlendFuncSeparate(0x302, 0x303, 1, 0x303);
        glUseProgram(self.program);
        glUniform2f(self.u_scale, 2.0 / w as f32, 2.0 / h as f32);
        glActiveTexture(0x84C1);
        glBindTexture(0x0DE1, self.img_tex);
        glActiveTexture(0x84C0);
        glBindTexture(0x0DE1, self.tex);
        glBindBuffer(0x8892, self.vbo);
        glBufferData(
            0x8892,
            (frame.verts.len() * std::mem::size_of::<Vertex>()) as isize,
            frame.verts.as_ptr() as *const c_void,
            0x88E0,
        );
        let stride = std::mem::size_of::<Vertex>() as i32;
        glEnableVertexAttribArray(0);
        glVertexAttribPointer(0, 2, 0x1406, 0, stride, 0 as *const c_void);
        glEnableVertexAttribArray(1);
        glVertexAttribPointer(1, 2, 0x1406, 0, stride, 8 as *const c_void);
        glEnableVertexAttribArray(2);
        glVertexAttribPointer(2, 4, 0x1401, 1, stride, 16 as *const c_void);
        glEnableVertexAttribArray(3);
        glVertexAttribPointer(3, 4, 0x1406, 0, stride, 20 as *const c_void);
        for op in &frame.ops {
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
                Op::Draw {
                    scissor,
                    first,
                    count,
                } => {
                    match scissor {
                        Some([sx, sy, sw, sh]) => {
                            glEnable(0x0C11);
                            glScissor(*sx, h - (*sy + *sh), *sw, *sh);
                        }
                        None => glDisable(0x0C11),
                    }
                    glDrawArrays(4, *first as i32, *count as i32);
                }
            }
        }
    }
}
