//! UZ: EGL: ANativeWindow ustida OpenGL ES 2.0 konteksti.
//! EN: EGL: an OpenGL ES 2.0 context on top of an ANativeWindow.
//! RU: EGL: контекст OpenGL ES 2.0 поверх ANativeWindow.

use std::ffi::c_void;
use std::ptr::null_mut;

#[link(name = "EGL")]
extern "C" {
    fn eglGetDisplay(id: *mut c_void) -> *mut c_void;
    fn eglInitialize(d: *mut c_void, major: *mut i32, minor: *mut i32) -> u32;
    fn eglChooseConfig(
        d: *mut c_void,
        attrs: *const i32,
        cfgs: *mut *mut c_void,
        n: i32,
        num: *mut i32,
    ) -> u32;
    fn eglGetConfigAttrib(d: *mut c_void, c: *mut c_void, a: i32, v: *mut i32) -> u32;
    fn eglCreateContext(
        d: *mut c_void,
        c: *mut c_void,
        share: *mut c_void,
        attrs: *const i32,
    ) -> *mut c_void;
    fn eglCreateWindowSurface(
        d: *mut c_void,
        c: *mut c_void,
        w: *mut c_void,
        attrs: *const i32,
    ) -> *mut c_void;
    fn eglMakeCurrent(
        d: *mut c_void,
        draw: *mut c_void,
        read: *mut c_void,
        ctx: *mut c_void,
    ) -> u32;
    fn eglSwapBuffers(d: *mut c_void, s: *mut c_void) -> u32;
    fn eglSwapInterval(d: *mut c_void, i: i32) -> u32;
    fn eglDestroySurface(d: *mut c_void, s: *mut c_void) -> u32;
    fn eglDestroyContext(d: *mut c_void, c: *mut c_void) -> u32;
    fn eglGetError() -> i32;
}

#[link(name = "android")]
extern "C" {
    fn ANativeWindow_setBuffersGeometry(
        w: *mut c_void,
        width: i32,
        height: i32,
        format: i32,
    ) -> i32;
}

pub struct Egl {
    dpy: *mut c_void,
    cfg: *mut c_void,
    ctx: *mut c_void,
    surf: *mut c_void,
}

impl Egl {
    pub fn new() -> Result<Egl, String> {
        unsafe {
            let dpy = eglGetDisplay(null_mut());
            if dpy.is_null() || eglInitialize(dpy, null_mut(), null_mut()) == 0 {
                return Err("eglInitialize".into());
            }
            let attrs = [
                0x3040, 4, 0x3033, 4, 0x3024, 8, 0x3023, 8, 0x3022, 8, 0x3021, 8, 0x3038,
            ];
            let mut cfg: *mut c_void = null_mut();
            let mut n = 0;
            if eglChooseConfig(dpy, attrs.as_ptr(), &mut cfg, 1, &mut n) == 0 || n < 1 {
                return Err("eglChooseConfig".into());
            }
            let ctx_attrs = [0x3098, 2, 0x3038];
            let ctx = eglCreateContext(dpy, cfg, null_mut(), ctx_attrs.as_ptr());
            if ctx.is_null() {
                return Err(format!("eglCreateContext {:x}", eglGetError()));
            }
            Ok(Egl {
                dpy,
                cfg,
                ctx,
                surf: null_mut(),
            })
        }
    }

    /// UZ: Oynaga sirt (surface) yaratadi va kontekstni joriy qiladi (oldingi sirt yo'q qilinadi).
    /// EN: Creates a surface for the window and makes the context current (the previous surface is destroyed).
    /// RU: Создаёт поверхность (surface) для окна и делает контекст текущим (прежняя поверхность уничтожается).
    pub fn set_window(&mut self, window: *mut c_void) -> Result<(), String> {
        unsafe {
            self.release_surface();
            if window.is_null() {
                return Ok(());
            }
            let mut fmt = 0;
            eglGetConfigAttrib(self.dpy, self.cfg, 0x302E, &mut fmt);
            ANativeWindow_setBuffersGeometry(window, 0, 0, fmt);
            self.surf = eglCreateWindowSurface(self.dpy, self.cfg, window, std::ptr::null());
            if self.surf.is_null() {
                return Err(format!("eglCreateWindowSurface {:x}", eglGetError()));
            }
            if eglMakeCurrent(self.dpy, self.surf, self.surf, self.ctx) == 0 {
                return Err(format!("eglMakeCurrent {:x}", eglGetError()));
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

    /// UZ: Kadrni ko'rsatadi. `false` — kontekst yo'qolgan (qayta yaratish kerak).
    /// EN: Presents the frame. `false` means the context was lost (it must be recreated).
    /// RU: Показывает кадр. `false` — контекст потерян (его нужно создать заново).
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
