package uz.arugram.app.ui.intro

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.PorterDuff
import android.graphics.PorterDuffColorFilter
import android.graphics.PorterDuffXfermode
import android.graphics.SurfaceTexture
import android.opengl.GLES20
import android.opengl.GLUtils
import android.os.Handler
import android.os.HandlerThread
import android.util.Log
import android.view.TextureView
import android.view.WindowManager
import uz.arugram.app.core.RustCore
import javax.microedition.khronos.egl.EGL10
import javax.microedition.khronos.egl.EGLConfig
import javax.microedition.khronos.egl.EGLContext
import javax.microedition.khronos.egl.EGLDisplay
import javax.microedition.khronos.egl.EGLSurface
import kotlin.math.min

/**
 * Telegram kirish oynasining 3D sahnasi (200x150 dp). DrKLO `IntroActivity.EGLThread` ning Kotlin'dagi nusxasi: EGL kontekst, teksturalarni
 * yuklash va har kadrda Rust sahnasini (`rust/src/intro_gl.rs`) chaqirish. Telegram kabi `TextureView` — Compose ichida alfa/o'tishlar ishlaydi.
 * Biror narsa ishlamasa `onFailed` chaqiriladi va Compose'dagi oddiy (Canvas) rasmga o'tiladi.
 *
 * Telegram for Android: Copyright (c) Nikolai Kudashov va hissa qo'shuvchilar, GPLv2+ (LICENSE, NOTICE).
 */
class IntroGlView(context: Context) : TextureView(context), TextureView.SurfaceTextureListener {
    @Volatile var page = 0
    @Volatile var scroll = 0f
    var onFailed: (() -> Unit)? = null

    private var bg: Int = Color.WHITE
    private var glThread: HandlerThread? = null
    private var glHandler: Handler? = null
    private var runner: Runner? = null

    init {
        surfaceTextureListener = this
    }

    /** Mavzu rangi (Telegram'dagi `windowBackgroundWhite`): fon va niqob teksturalari shu rangda. */
    fun setBackgroundArgb(color: Int) {
        if (color == bg) return
        bg = color
        val r = runner ?: return
        glHandler?.post { r.updateBackground(color) }
    }

    override fun onSurfaceTextureAvailable(surface: SurfaceTexture, width: Int, height: Int) {
        val t = HandlerThread("IntroGL").also { it.start() }
        val h = Handler(t.looper)
        val r = Runner(context.applicationContext, surface, this, bg, h)
        glThread = t
        glHandler = h
        runner = r
        h.post { r.start(width, height) }
    }

    override fun onSurfaceTextureSizeChanged(surface: SurfaceTexture, width: Int, height: Int) {
        val r = runner ?: return
        glHandler?.post { r.resize(width, height) }
    }

    override fun onSurfaceTextureDestroyed(surface: SurfaceTexture): Boolean {
        val r = runner
        val t = glThread
        runner = null
        glHandler?.post {
            r?.stop()
            t?.quit()
        }
        glHandler = null
        glThread = null
        return true
    }

    override fun onSurfaceTextureUpdated(surface: SurfaceTexture) {}

