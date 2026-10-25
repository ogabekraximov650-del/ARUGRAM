/* UZ: Telegram tanishtiruv chizuvchisini (IntroRenderer.c, GPL) Android'siz, EGL (Mesa, ekransiz) bilan kadrma-kadr ishga tushiradi; kadrlar stdout'ga xom RGBA (pastdan yuqoriga).
   EN: Runs the Telegram intro renderer (IntroRenderer.c, GPL) frame by frame without Android, on headless EGL (Mesa); frames go to stdout as raw RGBA (bottom-up).
   RU: Покадрово запускает отрисовщик интро Telegram (IntroRenderer.c, GPL) без Android, на безэкранном EGL (Mesa); кадры уходят в stdout как сырой RGBA (снизу вверх).
   Ishlatish / Usage: intro_gl <tex_dir> <bg_hex> <total_ms> <fps> <t1,t2,..,t5>  (t_i — i-sahifaga o'tish boshlanishi, ms) */
#include <EGL/egl.h>
#include <EGL/eglext.h>
#include <GLES2/gl2.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <jni.h>

#define W 600
#define H 450
void Java_org_telegram_messenger_Intro_onDrawFrame(JNIEnv *e, jclass c, jint d);
void Java_org_telegram_messenger_Intro_setScrollOffset(JNIEnv *e, jclass c, float o);
void Java_org_telegram_messenger_Intro_setPage(JNIEnv *e, jclass c, int32_t p);
void Java_org_telegram_messenger_Intro_setDate(JNIEnv *e, jclass c, float a);
void Java_org_telegram_messenger_Intro_setIcTextures(JNIEnv *e, jclass c, GLuint a, GLuint b, GLuint c1, GLuint d, GLuint e1, GLuint f, GLuint g, GLuint h, GLuint i);
void Java_org_telegram_messenger_Intro_setTelegramTextures(JNIEnv *e, jclass c, GLuint a, GLuint b, GLuint d);
void Java_org_telegram_messenger_Intro_setFastTextures(JNIEnv *e, jclass c, GLuint a, GLuint b, GLuint d, GLuint f);
void Java_org_telegram_messenger_Intro_setFreeTextures(JNIEnv *e, jclass c, GLuint a, GLuint b);
void Java_org_telegram_messenger_Intro_setPowerfulTextures(JNIEnv *e, jclass c, GLuint a, GLuint b, GLuint d, GLuint f);
void Java_org_telegram_messenger_Intro_setPrivateTextures(JNIEnv *e, jclass c, GLuint a, GLuint b);
void Java_org_telegram_messenger_Intro_onSurfaceCreated(JNIEnv *e, jclass c);
void Java_org_telegram_messenger_Intro_onSurfaceChanged(JNIEnv *e, jclass c, int32_t w, int32_t h, float s, int32_t a);
void Java_org_telegram_messenger_Intro_setBackgroundColor(JNIEnv *e, jclass c, float r, float g, float b, float a);

static GLuint tex[24];
static void upload(int idx, int w, int h, const unsigned char *px) {
    glGenTextures(1, &tex[idx]);
    glBindTexture(GL_TEXTURE_2D, tex[idx]);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, w, h, 0, GL_RGBA, GL_UNSIGNED_BYTE, px);
}
/* Android Bitmap straight-alpha emas (premultiplied) — shu sababli tekstura yuklashda ham premultiplied. */
static void load_file(int idx, const char *dir, const char *name, int tint, unsigned bg) {
    char path[512]; snprintf(path, sizeof path, "%s/%s.rgba", dir, name);
    FILE *f = fopen(path, "rb"); if (!f) { fprintf(stderr, "yo'q: %s\n", path); exit(2); }
    unsigned w, h; if (fread(&w, 4, 1, f) != 1 || fread(&h, 4, 1, f) != 1) exit(2);
    unsigned char *px = malloc((size_t)w * h * 4); if (fread(px, 1, (size_t)w * h * 4, f) != (size_t)w * h * 4) exit(2); fclose(f);
    for (size_t i = 0; i < (size_t)w * h; i++) {
        unsigned char *p = px + i * 4;
        if (tint) { p[0] = bg >> 16 & 255; p[1] = bg >> 8 & 255; p[2] = bg & 255; }
        p[0] = p[0] * p[3] / 255; p[1] = p[1] * p[3] / 255; p[2] = p[2] * p[3] / 255;
    }
    upload(idx, w, h, px); free(px);
}
static float cov(float d) { float v = 0.5f - d; return v < 0 ? 0 : v > 1 ? 1 : v; }
static void make_circle(int idx, unsigned color) {  /* ThemeColors.TELEGRAM_COLOR doira, 450x450 */
    int n = 450; unsigned char *px = malloc((size_t)n * n * 4);
    for (int y = 0; y < n; y++) for (int x = 0; x < n; x++) {
        float d = sqrtf((x + .5f - n / 2.f) * (x + .5f - n / 2.f) + (y + .5f - n / 2.f) * (y + .5f - n / 2.f)) - n / 2.f;
        float a = cov(d); unsigned char *p = px + ((size_t)y * n + x) * 4;
        p[0] = (color >> 16 & 255) * a; p[1] = (color >> 8 & 255) * a; p[2] = (color & 255) * a; p[3] = 255 * a;
    }
    upload(idx, n, n, px); free(px);
}
static void make_mask(int idx, unsigned bg) {  /* fon rangi, o'rtasida teshik doira (600x450) */
    unsigned char *px = malloc((size_t)W * H * 4);
    for (int y = 0; y < H; y++) for (int x = 0; x < W; x++) {
        float d = sqrtf((x + .5f - W / 2.f) * (x + .5f - W / 2.f) + (y + .5f - H / 2.f) * (y + .5f - H / 2.f)) - H / 2.f;
        float a = 1.f - cov(d); unsigned char *p = px + ((size_t)y * W + x) * 4;
        p[0] = (bg >> 16 & 255) * a; p[1] = (bg >> 8 & 255) * a; p[2] = (bg & 255) * a; p[3] = 255 * a;
    }
    upload(idx, W, H, px); free(px);
}
static float quint(float t) { t = t < 0 ? 0 : t > 1 ? 1 : t; t -= 1; return t * t * t * t * t + 1; }  /* ViewPager sInterpolator */

