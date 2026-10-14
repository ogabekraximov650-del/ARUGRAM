// `packages/video_player_android/android/src/main/assets/aru_enhance_fragment.glsl` -> `web/src/js/player/enhance-shader.js`
// (ilova va web BIR XIL shaderdan foydalanadi). Shader o'zgarsa: `node tool/shaders/sync-web.mjs`.
import { readFileSync, writeFileSync } from 'node:fs';
const src = readFileSync('packages/video_player_android/android/src/main/assets/aru_enhance_fragment.glsl', 'utf8');
const out = `// AVTOMATIK YARATILGAN — tahrir qilmang: \`node tool/shaders/sync-web.mjs\`\n// Manba: packages/video_player_android/android/src/main/assets/aru_enhance_fragment.glsl\nexport const ENHANCE_FRAG = ${JSON.stringify(src)};\n`;
writeFileSync('web/src/js/player/enhance-shader.js', out);
console.log('yozildi', src.length, 'belgi');
