//! Ovozlar: hammasi kodda sintez qilinadi (fayl yo'q). Mono, 16 bit, `RATE` Hz.
//! Kotlin tomoni `nativeGetSound(id)` bilan oladi va `AudioTrack` da o'ynatadi.

pub const RATE: u32 = 22050;

/// O'yin hodisalari (Kotlin shu raqamlar bilan ovozni tanlaydi).
pub const EV_SHOT_PISTOL: u8 = 0;
pub const EV_SHOT_RIFLE: u8 = 1;
pub const EV_SHOT_SHOTGUN: u8 = 2;
pub const EV_SHOT_SNIPER: u8 = 3;
pub const EV_STEP: u8 = 4;
pub const EV_RELOAD: u8 = 5;
pub const EV_PICKUP: u8 = 6;
pub const EV_HURT: u8 = 7;
pub const SOUND_COUNT: usize = 8;

struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.0 >> 8) as f32 / (1u32 << 24) as f32) * 2.0 - 1.0
    }
}

fn env(t: f32, k: f32) -> f32 {
    (-t * k).exp()
}

fn sine(hz: f32, t: f32) -> f32 {
    (std::f32::consts::TAU * hz * t).sin()
}

/// Past chastotali filtr (bir qutbli): `a` kichik bo'lsa — yumshoqroq.
struct Lp {
    y: f32,
    a: f32,
}
impl Lp {
    fn new(a: f32) -> Self {
        Self { y: 0.0, a }
    }
    fn run(&mut self, x: f32) -> f32 {
        self.y += self.a * (x - self.y);
        self.y
    }
}

fn finish(buf: Vec<f32>, gain: f32) -> Vec<i16> {
    buf.into_iter().map(|x| ((x * gain).tanh() * 30000.0) as i16).collect()
}

fn shot(dur: f32, noise_k: f32, lp_a: f32, hz: f32, thump_k: f32, echo: f32) -> Vec<i16> {
    let n = (dur * RATE as f32) as usize;
    let mut rng = Noise(0x1234_5678);
    let mut lp = Lp::new(lp_a);
    let mut dry = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let noise = lp.run(rng.next()) * env(t, noise_k);
        let thump = sine(hz * (1.0 - 0.6 * t.min(0.2) / 0.2 * 0.5), t) * env(t, thump_k);
        dry.push(noise * 1.1 + thump * 0.9);
    }
    let mut out = dry.clone();
    if echo > 0.0 {
        let d = (0.13 * RATE as f32) as usize;
        for i in d..n {
            out[i] += dry[i - d] * echo;
        }
    }
    finish(out, 1.6)
}

fn click(buf: &mut [f32], t0: f32, hz: f32) {
    let start = (t0 * RATE as f32) as usize;
    let mut rng = Noise(0x9e37_79b9);
    for i in start..buf.len().min(start + (0.05 * RATE as f32) as usize) {
        let t = (i - start) as f32 / RATE as f32;
        buf[i] += (rng.next() * 0.5 + sine(hz, t)) * env(t, 110.0) * 0.8;
    }
}

/// `id` ovozini PCM (i16) ko'rinishida yasaydi; noto'g'ri `id` — bo'sh.
pub fn render(id: usize) -> Vec<i16> {
    match id as u8 {
        EV_SHOT_PISTOL => shot(0.22, 38.0, 0.55, 150.0, 30.0, 0.0),
        EV_SHOT_RIFLE => shot(0.18, 48.0, 0.75, 120.0, 38.0, 0.0),
        EV_SHOT_SHOTGUN => shot(0.45, 13.0, 0.4, 75.0, 14.0, 0.0),
        EV_SHOT_SNIPER => shot(0.75, 9.0, 0.5, 60.0, 9.0, 0.35),
        EV_STEP => {
            let n = (0.12 * RATE as f32) as usize;
            let mut rng = Noise(77);
            let mut lp = Lp::new(0.12);
            finish(
                (0..n)
                    .map(|i| {
                        let t = i as f32 / RATE as f32;
                        lp.run(rng.next()) * env(t, 38.0) * 1.2 + sine(85.0, t) * env(t, 45.0) * 0.5
                    })
                    .collect(),
                0.9,
            )
        }
        EV_RELOAD => {
            let mut b = vec![0.0f32; (0.9 * RATE as f32) as usize];
            click(&mut b, 0.0, 1700.0);
            click(&mut b, 0.42, 2300.0);
            finish(b, 1.0)
        }
        EV_PICKUP => {
            let n = (0.24 * RATE as f32) as usize;
            let mut phase = 0.0f32;
            finish(
                (0..n)
                    .map(|i| {
                        let t = i as f32 / RATE as f32;
                        phase += std::f32::consts::TAU * (650.0 + 800.0 * t / 0.24) / RATE as f32;
                        phase.sin() * (t / 0.005).min(1.0) * env(t, 9.0)
                    })
                    .collect(),
                0.9,
            )
        }
        EV_HURT => {
            let n = (0.30 * RATE as f32) as usize;
            let mut rng = Noise(4242);
            let mut lp = Lp::new(0.2);
            finish(
                (0..n)
                    .map(|i| {
                        let t = i as f32 / RATE as f32;
                        sine(110.0, t) * env(t, 16.0) + lp.run(rng.next()) * env(t, 14.0) * 0.8
                    })
                    .collect(),
                1.2,
            )
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hamma_ovoz_bor_va_tolgan() {
        for id in 0..SOUND_COUNT {
            let s = render(id);
            assert!(s.len() > 1000, "ovoz {id} bo'sh");
            let peak = s.iter().map(|x| (*x as i32).abs()).max().unwrap();
            assert!(peak > 2000 && peak <= 32767, "ovoz {id} balandligi {peak}");
        }
        assert!(render(SOUND_COUNT).is_empty());
    }

    #[test]
    fn ovoz_sekin_tugaydi() {
        // oxirgi qismi jimroq bo'lishi kerak (chertib qolmasin)
        let s = render(EV_SHOT_SHOTGUN as usize);
        let tail = s[s.len() - 200..].iter().map(|x| (*x as i32).abs()).max().unwrap();
        assert!(tail < 3000);
    }
}
