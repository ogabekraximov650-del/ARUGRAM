struct Globals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    cam_pos: vec4<f32>,   // xyz; w = 1 — qurol (kamera fazosi, soya va tuman yo'q)
    sun_dir: vec4<f32>,
    fog: vec4<f32>,       // rgb (chiziqli), a — zichlik
    misc: vec4<f32>,      // x — vaqt
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_smp: sampler_comparison;

const SHADOW_SIZE: f32 = 2048.0;
const SUN_COLOR: vec3<f32> = vec3<f32>(1.0, 0.93, 0.80);

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) i_pos: vec4<f32>,
    @location(3) i_scale: vec4<f32>,
    @location(4) i_color: vec4<f32>,
    @location(5) i_rot: vec4<f32>,
};
struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) mat: f32,
    @location(4) lp: vec3<f32>,
    @location(5) ca: f32,
};

fn qrot(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    let w = qrot(v.i_rot, v.pos * v.i_scale.xyz) + v.i_pos.xyz;
    o.world = w;
    // o'lchamli (notekis) masshtabda normalni to'g'ri o'zgartirish
    let ns = v.normal / max(v.i_scale.xyz, vec3<f32>(0.0001));
    o.normal = normalize(qrot(v.i_rot, ns));
    o.color = v.i_color.rgb;
    o.mat = v.i_scale.w;
    o.lp = v.pos;
    o.ca = v.i_color.a;
    o.clip = g.view_proj * vec4<f32>(w, 1.0);
    return o;
}

@vertex
fn vs_shadow(v: VIn) -> @builtin(position) vec4<f32> {
    let w = qrot(v.i_rot, v.pos * v.i_scale.xyz) + v.i_pos.xyz;
    return g.light_vp * vec4<f32>(w, 1.0);
}

// ---------- kontur (anime uslubi): old yuzlarni yashirib, shaklni sal kattalashtirib chizish ----------
struct OutOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn vs_outline(v: VIn) -> OutOut {
    var o: OutOut;
    var w = qrot(v.i_rot, v.pos * v.i_scale.xyz) + v.i_pos.xyz;
    let ns = v.normal / max(v.i_scale.xyz, vec3<f32>(0.0001));
    let n = normalize(qrot(v.i_rot, ns));
    let d = length(w - g.cam_pos.xyz);
    // ekranda deyarli bir xil qalinlik: masofaga qarab
    w = w + n * (0.0035 + 0.0022 * d);
    o.world = w;
    o.color = v.i_color.rgb;
    o.clip = g.view_proj * vec4<f32>(w, 1.0);
    return o;
}

@fragment
fn fs_outline(i: OutOut) -> @location(0) vec4<f32> {
    let c = pow(i.color * vec3<f32>(0.30, 0.22, 0.32), vec3<f32>(2.2));
    let d = length(i.world - g.cam_pos.xyz);
    let f = 1.0 - exp(-d * g.fog.a);
    return vec4<f32>(encode(mix(c, g.fog.rgb, clamp(f, 0.0, 1.0))), 1.0);
}

// ---------- shovqin ----------
fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.34, 456.21));
    q = q + dot(q, q + 45.32);
    return fract(q.x * q.y);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    return vnoise(p) * 0.57 + vnoise(p * 2.07) * 0.29 + vnoise(p * 4.13) * 0.14;
}