    private class Runner(
        private val ctx: Context,
        private val surface: SurfaceTexture,
        private val view: IntroGlView,
        private var bg: Int,
        private val handler: Handler,
    ) {
        private val lib get() = RustCore.intro
        private val tex = IntArray(24)
        private lateinit var egl: EGL10
        private var display: EGLDisplay? = null
        private var eglContext: EGLContext? = null
        private var eglSurface: EGLSurface? = null
        @Volatile private var alive = false
        private var currentDate = 0L
        private var lastDraw = 0L
        private var refresh = 60f

        fun start(w: Int, h: Int) {
            val ok = try {
                initGl() && initScene(w, h)
            } catch (t: Throwable) {
                Log.e(TAG, "intro GL xatosi: $t")
                false
            }
            if (!ok) {
                stop()
                view.post { view.onFailed?.invoke() }
                return
            }
            alive = true
            currentDate = System.currentTimeMillis() - 1000
            try {
                val wm = ctx.getSystemService(Context.WINDOW_SERVICE) as WindowManager
                @Suppress("DEPRECATION")
                refresh = wm.defaultDisplay.supportedRefreshRates.maxOrNull() ?: 60f
            } catch (_: Throwable) {
            }
            handler.post(draw)
        }

        private fun initGl(): Boolean {
            egl = EGLContext.getEGL() as EGL10
            val d = egl.eglGetDisplay(EGL10.EGL_DEFAULT_DISPLAY)
            if (d == EGL10.EGL_NO_DISPLAY) return false
            display = d
            if (!egl.eglInitialize(d, IntArray(2))) return false
            var cfg: EGLConfig? = null
            for (depth in intArrayOf(24, 16)) {
                val spec = intArrayOf(
                    EGL10.EGL_RENDERABLE_TYPE, 4,
                    EGL10.EGL_RED_SIZE, 8, EGL10.EGL_GREEN_SIZE, 8, EGL10.EGL_BLUE_SIZE, 8, EGL10.EGL_ALPHA_SIZE, 8,
                    EGL10.EGL_DEPTH_SIZE, depth, EGL10.EGL_STENCIL_SIZE, 0,
                    EGL10.EGL_NONE,
                )
                val configs = arrayOfNulls<EGLConfig>(1)
                val n = IntArray(1)
                if (egl.eglChooseConfig(d, spec, configs, 1, n) && n[0] > 0) {
                    cfg = configs[0]
                    break
                }
            }
            cfg ?: return false
            eglContext = egl.eglCreateContext(d, cfg, EGL10.EGL_NO_CONTEXT, intArrayOf(0x3098, 2, EGL10.EGL_NONE)) ?: return false
            val s = egl.eglCreateWindowSurface(d, cfg, surface, null)
            if (s == null || s == EGL10.EGL_NO_SURFACE) return false
            eglSurface = s
            return egl.eglMakeCurrent(d, s, s, eglContext)
        }

        private fun initScene(w: Int, h: Int): Boolean {
            val rust = lib ?: return false
            GLES20.glGenTextures(24, tex, 0)
            val names = arrayOf(
                "fast_arrow_shadow", "fast_arrow", "fast_body", "fast_spiral", "ic_bubble_dot", "ic_bubble", "ic_cam_lens", "ic_cam", "ic_pencil",
                "ic_pin", "ic_smile_eye", "ic_smile", "ic_videocam", "knot_down", "knot_up", "powerful_infinity_white", "powerful_infinity", null,
                "powerful_star", "private_door", "private_screw", "tg_plane",
            )
            for (i in names.indices) {
                val n = names[i] ?: continue
                upload(i, asset(n))
            }
            uploadMasks()
            rust.rust_intro_set_textures(tex, 24)
            rust.rust_intro_set_background(Color.red(bg) / 255f, Color.green(bg) / 255f, Color.blue(bg) / 255f, Color.alpha(bg) / 255f)
            rust.rust_intro_surface_changed(w, h, min(w / 150f, h / 150f), 0)
            return rust.rust_intro_surface_created() == 1
        }

        private fun asset(name: String): Bitmap = ctx.assets.open("intro/$name.webp").use { BitmapFactory.decodeStream(it) }

        private fun upload(index: Int, bm: Bitmap) {
            GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, tex[index])
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR)
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR)
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_S, GLES20.GL_CLAMP_TO_EDGE)
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_T, GLES20.GL_CLAMP_TO_EDGE)
            GLUtils.texImage2D(GLES20.GL_TEXTURE_2D, 0, bm, 0)
            bm.recycle()
        }

        /** 17 — mavzu rangiga bo'yalgan niqob; 22 — ko'k doira (mavzuga bog'liq emas); 23 — fon rangidagi niqob, o'rtasida doira teshigi. */
        private fun uploadMasks() {
            val density = ctx.resources.displayMetrics.density
            val src = asset("powerful_mask")
            val tinted = Bitmap.createBitmap(src.width, src.height, Bitmap.Config.ARGB_8888)
            val p = Paint(Paint.ANTI_ALIAS_FLAG or Paint.DITHER_FLAG)
            p.colorFilter = PorterDuffColorFilter(bg, PorterDuff.Mode.SRC_IN)
            Canvas(tinted).drawBitmap(src, 0f, 0f, p)
            src.recycle()
            upload(17, tinted)

            val size = (150 * density).toInt().coerceAtLeast(2)
            val sphere = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
            val sp = Paint(Paint.ANTI_ALIAS_FLAG)
            sp.color = 0xFF229AF0.toInt()
            Canvas(sphere).drawCircle(size / 2f, size / 2f, size / 2f, sp)
            upload(22, sphere)

            val mask = Bitmap.createBitmap((200 * density).toInt().coerceAtLeast(2), size, Bitmap.Config.ARGB_8888)
            val c = Canvas(mask)
            c.drawColor(bg)
            val mp = Paint(Paint.ANTI_ALIAS_FLAG)
            mp.xfermode = PorterDuffXfermode(PorterDuff.Mode.CLEAR)
            c.drawCircle(mask.width / 2f, mask.height / 2f, size / 2f, mp)
            upload(23, mask)
        }

        fun updateBackground(color: Int) {
            if (!alive) return
            bg = color
            uploadMasks()
            lib?.rust_intro_set_background(Color.red(color) / 255f, Color.green(color) / 255f, Color.blue(color) / 255f, Color.alpha(color) / 255f)
        }

        fun resize(w: Int, h: Int) {
            if (!alive) return
            lib?.rust_intro_surface_changed(w, h, min(w / 150f, h / 150f), 0)
        }

        private val draw = object : Runnable {
            override fun run() {
                if (!alive) return
                val rust = lib ?: return
                val now = System.currentTimeMillis()
                val delta = min(now - lastDraw, 16L).toInt()
                rust.rust_intro_set_page(view.page)
                rust.rust_intro_set_scroll_offset(view.scroll)
                rust.rust_intro_set_date((now - currentDate) / 1000f)
                rust.rust_intro_draw_frame(delta)
                egl.eglSwapBuffers(display, eglSurface)
                lastDraw = now
                val spent = System.currentTimeMillis() - now
                handler.postDelayed(this, maxOf((1000f / refresh).toLong() - spent, 0L))
            }
        }

        fun stop() {
            alive = false
            try {
                lib?.rust_intro_destroy()
            } catch (_: Throwable) {
            }
            val d = display
            if (d != null) {
                egl.eglMakeCurrent(d, EGL10.EGL_NO_SURFACE, EGL10.EGL_NO_SURFACE, EGL10.EGL_NO_CONTEXT)
                eglSurface?.let { egl.eglDestroySurface(d, it) }
                eglContext?.let { egl.eglDestroyContext(d, it) }
                egl.eglTerminate(d)
            }
            eglSurface = null
            eglContext = null
            display = null
        }
    }

    private companion object {
        const val TAG = "IntroGL"
    }
}
