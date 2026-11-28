//! Chizish: osmon, dunyo (shaffof bo'lmagan va suv), tanlangan blok, HUD. Kamera atrofidagi (suzuvchi boshlang'ich nuqta) koordinatalar.
use crate::gl::*;
use crate::hud::{Hud, HV};
use crate::math::*;
use crate::mesh::Vertex;
use crate::sky::SkyState;
use crate::stream::Built;
use crate::text::Atlas;
use std::ffi::c_void;

pub const SHADOW_SIZE: i32 = 1024;
/// Soya xaritasining yarim kengligi (blok): o'yinchi atrofida ~50 m.
pub const SHADOW_HALF: f32 = 200.0;

const WORLD_VS: &str = "
attribute vec3 a_pos; attribute vec4 a_col; attribute vec4 a_nrm;
uniform mat4 u_vp; uniform vec3 u_off;
varying vec4 v_col; varying vec3 v_nrm; varying float v_dist; varying float v_emis; varying vec3 v_rel;
void main(){
  vec3 p = a_pos + u_off;
  v_rel = p;
  gl_Position = u_vp * vec4(p, 1.0);
  v_col = a_col; v_nrm = a_nrm.xyz; v_emis = a_nrm.w; v_dist = length(p);
}";

const WORLD_FS: &str = "
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
varying vec4 v_col; varying vec3 v_nrm; varying float v_dist; varying float v_emis; varying vec3 v_rel;
uniform vec3 u_ldir; uniform vec3 u_lcol; uniform vec3 u_amb; uniform vec3 u_fog; uniform vec3 u_up;
uniform float u_fogk; uniform float u_alpha; uniform float u_fogamt;
uniform mat4 u_lvp; uniform sampler2D u_sm; uniform float u_shadow;
float unpack(vec4 c){ return dot(c, vec4(1.0, 1.0/255.0, 1.0/65025.0, 1.0/16581375.0)); }
void main(){
  vec3 n = normalize(v_nrm);
  float ndl = max(dot(n, u_ldir), 0.0);
  float sh = 1.0;
  if (u_shadow > 0.5 && ndl > 0.0 && v_emis < 0.5) {
    vec4 sc = u_lvp * vec4(v_rel, 1.0);
    vec3 pc = sc.xyz * 0.5 + 0.5;
    if (pc.x > 0.01 && pc.x < 0.99 && pc.y > 0.01 && pc.y < 0.99 && pc.z < 0.99) {
      float bias = 0.0012 + (1.0 - ndl) * 0.004;
      float s = 0.0;
      for (int i = 0; i < 4; i++) {
        vec2 o = vec2(i == 0 || i == 2 ? -1.0 : 1.0, i < 2 ? -1.0 : 1.0) * 0.0009;
        s += (pc.z - bias > unpack(texture2D(u_sm, pc.xy + o))) ? 0.0 : 1.0;
      }
      sh = mix(0.40, 1.0, s * 0.25);
    }
  }
  vec3 light = u_amb * (0.72 + 0.28 * dot(n, u_up)) * 0.85 + u_lcol * ndl * 0.62 * sh;
  vec3 c = v_col.rgb * light * v_col.a;
  c = mix(c, v_col.rgb * 1.1, v_emis);
  float d = clamp((v_dist * u_fogk - 0.55) / 0.45, 0.0, 1.0);
  float f = d * d * (3.0 - 2.0 * d) * u_fogamt;
  // Havo tumani: uzoqdagi narsa ko'kimtir, quyosh tomonda yorug'roq (Rayleigh/Mie sochilishi)
  vec3 vd = normalize(v_rel);
  float sun = pow(max(dot(vd, u_ldir), 0.0), 5.0);
  vec3 fogc = u_fog * vec3(0.92, 0.98, 1.06) + u_lcol * sun * 0.35;
  c = mix(c, c * vec3(0.86, 0.93, 1.08), min(f * 1.4, 1.0));
  gl_FragColor = vec4(mix(c, fogc, f), u_alpha);
}";

const SH_VS: &str = "
attribute vec3 a_pos; uniform mat4 u_lvp; uniform vec3 u_off;
void main(){ gl_Position = u_lvp * vec4(a_pos + u_off, 1.0); }";

