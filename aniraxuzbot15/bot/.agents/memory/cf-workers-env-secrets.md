---
name: CF Workers env secrets
description: CF Workers da secretlar process.env orqali ishlamaydi, applyEnv() pattern kerak
---

**Muammo:** Cloudflare Workers da `wrangler secret put` orqali o'rnatilgan secretlar `process.env.SECRET` orqali emas, faqat fetch/scheduled handler ga uzatiladigan `env` parametri orqali keladi. Wrangler bundle vaqtida `process.env.SECRET` ni `undefined` ga almashtiradi.

**Qoida:** `worker.ts` da `fetch(request, env, ctx)` va `scheduled(event, env, ctx)` handler larida `applyEnv(env)` chaqirib, `config` obyektini yangilash kerak.

**How to apply:**
```ts
function applyEnv(env: Record<string, string | undefined>) {
  if (env.BOT_TOKEN) config.BOT_TOKEN = env.BOT_TOKEN;
  if (env.CF_API_TOKEN) config.CF_API_TOKEN = env.CF_API_TOKEN;
  if (env.TURSO_AUTH_TOKEN) config.TURSO_AUTH_TOKEN = env.TURSO_AUTH_TOKEN;
  // ...
}

export default {
  fetch(request, env, ctx) {
    applyEnv(env);
    return app.fetch(request, env, ctx);
  },
  async scheduled(event, env, ctx) {
    applyEnv(env);
    // ...
  }
};
```

**Why:** CF Workers runtime secrets are ONLY accessible via the env binding object passed to handlers. `process.env` reads are replaced at bundle time by wrangler/esbuild for known vars, and secrets are not available at module init time via process.env.
