// ARUGRAM "Tasvirni yaxshilash" — bitta o'tishli (single-pass) anime uchun fragment shader (GLSL ES 1.00).
// Ilova (Media3 GlShaderProgram) va web (WebGL) AYNAN SHU matndan foydalanadi (web/src/js/player/enhance-shader.js nusxasi).
//
// Bosqichlar:
//   1) Deband/denoise — tekis joylarda (past bitreytdagi gradient zinapoyalari, "chivin" shovqini) yumshatish + dither.
//   2) Kontrast-moslashuvchan o'tkirlashtirish (CAS g'oyasi, AMD FidelityFX CAS, MIT) — faqat chegara joylarda, tekis joyda kamaytirilgan.
//   3) Chiziqlarni qoraytirish (anime konturlari yupqa va aniq ko'rinsin).
//   4) Rang: to'yinganlik va kontrast.
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif

varying vec2 vTexSamplingCoord;
uniform sampler2D uTexSampler;
uniform vec2 uTexel;     // 1/eni, 1/bo'yi
uniform float uSharp;    // 0..1 o'tkirlik
uniform float uSat;      // 1.0 — o'zgarishsiz
uniform float uCon;      // 1.0 — o'zgarishsiz
uniform float uDeband;   // 0..1
uniform float uLine;     // 0..1
uniform float uSeed;     // kadrga qarab o'zgaradigan son (dither uchun)
uniform float uClarity;  // 0..1 — keng radiusli mahalliy kontrast ("ravshanlik")
uniform float uVib;      // 0..1 — vibrance: kam to'yingan ranglar ko'proq kuchayadi
uniform float uTone;     // 0..1 — ton egri chizig'i (S-egri: chuqur qora, toza yorug'lik; faqat ALU, qo'shimcha o'qish yo'q)

float luma(vec3 c) {
  return dot(c, vec3(0.2126, 0.7152, 0.0722));
}

float hash12(vec2 p) {
  vec3 p3 = fract(vec3(p.xyx) * 0.1031);
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.x + p3.y) * p3.z);
}