fn lin(c: vec3<f32>) -> vec3<f32> {
    return pow(c, vec3<f32>(2.2));
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn encode(c: vec3<f32>) -> vec3<f32> {
    return pow(aces(c), vec3<f32>(1.0 / 2.2));
}

// ---------- osmon ----------
fn sky_color(dir: vec3<f32>) -> vec3<f32> {
    let horizon = lin(vec3<f32>(0.78, 0.86, 0.95));
    let zenith = lin(vec3<f32>(0.22, 0.45, 0.85));
    let t = pow(clamp(dir.y, 0.0, 1.0), 0.55);
    var c = mix(horizon, zenith, t);
    let sd = max(dot(dir, -g.sun_dir.xyz), 0.0);
    c = c + SUN_COLOR * (pow(sd, 700.0) * 30.0 + pow(sd, 12.0) * 0.18);
    if (dir.y > 0.01) {
        let uv = dir.xz / (dir.y + 0.12) * 0.7 + vec2<f32>(g.misc.x * 0.012, 0.0);
        let cl = smoothstep(0.52, 0.85, fbm(uv));
        let lit = mix(vec3<f32>(0.75), vec3<f32>(1.05), clamp(dir.y * 2.0, 0.0, 1.0));
        c = mix(c, lit, cl * clamp(dir.y * 4.0, 0.0, 1.0) * 0.85);
    }
    return c;
}

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) vi: u32) -> SkyOut {
    var o: SkyOut;
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    let p = vec2<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0);
    o.clip = vec4<f32>(p, 0.99999, 1.0);
    o.ndc = p;
    return o;
}

@fragment
fn fs_sky(i: SkyOut) -> @location(0) vec4<f32> {
    let far = g.inv_view_proj * vec4<f32>(i.ndc, 1.0, 1.0);
    let dir = normalize(far.xyz / far.w - g.cam_pos.xyz);
    return vec4<f32>(encode(sky_color(dir)), 1.0);
}

// ---------- materiallar ----------
struct Surf {
    albedo: vec3<f32>,
    spec: f32,
    shin: f32,
    wrap: f32,      // yumshoq (teri/barg) yoritish uchun
    emissive: f32,
};

