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
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
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
    val gate by AppServices.gate.state.collectAsStateWithLifecycle()
    val needsConsent = !billing.active && season.free == true && gate.needsConsent
    val canWatch = billing.active || (season.free == true && !gate.needsConsent)
    LaunchedEffect(Unit) {
        AppServices.gate.sync()
        AppServices.gate.refresh()
    }
    // Pleyer ochiq paytda orqa fonda kanallarga qo'shilish to'xtaydi (tarmoq faqat videoniki).
    DisposableEffect(Unit) {
        AppServices.gate.busy.incrementAndGet()
        onDispose { AppServices.gate.busy.decrementAndGet() }
    }

    val player = remember { AruPlayer(ctx) }
    var episodes by remember { mutableStateOf<List<Episode>>(emptyList()) }
    var loadingEps by remember { mutableStateOf(true) }
    var seasons by remember { mutableStateOf<List<Season>>(emptyList()) }
    var info by remember { mutableStateOf<SeasonInfo?>(null) }
    var current by remember { mutableStateOf<Episode?>(null) }
    var currentUrl by remember { mutableStateOf("") }
    var viaTelegram by remember { mutableStateOf(false) }
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
    fun flushHistory() {
        val d = draft ?: return
        if (d.durationMs <= 0 || d.positionMs < 1000) return
        val stamp = "${d.positionMs}/${d.watchedMs}/${d.quality}/${d.newView}"
        if (stamp == d.lastStamp) return
        d.lastStamp = stamp
        AppServices.sync.putHistory(d.animeId, d.seasonId, d.epId, d.url, d.quality, d.positionMs, d.durationMs, d.watchedMs, d.newView)
        d.newView = false
    }

    fun saveProgress() {
        if (currentUrl.isNotEmpty() && player.durationMs > 0) AppServices.progress.save(currentUrl, player.positionMs, player.durationMs)
    }

    fun play(ep: Episode, quality: String? = null, resumeAt: Long? = null, resumePlaying: Boolean = true, preferWorker: Boolean = false) {
        scope.launch {
            val hist = AppServices.library.state.value.history.firstOrNull { it.animeId == season.animeId && it.seasonId == season.seasonId && it.epizodId == ep.id }
            val q = quality ?: chosenQuality ?: hist?.quality?.takeIf { ep.urls.containsKey(it) }
            val url = ep.urlFor(q)
            if (url.isEmpty()) {
                toast("Bu qismda video yo'q")
                return@launch
            }
            if (!preferWorker) {
                saveProgress()
                flushHistory()
                workerRetried = false
                draft = HistoryDraft(season.animeId, season.seasonId, ep.id, url, ep.qualityOf(url), hist?.watchedMs ?: 0)
                skippedRanges.clear()
            }
            current = ep
            currentUrl = url
            player.error = null
            val start = resumeAt ?: AppServices.progress.positionOf(url)
            val src = PlaySource.resolve(url, AppServices.telegram, preferWorker)
            if (src == null) {
                player.error = if (!uz.arumediatv.com.data.Net.isOnline(ctx)) "Bu qism yuklab olinmagan — internetga ulaning" else "Videoni ochib bo'lmadi"
                return@launch
            }
            viaTelegram = src.viaTelegram
            viaLocal = src.local
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
            if (++n % 10 == 0) saveProgress()
            if (n % 120 == 0) flushHistory()
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
        val act = ctx.findActivity()
        val w = act?.window
        if (fullscreen && act != null && w != null) {
            act.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
            WindowCompat.getInsetsController(w, w.decorView).apply {
                hide(WindowInsetsCompat.Type.systemBars())
                systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            }
        }
        onDispose {
            if (act != null && w != null) {
                act.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
                WindowCompat.getInsetsController(w, w.decorView).show(WindowInsetsCompat.Type.systemBars())
            }
        }
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

    if (needsConsent) {
        ConsentScreen(gate.channels.orEmpty(), loading = gate.channels == null, onBack = onBack, onGrant = { AppServices.gate.grant() }, onBilling = onOpenBilling)
    } else if (!canWatch) {
        SubRequiredScreen(onBack = onBack, onBilling = onOpenBilling)
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
                Text(season.name, color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        // ── Video ──
        val box = if (fullscreen) Modifier.fillMaxSize() else Modifier.padding(horizontal = 16.dp).fillMaxWidth().aspectRatio(16f / 9f).clip(RoundedCornerShape(16.dp))
        if (!collapsed) Box(box.background(Color.Black)) {
            if (needsConsent) {
                ConsentRequired(gate.channels.orEmpty(), onBack = onBack, onGrant = { AppServices.gate.grant() })
            } else if (!canWatch) {
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
                    onPrev = { if (idx > 0) play(sorted[idx - 1]) },
                    onNext = { if (idx >= 0 && idx + 1 < sorted.size) play(sorted[idx + 1]) },
                    onBack = { if (fullscreen) fullscreen = false else onBack() },
                    onToggleFullscreen = { fullscreen = !fullscreen },
                    onSettings = { showSettings = true },
                    onQuality = { showSettings = true },
                    onEpisodeList = { showEpisodeList = true },
                    onToggleLock = { locked = !locked },
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
                    Text("${season.sectionNo.takeIf { it > 0 } ?: 1}-bo'lim · ${ep.number}-qism", color = Color.White, fontSize = 11.5.sp, fontWeight = FontWeight.Bold)
                    NowStat(AruIcons.Visibility, formatCount(ep.views))
                    NowStat(AruIcons.Schedule, formatHours(ep.watchMs))
                    if (ep.addedAt > 0) NowStat(AruIcons.EventAvailable, formatMomentYmd(ep.addedAt))
                }
            } else Spacer(Modifier.height(8.dp))
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

            // ── Qism almashtirish: [⏮]  N-qism  [⏭] ──
            Row(
                Modifier.padding(horizontal = 16.dp).fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(AruColors.Card).border(1.dp, AruColors.Border, RoundedCornerShape(14.dp)).padding(horizontal = 6.dp, vertical = 5.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                NavBtn(AruIcons.SkipPrevious, idx > 0) { if (idx > 0) play(sorted[idx - 1]) }
                Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                    Text(
                        if (idx >= 0) "${sorted[idx].number}-qism" else if (sorted.isEmpty()) "—" else "Qismni tanlang",
                        color = if (idx >= 0) Color.White else Color.White.copy(alpha = 0.54f), fontSize = 16.sp, fontWeight = FontWeight.Bold, maxLines = 1,
                    )
                }
                NavBtn(AruIcons.SkipNext, idx >= 0 && idx + 1 < sorted.size) { if (idx >= 0 && idx + 1 < sorted.size) play(sorted[idx + 1]) }
            }
            Spacer(Modifier.height(6.dp))
            }

            androidx.compose.foundation.pager.HorizontalPager(pager, Modifier.weight(1f).fillMaxWidth(), beyondViewportPageCount = 1) { page ->
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
                    1 -> EpisodesTab(
                        canWatch = canWatch, loading = loadingEps, episodes = episodes,
                        currentId = current?.id, currentQuality = chosenQuality,
                        onPlay = { e, q ->
                            if (q != null) chosenQuality = q
                            play(e, quality = q)
                        },
                        onSubscribe = onOpenBilling,
                    )
                    2 -> SeasonsTab(seasons = seasons, current = season, onOpen = onOpenSeason)
                    else -> CommentsTab(season = season, onOpenProfile = onOpenProfile, onLogin = onLogin, onPacks = onPacks, expanded = commentsExpanded, onExpandedChange = { commentsExpanded = it })
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

/** Bepul bo'lim: majburiy kanallarga avtomatik qo'shilishga ruxsat so'raladi (Sozlamalarda o'chirish mumkin). */
@Composable
private fun ConsentRequired(channels: List<uz.arumediatv.com.data.GateChannel>, onBack: () -> Unit, onGrant: () -> Unit) {
    Column(Modifier.fillMaxSize().padding(16.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
        Text("Bepul ko'rish", color = Color.White, fontSize = 18.sp, fontWeight = FontWeight.Bold)
        Spacer(Modifier.height(4.dp))
        Text(
            "Bu bo'lim bepul. Buning uchun ilova Telegram hisobingiz bilan ${if (channels.isEmpty()) "majburiy" else "${channels.size} ta"} kanalga qo'shiladi.",
            color = Color.White.copy(alpha = 0.7f), fontSize = 12.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center,
        )
        Spacer(Modifier.height(12.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(
                "Orqaga", color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
                modifier = Modifier.clip(RoundedCornerShape(12.dp)).background(Color.White.copy(alpha = 0.12f)).clickable(onClick = onBack).padding(horizontal = 18.dp, vertical = 10.dp),
            )
            Text(
                "Ruxsat berish", color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
                modifier = Modifier.clip(RoundedCornerShape(12.dp)).background(AruColors.Accent).clickable(onClick = onGrant).padding(horizontal = 18.dp, vertical = 10.dp),
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
private fun NavBtn(icon: androidx.compose.ui.graphics.vector.ImageVector, enabled: Boolean, onClick: () -> Unit) {
    val shape = RoundedCornerShape(12.dp)
    Box(
        Modifier.width(68.dp).height(38.dp).clip(shape)
            .background(Color.White.copy(alpha = if (enabled) 0.10f else 0.035f))
            .border(1.dp, Color.White.copy(alpha = if (enabled) 0.20f else 0.06f), shape)
            .clickable(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { Icon(icon, null, tint = Color.White.copy(alpha = if (enabled) 0.95f else 0.22f), modifier = Modifier.size(24.dp)) }
}

/** "Obuna kerak" to'liq ekrani (Flutter `_SubRequiredScreen`). */
@Composable
private fun SubRequiredScreen(onBack: () -> Unit, onBilling: () -> Unit) {
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
            Text("Obuna kerak", color = Color.White, fontSize = 22.sp, fontWeight = FontWeight.ExtraBold, textAlign = androidx.compose.ui.text.style.TextAlign.Center)
            Spacer(Modifier.height(10.dp))
            Text(
                "Anime ko'rish uchun obuna bo'lishi kerak. Tariflar 1 kundan 30 kungacha — profil sahifasidan yoki quyidagi tugmadan tanlang.",
                color = Color.White.copy(alpha = 0.62f), fontSize = 13.5.sp, lineHeight = 21.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
            Spacer(Modifier.height(24.dp))
            Box(Modifier.fillMaxWidth().height(48.dp).clip(RoundedCornerShape(14.dp)).background(AruColors.Accent).clickable(onClick = onBilling), contentAlignment = Alignment.Center) {
                Text("Obuna olish", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.ExtraBold)
            }
            Spacer(Modifier.height(10.dp))
            Text("Obunani yangilash", color = Color.White.copy(alpha = 0.55f), fontSize = 13.sp, modifier = Modifier.clickable { AppServices.billing.load(force = true) }.padding(10.dp))
        }
    }
}

/** Bepul bo'lim: majburiy kanallarga avtomatik qo'shilish ruxsati (Flutter `_ChannelConsentScreen`). */
@Composable
private fun ConsentScreen(channels: List<uz.arumediatv.com.data.GateChannel>, loading: Boolean, onBack: () -> Unit, onGrant: () -> Unit, onBilling: () -> Unit) {
    val muted = Color.White.copy(alpha = 0.62f)
    Column(Modifier.fillMaxSize().background(AruColors.Bg).statusBarsPadding()) {
        Box(Modifier.padding(start = 12.dp, top = 8.dp).clip(CircleShape).clickable(onClick = onBack).padding(10.dp)) {
            Icon(Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", tint = Color.White, modifier = Modifier.size(24.dp))
        }
        Column(Modifier.weight(1f).fillMaxWidth().verticalScroll(androidx.compose.foundation.rememberScrollState()).padding(horizontal = 24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
            Icon(AruIcons.Campaign, null, tint = AruColors.Accent, modifier = Modifier.size(64.dp))
            Spacer(Modifier.height(18.dp))
            Text("Bepul ko'rish", color = Color.White, fontSize = 22.sp, fontWeight = FontWeight.ExtraBold)
            Spacer(Modifier.height(10.dp))
            Text(
                "Bu bo'limni bepul ko'rish uchun quyidagi kanallarga obuna bo'lishingiz kerak. Ruxsat bersangiz, ilova Telegram hisobingiz bilan ularga o'zi qo'shiladi (yopiq kanalga so'rov yuboradi). Kanaldan istalgan vaqtda Telegram'da chiqib ketishingiz mumkin.",
                color = muted, fontSize = 13.5.sp, lineHeight = 21.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
            Spacer(Modifier.height(16.dp))
            if (loading) androidx.compose.material3.CircularProgressIndicator(color = AruColors.Accent, modifier = Modifier.padding(12.dp))
            channels.forEach { c ->
                Row(Modifier.fillMaxWidth().padding(bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                    Icon(if (c.kind == "public") AruIcons.Campaign else Icons.Filled.Lock, null, tint = muted, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text(c.title.ifEmpty { c.url }, color = Color.White, fontSize = 14.sp)
                }
            }
            Spacer(Modifier.height(20.dp))
            Box(Modifier.fillMaxWidth().height(48.dp).clip(RoundedCornerShape(14.dp)).background(AruColors.Accent).clickable(onClick = onGrant), contentAlignment = Alignment.Center) {
                Text("Ruxsat berish va ko'rish", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.ExtraBold)
            }
            Spacer(Modifier.height(10.dp))
            Text("Kanalsiz — obuna olish", color = muted, fontSize = 13.sp, modifier = Modifier.clickable(onClick = onBilling).padding(10.dp))
        }
    }
}
