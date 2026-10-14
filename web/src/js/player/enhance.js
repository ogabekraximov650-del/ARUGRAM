// "Tasvirni yaxshilash" — WebGL (ilovadagi ExoPlayer effekti bilan BIR XIL shader: `enhance-shader.js`).
// <video> ustiga canvas qo'yiladi. Ikki o'tish:
//   1) kattalashtirish (manba ekrandan kichik bo'lsa): Catmull-Rom bikubik — bilinear'ga qaraganda ancha aniq, "zinapoyasiz";
//   2) yaxshilash shaderi: deband/denoise, anti-ringing li o'tkirlik, chiziqlar, ravshanlik, rang.
// WebGL ishlamasa `start()` false qaytaradi — chaqiruvchi CSS filtriga o'tadi.
import { ENHANCE_FRAG } from './enhance-shader.js';

const VERT = 'attribute vec2 p; varying vec2 vTexSamplingCoord; void main(){ vTexSamplingCoord = p * 0.5 + 0.5; gl_Position = vec4(p, 0.0, 1.0); }';
// Catmull-Rom (9 ta o'qish, bilinear hiylasi bilan) — kattalashtirish o'tishi.
const UP_FRAG = `
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
varying vec2 vTexSamplingCoord;
uniform sampler2D uTexSampler;
uniform vec2 uSrcSize;
vec4 tex(vec2 p) { return texture2D(uTexSampler, p); }
void main() {
  vec2 uv = vTexSamplingCoord;
  vec2 pos = uv * uSrcSize;
  vec2 c = floor(pos - 0.5) + 0.5;
  vec2 f = pos - c;
  vec2 w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
  vec2 w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
  vec2 w2 = f * (0.5 + f * (2.0 - 1.5 * f));
  vec2 w3 = f * f * (-0.5 + 0.5 * f);
  vec2 w12 = w1 + w2;
  vec2 o12 = w2 / w12;
  vec2 p0 = (c - 1.0) / uSrcSize;
  vec2 p3 = (c + 2.0) / uSrcSize;
  vec2 p12 = (c + o12) / uSrcSize;
  vec4 r = tex(vec2(p0.x, p0.y)) * w0.x * w0.y
         + tex(vec2(p12.x, p0.y)) * w12.x * w0.y
         + tex(vec2(p3.x, p0.y)) * w3.x * w0.y
         + tex(vec2(p0.x, p12.y)) * w0.x * w12.y
         + tex(vec2(p12.x, p12.y)) * w12.x * w12.y
         + tex(vec2(p3.x, p12.y)) * w3.x * w12.y
         + tex(vec2(p0.x, p3.y)) * w0.x * w3.y
         + tex(vec2(p12.x, p3.y)) * w12.x * w3.y
         + tex(vec2(p3.x, p3.y)) * w3.x * w3.y;
  gl_FragColor = vec4(clamp(r.rgb, 0.0, 1.0), 1.0);
}`;
// sharp, sat, con, deband, line, clarity, vib — ilovadagi AruEnhanceShaderProgram bilan bir xil
const LEVELS = {
  1: [0.45, 1.18, 1.07, 0.5, 0.0, 0.0, 0.0],
  2: [0.8, 1.35, 1.14, 0.8, 0.6, 0.0, 0.0],
  3: [1.0, 1.32, 1.16, 0.9, 0.8, 0.35, 0.55], // Maksimal
};
// Daraja bo'yicha chiqish o'lchami chegarasi (qurilma ko'tarmasa avtomatik pasayadi).
const CAP = { 1: [1280, 720], 2: [1920, 1080], 3: [2560, 1440] };
const MIN_SCALE = 0.75; // bundan past o'lchamda "piksellanish" bo'ladi — shunda effekt o'chadi

