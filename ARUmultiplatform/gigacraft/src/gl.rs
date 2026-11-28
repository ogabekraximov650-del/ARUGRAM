//! OpenGL ES 2.0 ga yupqa qatlam (libGLESv2 ni to'g'ridan-to'g'ri chaqiradi; tashqi kutubxona yo'q).
#![allow(non_snake_case)]
use std::ffi::{c_char, c_void, CString};

#[link(name = "GLESv2")]
extern "C" {
    pub fn glCreateShader(t: u32) -> u32;
    pub fn glShaderSource(s: u32, n: i32, src: *const *const c_char, len: *const i32);
    pub fn glCompileShader(s: u32);
    pub fn glGetShaderiv(s: u32, p: u32, out: *mut i32);
    pub fn glGetShaderInfoLog(s: u32, max: i32, len: *mut i32, log: *mut u8);
    pub fn glCreateProgram() -> u32;
    pub fn glAttachShader(p: u32, s: u32);
    pub fn glBindAttribLocation(p: u32, i: u32, name: *const c_char);
    pub fn glLinkProgram(p: u32);
    pub fn glGetProgramiv(p: u32, pn: u32, out: *mut i32);
    pub fn glGetProgramInfoLog(p: u32, max: i32, len: *mut i32, log: *mut u8);
    pub fn glUseProgram(p: u32);
    pub fn glGetUniformLocation(p: u32, name: *const c_char) -> i32;
    pub fn glUniform1i(l: i32, v: i32);
    pub fn glUniform1f(l: i32, v: f32);
    pub fn glUniform2f(l: i32, x: f32, y: f32);
    pub fn glUniform3f(l: i32, x: f32, y: f32, z: f32);
    pub fn glUniformMatrix4fv(l: i32, n: i32, transpose: u8, v: *const f32);
    pub fn glGenBuffers(n: i32, out: *mut u32);
    pub fn glDeleteBuffers(n: i32, b: *const u32);
    pub fn glBindBuffer(t: u32, b: u32);
    pub fn glBufferData(t: u32, size: isize, data: *const c_void, usage: u32);
    pub fn glEnableVertexAttribArray(i: u32);
    pub fn glDisableVertexAttribArray(i: u32);
    pub fn glVertexAttribPointer(i: u32, size: i32, t: u32, norm: u8, stride: i32, ptr: *const c_void);
    pub fn glDrawElements(mode: u32, count: i32, t: u32, ptr: *const c_void);
    pub fn glDrawArrays(mode: u32, first: i32, count: i32);
    pub fn glGenTextures(n: i32, out: *mut u32);
    pub fn glBindTexture(t: u32, tex: u32);
    pub fn glTexImage2D(t: u32, lvl: i32, ifmt: i32, w: i32, h: i32, border: i32, fmt: u32, ty: u32, data: *const c_void);
    pub fn glTexParameteri(t: u32, p: u32, v: i32);
    pub fn glPixelStorei(p: u32, v: i32);
    pub fn glActiveTexture(t: u32);
    pub fn glViewport(x: i32, y: i32, w: i32, h: i32);
    pub fn glClearColor(r: f32, g: f32, b: f32, a: f32);
    pub fn glClear(m: u32);
    pub fn glEnable(c: u32);
    pub fn glDisable(c: u32);
    pub fn glBlendFunc(s: u32, d: u32);
    pub fn glDepthMask(f: u8);
    pub fn glDepthFunc(f: u32);
    pub fn glGetError() -> u32;
    pub fn glReadPixels(x: i32, y: i32, w: i32, h: i32, fmt: u32, ty: u32, data: *mut c_void);
    pub fn glFinish();
    pub fn glPolygonOffset(factor: f32, units: f32);
    pub fn glGenFramebuffers(n: i32, out: *mut u32);
    pub fn glBindFramebuffer(t: u32, fb: u32);
    pub fn glFramebufferTexture2D(t: u32, attach: u32, textarget: u32, tex: u32, level: i32);
    pub fn glGenRenderbuffers(n: i32, out: *mut u32);
    pub fn glBindRenderbuffer(t: u32, rb: u32);
    pub fn glRenderbufferStorage(t: u32, fmt: u32, w: i32, h: i32);
    pub fn glFramebufferRenderbuffer(t: u32, attach: u32, rbt: u32, rb: u32);
    pub fn glCheckFramebufferStatus(t: u32) -> u32;
}

