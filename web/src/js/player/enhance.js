// "Tasvirni yaxshilash" — WebGL (ilovadagi ExoPlayer effekti bilan BIR XIL shader: `enhance-shader.js`).
// <video> ustiga canvas qo'yiladi: har kadr shaderdan o'tadi (deband/denoise, o'tkirlik, chiziqlar, rang) va ekran
// o'lchamigacha (1920 gacha) kattalashtiriladi. WebGL ishlamasa `start()` false qaytaradi — chaqiruvchi CSS filtriga o'tadi.
import { ENHANCE_FRAG } from './enhance-shader.js';

const VERT = 'attribute vec2 p; varying vec2 vTexSamplingCoord; void main(){ vTexSamplingCoord = p * 0.5 + 0.5; gl_Position = vec4(p, 0.0, 1.0); }';
// sharp, sat, con, deband, line — ilovadagi AruEnhanceShaderProgram bilan bir xil
const LEVELS = { 1: [0.35, 1.12, 1.04, 0.5, 0.0], 2: [0.65, 1.25, 1.09, 0.8, 0.5] };
const MAX_W = 1920;
const MAX_H = 1080;

export function createEnhancer(video, box) {
  let canvas = null;
  let gl = null;
  let prog = null;
  let tex = null;
  let loc = null;
  let level = 0;
  let broken = false;
  let active = false;
  let raf = 0;
  let vfc = 0;
  let seed = 0;
  let evs = null;

  function init() {
    if (gl || broken) return !broken;
    try {
      canvas = document.createElement('canvas');
      canvas.className = 'pl-enh';
      canvas.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;object-fit:contain;pointer-events:none;display:none;background:#000';
      gl = canvas.getContext('webgl', { alpha: false, antialias: false, depth: false, stencil: false, powerPreference: 'low-power' });
      if (!gl) throw new Error('webgl yo\'q');
      const sh = (type, src) => {
        const o = gl.createShader(type);
        gl.shaderSource(o, src); gl.compileShader(o);
        if (!gl.getShaderParameter(o, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(o) || 'shader');
        return o;
      };
      prog = gl.createProgram();
      gl.attachShader(prog, sh(gl.VERTEX_SHADER, VERT));
      gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, ENHANCE_FRAG));
      gl.linkProgram(prog);
      if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error('link');
      gl.useProgram(prog);
      const buf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
      const a = gl.getAttribLocation(prog, 'p');
      gl.enableVertexAttribArray(a);
      gl.vertexAttribPointer(a, 2, gl.FLOAT, false, 0, 0);
      tex = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
      const u = (n) => gl.getUniformLocation(prog, n);
      loc = { tex: u('uTexSampler'), texel: u('uTexel'), sharp: u('uSharp'), sat: u('uSat'), con: u('uCon'), deband: u('uDeband'), line: u('uLine'), seed: u('uSeed') };
      gl.uniform1i(loc.tex, 0);
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
    const cap = Math.min(MAX_W / w, MAX_H / h, 1);
    w = Math.max(2, Math.round(w * cap)); h = Math.max(2, Math.round(h * cap));
    if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
    return true;
  }

  function draw() {
    if (!active || !gl || video.readyState < 2) return;
    try {
      if (!fit()) return;
      const p = LEVELS[level] || LEVELS[1];
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, video);
      gl.uniform2f(loc.texel, 1 / canvas.width, 1 / canvas.height);
      gl.uniform1f(loc.sharp, p[0]); gl.uniform1f(loc.sat, p[1]); gl.uniform1f(loc.con, p[2]);
      gl.uniform1f(loc.deband, p[3]); gl.uniform1f(loc.line, p[4]);
      seed = (seed + 0.137) % 97;
      gl.uniform1f(loc.seed, seed);
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

  const once = () => requestAnimationFrame(draw);

  function start(lv) {
    if (!init()) return false;
    level = lv === 2 ? 2 : 1;
    if (!active) {
      active = true;
      canvas.style.display = 'block';
      evs = ['seeked', 'loadeddata', 'pause', 'playing', 'resize', 'emptied'];
      evs.forEach((n) => video.addEventListener(n, once));
      window.addEventListener('resize', once);
      schedule();
    }
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
