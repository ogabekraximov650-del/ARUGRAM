//! Birinchi shaxs kamerasi: joy + ko'rish burchaklari (yaw, pitch).

use glam::{Mat4, Vec3};

pub struct Camera {
    pub pos: Vec3,
    /// Gorizontal burilish (radian). 0 — -Z o'qi tomonga qaraydi, musbat qiymat chapga buriladi.
    pub yaw: f32,
    /// Vertikal burilish (radian), `±PITCH_MAX` bilan cheklangan.
    pub pitch: f32,
    pub fov_y: f32,
}

const PITCH_MAX: f32 = 1.45;

impl Camera {
    pub fn new(pos: Vec3) -> Self {
        Self { pos, yaw: 0.0, pitch: 0.0, fov_y: 70f32.to_radians() }
    }

    /// Qarash yo'nalishi (birlik vektor).
    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(-sy * cp, sp, -cy * cp)
    }

    /// Yerga parallel o'ng tomon.
    pub fn right(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(cy, 0.0, -sy)
    }

    /// Yerga parallel oldinga yo'nalish (yurish uchun).
    pub fn flat_forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(-sy, 0.0, -cy)
    }

    pub fn look(&mut self, dyaw: f32, dpitch: f32) {
        self.yaw = (self.yaw + dyaw) % std::f32::consts::TAU;
        self.pitch = (self.pitch + dpitch).clamp(-PITCH_MAX, PITCH_MAX);
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let view = Mat4::look_to_rh(self.pos, self.forward(), Vec3::Y);
        let proj = Mat4::perspective_rh(self.fov_y, aspect.max(0.01), 0.1, 400.0);
        proj * view
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boshida_minus_z_ga_qaraydi() {
        let c = Camera::new(Vec3::ZERO);
        let f = c.forward();
        assert!((f - Vec3::new(0.0, 0.0, -1.0)).length() < 1e-5);
    }

    #[test]
    fn yaw_chapga_buradi() {
        let mut c = Camera::new(Vec3::ZERO);
        c.look(std::f32::consts::FRAC_PI_2, 0.0);
        // 90° chapga: -X tomonga qaraydi.
        assert!((c.forward() - Vec3::new(-1.0, 0.0, 0.0)).length() < 1e-5);
        assert!((c.right() - Vec3::new(0.0, 0.0, -1.0)).length() < 1e-5);
    }

    #[test]
    fn pitch_chegaralangan() {
        let mut c = Camera::new(Vec3::ZERO);
        c.look(0.0, 10.0);
        assert!(c.pitch <= 1.45 + 1e-6);
        c.look(0.0, -20.0);
        assert!(c.pitch >= -1.45 - 1e-6);
    }
}
