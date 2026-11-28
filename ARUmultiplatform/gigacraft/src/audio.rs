//! Tovush: tashqi fayllarsiz, kodning o'zida sintez qilinadi (zarbalar, qadam, yomg'ir shovqini, sokin musiqa).
//! Android'da AAudio (libaaudio.so) `dlopen` orqali ochiladi; boshqa tizimlarda faqat sintezator (sinov uchun).
use std::sync::{Mutex, OnceLock};

pub const RATE: u32 = 22050;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sfx {
    Break,
    Place,
    Hurt,
    Pickup,
    Step,
    Eat,
    Hit,
    Groan,
    Craft,
    Moo,
}

#[derive(Clone, Copy)]
struct Voice {
    f: f32,
    slide: f32,
    noise: f32,
    amp: f32,
    decay: f32,
    t: f32,
    phase: f32,
}

pub struct Synth {
    voices: Vec<Voice>,
    rng: u32,
    pub rain: f32,
    pub music: bool,
    mt: f32,
    lp: f32,
    next_note: f32,
    note_v: Vec<Voice>,
}

impl Default for Synth {
    fn default() -> Self {
        Synth::new()
    }
}

impl Synth {
    pub fn new() -> Synth {
        Synth { voices: Vec::new(), rng: 0x9E37_79B9, rain: 0.0, music: true, mt: 0.0, lp: 0.0, next_note: 3.0, note_v: Vec::new() }
    }

    fn noise(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }

    pub fn trigger(&mut self, s: Sfx) {
        let v = |f: f32, slide: f32, noise: f32, amp: f32, decay: f32| Voice { f, slide, noise, amp, decay, t: 0.0, phase: 0.0 };
        let list: Vec<Voice> = match s {
            Sfx::Break => vec![v(140.0, -120.0, 0.9, 0.45, 9.0)],
            Sfx::Place => vec![v(220.0, -60.0, 0.35, 0.40, 14.0)],
            Sfx::Hurt => vec![v(300.0, -220.0, 0.25, 0.55, 6.0)],
            Sfx::Pickup => vec![v(660.0, 700.0, 0.0, 0.25, 9.0)],
            Sfx::Step => vec![v(90.0, -40.0, 0.8, 0.20, 22.0)],
            Sfx::Eat => vec![v(180.0, 0.0, 0.9, 0.30, 12.0), v(200.0, 0.0, 0.9, 0.0, 12.0)],
            Sfx::Hit => vec![v(180.0, -90.0, 0.6, 0.5, 14.0)],
            Sfx::Groan => vec![v(95.0, -25.0, 0.1, 0.45, 2.5), v(142.0, -30.0, 0.1, 0.3, 2.5)],
            Sfx::Craft => vec![v(520.0, 300.0, 0.0, 0.25, 7.0), v(780.0, 300.0, 0.0, 0.15, 7.0)],
            Sfx::Moo => vec![v(130.0, 30.0, 0.05, 0.4, 1.6)],
        };
        for x in list {
            if self.voices.len() < 24 {
                self.voices.push(x);
            }
        }
    }

