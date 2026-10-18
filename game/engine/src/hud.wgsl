struct Screen { size: vec4<f32> };
@group(0) @binding(0) var<uniform> s: Screen;

struct VIn {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
};
struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    let x = v.pos.x / s.size.x * 2.0 - 1.0;
    let y = 1.0 - v.pos.y / s.size.y * 2.0;
    o.clip = vec4<f32>(x, y, 0.0, 1.0);
    o.color = v.color;
    return o;
}

@fragment
fn fs_main(i: VOut) -> @location(0) vec4<f32> {
    return i.color;
}