void main() {
  vec2 uv = vTexSamplingCoord;
  vec2 t = uTexel;

  vec3 e = texture2D(uTexSampler, uv).rgb;
  vec3 b = texture2D(uTexSampler, uv + vec2(0.0, -t.y)).rgb;
  vec3 d = texture2D(uTexSampler, uv + vec2(-t.x, 0.0)).rgb;
  vec3 f = texture2D(uTexSampler, uv + vec2(t.x, 0.0)).rgb;
  vec3 h = texture2D(uTexSampler, uv + vec2(0.0, t.y)).rgb;

  float le = luma(e);
  float lmin = min(min(min(luma(b), luma(d)), min(luma(f), luma(h))), le);
  float lmax = max(max(max(luma(b), luma(d)), max(luma(f), luma(h))), le);
  float contrast = lmax - lmin;

  // 1) Deband / denoise: faqat tekis joyda (mahalliy kontrast kichik).
  vec3 col = e;
  if (uDeband > 0.0) {
    // Diagonal qo'shnilar faqat deband yoqilganda o'qiladi (kuchsiz telefonlarda yengil rejim 5 ta o'qish bilan ishlaydi).
    vec3 a = texture2D(uTexSampler, uv + vec2(-t.x, -t.y)).rgb;
    vec3 c = texture2D(uTexSampler, uv + vec2(t.x, -t.y)).rgb;
    vec3 g = texture2D(uTexSampler, uv + vec2(-t.x, t.y)).rgb;
    vec3 i = texture2D(uTexSampler, uv + vec2(t.x, t.y)).rgb;
    vec3 far0 = texture2D(uTexSampler, uv + vec2(-2.0 * t.x, 0.0)).rgb;
    vec3 far1 = texture2D(uTexSampler, uv + vec2(2.0 * t.x, 0.0)).rgb;
    vec3 far2 = texture2D(uTexSampler, uv + vec2(0.0, -2.0 * t.y)).rgb;
    vec3 far3 = texture2D(uTexSampler, uv + vec2(0.0, 2.0 * t.y)).rgb;
    vec3 avg = (a + b + c + d + e + f + g + h + i + far0 + far1 + far2 + far3) / 13.0;
    float spread = max(contrast, max(abs(luma(far0) - le), max(abs(luma(far1) - le),
                   max(abs(luma(far2) - le), abs(luma(far3) - le)))));
    float flatAmt = 1.0 - smoothstep(0.015, 0.07, spread);
    col = mix(e, avg, flatAmt * uDeband);
    // uchburchakli dither: zinapoyalar ko'rinmasin
    float n = hash12(gl_FragCoord.xy + uSeed) + hash12(gl_FragCoord.yx * 1.37 + uSeed * 1.7) - 1.0;
    col += n * (1.2 / 255.0) * flatAmt * uDeband;
  }

  // 2) CAS tipidagi o'tkirlashtirish (chegaralarda), tekis joyda kuchsiz.
  if (uSharp > 0.0) {
    vec3 mn = min(min(min(d, e), min(f, b)), h);
    vec3 mx = max(max(max(d, e), max(f, b)), h);
    vec3 amp = sqrt(clamp(min(mn, 1.0 - mx) / max(mx, vec3(0.0001)), 0.0, 1.0));
    float peak = -1.0 / mix(8.0, 5.0, clamp(uSharp, 0.0, 1.0));
    vec3 w = amp * peak;
    vec3 rcp = 1.0 / (1.0 + 4.0 * w);
    vec3 sharp = (w * (b + d + f + h) + col) * rcp;
    // Anti-ringing: natija mahalliy min/max oralig'idan chiqmasin (galo va "piksellanish" bo'lmasin).
    sharp = clamp(sharp, mn - 0.01, mx + 0.01);
    float edge = smoothstep(0.03, 0.16, contrast);
    col = mix(col, sharp, edge * 0.9 + 0.1);
  }

  // 3) Chiziqlarni qoraytirish: atrofidan qoraroq piksel (kontur) biroz qoraytiriladi.
  if (uLine > 0.0) {
    float around = (luma(b) + luma(d) + luma(f) + luma(h)) * 0.25;
    float dark = clamp((around - luma(col)) * 1.4, 0.0, 0.14);
    col *= 1.0 - dark * uLine;
  }

  // 3b) Ravshanlik: keng radiusli (3 piksel) mahalliy o'rtachadan farqni kuchaytirish — tekstura/shakllar "jonlanadi".
  if (uClarity > 0.0) {
    vec3 m0 = texture2D(uTexSampler, uv + vec2(-3.0 * t.x, -3.0 * t.y)).rgb;
    vec3 m1 = texture2D(uTexSampler, uv + vec2(3.0 * t.x, -3.0 * t.y)).rgb;
    vec3 m2 = texture2D(uTexSampler, uv + vec2(-3.0 * t.x, 3.0 * t.y)).rgb;
    vec3 m3 = texture2D(uTexSampler, uv + vec2(3.0 * t.x, 3.0 * t.y)).rgb;
    vec3 mean = (m0 + m1 + m2 + m3 + col * 2.0) / 6.0;
    float mid = 1.0 - abs(luma(col) - 0.5) * 1.4;   // o'rta tonlarda kuchliroq, qora/oqda kuchsiz
    col += (col - mean) * uClarity * clamp(mid, 0.2, 1.0);
  }

  // 4) Rang: vibrance, to'yinganlik va kontrast.
  float l = luma(col);
  if (uVib > 0.0) {
    float satNow = max(col.r, max(col.g, col.b)) - min(col.r, min(col.g, col.b));
    col = mix(vec3(l), col, 1.0 + uVib * (1.0 - clamp(satNow * 1.6, 0.0, 1.0)));
    l = luma(col);
  }
  col = mix(vec3(l), col, uSat);
  col = (col - 0.5) * uCon + 0.5;

  // 5) "Anime HD" ton egri chizig'i: yorqinlikka S-egri (smoothstep) qo'llab, ranglar nisbatini saqlaymiz — qora chuqurroq,
  //    yorug' joylar toza, tasvir "kinematografik" ko'rinadi. Faqat hisob-kitob: kuchsiz telefonda ham deyarli bepul.
  if (uTone > 0.0) {
    float lt = clamp(luma(col), 0.0, 1.0);
    float st = lt * lt * (3.0 - 2.0 * lt);
    col *= clamp(mix(lt, st, uTone) / max(lt, 0.004), 0.0, 1.6);
  }

  gl_FragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