    /// `out` ga 16 bitli mono namunalarni yozadi.
    pub fn render(&mut self, out: &mut [i16]) {
        let dt = 1.0 / RATE as f32;
        for o in out.iter_mut() {
            let mut sum = 0.0f32;
            // Qisqa ovozlar
            let mut i = 0;
            while i < self.voices.len() {
                let mut vc = self.voices[i];
                let env = (-vc.t * vc.decay).exp();
                if env < 0.004 {
                    self.voices.swap_remove(i);
                    continue;
                }
                let f = (vc.f + vc.slide * (1.0 - env)).max(20.0);
                vc.phase += f * dt;
                if vc.phase > 1.0 {
                    vc.phase -= 1.0;
                }
                let n = self.noise();
                let tone = (vc.phase * std::f32::consts::TAU).sin();
                sum += vc.amp * env * (tone * (1.0 - vc.noise) + n * vc.noise);
                vc.t += dt;
                self.voices[i] = vc;
                i += 1;
            }
            // Yomg'ir: past chastotali filtrlangan shovqin
            if self.rain > 0.01 {
                let n = self.noise();
                self.lp += (n - self.lp) * 0.25;
                sum += self.lp * 0.22 * self.rain;
            }
            // Sokin musiqa: sekin-asta ttanlanadigan pentatonik notalar
            if self.music {
                self.mt += dt;
                if self.mt >= self.next_note {
                    self.mt = 0.0;
                    self.next_note = 2.5 + (self.noise() + 1.0) * 2.5;
                    const SCALE: [f32; 7] = [261.6, 293.7, 329.6, 392.0, 440.0, 523.3, 587.3];
                    let k = ((self.noise() + 1.0) * 0.5 * 6.99) as usize;
                    let base = SCALE[k.min(6)] * 0.5;
                    if self.note_v.len() < 6 {
                        self.note_v.push(Voice { f: base, slide: 0.0, noise: 0.0, amp: 0.07, decay: 0.9, t: 0.0, phase: 0.0 });
                        self.note_v.push(Voice { f: base * 1.5, slide: 0.0, noise: 0.0, amp: 0.035, decay: 0.9, t: 0.0, phase: 0.0 });
                    }
                }
                let mut j = 0;
                while j < self.note_v.len() {
                    let mut vc = self.note_v[j];
                    let env = (-vc.t * vc.decay).exp() * (1.0 - (-vc.t * 6.0).exp());
                    if vc.t > 1.0 && env < 0.003 {
                        self.note_v.swap_remove(j);
                        continue;
                    }
                    vc.phase += vc.f * dt;
                    if vc.phase > 1.0 {
                        vc.phase -= 1.0;
                    }
                    sum += vc.amp * env * (vc.phase * std::f32::consts::TAU).sin();
                    vc.t += dt;
                    self.note_v[j] = vc;
                    j += 1;
                }
            }
            *o = (sum.clamp(-1.0, 1.0) * 30000.0) as i16;
        }
    }
}

fn global() -> &'static Mutex<Synth> {
    static S: OnceLock<Mutex<Synth>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Synth::new()))
}

pub fn play(s: Sfx) {
    if let Ok(mut g) = global().lock() {
        g.trigger(s);
    }
}

pub fn set_rain(r: f32) {
    if let Ok(mut g) = global().lock() {
        g.rain = r.clamp(0.0, 1.0);
    }
}

pub fn set_music(on: bool) {
    if let Ok(mut g) = global().lock() {
        g.music = on;
    }
}

/// Chiqish oqimini ochadi (bir marta). Xato bo'lsa jim o'tadi.
pub fn start() {
    #[cfg(target_os = "android")]
    backend::start();
}

#[cfg(target_os = "android")]
mod backend {
    use super::*;
    use std::ffi::{c_char, c_void, CString};

    extern "C" {
        fn dlopen(path: *const c_char, flags: i32) -> *mut c_void;
        fn dlsym(h: *mut c_void, name: *const c_char) -> *mut c_void;
    }