fn surface(m: i32, c: vec3<f32>, w: vec3<f32>, n: vec3<f32>, far: bool) -> Surf {
    var s: Surf;
    s.albedo = lin(c);
    s.spec = 0.04;
    s.shin = 16.0;
    s.wrap = 0.0;
    s.emissive = 0.0;
    if (far) {
        // uzoqda protsedur tekstura kerak emas — arzon yo'l
        s.albedo = lin(c * select(1.0, 0.92, m == 1));
        if (m == 5) { s.spec = 0.9; s.shin = 90.0; }
        if (m == 8) { s.spec = 0.7; s.shin = 50.0; }
        if (m == 10) { s.emissive = 1.0; }
        if (m == 9) { s.wrap = 0.5; }
        return s;
    }
    let wall_u = w.x * abs(n.z) + w.z * abs(n.x);
    let vert = 1.0 - abs(n.y);
    if (m == 1) {
        // o'tloq: shovqin + qiyalikda tuproq
        let k = vnoise(w.xz * 0.35) * 0.6 + vnoise(w.xz * 1.9) * 0.4;
        var a = mix(lin(c * 0.72), lin(c * 1.18), k);
        let blade = vnoise(w.xz * 9.0);
        a = a * (0.9 + 0.2 * blade);
        let dirt = lin(vec3<f32>(0.42, 0.32, 0.21));
        s.albedo = mix(a, dirt * (0.8 + 0.4 * k), smoothstep(0.10, 0.28, 1.0 - n.y));
        s.wrap = 0.25;
    } else if (m == 2) {
        // g'isht
        if (abs(n.y) > 0.7) {
            s.albedo = lin(c * 0.7);
        } else {
            let row = floor(w.y / 0.075);
            let off = select(0.0, 0.5, (i32(row) & 1) == 1);
            let bu = wall_u / 0.24 + off;
            let fu = fract(bu);
            let fv = fract(w.y / 0.075);
            let mortar = select(0.0, 1.0, fu < 0.045 || fv < 0.12);
            let tone = 0.8 + 0.4 * hash21(vec2<f32>(floor(bu), row));
            s.albedo = mix(lin(c) * tone, lin(vec3<f32>(0.72, 0.70, 0.66)), mortar);
        }
    } else if (m == 3) {
        s.albedo = lin(c) * (0.92 + 0.16 * fbm(vec2<f32>(wall_u, w.y) * 3.0));
    } else if (m == 4) {
        // tom: qator-qator yassi cherepitsa
        let row = floor(w.y * 6.0 + (w.x + w.z) * 0.0);
        let col = floor((w.x + w.z) * 4.0 + row * 0.5);
        let fv = fract(w.y * 6.0);
        s.albedo = lin(c) * (0.78 + 0.35 * hash21(vec2<f32>(col, row))) * (0.8 + 0.2 * smoothstep(0.0, 0.4, fv));
        s.spec = 0.08;
    } else if (m == 5) {
        s.albedo = lin(c) * 0.5;
        s.spec = 0.9;
        s.shin = 90.0;
    } else if (m == 6) {
        s.albedo = lin(c) * (0.8 + 0.4 * vnoise(w.xz * 14.0)) * (0.9 + 0.2 * fbm(w.xz * 0.8));
    } else if (m == 7) {
        let u = wall_u + w.y * 0.0;
        let plank = floor(select(w.y, wall_u, abs(n.y) < 0.7) * 7.0 + select(wall_u, w.z, abs(n.y) > 0.7) * 0.0);
        let grain = vnoise(vec2<f32>(wall_u * 1.5, w.y * 40.0 + w.z * 40.0 * abs(n.y)));
        s.albedo = lin(c) * (0.8 + 0.35 * grain) * (0.85 + 0.3 * hash21(vec2<f32>(plank, 3.0)));
        s.spec = 0.06;
    } else if (m == 8) {
        s.albedo = lin(c) * (0.85 + 0.3 * vnoise(vec2<f32>(w.x + w.z, w.y * 60.0)));
        s.spec = 0.7;
        s.shin = 50.0;
    } else if (m == 9) {
        let k = vnoise(w.xz * 6.0 + w.y * 5.0);
        s.albedo = lin(c * (0.75 + 0.5 * k));
        s.wrap = 0.5;
    } else if (m == 10) {
        s.albedo = lin(c);
        s.emissive = 1.0;
    } else if (m == 11) {
        s.albedo = lin(c) * (0.94 + 0.12 * vnoise(w.xz * 30.0 + w.y * 30.0));
        s.wrap = 0.2;
    } else if (m == 12) {
        s.albedo = lin(c) * (0.85 + 0.25 * fbm(w.xz * 2.0 + w.y * 2.0));
    } else if (m == 13) {
        s.albedo = lin(c);
        s.wrap = 0.55;
        s.spec = 0.06;
        s.shin = 20.0;
    } else if (m == 14) {
        s.albedo = lin(c);
        s.wrap = 0.2;
        s.spec = 0.45;
        s.shin = 28.0;
    } else if (m == 15) {
        s.albedo = lin(c) * (0.7 + 0.5 * fbm(w.xz * 2.5 + w.y * 3.0));
        s.shin = 8.0;
    } else {
        s.albedo = lin(c);
    }
    return s;
}

fn shadow_at(world: vec3<f32>, n: vec3<f32>) -> f32 {
    var shadow = 1.0;
    if (g.cam_pos.w < 0.5) {
        let ls = g.light_vp * vec4<f32>(world + n * 0.06, 1.0);
        let uv = vec2<f32>(ls.x * 0.5 + 0.5, 1.0 - (ls.y * 0.5 + 0.5));
        let texel = 1.0 / SHADOW_SIZE;
        var sh = 0.0;
        for (var ox = 0; ox < 2; ox = ox + 1) {
            for (var oy = 0; oy < 2; oy = oy + 1) {
                let o = (vec2<f32>(f32(ox), f32(oy)) - vec2<f32>(0.5)) * texel * 1.5;
                sh = sh + textureSampleCompare(shadow_map, shadow_smp, clamp(uv + o, vec2<f32>(0.001), vec2<f32>(0.999)), ls.z);
            }
        }
        sh = sh / 4.0;
        let inside = select(0.0, 1.0, uv.x > 0.0 && uv.x < 1.0 && uv.y > 0.0 && uv.y < 1.0 && ls.z < 1.0);
        shadow = mix(1.0, sh, inside);
    }
    return shadow;
}