const SH_FS: &str = "
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
void main(){
  vec4 enc = fract(gl_FragCoord.z * vec4(1.0, 255.0, 65025.0, 16581375.0));
  enc -= enc.yzww * vec4(1.0/255.0, 1.0/255.0, 1.0/255.0, 0.0);
  gl_FragColor = enc;
}";

const SKY_VS: &str = "
attribute vec2 a_p; varying vec2 v_p;
void main(){ v_p = a_p; gl_Position = vec4(a_p, 0.999, 1.0); }";

const SKY_FS: &str = "
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
varying vec2 v_p;
uniform vec3 u_fwd; uniform vec3 u_right; uniform vec3 u_camup; uniform vec2 u_tan;
uniform vec3 u_sun; uniform vec3 u_top; uniform vec3 u_hor; uniform float u_night;
uniform vec3 u_up; uniform float u_atm; uniform float u_sunel;
float hash(vec3 p){ p = fract(p * 0.3183099 + vec3(0.1, 0.2, 0.3)); p *= 17.0; return fract(p.x * p.y * p.z * (p.x + p.y + p.z)); }
void main(){
  vec3 d = normalize(u_fwd + u_right * (v_p.x * u_tan.x) + u_camup * (v_p.y * u_tan.y));
  float h = dot(d, u_up);
  vec3 col = mix(u_hor, u_top, pow(clamp(h, 0.0, 1.0), 0.55));
  if (h < 0.0) col = mix(u_hor, u_hor * 0.6, clamp(-h * 4.0, 0.0, 1.0));
  col = col * u_atm;
  float sa = max(u_night, 1.0 - u_atm * 0.85);
  float sd = max(dot(d, u_sun), 0.0);
  vec3 glow = mix(vec3(1.0, 0.55, 0.25), vec3(1.0, 0.9, 0.7), clamp(u_sunel * 2.0, 0.0, 1.0));
  col += glow * (pow(sd, 6.0) * 0.25 + pow(sd, 64.0) * 0.35);
  col += vec3(1.0, 0.97, 0.88) * smoothstep(0.9985, 0.9990, sd) * 2.0;
  float md = max(dot(d, -u_sun), 0.0);
  col += vec3(0.85, 0.9, 1.0) * smoothstep(0.9985, 0.9990, md) * sa * 1.2;
  if (sa > 0.01 && (h > -0.1 || u_atm < 0.9)) {
    vec3 q = d * 90.0; vec3 id = floor(q); float r = hash(id);
    float star = step(0.985, r);
    float tw = smoothstep(0.35, 0.0, length(fract(q) - 0.5));
    col += vec3(1.0) * star * tw * sa * (0.5 + 0.5 * hash(id + 7.0));
  }
  gl_FragColor = vec4(col, 1.0);
}";

const HUD_VS: &str = "
attribute vec2 a_p; attribute vec2 a_uv; attribute vec4 a_c;
uniform vec2 u_scr; varying vec2 v_uv; varying vec4 v_c;
void main(){ gl_Position = vec4(a_p.x / u_scr.x * 2.0 - 1.0, 1.0 - a_p.y / u_scr.y * 2.0, 0.0, 1.0); v_uv = a_uv; v_c = a_c; }";

const HUD_FS: &str = "
precision mediump float;
varying vec2 v_uv; varying vec4 v_c; uniform sampler2D u_tex;
void main(){ gl_FragColor = vec4(v_c.rgb, v_c.a * texture2D(u_tex, v_uv).a); }";

/// GPU dagi mesh (bir nechta bufer bo'lishi mumkin: har biri <= 65532 cho'qqi).
pub struct GpuMesh {
    opaque: Vec<(u32, i32)>,
    water: Vec<(u32, i32)>,
    pub origin: [f64; 3],
    center: [f32; 3],
    radius: f32,
    pub quads: usize,
}

fn upload_parts(parts: &[Vec<Vertex>]) -> Vec<(u32, i32)> {
    let mut out = Vec::new();
    for p in parts {
        if p.is_empty() {
            continue;
        }
        let b = gen_buffer();
        unsafe { glBindBuffer(ARRAY_BUFFER, b) };
        buffer_data(ARRAY_BUFFER, p, STATIC_DRAW);
        out.push((b, (p.len() / 4 * 6) as i32));
    }
    out
}

