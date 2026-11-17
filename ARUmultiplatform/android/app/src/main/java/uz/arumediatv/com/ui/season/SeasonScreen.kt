package uz.arumediatv.com.ui.season

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.pm.ActivityInfo
import android.view.WindowManager
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import uz.arumediatv.com.ui.common.AruIcons
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.filled.Lock
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.Episode
import uz.arumediatv.com.data.LocalOverlay
import uz.arumediatv.com.data.HistoryItem
import uz.arumediatv.com.data.PlaySource
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.data.SeasonInfo
import uz.arumediatv.com.ui.theme.AruColors

private fun Context.findActivity(): Activity? {
    var c: Context = this
    while (c is ContextWrapper) {
        if (c is Activity) return c
        c = c.baseContext
    }
    return null
}

/** Tomosha tarixiga yoziladigan joriy qism (serverga `SyncQueue` orqali, qism almashganda/chiqishda). */
private class HistoryDraft(
    val animeId: Long,
    val seasonId: Long,
    val epId: Long,
    val url: String,
    val quality: String,
    var watchedMs: Long,
    var newView: Boolean = true,
    var positionMs: Long = 0,
    var durationMs: Long = 0,
    var lastStamp: String = "",
    var lastSyncStamp: String = "",
    /** Ekranda allaqachon hisobga olingan ko'rish va tomosha vaqti (qurilmada darhol ko'rinishi uchun). */
    var viewNoted: Boolean = false,
    var msNoted: Long = 0,
)