fn in_ellipse(p: vec2<f32>, r: vec2<f32>) -> f32 {
    return length(p / r);
}

/// Anime yuzi: bosh sharining mahalliy koordinatalarida (u,v in [-1,1], old tomon -z) ko'z, qosh, yonoq, og'iz.
fn face_color(base: vec3<f32>, lp: vec3<f32>, open: f32) -> vec3<f32> {
    var c = base;
    if (lp.z > -0.02) {
        return c;
    }
    let u = lp.x * 2.0;
    let v = lp.y * 2.0;
    // yonoq qizarishi
    for (var k = 0; k < 2; k = k + 1) {
        let sx = select(-1.0, 1.0, k == 1);
        let b = in_ellipse(vec2<f32>(u - sx * 0.50, v + 0.46), vec2<f32>(0.17, 0.08));
        c = mix(c, vec3<f32>(1.0, 0.55, 0.62), (1.0 - smoothstep(0.4, 1.0, b)) * 0.55);
    }
    for (var k = 0; k < 2; k = k + 1) {
        let sx = select(-1.0, 1.0, k == 1);
        let e = vec2<f32>(u - sx * 0.43, v + 0.12);
        let ry = 0.30 * max(open, 0.08);
        let d = in_ellipse(e, vec2<f32>(0.27, ry));
        if (d < 1.0) {
            // oq qismi, ko'k qorachiq (tepasi to'q, pasti och), qorachiq, yaltirashlar
            c = vec3<f32>(0.97, 0.97, 1.0);
            let di = in_ellipse(e - vec2<f32>(0.0, -0.01), vec2<f32>(0.20, ry * 0.95));
            if (di < 1.0) {
                let t = clamp(e.y / max(ry, 0.001) * 0.5 + 0.5, 0.0, 1.0);
                var ic = mix(vec3<f32>(0.55, 0.85, 1.0), vec3<f32>(0.10, 0.18, 0.62), t);
                ic = mix(ic, vec3<f32>(0.05, 0.08, 0.32), smoothstep(0.78, 1.0, di));
                c = ic;
                let dp = in_ellipse(e - vec2<f32>(0.0, -0.02), vec2<f32>(0.085, ry * 0.55));
                if (dp < 1.0) { c = vec3<f32>(0.03, 0.04, 0.16); }
            }
            if (in_ellipse(e - vec2<f32>(-0.07 * sx, 0.12 * open), vec2<f32>(0.055, 0.075 * max(open, 0.2))) < 1.0) { c = vec3<f32>(1.0); }
            if (in_ellipse(e - vec2<f32>(0.08 * sx, -0.14 * open), vec2<f32>(0.03, 0.04 * max(open, 0.2))) < 1.0) { c = vec3<f32>(0.95, 0.98, 1.0); }
        }
        // yuqori kiprik (qalin), pastki ingichka chiziq
        if (d > 0.88 && d < 1.12 && e.y > -0.4 * ry) {
            c = vec3<f32>(0.14, 0.06, 0.16);
        }
        if (d > 0.96 && d < 1.06 && e.y < -0.5 * ry) {
            c = mix(c, vec3<f32>(0.55, 0.30, 0.35), 0.7);
        }
        // qosh
        let b = vec2<f32>(u - sx * 0.45, v - 0.30 - 0.10 * (u * sx - 0.45));
        if (abs(b.y) < 0.022 && abs(b.x) < 0.17) {
            c = mix(c, vec3<f32>(0.55, 0.22, 0.42), 0.9);
        }
    }
    // burun (kichik soya) va og'iz
    if (in_ellipse(vec2<f32>(u, v + 0.40), vec2<f32>(0.035, 0.025)) < 1.0) { c = c * vec3<f32>(0.9, 0.8, 0.8); }
    let m = vec2<f32>(u, v + 0.62);
    if (abs(m.y + 0.7 * m.x * m.x - 0.02) < 0.017 && abs(m.x) < 0.14) {
        c = vec3<f32>(0.62, 0.22, 0.30);
    }
    return c;
}

