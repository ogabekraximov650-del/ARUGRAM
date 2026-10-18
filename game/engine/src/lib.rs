//! ARU o'yin dvigateli (tajriba bosqichi): 3D dunyo, kamera, sensorli boshqaruv.

pub mod camera;
pub mod hud;
pub mod input;
pub mod renderer;
pub mod world;

#[cfg(target_os = "android")]
mod android;

use camera::Camera;
use glam::Vec3;
use input::{Input, TouchAction};

/// O'yin holati (GPU'dan mustaqil — testlash oson).
pub struct Game {
    pub camera: Camera,
    pub input: Input,
    pub width: u32,
    pub height: u32,
    pub density: f32,
}

impl Game {
    pub fn new(width: u32, height: u32, density: f32) -> Self {
        Self {
            camera: Camera::new(Vec3::new(0.0, 1.7, 8.0)),
            input: Input::new(width, density),
            width,
            height,
            density,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.input.resize(width);
    }

    pub fn touch(&mut self, action: TouchAction, id: i32, x: f32, y: f32) {
        self.input.touch(action, id, x, y);
    }

    pub fn update(&mut self, dt: f32) {
        self.input.update(dt, &mut self.camera);
    }

    pub fn hud(&self) -> Vec<hud::HudVertex> {
        hud::build(&self.input, self.width as f32, self.height as f32, self.density)
    }
}