/**
 * Bo'lim sahifasi + pleyer (Flutter `VideoPlayerScreen` ga mos): tepada video, ostida Ma'lumot | Qismlar | Bo'limlar | Izohlar.
 * Ruxsat: obuna faol yoki bo'lim bepul. Manba: Telegram (mahalliy server), bo'lmasa worker keshi.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SeasonScreen(
    season: Season,
    onBack: () -> Unit,
    onOpenSeason: (Season) -> Unit,
    onOpenBilling: () -> Unit,
    onOpenProfile: (Long) -> Unit,
    onLogin: () -> Unit,
    startEpisodeId: Long = 0,
    startQuality: String? = null,
    onPacks: () -> Unit = {},
) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val repo = AppServices.seasonData
    val billing by AppServices.billing.state.collectAsStateWithLifecycle()
    // ── Ruxsat: obuna (yoki reklama orqali BAZADA qo'shilgan 1 soat) faol bo'lishi kerak; boshqasi pleyerni ochmaydi ──
    // Muddat serverda (`subs_db`), shuning uchun reklama bitta qurilmada ko'rilsa boshqasida ham ishlaydi.
    var adNow by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(billing.until) {
        val day = 24 * 60 * 60_000L
        while (true) {
            adNow = System.currentTimeMillis()
            val left = billing.until - adNow
            if (left <= 0) break
            delay(if (left <= day) 1000L else minOf(left - day + 1, 60_000L))
        }
    }
    val adLeftMs = (billing.until - adNow).coerceAtLeast(0)
    var adBusy by remember { mutableStateOf(false) }
    var adMsg by remember { mutableStateOf<String?>(null) }
    val watchAd: () -> Unit = {
        val act = ctx.findActivity()
        if (act == null) adMsg = "Reklamani ochib bo'lmadi."
        else {
            adBusy = true
            adMsg = null
            uz.arumediatv.com.data.RewardedAds.show(
                act,
                onReward = { scope.launch { adMsg = AppServices.billing.adReward(); adBusy = false } },
                onFail = { adMsg = it; adBusy = false },
            )
        }
    }
    val canWatch = billing.until > adNow
    // Pleyer ochiq paytda orqa fonda kadr yaratish to'xtaydi (tarmoq faqat videoniki).
    DisposableEffect(Unit) {
        AppServices.playerBusy.incrementAndGet()
        onDispose { AppServices.playerBusy.decrementAndGet() }
    }

    val player = remember { AruPlayer(ctx) }
    // Ruxsat tugadi: ijro pauzaga qo'yiladi, joy saqlanadi.
    LaunchedEffect(canWatch) { if (!canWatch) { player.pause(); uz.arumediatv.com.data.RewardedAds.ensureInit(ctx) } }
    var episodes by remember { mutableStateOf<List<Episode>>(emptyList()) }
    var loadingEps by remember { mutableStateOf(true) }
    var seasons by remember { mutableStateOf<List<Season>>(emptyList()) }
    var info by remember { mutableStateOf<SeasonInfo?>(null) }
    var current by remember { mutableStateOf<Episode?>(null) }
    var currentUrl by remember { mutableStateOf("") }
    // Pleyerda HOZIR ochiq video manzili (manba hal bo'lguncha bo'sh): har soniyada saqlash eski videoning joyini yangi qismga yozib yubormasin.
    var progressUrl by remember { mutableStateOf("") }
    var viaTelegram by remember { mutableStateOf(false) }
    var viaVd by remember { mutableStateOf(false) }
    var viaLocal by remember { mutableStateOf(false) }
    var chosenQuality by remember { mutableStateOf<String?>(null) }
    var tab by rememberSaveable { mutableIntStateOf(0) }
    var fullscreen by rememberSaveable { mutableStateOf(false) }
    var locked by remember { mutableStateOf(false) }
    var showSettings by remember { mutableStateOf(false) }
    var sleepEndAt by remember { mutableLongStateOf(0L) }
    val prefs = remember { ctx.getSharedPreferences("aru_player", Context.MODE_PRIVATE) }
    var autoNext by remember { mutableStateOf(prefs.getBoolean("auto_next", true)) }
    var autoSkipIntro by remember { mutableStateOf(prefs.getBoolean("auto_skip_intro", false)) }
    val skippedRanges = remember { HashSet<Long>() }
    var draft by remember { mutableStateOf<HistoryDraft?>(null) }
    var workerRetried by remember { mutableStateOf(false) }

    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()

    val sorted = remember(episodes) { episodes.sortedBy { it.number } }

    // ── Tomosha tarixi: qism almashganda / chiqishda yoziladi (SyncQueue — bitta paket) ──
    // `sync=false`: faqat ro'yxat va telefon xotirasi (har soniyada, arzon); `sync=true`: serverga yuborish navbati ham (SyncQueue o'zi 2 daqiqada bir paket qiladi).
    fun flushHistory(sync: Boolean = true) {
        val d = draft ?: return
        if (d.durationMs <= 0 || d.positionMs < 1000) return
        val stamp = "${d.positionMs}/${d.watchedMs}/${d.quality}/${d.newView}"
        if (sync && stamp != d.lastSyncStamp) {
            d.lastSyncStamp = stamp
            AppServices.sync.putHistory(d.animeId, d.seasonId, d.epId, d.url, d.quality, d.positionMs, d.durationMs, d.watchedMs, d.newView)
        }
        if (stamp == d.lastStamp) return
        d.lastStamp = stamp
        // UZ: ko'rish (+1) va tomosha vaqti shu zahoti ekranda; baho/ko'rish ro'yxatlari ham. EN: instant local counters.
        val dv = if (d.newView && !d.viewNoted) 1L else 0L
        val dm = (d.watchedMs - d.msNoted).coerceAtLeast(0)
        if (dv > 0 || dm >= 5_000) {
            if (dv > 0) d.viewNoted = true
            d.msNoted = d.watchedMs
            info = info?.let { LocalOverlay.addWatch(it, dv, dm) }
            if (dv > 0) {
                episodes = episodes.map { e -> if (e.id == d.epId) e.copy(views = e.views + 1, watchMs = e.watchMs + dm) else e }
                AppServices.seasons.patch(d.animeId, d.seasonId) { LocalOverlay.viewed(it, 1) }
            } else episodes = episodes.map { e -> if (e.id == d.epId) e.copy(watchMs = e.watchMs + dm) else e }
        }
        // Ro'yxatda (Kutubxona > Tarix) darhol ko'rinadi va telefon xotirasiga yoziladi — server paketini kutmaydi.
        sorted.firstOrNull { it.id == d.epId }?.let { ep ->
            AppServices.library.applyHistory(
                HistoryItem(
                    animeId = d.animeId, seasonId = d.seasonId, episodeNo = ep.number, sectionNo = season.sectionNo,
                    title = season.name, photoUrl = season.photoUrl, positionMs = d.positionMs, durationMs = d.durationMs,
                    views = if (d.newView) 1 else 0, watchedMs = d.watchedMs, quality = d.quality, epizodId = d.epId,
                    updatedAt = System.currentTimeMillis(), videoUrl = d.url,
                ),
            )
        }
        if (sync) d.newView = false
    }

    fun saveProgress() {
        if (progressUrl.isNotEmpty() && player.durationMs > 0) {
            current?.let { e -> AppServices.progress.saveEpisode("${season.animeId}:${season.seasonId}:${e.id}", player.positionMs, player.durationMs, e.qualityOf(progressUrl)) }
        }
    }

    fun play(ep: Episode, quality: String? = null, resumeAt: Long? = null, resumePlaying: Boolean = true, preferWorker: Boolean = false) {
        if (!canWatch) return
        scope.launch {
            val hist = AppServices.library.state.value.history.firstOrNull { it.animeId == season.animeId && it.seasonId == season.seasonId && it.epizodId == ep.id }
            val epKey = "${season.animeId}:${season.seasonId}:${ep.id}"
            val saved = AppServices.progress.episodeState(epKey)
            var q = quality ?: chosenQuality ?: saved?.second?.takeIf { ep.urls.containsKey(it) } ?: hist?.quality?.takeIf { ep.urls.containsKey(it) }
            // Internet yo'q: telefonda to'liq bor sifat tanlanadi (aks holda boshqa sifat ochilib, "bu qism yuklab olinmagan" chiqardi va saqlangan joy topilmasdi).
            if (!uz.arumediatv.com.data.Net.isOnline(ctx)) {
                val have = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
                    val cands = listOfNotNull(q) + ep.urls.keys.filter { it != q }
                    cands.firstOrNull { k -> ep.urls[k]?.let { AppServices.downloads.isComplete(it) } == true }
                }
                if (have != null) q = have
            }
            val url = ep.urlFor(q)
            if (url.isEmpty()) {
                toast("Bu qismda video yo'q")
                return@launch
            }
            if (!preferWorker) {
                saveProgress()
                flushHistory()
                workerRetried = false
                draft = HistoryDraft(season.animeId, season.seasonId, ep.id, url, ep.qualityOf(url), hist?.watchedMs ?: 0).also { it.msNoted = it.watchedMs }
                skippedRanges.clear()
            }
            progressUrl = ""
            current = ep
            currentUrl = url
            player.error = null
            // Davom etish joyi: QISM bo'yicha saqlangani (sifatdan mustaqil) birinchi; server tarixi undan YANGIROQ bo'lsa (boshqa qurilma) — o'sha;
            // faylga bog'liq joy (`positionOf`) eng oxirida: u sifat almashtirilganda ESKI joyda qolib ketardi (25-daqiqadan chiqib, qayta kirganda 12-daqiqadan boshlanardi).
            val histPos = hist?.positionMs?.takeIf { it >= 3000 && (hist.durationMs <= 0 || it <= hist.durationMs - 5000) } ?: 0L
            val localPos = saved?.first ?: 0L
            val localStamp = AppServices.progress.episodeStamp(epKey)
            val start = uz.arumediatv.com.data.ResumePoint.choose(resumeAt, localPos, localStamp, histPos, hist?.updatedAt ?: 0L, AppServices.progress.positionOf(url))
            val src = PlaySource.resolve(url, AppServices.telegram, preferWorker)
            if (src == null) {
                player.error = if (!uz.arumediatv.com.data.Net.isOnline(ctx)) "Bu qism yuklab olinmagan — internetga ulaning" else "Videoni ochib bo'lmadi"
                return@launch
            }
            viaTelegram = src.viaTelegram
            viaVd = src.viaVd
            viaLocal = src.local
            progressUrl = url
            player.open(src.uri, start, resumePlaying)
        }
    }

    // ── Yuklash ──
    LaunchedEffect(season.key, canWatch) {
        AppServices.library.load()
        info = repo.infoFromDisk(season.animeId, season.seasonId)
        launch { repo.loadInfo(season.animeId, season.seasonId)?.let { info = it } }
        launch {
            repo.cachedSeasons(season.animeId)?.let { seasons = it }
            repo.fetchSeasons(season.animeId)?.let { seasons = it }
        }
        if (!canWatch) {
            loadingEps = false
            return@LaunchedEffect
        }
        repo.cachedEpisodes(season.animeId, season.seasonId)?.let { episodes = it; loadingEps = false }
        val fresh = repo.fetchEpisodes(season.animeId, season.seasonId)
        if (fresh != null) episodes = fresh
        loadingEps = false
        // Avto ochish: oxirgi ko'rilgan qism (tarixdan), bo'lmasa birinchisi.
        if (AppServices.auth.isLoggedIn) withTimeoutOrNull(1500) { AppServices.library.state.first { it.loaded } }
        val list = episodes.sortedBy { it.number }
        val last = AppServices.library.state.value.history
            .filter { it.animeId == season.animeId && it.seasonId == season.seasonId }.maxByOrNull { it.updatedAt }
        val wanted = list.firstOrNull { it.id == startEpisodeId }
        val start = wanted ?: list.firstOrNull { it.id == last?.epizodId } ?: list.firstOrNull()
        if (start != null && current == null) {
            if (wanted != null) {
                startQuality?.let { chosenQuality = it }
                play(wanted, quality = startQuality)
            } else play(start, resumePlaying = false)
        }
    }

    // ── Jonli yangilanish: qism/bo'lim o'zgarsa (admin qo'shdi/tahrirladi) ro'yxat bir necha soniyada yangilanadi, ijro buzilmaydi ──
    if (canWatch) uz.arumediatv.com.data.LiveEffect("content") {
        repo.fetchEpisodes(season.animeId, season.seasonId, force = true)?.let { episodes = it }
        repo.fetchSeasons(season.animeId, force = true)?.let { seasons = it }
    }

    // ── Ijro nuqtasi: har 500 ms; 5 soniyada eslab qolish; tomosha vaqti ──
    LaunchedEffect(player) {
        var n = 0
        while (true) {
            delay(500)
            player.tick()
            val d = draft
            if (d != null && player.durationMs > 0) {
                d.positionMs = player.positionMs
                d.durationMs = player.durationMs
                if (player.playing) d.watchedMs = (d.watchedMs + 500).coerceAtMost(player.durationMs)
            }
            if (++n % 2 == 0) { saveProgress(); flushHistory(sync = n % 20 == 0) } // har 1 soniyada: joy + Tarix ro'yxati + telefon xotirasi; serverga yuborish navbati 10 soniyada (tick 500 ms)
            // Intro: avto o'tkazish
            val ep = current
            if (ep != null && autoSkipIntro) {
                val r = ep.intro.firstOrNull { player.positionMs in it }
                if (r != null && skippedRanges.add(r.first)) player.seekTo(r.last)
            }
        }
    }

    // ── Qism tugadi: keyingisi ──
    LaunchedEffect(player.ended) {
        if (!player.ended) return@LaunchedEffect
        saveProgress()
        flushHistory()
        val idx = sorted.indexOfFirst { it.id == current?.id }
        if (autoNext && idx >= 0 && idx + 1 < sorted.size) play(sorted[idx + 1])
    }

    // ── Xato: Telegram bo'lsa bir marta worker yo'liga o'tamiz ──
    LaunchedEffect(player.error) {
        val ep = current
        // Yangi tizim (vd) ochilmadi — eski yo'lga (Telegram) FAQAT Telegram hisobi ulangan bo'lsa o'tamiz. Aks holda eski yo'lda bu fayl YO'Q
        // (worker `/api/play/<nom>` uni topa olmaydi): o'tsak sekin tarmoqdagi bitta vaqtinchalik xato videoni butunlay "o'ldirardi" (cheksiz aylanish).
        if (player.error != null && viaVd && ep != null) {
            if (AppServices.telegram.state.value.authorized) {
                uz.arumediatv.com.data.Names.fileNameOf(currentUrl).takeIf { it.isNotEmpty() }?.let { uz.arumediatv.com.data.PlaySource.vdFailed.add(it) }
                viaVd = false
                play(ep, resumeAt = player.positionMs, preferWorker = false)
            }
            return@LaunchedEffect
        }
        if (player.error != null && (viaTelegram || viaLocal) && !workerRetried && ep != null) {
            workerRetried = true
            play(ep, resumeAt = player.positionMs, preferWorker = true)
        }
    }

    // ── Uyqu taymeri ──
    LaunchedEffect(sleepEndAt) {
        if (sleepEndAt <= 0) return@LaunchedEffect
        delay((sleepEndAt - System.currentTimeMillis()).coerceAtLeast(0))
        player.pause()
        sleepEndAt = 0
        toast("Uyqu taymeri: pauza qilindi")
    }

    // ── Ekran fonga ketsa — pauza, tarix yoziladi ──
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    DisposableEffect(lifecycle) {
        val ob = LifecycleEventObserver { _, e ->
            if (e == Lifecycle.Event.ON_PAUSE) {
                player.pause()
                saveProgress()
                flushHistory()
            }
        }
        lifecycle.addObserver(ob)
        onDispose { lifecycle.removeObserver(ob) }
    }
    DisposableEffect(Unit) {
        val act = ctx.findActivity()
        act?.window?.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        onDispose {
            saveProgress()
            flushHistory()
            player.release()
            act?.window?.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        }
    }

    // ── To'liq ekran: gorizontal + tizim panellari yashirin ──
    DisposableEffect(fullscreen) {
        uz.arumediatv.com.core.Immersive.on = fullscreen
        val act = ctx.findActivity()
        val w = act?.window
        if (fullscreen && act != null && w != null) {
            act.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
            // Kameraga o'yilgan joy (notch) atrofida qora chekka qolmasin: video butun ekranga sig'sin.
            if (android.os.Build.VERSION.SDK_INT >= 28) {
                w.attributes = w.attributes.apply {
                    layoutInDisplayCutoutMode = if (android.os.Build.VERSION.SDK_INT >= 30) WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
                    else WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
                }
            }
            WindowCompat.getInsetsController(w, w.decorView).apply {
                hide(WindowInsetsCompat.Type.systemBars())
                systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            }
        }
        onDispose {
            uz.arumediatv.com.core.Immersive.on = false
            if (act != null && w != null) {
                act.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
                if (android.os.Build.VERSION.SDK_INT >= 28) {
                    w.attributes = w.attributes.apply { layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_DEFAULT }
                }
                WindowCompat.getInsetsController(w, w.decorView).show(WindowInsetsCompat.Type.systemBars())
            }
        }
    }
    // To'liq ekranda tizim panellari (vaqt, quvvat, tugmalar) pleyer tugmalari bilan birga chiqadi va birga yashirinadi.
    var barsShown by remember { mutableStateOf(false) }
    LaunchedEffect(fullscreen, barsShown) {
        if (!fullscreen) return@LaunchedEffect
        val w = ctx.findActivity()?.window ?: return@LaunchedEffect
        fun apply() {
            WindowCompat.getInsetsController(w, w.decorView).apply {
                systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                if (barsShown) show(WindowInsetsCompat.Type.systemBars()) else hide(WindowInsetsCompat.Type.systemBars())
            }
        }
        apply()
        // Aylantirish tugagach tizim panellari ba'zan o'zi qaytadi: yashirin bo'lsa qayta yashiramiz.
        if (!barsShown) repeat(3) { delay(500); apply() }
    }
    BackHandler(enabled = fullscreen) { fullscreen = false }
    BackHandler(enabled = !fullscreen, onBack = onBack)

    val idx = sorted.indexOfFirst { it.id == current?.id }
    val ep = current
    val introActive = ep?.intro?.any { player.positionMs in it } == true

    val pager = androidx.compose.foundation.pager.rememberPagerState(initialPage = tab) { 4 }
    val pagerScope = androidx.compose.runtime.rememberCoroutineScope()
    LaunchedEffect(pager.currentPage) { tab = pager.currentPage }
    var showEpisodeList by remember { mutableStateOf(false) }
    var commentsExpanded by remember { mutableStateOf(false) }
    LaunchedEffect(pager.currentPage) { if (pager.currentPage != 3) commentsExpanded = false }
    // Izohlar to'liq ekranga yoyilganda tepadagi hamma narsa (nom, pleyer, tablar) yig'iladi.
    val collapsed = commentsExpanded && pager.currentPage == 3 && !fullscreen

    if (!canWatch) {
        SubRequiredScreen(onBack = onBack, onBilling = onOpenBilling, onWatchAd = watchAd, adBusy = adBusy, adMsg = adMsg, adExpired = billing.until > 0, adReady = uz.arumediatv.com.data.RewardedAds.ready())
    } else
    Column(Modifier.fillMaxSize().background(AruColors.Bg).then(if (fullscreen) Modifier else Modifier.statusBarsPadding())) {
        if (!fullscreen && !collapsed) {
            // ── Orqaga + nom (Flutter bilan bir xil) ──
            Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                val bs = RoundedCornerShape(14.dp)
                Box(Modifier.clip(bs).background(AruColors.Card).border(1.dp, AruColors.Border, bs).clickable(onClick = onBack).padding(8.dp)) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", tint = Color.White, modifier = Modifier.size(24.dp))
                }
                Spacer(Modifier.width(12.dp))
                Text(
                    "${season.sectionNo.takeIf { it > 0 } ?: 1}-bo'lim  ${season.name}", color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.Bold,
                    maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false),
                )
                if (canWatch && adLeftMs in 1..(24 * 60 * 60_000L)) {
                    Spacer(Modifier.width(8.dp))
                    val m = adLeftMs / 60_000
                    val sec = (adLeftMs / 1000) % 60
                    Text(
                        "⏱ %d:%02d:%02d".format(m / 60, m % 60, sec), color = Color.White, fontSize = 12.sp, fontWeight = FontWeight.Bold,
                        modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(AruColors.Accent.copy(alpha = 0.85f)).padding(horizontal = 8.dp, vertical = 4.dp),
                    )
                }
            }
        }
        // ── Video ──
        val box = if (fullscreen) Modifier.fillMaxSize() else Modifier.padding(horizontal = 16.dp).fillMaxWidth().aspectRatio(16f / 9f).clip(RoundedCornerShape(16.dp))
        if (!collapsed) Box(box.background(Color.Black)) {
            if (!canWatch) {
                SubRequired(onBack = onBack, onBilling = onOpenBilling)
            } else {
                PlayerSurface(player, Modifier.fillMaxSize())
                val bolim = season.sectionNo.takeIf { it > 0 } ?: 1
                PlayerControls(
                    player = player,
                    title = season.name,
                    subtitle = ep?.let { e -> "$bolim-bo'lim · ${e.number}-qism" + if (e.name.isNotBlank()) " · ${e.name}" else "" }.orEmpty(),
                    fullscreen = fullscreen,
                    locked = locked,
                    hasPrev = idx > 0,
                    hasNext = idx >= 0 && idx + 1 < sorted.size,
                    introActive = introActive,
                    speedLabel = if (player.speed == player.speed.toInt().toFloat()) "${player.speed.toInt()}x" else "${player.speed}x",
                    onSkipIntro = { ep?.intro?.firstOrNull { player.positionMs in it }?.let { player.seekTo(it.last) } },
                    onPrev = { if (idx > 0) play(sorted[idx - 1], resumePlaying = player.exo.playWhenReady) },
                    onNext = { if (idx >= 0 && idx + 1 < sorted.size) play(sorted[idx + 1], resumePlaying = player.exo.playWhenReady) },
                    onBack = { if (fullscreen) fullscreen = false else onBack() },
                    onToggleFullscreen = { fullscreen = !fullscreen },
                    onSettings = { showSettings = true },
                    onQuality = { showSettings = true },
                    onEpisodeList = { showEpisodeList = true },
                    onToggleLock = { locked = !locked },
                    onControlsShown = { barsShown = it },
                    onRetry = { ep?.let { e -> play(e, resumeAt = player.positionMs, preferWorker = !viaTelegram) } },
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }

        if (!fullscreen) {
            // ── Hozir ijro: "1-bo'lim · 1-qism 👁 1 ⏱ 0:00 📅 sana" ──
            if (!collapsed) {
            if (ep != null) {
                val cs = RoundedCornerShape(11.dp)
                @OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
                androidx.compose.foundation.layout.FlowRow(
                    Modifier.padding(start = 16.dp, end = 16.dp, top = 6.dp).clip(cs).background(AruColors.Card).border(1.dp, AruColors.Border, cs).padding(horizontal = 10.dp, vertical = 5.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    Text("${ep.number}-qism", color = Color.White, fontSize = 11.5.sp, fontWeight = FontWeight.Bold)
                    NowStat(AruIcons.Visibility, formatCount(ep.views))
                    NowStat(AruIcons.Schedule, formatHours(ep.watchMs))
                    if (ep.addedAt > 0) NowStat(AruIcons.EventAvailable, formatMomentYmd(ep.addedAt))
                }
            } else Spacer(Modifier.height(8.dp))
            if (ep != null) CurrentDownloadStrip(ep)
            Spacer(Modifier.height(8.dp))

            // ── Tablar (shisha qutida) ──
            val titles = listOf("Ma'lumot", "Qismlar", "Bo'limlar", "Izohlar")
            val ts = RoundedCornerShape(14.dp)
            Row(
                Modifier.padding(horizontal = 16.dp).fillMaxWidth().clip(ts).background(AruColors.Card).border(1.dp, AruColors.Border, ts).padding(3.dp),
            ) {
                titles.forEachIndexed { i, t ->
                    val active = pager.currentPage == i
                    Box(
                        Modifier.weight(1f).height(32.dp).clip(RoundedCornerShape(11.dp))
                            .then(if (active) Modifier.background(AruColors.Accent) else Modifier)
                            .clickable { pagerScope.launch { pager.animateScrollToPage(i) } },
                        contentAlignment = Alignment.Center,
                    ) { Text(t, color = if (active) Color.White else Color.White.copy(alpha = 0.54f), fontSize = 13.sp, fontWeight = if (active) FontWeight.SemiBold else FontWeight.Normal, maxLines = 1) }
                }
            }
            Spacer(Modifier.height(6.dp))

            }

            androidx.compose.foundation.pager.HorizontalPager(pager, Modifier.weight(1f).fillMaxWidth(), beyondViewportPageCount = if (uz.arumediatv.com.core.Perf.low) 0 else 1) { page ->
                when (page) {
                    0 -> InfoTab(
                        season = season, info = info,
                        onRate = { stars ->
                            if (!AppServices.auth.isLoggedIn) toast("Baholash uchun hisobga kiring")
                            else info = repo.rate(season.animeId, season.seasonId, stars, info ?: SeasonInfo(org.json.JSONObject(), 0.0, 0, false))
                        },
                        onFavorite = {
                            if (!AppServices.auth.isLoggedIn) toast("Sevimlilarga qo'shish uchun hisobga kiring")
                            else info = repo.setFavorite(season, info?.isFav != true, info ?: SeasonInfo(org.json.JSONObject(), 0.0, 0, false))
                        },
                    )
                    1 -> if (DialPrefs.mode != 0 && canWatch && episodes.isNotEmpty()) EpisodeDialTab(
                        episodes = episodes, currentId = current?.id, player = player, left = DialPrefs.mode == 2,
                        hasPrev = idx > 0, hasNext = idx >= 0 && idx + 1 < sorted.size,
                        onPlay = { e -> play(e) },
                        onPrev = { if (idx > 0) play(sorted[idx - 1], resumePlaying = player.exo.playWhenReady) },
                        onNext = { if (idx >= 0 && idx + 1 < sorted.size) play(sorted[idx + 1], resumePlaying = player.exo.playWhenReady) },
                    ) else EpisodesTab(
                        canWatch = canWatch, loading = loadingEps, episodes = episodes,
                        currentId = current?.id, currentQuality = chosenQuality,
                        onPlay = { e, q ->
                            if (q != null) chosenQuality = q
                            play(e, quality = q)
                        },
                        onSubscribe = onOpenBilling,
                        onShare = { e -> uz.arumediatv.com.data.DeepLink.share(ctx, season.name, e.number, season.animeId, season.seasonId, e.id) },
                        onDeleted = { e, q ->
                            // Hozir ijro etilayotgan sifat o'chirilsa — oqim uziladi: shu joydan qaytadan ochamiz (Flutterdagidek).
                            if (current?.id == e.id && currentUrl == e.urlFor(q)) {
                                play(e, quality = q, resumeAt = player.positionMs, resumePlaying = player.exo.playWhenReady, preferWorker = true)
                            }
                        },
                    )
                    2 -> SeasonsTab(seasons = seasons, current = season, onOpen = onOpenSeason)
                    else -> CommentsTab(season = season, onOpenProfile = onOpenProfile, onLogin = onLogin, onPacks = onPacks, expanded = commentsExpanded, onExpandedChange = { commentsExpanded = it })
                }
            }

            // ── Qism almashtirish (pastda, bosh barmoq zonasida): [⏮ 3-qism]  [5-qism ⏭] ──
            if (DialPrefs.mode == 0 && pager.currentPage == 1 && !collapsed && sorted.isNotEmpty()) {
                val prev = if (idx > 0) sorted[idx - 1] else null
                val next = if (idx >= 0 && idx + 1 < sorted.size) sorted[idx + 1] else null
                val bs = RoundedCornerShape(20.dp)
                Row(
                    Modifier.padding(horizontal = 12.dp).padding(top = 6.dp, bottom = 10.dp).fillMaxWidth().clip(bs)
                        .background(AruColors.Card).border(1.dp, AruColors.Border, bs).padding(8.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    NavBtn(AruIcons.SkipPrevious, prev?.let { "${it.number}-qism" } ?: "Oldingi", true, false, prev != null) { prev?.let { play(it, resumePlaying = player.exo.playWhenReady) } }
                    NavBtn(AruIcons.SkipNext, next?.let { "${it.number}-qism" } ?: "Keyingi", false, true, next != null) { next?.let { play(it, resumePlaying = player.exo.playWhenReady) } }
                }
            }
        }
    }

    if (showEpisodeList) {
        EpisodeListSheet(sorted, current?.id, onPick = { e -> play(e) }, onDismiss = { showEpisodeList = false })
    }

    if (showSettings) {
        PlayerSettingsSheet(
            episode = current, currentUrl = currentUrl,
            speed = player.speed, autoNext = autoNext, autoSkipIntro = autoSkipIntro, sleepEndAt = sleepEndAt,
            onQuality = { q ->
                chosenQuality = q
                current?.let { e -> play(e, quality = q, resumeAt = player.positionMs, resumePlaying = player.playing || !player.ready) }
            },
            onSpeed = { player.changeSpeed(it) },
            onAutoNext = { autoNext = it; prefs.edit().putBoolean("auto_next", it).apply() },
            onAutoSkip = { autoSkipIntro = it; prefs.edit().putBoolean("auto_skip_intro", it).apply() },
            onSleep = { minutes -> sleepEndAt = if (minutes <= 0) 0 else System.currentTimeMillis() + minutes * 60_000L },
            onDismiss = { showSettings = false },
            onDeleted = { e, q ->
                if (current?.id == e.id && currentUrl == e.urlFor(q)) {
                    play(e, quality = q, resumeAt = player.positionMs, resumePlaying = player.exo.playWhenReady, preferWorker = true)
                }
            },
        )
    }
}

@Composable
private fun SubRequired(onBack: () -> Unit, onBilling: () -> Unit) {
    Column(Modifier.fillMaxSize().padding(16.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
        Text("Obuna kerak", color = Color.White, fontSize = 18.sp, fontWeight = FontWeight.Bold)
        Spacer(Modifier.height(4.dp))
        Text("Bu bo'limni ko'rish uchun obuna bo'ling", color = Color.White.copy(alpha = 0.65f), fontSize = 12.sp)
        Spacer(Modifier.height(12.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(
                "Orqaga", color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
                modifier = Modifier.clip(RoundedCornerShape(12.dp)).background(Color.White.copy(alpha = 0.12f)).clickable(onClick = onBack).padding(horizontal = 18.dp, vertical = 10.dp),
            )
            Text(
                "Obuna olish", color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
                modifier = Modifier.clip(RoundedCornerShape(12.dp)).background(AruColors.Accent).clickable(onClick = onBilling).padding(horizontal = 18.dp, vertical = 10.dp),
            )
        }
    }
}

@Composable
private fun NowStat(icon: androidx.compose.ui.graphics.vector.ImageVector, text: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Icon(icon, null, tint = Color.White.copy(alpha = 0.45f), modifier = Modifier.size(11.5.dp))
        Spacer(Modifier.width(3.dp))
        Text(text, color = Color.White.copy(alpha = 0.62f), fontSize = 10.5.sp)
    }
}

@Composable
private fun RowScope.NavBtn(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, iconFirst: Boolean, primary: Boolean, enabled: Boolean, onClick: () -> Unit) {
    val shape = RoundedCornerShape(14.dp)
    val tint = Color.White.copy(alpha = if (enabled) 0.97f else 0.25f)
    val bg = if (primary && enabled) AruColors.Accent else Color.White.copy(alpha = if (enabled) 0.10f else 0.035f)
    val br = if (primary && enabled) AruColors.Accent else Color.White.copy(alpha = if (enabled) 0.20f else 0.06f)
    Row(
        Modifier.weight(1f).height(56.dp).clip(shape).background(bg).border(1.dp, br, shape)
            .clickable(enabled = enabled, onClick = onClick),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (iconFirst) Icon(icon, null, tint = tint, modifier = Modifier.size(26.dp))
        Text(label, color = tint, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, modifier = Modifier.padding(horizontal = 6.dp))
        if (!iconFirst) Icon(icon, null, tint = tint, modifier = Modifier.size(26.dp))
    }
}

/** "Obuna kerak" to'liq ekrani (Flutter `_SubRequiredScreen`). */
@Composable
private fun SubRequiredScreen(onBack: () -> Unit, onBilling: () -> Unit, onWatchAd: () -> Unit, adBusy: Boolean, adMsg: String?, adExpired: Boolean, adReady: Boolean) {
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    Column(Modifier.fillMaxSize().background(AruColors.Bg).statusBarsPadding()) {
        Box(Modifier.padding(start = 12.dp, top = 8.dp).clip(CircleShape).clickable(onClick = onBack).padding(10.dp)) {
            Icon(Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", tint = Color.White, modifier = Modifier.size(24.dp))
        }
        Column(Modifier.weight(1f).fillMaxWidth().padding(horizontal = 24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
            Box(
                Modifier.size(88.dp).clip(CircleShape).background(Brush.linearGradient(listOf(AruColors.Gold, AruColors.Accent2))),
                contentAlignment = Alignment.Center,
            ) { Icon(AruIcons.WorkspacePremium, null, tint = AruColors.AccentTint, modifier = Modifier.size(44.dp)) }
            Spacer(Modifier.height(22.dp))
            Text("Reklama yoki obuna kerak", color = Color.White, fontSize = 22.sp, fontWeight = FontWeight.ExtraBold, textAlign = androidx.compose.ui.text.style.TextAlign.Center)
            Spacer(Modifier.height(10.dp))
            Text(
                if (adExpired) "Ko'rish muddatingiz tugadi. Anime ko'rish uchun reklama ko'rishingiz yoki obuna sotib olishingiz kerak. Ko'rilgan joyingiz saqlangan."
                else "Anime ko'rish uchun reklama ko'rishingiz yoki obuna sotib olishingiz kerak. Reklamani oxirigacha ko'rsangiz, ko'rish muddatingizga 1 soat qo'shiladi.",
                color = Color.White.copy(alpha = 0.62f), fontSize = 13.5.sp, lineHeight = 21.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
            Spacer(Modifier.height(24.dp))
            Box(
                Modifier.fillMaxWidth().height(48.dp).clip(RoundedCornerShape(14.dp)).background(Color.White.copy(alpha = if (adBusy) 0.06f else 0.14f)).clickable(enabled = !adBusy, onClick = onWatchAd),
                contentAlignment = Alignment.Center,
            ) {
                Text(if (adBusy) "Reklama ochilmoqda..." else "Reklama ko'rish — +1 soat", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.ExtraBold)
            }
            if (adMsg != null) {
                Spacer(Modifier.height(8.dp))
                Text(adMsg, color = Color(0xFFFF8A80), fontSize = 12.5.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center)
            } else if (!adReady) {
                Spacer(Modifier.height(8.dp))
                Text("Reklama yuklanmoqda...", color = Color.White.copy(alpha = 0.4f), fontSize = 12.sp)
            }
            Spacer(Modifier.height(10.dp))
            Box(Modifier.fillMaxWidth().height(48.dp).clip(RoundedCornerShape(14.dp)).background(AruColors.Accent).clickable(onClick = onBilling), contentAlignment = Alignment.Center) {
                Text("Obuna olish", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.ExtraBold)
            }
            Spacer(Modifier.height(10.dp))
            Text("Obunani yangilash", color = Color.White.copy(alpha = 0.55f), fontSize = 13.sp, modifier = Modifier.clickable { scope.launch { AppServices.billing.load(force = true) } }.padding(10.dp))
        }
    }
}

/** Hozir ijro etilayotgan qismning yuklanishi (sozlamalardan boshlangan): "Yuklanmoqda · 720p  12.3%" va ingichka chiziq. Yuklash tugagach yo'qoladi. */
@Composable
private fun CurrentDownloadStrip(ep: Episode) {
    val all by AppServices.downloads.stats.collectAsStateWithLifecycle()
    var q = ""
    var ratio = 0f
    ep.qualities.forEach { name ->
        val st = all[ep.urls[name].orEmpty()] ?: return@forEach
        if ((st.downloading || st.queued) && !st.complete && (q.isEmpty() || st.ratio >= ratio)) { q = name; ratio = st.ratio }
    }
    if (q.isEmpty()) return
    Column(Modifier.padding(start = 18.dp, end = 18.dp, top = 6.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text("Yuklanmoqda · ${ep.qualityTitle(q)}", color = AruColors.Accent2, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
            Text(String.format(java.util.Locale.US, "%.1f%%", ratio * 100), color = AruColors.Accent2, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
        }
        Spacer(Modifier.height(4.dp))
        Box(Modifier.fillMaxWidth().height(3.dp).clip(RoundedCornerShape(2.dp)).background(Color.White.copy(alpha = 0.12f))) {
            Box(Modifier.fillMaxWidth(ratio.coerceIn(0f, 1f)).fillMaxHeight().background(Brush.horizontalGradient(listOf(AruColors.Accent, AruColors.Accent2))))
        }
    }
}
