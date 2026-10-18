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

    // 1) boshlang'ich ko'rinish; 2) joystik bilan oldinga + o'ngga qarash
    for shot in 0..3 {
        if shot == 1 {
            g.touch(TouchAction::Down, 1, 300.0, 400.0);
            g.touch(TouchAction::Move, 1, 300.0, 320.0);
            g.touch(TouchAction::Down, 2, 900.0, 300.0);
            g.touch(TouchAction::Move, 2, 700.0, 280.0);
            for _ in 0..60 {
                g.update(1.0 / 30.0);
            }
        }
        if shot == 2 {
            for _ in 0..40 {
                g.update(1.0 / 30.0);
            }
            g.touch(TouchAction::Move, 2, 400.0, 300.0);
            g.update(1.0 / 30.0);
        }
        let hud = g.hud();
        r.render(&view, &g.camera, &hud);
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
