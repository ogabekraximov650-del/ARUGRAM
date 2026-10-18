struct Globals {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    fog: vec4<f32>,
    light_vp: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_smp: sampler_comparison;

const SHADOW_SIZE: f32 = 2048.0;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) i_pos: vec4<f32>,
    @location(3) i_scale: vec4<f32>,
    @location(4) i_color: vec4<f32>,
};
struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

fn rot(v: vec3<f32>, yaw: f32) -> vec3<f32> {
    let c = cos(yaw);
    let s = sin(yaw);
    return vec3<f32>(v.x * c + v.z * s, v.y, -v.x * s + v.z * c);
}

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    let w = rot(v.pos * v.i_scale.xyz, v.i_pos.w) + v.i_pos.xyz;
    o.world = w;
    o.normal = rot(v.normal, v.i_pos.w);
    o.color = v.i_color;
    o.clip = g.view_proj * vec4<f32>(w, 1.0);
    return o;
}

@vertex
fn vs_shadow(v: VIn) -> @builtin(position) vec4<f32> {
    let w = rot(v.pos * v.i_scale.xyz, v.i_pos.w) + v.i_pos.xyz;
    return g.light_vp * vec4<f32>(w, 1.0);
}

@fragment
fn fs_main(i: VOut) -> @location(0) vec4<f32> {
    let n = normalize(i.normal);
    let lambert = max(dot(n, -g.sun_dir.xyz), 0.0);

    // soya: 3x3 PCF
    let ls = g.light_vp * vec4<f32>(i.world + n * 0.06, 1.0);
    let uv = vec2<f32>(ls.x * 0.5 + 0.5, 1.0 - (ls.y * 0.5 + 0.5));
    let texel = 1.0 / SHADOW_SIZE;
    var sh = 0.0;
    for (var ox = -1; ox <= 1; ox = ox + 1) {
        for (var oy = -1; oy <= 1; oy = oy + 1) {
            let o = vec2<f32>(f32(ox), f32(oy)) * texel;
            sh = sh + textureSampleCompare(shadow_map, shadow_smp, clamp(uv + o, vec2<f32>(0.001), vec2<f32>(0.999)), ls.z);
        }
    }
    sh = sh / 9.0;
    let inside = select(0.0, 1.0, uv.x > 0.0 && uv.x < 1.0 && uv.y > 0.0 && uv.y < 1.0 && ls.z < 1.0);
    let shadow = mix(1.0, sh, inside);
    let hemi = 0.5 + 0.5 * n.y;
    let light = 0.30 + 0.25 * hemi + 0.75 * lambert * shadow;
    var col = i.color.rgb * light;

    // zamin to'ri: 2 m li katakchalar
    let p = i.world.xz / 2.0;
    let c = abs(fract(p - 0.5) - 0.5) / max(fwidth(p), vec2<f32>(0.0001));
    let line = 1.0 - min(min(c.x, c.y), 1.0);
    let on_top = select(0.0, 1.0, n.y > 0.5 && i.color.a > 0.5);
    col = mix(col, col * 0.72, line * 0.55 * on_top);

    let d = length(i.world - g.cam_pos.xyz);
    let f = 1.0 - exp(-pow(d * g.fog.a, 2.0));
    col = mix(col, g.fog.rgb, clamp(f, 0.0, 1.0));
    return vec4<f32>(col, 1.0);
}