impl GpuMesh {
    pub fn upload(b: &Built) -> GpuMesh {
        let c = [
            (b.min[0] + b.max[0]) * 0.5,
            (b.min[1] + b.max[1]) * 0.5,
            (b.min[2] + b.max[2]) * 0.5,
        ];
        let r = v3(b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]).len() * 0.5 + 1.0;
        GpuMesh {
            opaque: upload_parts(&b.mesh.opaque),
            water: upload_parts(&b.mesh.water),
            origin: b.key.origin(),
            center: c,
            radius: r,
            quads: b.mesh.quads(),
        }
    }

    pub fn free(&mut self) {
        for (b, _) in self.opaque.drain(..).chain(self.water.drain(..)) {
            free_buffer(b);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.opaque.is_empty() && self.water.is_empty()
    }
}

pub struct View {
    pub pos: [f64; 3],
    /// Kamera asosi: o'ng, tepa, oldinga (birlik vektorlar).
    pub right: V3,
    pub up: V3,
    pub fwd: V3,
    /// Joyning "tepa" yo'nalishi (sayyora markazidan tashqariga).
    pub local_up: V3,
    /// Atmosfera zichligi (1 — yer yuzida, 0 — kosmosda).
    pub atm: f32,
    pub fov: f32,
    pub w: i32,
    pub h: i32,
    pub near: f32,
    pub far: f32,
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Stats {
    pub drawn: usize,
    pub culled: usize,
    pub tris: usize,
}

struct WorldU {
    vp: i32,
    off: i32,
    ldir: i32,
    lcol: i32,
    amb: i32,
    fog: i32,
    fogk: i32,
    alpha: i32,
    up: i32,
    fogamt: i32,
    lvp: i32,
    sm: i32,
    shadow: i32,
}

pub struct Renderer {
    world_p: u32,
    sky_p: u32,
    hud_p: u32,
    wu: WorldU,
    sky_u: [i32; 12],
    hud_scr: i32,
    hud_tex_u: i32,
    idx: u32,
    hud_vbo: u32,
    sky_vbo: u32,
    dyn_vbo: u32,
    tex: u32,
    sh_prog: u32,
    sh_lvp: i32,
    sh_off: i32,
    sh_fbo: u32,
    sh_tex: u32,
    sh_ok: bool,
}

fn check(what: &str) -> Result<(), String> {
    let e = unsafe { glGetError() };
    if e != 0 {
        Err(format!("GL xato {e:#x} ({what})"))
    } else {
        Ok(())
    }
}

impl Renderer {
    /// Joriy GL kontekstida resurslarni yaratadi.
    pub fn new(atlas: &Atlas) -> Result<Renderer, String> {
        let world_p = program(WORLD_VS, WORLD_FS, &["a_pos", "a_col", "a_nrm"])?;
        let sky_p = program(SKY_VS, SKY_FS, &["a_p"])?;
        let hud_p = program(HUD_VS, HUD_FS, &["a_p", "a_uv", "a_c"])?;
        let wu = WorldU {
            vp: uniform(world_p, "u_vp"),
            off: uniform(world_p, "u_off"),
            ldir: uniform(world_p, "u_ldir"),
            lcol: uniform(world_p, "u_lcol"),
            amb: uniform(world_p, "u_amb"),
            fog: uniform(world_p, "u_fog"),
            fogk: uniform(world_p, "u_fogk"),
            alpha: uniform(world_p, "u_alpha"),
            up: uniform(world_p, "u_up"),
            fogamt: uniform(world_p, "u_fogamt"),
            lvp: uniform(world_p, "u_lvp"),
            sm: uniform(world_p, "u_sm"),
            shadow: uniform(world_p, "u_shadow"),
        };
        let names = ["u_fwd", "u_right", "u_camup", "u_tan", "u_sun", "u_top", "u_hor", "u_night", "u_up", "u_atm", "u_sunel", "u_unused"];
        let mut sky_u = [0i32; 12];
        for (i, n) in names.iter().enumerate() {
            sky_u[i] = uniform(sky_p, n);
        }
        let idx = gen_buffer();
        unsafe { glBindBuffer(ELEMENT_ARRAY_BUFFER, idx) };
        buffer_data(ELEMENT_ARRAY_BUFFER, &quad_indices(), STATIC_DRAW);
        let sky_vbo = gen_buffer();
        unsafe { glBindBuffer(ARRAY_BUFFER, sky_vbo) };
        buffer_data(ARRAY_BUFFER, &[-1.0f32, -1.0, 3.0, -1.0, -1.0, 3.0], STATIC_DRAW);
        let hud_vbo = gen_buffer();
        let dyn_vbo = gen_buffer();
        let mut tex = 0;
        unsafe {
            glGenTextures(1, &mut tex);
            glBindTexture(TEXTURE_2D, tex);
            glPixelStorei(UNPACK_ALIGNMENT, 1);
            glTexImage2D(
                TEXTURE_2D,
                0,
                ALPHA as i32,
                atlas.w as i32,
                atlas.h as i32,
                0,
                ALPHA,
                UNSIGNED_BYTE,
                atlas.px.as_ptr() as *const c_void,
            );
            glTexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR);
            glTexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR);
            glTexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, CLAMP_TO_EDGE);
            glTexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, CLAMP_TO_EDGE);
        }
        let sh_prog = program(SH_VS, SH_FS, &["a_pos"])?;
        let (mut sh_fbo, mut sh_tex, mut sh_rb) = (0u32, 0u32, 0u32);
        let mut sh_ok = false;
        unsafe {
            glGenTextures(1, &mut sh_tex);
            glBindTexture(TEXTURE_2D, sh_tex);
            glTexImage2D(TEXTURE_2D, 0, RGBA as i32, SHADOW_SIZE, SHADOW_SIZE, 0, RGBA, UNSIGNED_BYTE, std::ptr::null());
            glTexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, NEAREST);
            glTexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, NEAREST);
            glTexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, CLAMP_TO_EDGE);
            glTexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, CLAMP_TO_EDGE);
            glGenRenderbuffers(1, &mut sh_rb);
            glBindRenderbuffer(RENDERBUFFER, sh_rb);
            glRenderbufferStorage(RENDERBUFFER, DEPTH_COMPONENT16, SHADOW_SIZE, SHADOW_SIZE);
            glGenFramebuffers(1, &mut sh_fbo);
            glBindFramebuffer(FRAMEBUFFER, sh_fbo);
            glFramebufferTexture2D(FRAMEBUFFER, COLOR_ATTACHMENT0, TEXTURE_2D, sh_tex, 0);
            glFramebufferRenderbuffer(FRAMEBUFFER, DEPTH_ATTACHMENT, RENDERBUFFER, sh_rb);
            if glCheckFramebufferStatus(FRAMEBUFFER) == FRAMEBUFFER_COMPLETE {
                sh_ok = true;
            }
            glBindFramebuffer(FRAMEBUFFER, 0);
            glBindTexture(TEXTURE_2D, 0);
        }
        let _ = unsafe { glGetError() };
        check("boshlash")?;
        Ok(Renderer {
            world_p,
            sky_p,
            hud_p,
            hud_scr: uniform(hud_p, "u_scr"),
            hud_tex_u: uniform(hud_p, "u_tex"),
            wu,
            sky_u,
            idx,
            hud_vbo,
            sky_vbo,
            dyn_vbo,
            tex,
            sh_off: uniform(sh_prog, "u_off"),
            sh_lvp: uniform(sh_prog, "u_lvp"),
            sh_prog,
            sh_fbo,
            sh_tex,
            sh_ok,
        })
    }

    fn set_world_attribs(&self, vbo: u32) {
        unsafe {
            glBindBuffer(ARRAY_BUFFER, vbo);
            glEnableVertexAttribArray(0);
            glEnableVertexAttribArray(1);
            glEnableVertexAttribArray(2);
            glVertexAttribPointer(0, 3, FLOAT, 0, 20, std::ptr::null());
            glVertexAttribPointer(1, 4, UNSIGNED_BYTE, 1, 20, 12usize as *const c_void);
            glVertexAttribPointer(2, 4, BYTE, 1, 20, 16usize as *const c_void);
        }
    }

    fn draw_sky(&self, v: &View, sky: &SkyState) {
        let (f, r, u) = (v.fwd, v.right, v.up);
        let ty = (v.fov * 0.5).tan();
        let tx = ty * v.w as f32 / v.h as f32;
        unsafe {
            glUseProgram(self.sky_p);
            glUniform3f(self.sky_u[0], f.x, f.y, f.z);
            glUniform3f(self.sky_u[1], r.x, r.y, r.z);
            glUniform3f(self.sky_u[2], u.x, u.y, u.z);
            glUniform2f(self.sky_u[3], tx, ty);
            glUniform3f(self.sky_u[4], sky.sun.x, sky.sun.y, sky.sun.z);
            glUniform3f(self.sky_u[5], sky.top[0], sky.top[1], sky.top[2]);
            glUniform3f(self.sky_u[6], sky.hor[0], sky.hor[1], sky.hor[2]);
            glUniform1f(self.sky_u[7], sky.night);
            glUniform3f(self.sky_u[8], v.local_up.x, v.local_up.y, v.local_up.z);
            glUniform1f(self.sky_u[9], v.atm);
            glUniform1f(self.sky_u[10], sky.sun.dot(v.local_up));
            glBindBuffer(ARRAY_BUFFER, self.sky_vbo);
            glEnableVertexAttribArray(0);
            glDisableVertexAttribArray(1);
            glDisableVertexAttribArray(2);
            glVertexAttribPointer(0, 2, FLOAT, 0, 8, std::ptr::null());
            glDrawArrays(TRIANGLES, 0, 3);
        }
    }

    /// Butun sahnani chizadi (osmon, relyef, suv, tanlangan blok).
    pub fn draw_scene(
        &mut self,
        v: &View,
        sky: &SkyState,
        fog_dist: f32,
        meshes: &[&GpuMesh],
        highlight: Option<&(D3, Vec<Vertex>)>,
        dynamic: Option<&(D3, Vec<Vertex>)>,
        shadow_meshes: &[&GpuMesh],
        stale: &[&GpuMesh],
    ) -> Stats {
        let mut st = Stats::default();
        // Soya xaritasi: yaqin ustunlar quyosh tomondan chiziladi
        let use_shadow = self.sh_ok && !shadow_meshes.is_empty() && sky.light_dir.dot(v.local_up) > 0.08 && v.atm > 0.5;
        let lvp = light_matrix(sky.light_dir, v.up, SHADOW_HALF, SHADOW_HALF * 6.0);
        if use_shadow {
            unsafe {
                glBindFramebuffer(FRAMEBUFFER, self.sh_fbo);
                glViewport(0, 0, SHADOW_SIZE, SHADOW_SIZE);
                glClearColor(1.0, 1.0, 1.0, 1.0);
                glClear(COLOR_BUFFER_BIT | DEPTH_BUFFER_BIT);
                glDisable(BLEND);
                glDisable(CULL_FACE);
                glEnable(DEPTH_TEST);
                glDepthFunc(LEQUAL);
                glDepthMask(1);
                glUseProgram(self.sh_prog);
                glUniformMatrix4fv(self.sh_lvp, 1, 0, lvp.as_ptr());
                glBindBuffer(ELEMENT_ARRAY_BUFFER, self.idx);
            }
            for m in shadow_meshes {
                let off = [(m.origin[0] - v.pos[0]) as f32, (m.origin[1] - v.pos[1]) as f32, (m.origin[2] - v.pos[2]) as f32];
                unsafe { glUniform3f(self.sh_off, off[0], off[1], off[2]) };
                for &(vbo, n) in &m.opaque {
                    self.set_world_attribs(vbo);
                    unsafe { glDrawElements(TRIANGLES, n, UNSIGNED_SHORT, std::ptr::null()) };
                }
            }
            unsafe { glBindFramebuffer(FRAMEBUFFER, 0) };
        }
        unsafe {
            glViewport(0, 0, v.w, v.h);
            glClearColor(sky.hor[0], sky.hor[1], sky.hor[2], 1.0);
            glClear(COLOR_BUFFER_BIT | DEPTH_BUFFER_BIT);
            glDisable(DEPTH_TEST);
            glDisable(BLEND);
            glDisable(CULL_FACE);
        }
        self.draw_sky(v, sky);
        let proj = perspective(v.fov, v.w as f32 / v.h as f32, v.near, v.far);
        let vp = mul(&proj, &view_basis(v.right, v.up, v.fwd));
        let fr = frustum(&vp);
        unsafe {
            glEnable(DEPTH_TEST);
            glDepthFunc(LEQUAL);
            glDepthMask(1);
            glUseProgram(self.world_p);
            glUniformMatrix4fv(self.wu.vp, 1, 0, vp.as_ptr());
            glUniform3f(self.wu.ldir, sky.light_dir.x, sky.light_dir.y, sky.light_dir.z);
            glUniform3f(self.wu.lcol, sky.light_col[0], sky.light_col[1], sky.light_col[2]);
            glUniform3f(self.wu.amb, sky.amb[0], sky.amb[1], sky.amb[2]);
            glUniform3f(self.wu.fog, sky.fog[0], sky.fog[1], sky.fog[2]);
            glUniform1f(self.wu.fogk, 1.0 / fog_dist.max(1.0));
            glUniform1f(self.wu.alpha, 1.0);
            glUniform3f(self.wu.up, v.local_up.x, v.local_up.y, v.local_up.z);
            glUniformMatrix4fv(self.wu.lvp, 1, 0, lvp.as_ptr());
            glUniform1f(self.wu.shadow, if use_shadow { 1.0 } else { 0.0 });
            glActiveTexture(TEXTURE0 + 1);
            glBindTexture(TEXTURE_2D, self.sh_tex);
            glUniform1i(self.wu.sm, 1);
            glActiveTexture(TEXTURE0);
            glUniform1f(self.wu.fogamt, v.atm);
            glBindBuffer(ELEMENT_ARRAY_BUFFER, self.idx);
        }
        let mut vis: Vec<(&GpuMesh, [f32; 3])> = Vec::with_capacity(meshes.len());
        for m in meshes {
            let off = [
                (m.origin[0] - v.pos[0]) as f32,
                (m.origin[1] - v.pos[1]) as f32,
                (m.origin[2] - v.pos[2]) as f32,
            ];
            let c = v3(off[0] + m.center[0], off[1] + m.center[1], off[2] + m.center[2]);
            if !sphere_visible(&fr, c, m.radius) {
                st.culled += 1;
                continue;
            }
            vis.push((m, off));
        }
        for (m, off) in &vis {
            unsafe { glUniform3f(self.wu.off, off[0], off[1], off[2]) };
            for &(vbo, n) in &m.opaque {
                self.set_world_attribs(vbo);
                unsafe { glDrawElements(TRIANGLES, n, UNSIGNED_SHORT, std::ptr::null()) };
                st.tris += (n / 3) as usize;
            }
            st.drawn += 1;
        }
        // Eskirgan (almashtirilayotgan) meshlar: chuqurlik bo'yicha biroz orqaga suriladi — yangi mesh ustun turadi, titrash yo'q
        unsafe {
            glEnable(POLYGON_OFFSET_FILL);
            glPolygonOffset(3.0, 512.0);
        }
        for m in stale {
            let off = [
                (m.origin[0] - v.pos[0]) as f32,
                (m.origin[1] - v.pos[1]) as f32,
                (m.origin[2] - v.pos[2]) as f32,
            ];
            let c = v3(off[0] + m.center[0], off[1] + m.center[1], off[2] + m.center[2]);
            if !sphere_visible(&fr, c, m.radius) {
                continue;
            }
            unsafe { glUniform3f(self.wu.off, off[0], off[1], off[2]) };
            for &(vbo, n) in &m.opaque {
                self.set_world_attribs(vbo);
                unsafe { glDrawElements(TRIANGLES, n, UNSIGNED_SHORT, std::ptr::null()) };
            }
        }
        unsafe { glDisable(POLYGON_OFFSET_FILL) };
        // Jonzotlar va zarrachalar: har kadr qayta yuklanadigan, to'liq qoplovchi quti-modellar
        if let Some((origin, verts)) = dynamic {
            let o = [(origin[0] - v.pos[0]) as f32, (origin[1] - v.pos[1]) as f32, (origin[2] - v.pos[2]) as f32];
            unsafe {
                glBindBuffer(ARRAY_BUFFER, self.dyn_vbo);
                glUniform1f(self.wu.alpha, 1.0);
                glUniform3f(self.wu.off, o[0], o[1], o[2]);
            }
            buffer_data(ARRAY_BUFFER, verts, DYNAMIC_DRAW);
            self.set_world_attribs(self.dyn_vbo);
            unsafe { glDrawElements(TRIANGLES, (verts.len() / 4 * 6) as i32, UNSIGNED_SHORT, std::ptr::null()) };
            st.tris += verts.len() / 2;
        }
        // Suv: yarim shaffof, chuqurlikka yozmaydi
        unsafe {
            glEnable(BLEND);
            glBlendFunc(SRC_ALPHA, ONE_MINUS_SRC_ALPHA);
            glDepthMask(0);
            glUniform1f(self.wu.alpha, 0.62);
        }
        for (m, off) in &vis {
            if m.water.is_empty() {
                continue;
            }
            unsafe { glUniform3f(self.wu.off, off[0], off[1], off[2]) };
            for &(vbo, n) in &m.water {
                self.set_world_attribs(vbo);
                unsafe { glDrawElements(TRIANGLES, n, UNSIGNED_SHORT, std::ptr::null()) };
                st.tris += (n / 3) as usize;
            }
        }
        // Tanlangan blok: yengil oq qoplama
        if let Some((origin, verts)) = highlight {
            let o = [
                (origin[0] - v.pos[0]) as f32,
                (origin[1] - v.pos[1]) as f32,
                (origin[2] - v.pos[2]) as f32,
            ];
            unsafe {
                glBindBuffer(ARRAY_BUFFER, self.dyn_vbo);
                glUniform1f(self.wu.alpha, 0.30);
                glUniform3f(self.wu.off, o[0], o[1], o[2]);
            }
            buffer_data(ARRAY_BUFFER, verts, DYNAMIC_DRAW);
            self.set_world_attribs(self.dyn_vbo);
            unsafe { glDrawElements(TRIANGLES, 36, UNSIGNED_SHORT, std::ptr::null()) };
        }
        unsafe {
            glDepthMask(1);
            glDisable(BLEND);
        }
        st
    }

    /// HUD ni chizadi (ekran pikselida).
    pub fn draw_hud(&mut self, hud: &Hud, w: i32, h: i32) {
        if hud.v.is_empty() {
            return;
        }
        unsafe {
            glDisable(DEPTH_TEST);
            glEnable(BLEND);
            glBlendFunc(SRC_ALPHA, ONE_MINUS_SRC_ALPHA);
            glUseProgram(self.hud_p);
            glUniform2f(self.hud_scr, w as f32, h as f32);
            glActiveTexture(TEXTURE0);
            glBindTexture(TEXTURE_2D, self.tex);
            glUniform1i(self.hud_tex_u, 0);
            glBindBuffer(ARRAY_BUFFER, self.hud_vbo);
        }
        buffer_data(ARRAY_BUFFER, &hud.v, DYNAMIC_DRAW);
        let stride = std::mem::size_of::<HV>() as i32;
        unsafe {
            glEnableVertexAttribArray(0);
            glEnableVertexAttribArray(1);
            glEnableVertexAttribArray(2);
            glVertexAttribPointer(0, 2, FLOAT, 0, stride, std::ptr::null());
            glVertexAttribPointer(1, 2, FLOAT, 0, stride, 8usize as *const c_void);
            glVertexAttribPointer(2, 4, UNSIGNED_BYTE, 1, stride, 16usize as *const c_void);
            glDrawArrays(TRIANGLES, 0, hud.v.len() as i32);
            glDisable(BLEND);
        }
    }

    /// Ekran piksellarini o'qiydi (RGBA, yuqoridan pastga) — testlardagi rasmlar uchun.
    pub fn read_pixels(&self, w: i32, h: i32) -> Vec<u8> {
        let mut px = vec![0u8; (w * h * 4) as usize];
        unsafe {
            glFinish();
            glReadPixels(0, 0, w, h, RGBA, UNSIGNED_BYTE, px.as_mut_ptr() as *mut c_void);
        }
        let row = (w * 4) as usize;
        let mut out = vec![0u8; px.len()];
        for y in 0..h as usize {
            out[y * row..(y + 1) * row].copy_from_slice(&px[(h as usize - 1 - y) * row..(h as usize - y) * row]);
        }
        out
    }
}
