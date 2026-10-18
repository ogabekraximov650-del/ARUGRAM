//! Oflayn chizish: bir necha kadrni PNG ga yozadi (Vulkan/llvmpipe yoki haqiqiy GPU).
//! Ishlatish: cargo run --release --example preview -- <chiqish_papkasi>

use aru_game::input::TouchAction;
use aru_game::renderer::Renderer;
use aru_game::{world, Game};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let (w, h) = (1280u32, 576u32);
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("adapter yo'q");
    println!("GPU: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
        .expect("device");
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut r = Renderer::new(device, queue, format, w, h, &world::build());
    let tex = r.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("out"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    let mut g = Game::new(w, h, 2.5);

    // 0) boshlang'ich; 1) bot oldida + otish; 2) yurish va sakrash
    for shot in 0..3 {
        if shot == 1 {
            g.bots[0].pos = glam::Vec3::new(1.5, 0.0, 2.0);
            g.bots[1].pos = glam::Vec3::new(-6.0, 0.0, 0.0);
            let (fc, _) = g.input.fire_button();
            g.touch(TouchAction::Down, 3, fc.x, fc.y);
            g.update(1.0 / 60.0);
            g.update(1.0 / 60.0);
        }
        if shot == 2 {
            g.touch(TouchAction::Up, 3, 0.0, 0.0);
            g.touch(TouchAction::Down, 1, 300.0, 400.0);
            g.touch(TouchAction::Move, 1, 300.0, 340.0);
            for _ in 0..90 {
                g.update(1.0 / 30.0);
            }
            let (jc, _) = g.input.jump_button();
            g.touch(TouchAction::Down, 4, jc.x, jc.y);
            for _ in 0..8 {
                g.update(1.0 / 60.0);
            }
        }
        let hud = g.hud();
        let dynamic = g.dynamic_instances();
        r.render(&view, &g.camera, &hud, &dynamic);
        let bpr = w * 4;
        let buf = r.device().create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (bpr * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = r.device().create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        r.queue().submit(Some(enc.finish()));
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        r.device().poll(wgpu::Maintain::Wait);
        let data = slice.get_mapped_range().to_vec();
        let path = format!("{out}/preview_{shot}.png");
        image::save_buffer(&path, &data, w, h, image::ColorType::Rgba8).unwrap();
        println!("yozildi: {path}");
    }
}
