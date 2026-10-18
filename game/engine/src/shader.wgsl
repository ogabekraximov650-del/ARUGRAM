struct Globals {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    fog: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;

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

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    let w = v.pos * v.i_scale.xyz + v.i_pos.xyz;
    o.world = w;
    o.normal = v.normal;
    o.color = v.i_color;
    o.clip = g.view_proj * vec4<f32>(w, 1.0);
    return o;
}

@fragment
fn fs_main(i: VOut) -> @location(0) vec4<f32> {
    let n = normalize(i.normal);
    let lambert = max(dot(n, -g.sun_dir.xyz), 0.0);
    let hemi = 0.5 + 0.5 * n.y;
    let light = 0.30 + 0.25 * hemi + 0.75 * lambert;
    var col = i.color.rgb * light;

    // zamin to'ri: 2 m li katakchalar
    let p = i.world.xz / 2.0;
    let c = abs(fract(p - 0.5) - 0.5) / max(fwidth(p), vec2<f32>(0.0001));
    let line = 1.0 - min(min(c.x, c.y), 1.0);
    let on_top = select(0.0, 1.0, n.y > 0.5) * i.color.a;
    col = mix(col, col * 0.72, line * 0.55 * on_top);

    let d = length(i.world - g.cam_pos.xyz);
    let f = 1.0 - exp(-pow(d * g.fog.a, 2.0));
    col = mix(col, g.fog.rgb, clamp(f, 0.0, 1.0));
    return vec4<f32>(col, 1.0);
}
