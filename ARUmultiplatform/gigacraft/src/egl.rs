//! EGL: OpenGL ES 2.0 konteksti (Android'da oyna ustida, kompyuterda — testlar uchun ko'rinmas "pbuffer").
use std::ffi::c_void;
use std::ptr::null_mut;

#[link(name = "EGL")]
extern "C" {
    fn eglGetDisplay(id: *mut c_void) -> *mut c_void;
    fn eglGetPlatformDisplay(platform: u32, native: *mut c_void, attribs: *const isize) -> *mut c_void;
    fn eglInitialize(d: *mut c_void, major: *mut i32, minor: *mut i32) -> u32;
    fn eglBindAPI(api: u32) -> u32;
    fn eglChooseConfig(d: *mut c_void, attrs: *const i32, cfgs: *mut *mut c_void, n: i32, num: *mut i32) -> u32;
    pub fn eglGetConfigAttrib(d: *mut c_void, c: *mut c_void, a: i32, v: *mut i32) -> u32;
    fn eglCreateContext(d: *mut c_void, c: *mut c_void, share: *mut c_void, attrs: *const i32) -> *mut c_void;
    fn eglCreateWindowSurface(d: *mut c_void, c: *mut c_void, w: *mut c_void, attrs: *const i32) -> *mut c_void;
    fn eglCreatePbufferSurface(d: *mut c_void, c: *mut c_void, attrs: *const i32) -> *mut c_void;
    fn eglMakeCurrent(d: *mut c_void, draw: *mut c_void, read: *mut c_void, ctx: *mut c_void) -> u32;
    fn eglSwapBuffers(d: *mut c_void, s: *mut c_void) -> u32;
    fn eglSwapInterval(d: *mut c_void, i: i32) -> u32;
    fn eglDestroySurface(d: *mut c_void, s: *mut c_void) -> u32;
    fn eglDestroyContext(d: *mut c_void, c: *mut c_void) -> u32;
    fn eglGetError() -> i32;
}

pub struct Egl {
    pub dpy: *mut c_void,
    pub cfg: *mut c_void,
    ctx: *mut c_void,
    surf: *mut c_void,
}

unsafe impl Send for Egl {}

const EGL_SURFACE_TYPE: i32 = 0x3033;
const EGL_RENDERABLE_TYPE: i32 = 0x3040;

impl Egl {
    /// `pbuffer = true` — ko'rinmas sirt (testlar), aks holda oyna uchun sozlanadi.
    pub fn new(pbuffer: bool) -> Result<Egl, String> {
        unsafe {
            let mut dpy = null_mut();
            if pbuffer {
                // Mesa "surfaceless" platformasi: ekran/X siz ishlaydi.
                dpy = eglGetPlatformDisplay(0x31DD, null_mut(), std::ptr::null());
            }
            if dpy.is_null() {
                dpy = eglGetDisplay(null_mut());
            }
            if dpy.is_null() || eglInitialize(dpy, null_mut(), null_mut()) == 0 {
                return Err(format!("eglInitialize xato {:x}", eglGetError()));
            }
            eglBindAPI(0x30A0);
            let attrs = [
                EGL_SURFACE_TYPE,
                if pbuffer { 1 } else { 4 },
                EGL_RENDERABLE_TYPE,
                4,
                0x3024,
                8,
                0x3023,
                8,
                0x3022,
                8,
                0x3025,
                24,
                0x3038,
            ];
            let mut cfg: *mut c_void = null_mut();
            let mut n = 0;
            if eglChooseConfig(dpy, attrs.as_ptr(), &mut cfg, 1, &mut n) == 0 || n < 1 {
                return Err(format!("eglChooseConfig xato {:x}", eglGetError()));
            }
            let ctx_attrs = [0x3098, 2, 0x3038];
            let ctx = eglCreateContext(dpy, cfg, null_mut(), ctx_attrs.as_ptr());
            if ctx.is_null() {
                return Err(format!("eglCreateContext xato {:x}", eglGetError()));
            }
            Ok(Egl { dpy, cfg, ctx, surf: null_mut() })
        }
    }

    /// Ko'rinmas sirt yaratib, kontekstni joriy qiladi.
    pub fn make_pbuffer(&mut self, w: i32, h: i32) -> Result<(), String> {
        unsafe {
            self.release_surface();
            let a = [0x3057, w, 0x3056, h, 0x3038];
            self.surf = eglCreatePbufferSurface(self.dpy, self.cfg, a.as_ptr());
            if self.surf.is_null() {
                return Err(format!("eglCreatePbufferSurface xato {:x}", eglGetError()));
            }
            if eglMakeCurrent(self.dpy, self.surf, self.surf, self.ctx) == 0 {
                return Err(format!("eglMakeCurrent xato {:x}", eglGetError()));
            }
            Ok(())
        }
    }

    /// Oynaga sirt yaratadi va kontekstni joriy qiladi (Android).
    pub fn set_window(&mut self, window: *mut c_void) -> Result<(), String> {
        unsafe {
            self.release_surface();
            if window.is_null() {
                return Ok(());
            }
            self.surf = eglCreateWindowSurface(self.dpy, self.cfg, window, std::ptr::null());
            if self.surf.is_null() {
                return Err(format!("eglCreateWindowSurface xato {:x}", eglGetError()));
            }
            if eglMakeCurrent(self.dpy, self.surf, self.surf, self.ctx) == 0 {
                return Err(format!("eglMakeCurrent xato {:x}", eglGetError()));
            }
            eglSwapInterval(self.dpy, 1);
            Ok(())
        }
    }

    pub fn release_surface(&mut self) {
        unsafe {
            if !self.surf.is_null() {
                eglMakeCurrent(self.dpy, null_mut(), null_mut(), null_mut());
                eglDestroySurface(self.dpy, self.surf);
                self.surf = null_mut();
            }
        }
    }

    /// Kadrni ko'rsatadi. `false` — kontekst yo'qolgan.
    pub fn swap(&mut self) -> bool {
        unsafe { eglSwapBuffers(self.dpy, self.surf) != 0 || eglGetError() != 0x300E }
    }
}

impl Drop for Egl {
    fn drop(&mut self) {
        self.release_surface();
        unsafe {
            eglDestroyContext(self.dpy, self.ctx);
        }
    }
}
