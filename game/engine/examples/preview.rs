//! Oflayn chizish: bir necha kadrni PNG ga yozadi (Vulkan/llvmpipe yoki haqiqiy GPU).
//! Ishlatish: cargo run --release --example preview -- <chiqish_papkasi>

use aru_game::input::TouchAction;
use aru_game::renderer::{Frame, Renderer};
use aru_game::{world, Game};
use glam::Vec3;

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
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None)).expect("device");
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut r = Renderer::new(device, queue, format, w, h, &world::build().statics);
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
    let wd = world::build();
    let roof = wd.statics.0[aru_game::scene::Mesh::Prism as usize][0].pos;
    let mut g = Game::new(w, h, 2.5);
    g.zone.t = 0.0;

    // (nom, kamera joyi, yaw, pitch, qo'shimcha harakat)
    for shot in [0, 8, 2, 3, 4, 5, 6, 1, 7] {
        match shot {
            0 => {
                // boshlang'ich ko'rinish (to'pponcha)
                for _ in 0..30 {
                    g.update(1.0 / 60.0);
                }
            }
            1 => {
                // otish
                let (fc, _) = g.input.fire_button();
                g.touch(TouchAction::Down, 3, fc.x, fc.y);
                g.update(1.0 / 60.0);
                g.update(1.0 / 60.0);
            }
            2 => {
                // qizga yaqin
                g.touch(TouchAction::Up, 3, 0.0, 0.0);
                g.camera.pos = Vec3::new(0.0, 1.5, 10.0);
                g.camera.yaw = 0.0;
                g.camera.pitch = -0.05;
                g.companion.pos = Vec3::new(0.4, 0.0, 8.2);
                g.companion.yaw = std::f32::consts::PI;
            }
            3 => {
                // qishloq: yuqoridan
                g.camera.pos = Vec3::new(-34.0, 9.0, 40.0);
                g.camera.yaw = -0.55;
                g.camera.pitch = -0.22;
            }
            4 => {
                // ombor va tepaliklar tomon
                g.camera.pos = Vec3::new(roof[0] + 9.0, 2.2, roof[2] + 9.0);
                g.camera.yaw = 0.78;
                g.camera.pitch = -0.08;
            }
            5 => {
                // uy ichi: birinchi uyga qarab
                g.camera.pos = Vec3::new(roof[0], 1.9, roof[2]);
                g.camera.yaw = 0.6;
                g.camera.pitch = -0.1;
            }
            6 => {
                // avtomat bilan
                g.slots.push(aru_game::game::Slot { kind: aru_game::game::WeaponKind::Rifle, mag: 30, reserve: 60 });
                g.cur = 1;
                g.camera.pos = Vec3::new(0.0, 1.7, 14.0);
                g.camera.yaw = 0.3;
                g.camera.pitch = 0.0;
                for _ in 0..40 {
                    g.update(1.0 / 60.0);
                }
            }
            8 => {
                // yuz yaqindan
                g.touch(TouchAction::Up, 7, 0.0, 0.0);
                g.camera.pos = Vec3::new(0.4, 1.58, 9.2);
                g.camera.yaw = 0.0;
                g.camera.pitch = 0.0;
                g.companion.pos = Vec3::new(0.4, 0.0, 8.2);
                g.companion.yaw = std::f32::consts::PI;
            }
            _ => {
                // sozlamalar
                let (gc, _) = g.input.gear_button();
                g.touch(TouchAction::Down, 7, gc.x, gc.y);
                g.touch(TouchAction::Up, 7, gc.x, gc.y);
            }
        }
        let hud = g.hud();
        let fd = g.frame();
        let frame = Frame { dynamic: &fd.dynamic, dynamic_shadow: fd.dynamic_shadow, viewmodel: &fd.viewmodel, hud: &hud, time: g.time };
        r.render(&view, &g.camera, &frame);
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