/// Anime (cel) yoritish: 2 pog'onali soya, sovuq soya rangi, qirra yorug'ligi, soch yaltirashi.
fn toon(i: VOut, m: i32, n: vec3<f32>, shadow: f32) -> vec4<f32> {
    var base = i.color;
    if (m == 18) {
        base = face_color(base, i.lp, i.ca);
    }
    let alb = lin(base);
    let l = -g.sun_dir.xyz;
    let v = normalize(g.cam_pos.xyz - i.world);
    let h = dot(n, l) * 0.5 + 0.5;
    let lit = smoothstep(0.46, 0.54, h) * shadow;
    let shade = alb * vec3<f32>(0.62, 0.60, 0.80);
    var col = mix(shade, alb * 1.08, lit);
    col = col + alb * lin(vec3<f32>(0.40, 0.55, 0.80)) * 0.25;
    if (m == 17) {
        // soch: "farishta halqasi" yaltirashi
        let ring = smoothstep(0.0, 0.05, 0.10 - abs(n.y - 0.40)) * lit;
        col = mix(col, lin(vec3<f32>(0.85, 1.0, 0.95)), ring * 0.30);
    }
    let rim = pow(1.0 - max(dot(n, v), 0.0), 3.0) * 0.30;
    col = col + SUN_COLOR * rim * lit * 0.6;
    let d = length(i.world - g.cam_pos.xyz);
    let f = select(1.0 - exp(-d * g.fog.a), 0.0, g.cam_pos.w > 0.5);
    col = mix(col, g.fog.rgb, clamp(f, 0.0, 1.0));
    return vec4<f32>(encode(col), 1.0);
}

@fragment
fn fs_main(i: VOut) -> @location(0) vec4<f32> {
    let n = normalize(i.normal);
    let m = i32(round(i.mat));
    let shadow = shadow_at(i.world, n);
    if (m >= 16) {
        return toon(i, m, n, shadow);
    }
    let dist0 = length(i.world - g.cam_pos.xyz);
    let s = surface(m, i.color, i.world, n, dist0 > 55.0 && g.cam_pos.w < 0.5);
    if (s.emissive > 0.5) {
        let d = length(i.world - g.cam_pos.xyz);
        let f = select(1.0 - exp(-d * g.fog.a), 0.0, g.cam_pos.w > 0.5);
        return vec4<f32>(encode(mix(s.albedo * 3.0, g.fog.rgb, f)), 1.0);
    }

    let l = -g.sun_dir.xyz;
    let ndl = dot(n, l);
    let lambert = max((ndl + s.wrap) / (1.0 + s.wrap), 0.0);

    // ambient: osmon + yer aksi
    let sky_amb = lin(vec3<f32>(0.40, 0.55, 0.80)) * 0.55;
    let gnd_amb = lin(vec3<f32>(0.42, 0.36, 0.28)) * 0.35;
    let amb = mix(gnd_amb, sky_amb, 0.5 + 0.5 * n.y);
    var col = s.albedo * (amb + SUN_COLOR * 2.6 * lambert * shadow);

    // yaltirash (Blinn)
    let v = normalize(g.cam_pos.xyz - i.world);
    let h = normalize(l + v);
    let sp = pow(max(dot(n, h), 0.0), s.shin) * s.spec * select(0.0, 1.0, ndl > 0.0);
    col = col + SUN_COLOR * 2.6 * sp * shadow * (s.shin / 16.0 * 0.3 + 0.5);
    if (m == 5) {
        // oyna: osmon aksi (Frenel)
        let fr = pow(1.0 - max(dot(n, v), 0.0), 3.0);
        col = col + sky_color(reflect(-v, n)) * (0.15 + 0.6 * fr);
    }

    if (g.cam_pos.w < 0.5) {
        let d = length(i.world - g.cam_pos.xyz);
        let f = 1.0 - exp(-d * g.fog.a);
        col = mix(col, g.fog.rgb, clamp(f, 0.0, 1.0));
    }
    return vec4<f32>(encode(col), 1.0);
}