    type CreateBuilder = unsafe extern "C" fn(*mut *mut c_void) -> i32;
    type SetI32 = unsafe extern "C" fn(*mut c_void, i32);
    type SetCb = unsafe extern "C" fn(*mut c_void, DataCb, *mut c_void);
    type OpenStream = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32;
    type StreamOp = unsafe extern "C" fn(*mut c_void) -> i32;
    type DataCb = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, i32) -> i32;

    unsafe extern "C" fn data_cb(_stream: *mut c_void, _user: *mut c_void, audio: *mut c_void, frames: i32) -> i32 {
        let out = std::slice::from_raw_parts_mut(audio as *mut i16, frames.max(0) as usize);
        match global().try_lock() {
            Ok(mut g) => g.render(out),
            Err(_) => out.iter_mut().for_each(|s| *s = 0),
        }
        0
    }

    unsafe fn sym<T>(h: *mut c_void, name: &str) -> Option<T> {
        let c = CString::new(name).ok()?;
        let p = dlsym(h, c.as_ptr());
        if p.is_null() {
            None
        } else {
            Some(std::mem::transmute_copy::<*mut c_void, T>(&p))
        }
    }

    pub fn start() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| unsafe {
            let lib = CString::new("libaaudio.so").unwrap();
            let h = dlopen(lib.as_ptr(), 2);
            if h.is_null() {
                return;
            }
            let go = || -> Option<()> {
                let create: CreateBuilder = sym(h, "AAudio_createStreamBuilder")?;
                let set_fmt: SetI32 = sym(h, "AAudioStreamBuilder_setFormat")?;
                let set_ch: SetI32 = sym(h, "AAudioStreamBuilder_setChannelCount")?;
                let set_rate: SetI32 = sym(h, "AAudioStreamBuilder_setSampleRate")?;
                let set_perf: SetI32 = sym(h, "AAudioStreamBuilder_setPerformanceMode")?;
                let set_cb: SetCb = sym(h, "AAudioStreamBuilder_setDataCallback")?;
                let open: OpenStream = sym(h, "AAudioStreamBuilder_openStream")?;
                let delete: StreamOp = sym(h, "AAudioStreamBuilder_delete")?;
                let start: StreamOp = sym(h, "AAudioStream_requestStart")?;
                let mut b: *mut c_void = std::ptr::null_mut();
                if create(&mut b) != 0 || b.is_null() {
                    return None;
                }
                set_fmt(b, 1); // PCM_I16
                set_ch(b, 1);
                set_rate(b, RATE as i32);
                set_perf(b, 12); // POWER_SAVING emas: NONE=10, POWER_SAVING=11, LOW_LATENCY=12
                set_cb(b, data_cb, std::ptr::null_mut());
                let mut st: *mut c_void = std::ptr::null_mut();
                let r = open(b, &mut st);
                delete(b);
                if r != 0 || st.is_null() {
                    return None;
                }
                start(st);
                Some(())
            };
            let _ = go();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sintez_tovush_chiqaradi_va_tinadi() {
        let mut s = Synth::new();
        s.music = false;
        let mut buf = vec![0i16; 4000];
        s.render(&mut buf);
        assert!(buf.iter().all(|&x| x == 0), "jim bo'lishi kerak");
        for sfx in [Sfx::Break, Sfx::Place, Sfx::Hurt, Sfx::Pickup, Sfx::Step, Sfx::Eat, Sfx::Hit, Sfx::Groan, Sfx::Craft, Sfx::Moo] {
            s.trigger(sfx);
            let mut b = vec![0i16; 2000];
            s.render(&mut b);
            assert!(b.iter().any(|&x| x.abs() > 500), "{sfx:?} ovozsiz");
        }
        // uzoq vaqtdan keyin hammasi tinadi
        let mut long = vec![0i16; RATE as usize * 6];
        s.render(&mut long);
        let tail = &long[long.len() - 500..];
        assert!(tail.iter().all(|&x| x == 0));
    }

    #[test]
    fn yomgir_va_musiqa() {
        let mut s = Synth::new();
        s.rain = 1.0;
        s.music = false;
        let mut b = vec![0i16; 3000];
        s.render(&mut b);
        assert!(b.iter().any(|&x| x.abs() > 300));
        let mut s2 = Synth::new();
        s2.next_note = 0.01;
        let mut b2 = vec![0i16; RATE as usize];
        s2.render(&mut b2);
        assert!(b2.iter().any(|&x| x.abs() > 200), "musiqa yo'q");
        assert!(b2.iter().all(|&x| x.abs() < 20000));
    }
}
