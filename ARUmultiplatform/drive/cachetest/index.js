// UZ: Sinov: /t keshlanadi (tag t1), POST /purge tag bo'yicha o'chiradi. EN: Test: /t cached with tag, POST /purge purges by tag. RU: Тест: /t кешируется с тегом, POST /purge очищает по тегу.
export default {
  async fetch(request, env, ctx) {
    const u = new URL(request.url);
    if (u.pathname === "/t") {
      return new Response(JSON.stringify({ gen: Date.now() + "-" + Math.random().toString(36).slice(2, 8), colo: request.cf && request.cf.colo }), {
        headers: { "content-type": "application/json", "cache-control": "public, max-age=86400", "cache-tag": "t1" },
      });
    }
    if (u.pathname === "/purge" && request.method === "POST") {
      try {
        const r = await ctx.cache.purge({ tags: ["t1"] });
        return new Response(JSON.stringify({ ok: true, r: r === undefined ? null : r, colo: request.cf && request.cf.colo }));
      } catch (e) {
        return new Response(JSON.stringify({ ok: false, error: String(e) }), { status: 500 });
      }
    }
    return new Response("cachetest", { headers: { "cache-control": "no-store" } });
  },
};