int main(int argc, char **argv) {
    if (argc < 6) { fprintf(stderr, "ishlatish: intro_gl tex_dir bg_hex total_ms fps t1,..,t5\n"); return 1; }
    const char *dir = argv[1]; unsigned bg = (unsigned)strtoul(argv[2], 0, 16);
    int total = atoi(argv[3]), fps = atoi(argv[4]); float ts[8] = {0}; int nt = 0;
    for (char *s = strtok(argv[5], ","); s && nt < 6; s = strtok(0, ",")) ts[nt++] = atof(s);
    const float SWIPE_MS = 450;

    EGLDisplay dpy = eglGetPlatformDisplay(EGL_PLATFORM_SURFACELESS_MESA, EGL_DEFAULT_DISPLAY, 0);
    EGLint maj, min; if (!eglInitialize(dpy, &maj, &min)) { fprintf(stderr, "eglInitialize xato\n"); return 3; }
    eglBindAPI(EGL_OPENGL_ES_API);
    EGLint ca[] = {EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT, EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_ALPHA_SIZE, 8, EGL_NONE};
    EGLConfig cfg; EGLint nc; eglChooseConfig(dpy, ca, &cfg, 1, &nc); if (!nc) { fprintf(stderr, "config yo'q\n"); return 3; }
    EGLint pa[] = {EGL_WIDTH, W, EGL_HEIGHT, H, EGL_NONE};
    EGLSurface sf = eglCreatePbufferSurface(dpy, cfg, pa);
    EGLint xa[] = {EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE};
    EGLContext cx = eglCreateContext(dpy, cfg, EGL_NO_CONTEXT, xa);
    if (!eglMakeCurrent(dpy, sf, sf, cx)) { fprintf(stderr, "makeCurrent xato\n"); return 3; }

    static const char *names[] = {"fast_arrow_shadow", "fast_arrow", "fast_body", "fast_spiral", "ic_bubble_dot", "ic_bubble", "ic_cam_lens", "ic_cam", "ic_pencil", "ic_pin",
        "ic_smile_eye", "ic_smile", "ic_videocam", "knot_down", "knot_up", "powerful_infinity_white", "powerful_infinity", "powerful_mask", "powerful_star", "private_door", "private_screw", "tg_plane"};
    for (int i = 0; i < 22; i++) load_file(i, dir, names[i], i == 17, bg);   /* 17: powerful_mask fon rangida bo'yaladi */
    make_circle(22, 0x229AF0); make_mask(23, bg);
    JNIEnv *E = 0; jclass C = 0;
    Java_org_telegram_messenger_Intro_setPrivateTextures(E, C, tex[19], tex[20]);
    Java_org_telegram_messenger_Intro_setFreeTextures(E, C, tex[14], tex[13]);
    Java_org_telegram_messenger_Intro_setFastTextures(E, C, tex[2], tex[3], tex[1], tex[0]);
    Java_org_telegram_messenger_Intro_setIcTextures(E, C, tex[4], tex[5], tex[6], tex[7], tex[8], tex[9], tex[10], tex[11], tex[12]);
    Java_org_telegram_messenger_Intro_setTelegramTextures(E, C, tex[22], tex[21], tex[23]);
    Java_org_telegram_messenger_Intro_setPowerfulTextures(E, C, tex[17], tex[18], tex[16], tex[15]);
    Java_org_telegram_messenger_Intro_setBackgroundColor(E, C, (bg >> 16 & 255) / 255.f, (bg >> 8 & 255) / 255.f, (bg & 255) / 255.f, 1.f);
    Java_org_telegram_messenger_Intro_onSurfaceCreated(E, C);
    Java_org_telegram_messenger_Intro_onSurfaceChanged(E, C, W, H, fminf(W / 150.f, H / 150.f), 0);

    unsigned char *buf = malloc((size_t)W * H * 4);
    int frames = total * fps / 1000;
    for (int i = 0; i < frames; i++) {
        float ms = i * 1000.f / fps; int page = 0; float off = 0;
        for (int k = 0; k < nt; k++) if (ms >= ts[k]) { page = k + 1; off = -(1.f - quint((ms - ts[k]) / SWIPE_MS)); }
        /* ViewPager: sahifa darhol almashadi, siljish -1 → 0 (oldinga). */
        Java_org_telegram_messenger_Intro_setPage(E, C, page);
        Java_org_telegram_messenger_Intro_setScrollOffset(E, C, off);
        Java_org_telegram_messenger_Intro_setDate(E, C, 1.0f + ms / 1000.f);
        Java_org_telegram_messenger_Intro_onDrawFrame(E, C, 16);
        glFinish(); glReadPixels(0, 0, W, H, GL_RGBA, GL_UNSIGNED_BYTE, buf);
        if (fwrite(buf, 1, (size_t)W * H * 4, stdout) != (size_t)W * H * 4) return 4;
    }
    return 0;
}