export function createEnhancer(video, box) {
  let canvas = null;
  let gl = null;
  let progE = null; // yaxshilash
  let progU = null; // kattalashtirish
  let texV = null; // video kadri
  let texF = null; // kattalashtirilgan kadr (FBO)
  let fbo = null;
  let fboW = 0;
  let fboH = 0;
  let locE = null;
  let locU = null;
  let level = 0;
  let broken = false;
  let active = false;
  let raf = 0;
  let vfc = 0;
  let seed = 0;
  let scale = 1; // 1 -> 0.9 -> 0.75 (kadr tashlansa avtomatik pasayadi)
  let guardT = 0;
  let lastDropped = 0;
  let lastTotal = 0;
  let guardHits = 0;
  let evs = null;
  let buf = null;

  function mkProg(gl2, vs, fs) {
    const sh = (type, src) => {
      const o = gl2.createShader(type);
      gl2.shaderSource(o, src); gl2.compileShader(o);
      if (!gl2.getShaderParameter(o, gl2.COMPILE_STATUS)) throw new Error(gl2.getShaderInfoLog(o) || 'shader');
      return o;
    };
    const p = gl2.createProgram();
    gl2.attachShader(p, sh(gl2.VERTEX_SHADER, vs));
    gl2.attachShader(p, sh(gl2.FRAGMENT_SHADER, fs));
    gl2.linkProgram(p);
    if (!gl2.getProgramParameter(p, gl2.LINK_STATUS)) throw new Error('link');
    return p;
  }

  function mkTex() {
    const t = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, t);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    return t;
  }

  function init() {
    if (gl || broken) return !broken;
    try {
      canvas = document.createElement('canvas');
      canvas.className = 'pl-enh';
      canvas.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;object-fit:contain;pointer-events:none;display:none;background:#000';
      gl = canvas.getContext('webgl', { alpha: false, antialias: false, depth: false, stencil: false, powerPreference: 'high-performance' });
      if (!gl) throw new Error('webgl yo\'q');
      progE = mkProg(gl, VERT, ENHANCE_FRAG);
      progU = mkProg(gl, VERT, UP_FRAG);
      buf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
      texV = mkTex();
      texF = mkTex();
      fbo = gl.createFramebuffer();
      const u = (p, n) => gl.getUniformLocation(p, n);
      locE = { tex: u(progE, 'uTexSampler'), texel: u(progE, 'uTexel'), sharp: u(progE, 'uSharp'), sat: u(progE, 'uSat'), con: u(progE, 'uCon'), deband: u(progE, 'uDeband'), line: u(progE, 'uLine'), seed: u(progE, 'uSeed'), clarity: u(progE, 'uClarity'), vib: u(progE, 'uVib') };
      locU = { tex: u(progU, 'uTexSampler'), size: u(progU, 'uSrcSize') };
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
      video.insertAdjacentElement('afterend', canvas);
      canvas.addEventListener('webglcontextlost', (e) => { e.preventDefault(); fail(); });
      return true;
    } catch (_) {
      fail();
      return false;
    }
  }

  function fail() {
    broken = true;
    stop();
    try { canvas?.remove(); } catch (_) { /* */ }
    canvas = null; gl = null;
  }

  function fit() {
    const vw = video.videoWidth; const vh = video.videoHeight;
    if (!vw || !vh) return false;
    const r = box.getBoundingClientRect();
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const k = Math.min((r.width || vw) / vw, (r.height || vh) / vh) * dpr;
    let w = Math.max(2, Math.round(vw * k)); let h = Math.max(2, Math.round(vh * k));
    const [mw, mh] = CAP[level] || CAP[1];
    const cap = Math.min(mw / w, mh / h, 1) * scale;
    w = Math.max(2, Math.round(w * cap)); h = Math.max(2, Math.round(h * cap));
    if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
    return true;
  }

  function bindQuad(prog) {
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    const a = gl.getAttribLocation(prog, 'p');
    gl.enableVertexAttribArray(a);
    gl.vertexAttribPointer(a, 2, gl.FLOAT, false, 0, 0);
  }

  function draw() {
    if (!active || !gl || video.readyState < 2) return;
    try {
      if (!fit()) return;
      const vw = video.videoWidth; const vh = video.videoHeight;
      const W = canvas.width; const H = canvas.height;
      const p = LEVELS[level] || LEVELS[1];
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, texV);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, video);
      let srcTex = texV;
      // 1-o'tish: ekran manbadan sezilarli katta bo'lsa Catmull-Rom bilan kattalashtiramiz.
      if (W > vw * 1.08 || H > vh * 1.08) {
        if (fboW !== W || fboH !== H) {
          gl.bindTexture(gl.TEXTURE_2D, texF);
          gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, W, H, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
          gl.bindFramebuffer(gl.FRAMEBUFFER, fbo);
          gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, texF, 0);
          fboW = W; fboH = H;
        }
        gl.bindFramebuffer(gl.FRAMEBUFFER, fbo);
        gl.viewport(0, 0, W, H);
        gl.useProgram(progU);
        bindQuad(progU);
        gl.bindTexture(gl.TEXTURE_2D, texV);
        gl.uniform1i(locU.tex, 0);
        gl.uniform2f(locU.size, vw, vh);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
        gl.bindFramebuffer(gl.FRAMEBUFFER, null);
        srcTex = texF;
      }
      // 2-o'tish: yaxshilash shaderi (ekranga).
      gl.viewport(0, 0, W, H);
      gl.useProgram(progE);
      bindQuad(progE);
      gl.bindTexture(gl.TEXTURE_2D, srcTex);
      gl.uniform1i(locE.tex, 0);
      gl.uniform2f(locE.texel, 1 / W, 1 / H);
      gl.uniform1f(locE.sharp, p[0]); gl.uniform1f(locE.sat, p[1]); gl.uniform1f(locE.con, p[2]);
      gl.uniform1f(locE.deband, p[3]); gl.uniform1f(locE.line, p[4]);
      gl.uniform1f(locE.clarity, p[5] || 0); gl.uniform1f(locE.vib, p[6] || 0);
      seed = (seed + 0.137) % 97;
      gl.uniform1f(locE.seed, seed);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    } catch (_) {
      // masalan, tarmoqdan kelgan video (CORS) — texImage2D taqiqlanadi: CSS filtriga qaytiladi
      fail();
      try { window.dispatchEvent(new Event('aru-enhance-failed')); } catch (_e) { /* */ }
    }
  }

  function schedule() {
    if (!active) return;
    if (typeof video.requestVideoFrameCallback === 'function') {
      vfc = video.requestVideoFrameCallback(() => { vfc = 0; draw(); schedule(); });
    } else {
      raf = requestAnimationFrame(() => { raf = 0; if (!video.paused) draw(); schedule(); });
    }
  }

  // Himoya: kadr tashlansa chiqish o'lchami pasayadi (1 -> 0.9 -> 0.75); undan ham pastda piksellanadi — effekt o'chadi.
  function guard() {
    if (!active || !video.getVideoPlaybackQuality) return;
    const q = video.getVideoPlaybackQuality();
    const dd = q.droppedVideoFrames - lastDropped; const dt = q.totalVideoFrames - lastTotal;
    lastDropped = q.droppedVideoFrames; lastTotal = q.totalVideoFrames;
    if (video.paused || dt < 20) return;
    if (dd / dt > 0.1) {
      guardHits += 1;
      if (scale > MIN_SCALE) { scale = scale > 0.9 ? 0.9 : MIN_SCALE; guardHits = 0; } else if (guardHits >= 2) {
        fail();
        try { window.dispatchEvent(new Event('aru-enhance-failed')); } catch (_) { /* */ }
      }
    } else guardHits = 0;
  }

  const once = () => requestAnimationFrame(draw);

  function start(lv) {
    if (!init()) return false;
    level = lv >= 3 ? 3 : lv === 2 ? 2 : 1;
    if (!active) {
      active = true;
      canvas.style.display = 'block';
      evs = ['seeked', 'loadeddata', 'pause', 'playing', 'resize', 'emptied'];
      evs.forEach((n) => video.addEventListener(n, once));
      window.addEventListener('resize', once);
      scale = 1; guardHits = 0;
      const q0 = video.getVideoPlaybackQuality?.(); lastDropped = q0?.droppedVideoFrames || 0; lastTotal = q0?.totalVideoFrames || 0;
      guardT = setInterval(guard, 2500);
      schedule();
    } else { scale = 1; guardHits = 0; }
    once();
    return true;
  }

  function stop() {
    active = false;
    if (vfc && typeof video.cancelVideoFrameCallback === 'function') { try { video.cancelVideoFrameCallback(vfc); } catch (_) { /* */ } }
    if (raf) cancelAnimationFrame(raf);
    vfc = 0; raf = 0;
    if (evs) { evs.forEach((n) => video.removeEventListener(n, once)); evs = null; }
    window.removeEventListener('resize', once);
    clearInterval(guardT); guardT = 0;
    if (canvas) canvas.style.display = 'none';
  }

  function destroy() {
    stop();
    try { canvas?.remove(); } catch (_) { /* */ }
    try { gl?.getExtension('WEBGL_lose_context')?.loseContext(); } catch (_) { /* */ }
    canvas = null; gl = null;
  }

  return { start, stop, destroy, get broken() { return broken; } };
}
