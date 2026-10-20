//! Animatsiya: easing, tween va prujina (spring).

/// Vaqt egri chiziqlari. `t` 0..=1 oralig'ida.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// Oxirida biroz oshib ketib qaytadi (Android `OvershootInterpolator`).
    Overshoot(f32),
}

impl Easing {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t * t,
            Easing::EaseOut => 1.0 - (1.0 - t).powi(3),
            Easing::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::Overshoot(k) => {
                let t = t - 1.0;
                t * t * ((k + 1.0) * t + k) + 1.0
            }
        }
    }
}

/// Ikki qiymat orasidagi vaqtga bog'liq o'tish.
#[derive(Clone, Copy, Debug)]
pub struct Tween {
    pub from: f32,
    pub to: f32,
    pub duration_ms: f32,
    pub easing: Easing,
    elapsed_ms: f32,
}

impl Tween {
    pub fn new(from: f32, to: f32, duration_ms: f32, easing: Easing) -> Self {
        Self {
            from,
            to,
            duration_ms: duration_ms.max(1.0),
            easing,
            elapsed_ms: 0.0,
        }
    }
    /// `dt_ms` ga oldinga siljiydi va joriy qiymatni qaytaradi.
    pub fn step(&mut self, dt_ms: f32) -> f32 {
        self.elapsed_ms = (self.elapsed_ms + dt_ms).min(self.duration_ms);
        self.value()
    }
    pub fn value(&self) -> f32 {
        let p = self.easing.apply(self.elapsed_ms / self.duration_ms);
        self.from + (self.to - self.from) * p
    }
    pub fn finished(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }
}

/// Fizik prujina: `stiffness` (qattiqlik) va `damping_ratio` (1.0 — tebranishsiz).
#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
    pub stiffness: f32,
    pub damping_ratio: f32,
}

impl Spring {
    pub fn new(value: f32, stiffness: f32, damping_ratio: f32) -> Self {
        Self {
            value,
            velocity: 0.0,
            target: value,
            stiffness,
            damping_ratio,
        }
    }
    /// Yarim-oshkor Eyler integratsiyasi, kichik qadamlarga bo'lib (barqarorlik uchun).
    pub fn step(&mut self, dt_ms: f32) -> f32 {
        let mut left = dt_ms / 1000.0;
        let damping = 2.0 * self.damping_ratio * self.stiffness.sqrt();
        while left > 0.0 {
            let dt = left.min(1.0 / 240.0);
            let accel = self.stiffness * (self.target - self.value) - damping * self.velocity;
            self.velocity += accel * dt;
            self.value += self.velocity * dt;
            left -= dt;
        }
        self.value
    }
    pub fn at_rest(&self) -> bool {
        (self.target - self.value).abs() < 0.001 && self.velocity.abs() < 0.001
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_endpoints() {
        for e in [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::Overshoot(1.5),
        ] {
            assert!((e.apply(0.0)).abs() < 1e-5, "{e:?} boshi");
            assert!((e.apply(1.0) - 1.0).abs() < 1e-5, "{e:?} oxiri");
        }
    }

    #[test]
    fn overshoot_exceeds_one() {
        let m = (0..=100)
            .map(|i| Easing::Overshoot(1.5).apply(i as f32 / 100.0))
            .fold(0.0f32, f32::max);
        assert!(m > 1.0);
    }

    #[test]
    fn tween_reaches_target() {
        let mut t = Tween::new(0.0, 10.0, 200.0, Easing::Linear);
        assert!((t.step(100.0) - 5.0).abs() < 1e-4);
        assert_eq!(t.step(500.0), 10.0);
        assert!(t.finished());
    }

    #[test]
    fn spring_settles() {
        let mut s = Spring::new(0.0, 400.0, 1.0);
        s.target = 1.0;
        for _ in 0..200 {
            s.step(16.0);
        }
        assert!(s.at_rest());
        assert!((s.value - 1.0).abs() < 0.01);
    }
}