pub const VERTEX_SHADER: u32 = 0x8B31;
pub const FRAGMENT_SHADER: u32 = 0x8B30;
pub const COMPILE_STATUS: u32 = 0x8B81;
pub const LINK_STATUS: u32 = 0x8B82;
pub const ARRAY_BUFFER: u32 = 0x8892;
pub const ELEMENT_ARRAY_BUFFER: u32 = 0x8893;
pub const STATIC_DRAW: u32 = 0x88E4;
pub const DYNAMIC_DRAW: u32 = 0x88E8;
pub const FLOAT: u32 = 0x1406;
pub const BYTE: u32 = 0x1400;
pub const UNSIGNED_BYTE: u32 = 0x1401;
pub const UNSIGNED_SHORT: u32 = 0x1403;
pub const TRIANGLES: u32 = 0x0004;
pub const TEXTURE_2D: u32 = 0x0DE1;
pub const TEXTURE0: u32 = 0x84C0;
pub const TEXTURE_MIN_FILTER: u32 = 0x2801;
pub const TEXTURE_MAG_FILTER: u32 = 0x2800;
pub const TEXTURE_WRAP_S: u32 = 0x2802;
pub const TEXTURE_WRAP_T: u32 = 0x2803;
pub const LINEAR: i32 = 0x2601;
pub const CLAMP_TO_EDGE: i32 = 0x812F;
pub const ALPHA: u32 = 0x1906;
pub const RGBA: u32 = 0x1908;
pub const UNPACK_ALIGNMENT: u32 = 0x0CF5;
pub const DEPTH_TEST: u32 = 0x0B71;
pub const BLEND: u32 = 0x0BE2;
pub const CULL_FACE: u32 = 0x0B44;
pub const SRC_ALPHA: u32 = 0x0302;
pub const ONE_MINUS_SRC_ALPHA: u32 = 0x0303;
pub const COLOR_BUFFER_BIT: u32 = 0x4000;
pub const DEPTH_BUFFER_BIT: u32 = 0x100;
pub const LEQUAL: u32 = 0x0203;
pub const POLYGON_OFFSET_FILL: u32 = 0x8037;
pub const FRAMEBUFFER: u32 = 0x8D40;
pub const RENDERBUFFER: u32 = 0x8D41;
pub const COLOR_ATTACHMENT0: u32 = 0x8CE0;
pub const DEPTH_ATTACHMENT: u32 = 0x8D00;
pub const DEPTH_COMPONENT16: u32 = 0x81A5;
pub const FRAMEBUFFER_COMPLETE: u32 = 0x8CD5;
pub const NEAREST: i32 = 0x2600;

fn shader(kind: u32, src: &str) -> Result<u32, String> {
    unsafe {
        let s = glCreateShader(kind);
        let c = CString::new(src).map_err(|e| e.to_string())?;
        let p = c.as_ptr();
        glShaderSource(s, 1, &p, std::ptr::null());
        glCompileShader(s);
        let mut ok = 0;
        glGetShaderiv(s, COMPILE_STATUS, &mut ok);
        if ok == 0 {
            let mut buf = vec![0u8; 2048];
            let mut n = 0;
            glGetShaderInfoLog(s, 2048, &mut n, buf.as_mut_ptr());
            buf.truncate(n.max(0) as usize);
            return Err(format!(
                "shader {} xatosi: {}",
                if kind == VERTEX_SHADER { "vertex" } else { "fragment" },
                String::from_utf8_lossy(&buf)
            ));
        }
        Ok(s)
    }
}

/// Dasturni yig'adi; `attribs` — atribut nomlari (indeks = tartib raqami).
pub fn program(vs: &str, fs: &str, attribs: &[&str]) -> Result<u32, String> {
    unsafe {
        let v = shader(VERTEX_SHADER, vs)?;
        let f = shader(FRAGMENT_SHADER, fs)?;
        let p = glCreateProgram();
        glAttachShader(p, v);
        glAttachShader(p, f);
        for (i, a) in attribs.iter().enumerate() {
            let c = CString::new(*a).unwrap();
            glBindAttribLocation(p, i as u32, c.as_ptr());
        }
        glLinkProgram(p);
        let mut ok = 0;
        glGetProgramiv(p, LINK_STATUS, &mut ok);
        if ok == 0 {
            let mut buf = vec![0u8; 2048];
            let mut n = 0;
            glGetProgramInfoLog(p, 2048, &mut n, buf.as_mut_ptr());
            buf.truncate(n.max(0) as usize);
            return Err(format!("dastur ulanmadi: {}", String::from_utf8_lossy(&buf)));
        }
        Ok(p)
    }
}

pub fn uniform(p: u32, name: &str) -> i32 {
    let c = CString::new(name).unwrap();
    unsafe { glGetUniformLocation(p, c.as_ptr()) }
}

pub fn gen_buffer() -> u32 {
    let mut b = 0;
    unsafe { glGenBuffers(1, &mut b) };
    b
}

pub fn buffer_data<T>(target: u32, data: &[T], usage: u32) {
    unsafe {
        glBufferData(
            target,
            std::mem::size_of_val(data) as isize,
            data.as_ptr() as *const c_void,
            usage,
        );
    }
}

pub fn free_buffer(b: u32) {
    unsafe { glDeleteBuffers(1, &b) };
}

/// Shared indeks buferi: 16383 ta to'rtburchak, har biri (0,1,2, 0,2,3).
pub fn quad_indices() -> Vec<u16> {
    let mut v = Vec::with_capacity(16383 * 6);
    for q in 0..16383u32 {
        let b = (q * 4) as u16;
        v.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indekslar_uchburchak() {
        let i = quad_indices();
        assert_eq!(i.len(), 16383 * 6);
        assert_eq!(&i[0..6], &[0, 1, 2, 0, 2, 3]);
        assert_eq!(&i[6..12], &[4, 5, 6, 4, 6, 7]);
        assert_eq!(*i.last().unwrap(), 16383 * 4 - 1);
    }
}
