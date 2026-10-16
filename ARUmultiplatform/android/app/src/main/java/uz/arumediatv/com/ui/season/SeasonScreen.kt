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
) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val repo = AppServices.seasonData
    val billing by AppServices.billing.state.collectAsStateWithLifecycle()
    val canWatch = billing.active || season.free == true

    val player = remember { AruPlayer(ctx) }
    var episodes by remember { mutableStateOf<List<Episode>>(emptyList()) }
    var loadingEps by remember { mutableStateOf(true) }
    var seasons by remember { mutableStateOf<List<Season>>(emptyList()) }
    var info by remember { mutableStateOf<SeasonInfo?>(null) }
    var current by remember { mutableStateOf<Episode?>(null) }
    var currentUrl by remember { mutableStateOf("") }
    var viaTelegram by remember { mutableStateOf(false) }
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
                player.error = "Videoni ochib bo'lmadi"
                return@launch
            }
            viaTelegram = src.viaTelegram
            player.open(src.uri, start, resumePlaying)
        }
    }

    // ── Yuklash ──
    LaunchedEffect(season.key) {
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
        val start = list.firstOrNull { it.id == last?.epizodId } ?: list.firstOrNull()
        if (start != null && current == null) play(start, resumePlaying = false)
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
        if (player.error != null && viaTelegram && !workerRetried && ep != null) {
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

    Column(Modifier.fillMaxSize().background(AruColors.Bg)) {
        // ── Video ──
        val box = if (fullscreen) Modifier.fillMaxSize() else Modifier.fillMaxWidth().aspectRatio(16f / 9f)
        Box(box.background(Color.Black)) {
            if (!canWatch) {
                SubRequired(onBack = onBack, onBilling = onOpenBilling)
            } else {
                PlayerSurface(player, Modifier.fillMaxSize())
                PlayerControls(
                    player = player,
                    title = season.name,
                    subtitle = listOfNotNull(
                        ep?.let { "${it.number}-qism" },
                        ep?.qualityOf(currentUrl)?.takeIf { it.isNotEmpty() }?.let { q -> ep.qualityTitle(q) },
                        if (current != null) (if (viaTelegram) "Telegram" else "Server") else null,
                    ).joinToString(" · "),
                    fullscreen = fullscreen,
                    locked = locked,
                    hasPrev = idx > 0,
                    hasNext = idx >= 0 && idx + 1 < sorted.size,
                    introActive = introActive,
                    onSkipIntro = { ep?.intro?.firstOrNull { player.positionMs in it }?.let { player.seekTo(it.last) } },
                    onPrev = { if (idx > 0) play(sorted[idx - 1]) },
                    onNext = { if (idx >= 0 && idx + 1 < sorted.size) play(sorted[idx + 1]) },
                    onBack = { if (fullscreen) fullscreen = false else onBack() },
                    onToggleFullscreen = { fullscreen = !fullscreen },
                    onSettings = { showSettings = true },
                    onToggleLock = { locked = !locked },
                    onRetry = { ep?.let { e -> play(e, resumeAt = player.positionMs, preferWorker = !viaTelegram) } },
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }

        if (!fullscreen) {
            // ── Sarlavha + sevimli ──
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(season.name, color = Color.White, fontSize = 18.sp, fontWeight = FontWeight.Bold, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    val sub = buildList {
                        if (season.sectionNo > 0) add("${season.sectionNo}-bo'lim")
                        val c = info?.long("epizod_count")?.toInt()?.takeIf { it > 0 } ?: season.episodes
                        if (c > 0) add("$c ta qism")
                        info?.takeIf { it.ratingCount > 0 }?.let { add("★ ${String.format(java.util.Locale.US, "%.1f", it.rating)}") }
                    }.joinToString(" · ")
                    if (sub.isNotEmpty()) Text(sub, color = AruColors.TextDim, fontSize = 12.sp)
                }
                val fav = info?.isFav == true
                Box(
                    Modifier.size(42.dp).clip(CircleShape).background(if (fav) AruColors.AccentTint else AruColors.Card)
                        .border(1.dp, if (fav) AruColors.Accent.copy(alpha = 0.5f) else AruColors.Border, CircleShape)
                        .clickable {
                            if (!AppServices.auth.isLoggedIn) toast("Sevimlilarga qo'shish uchun hisobga kiring")
                            else info = repo.setFavorite(season, !fav, info ?: SeasonInfo(org.json.JSONObject(), 0.0, 0, false))
                        },
                    contentAlignment = Alignment.Center,
                ) { Icon(Icons.Filled.Favorite, "Sevimli", tint = if (fav) AruColors.Accent2 else Color.White.copy(alpha = 0.4f), modifier = Modifier.size(22.dp)) }
            }

            // ── Tablar ──
            val titles = listOf("Ma'lumot", "Qismlar", "Bo'limlar", "Izohlar")
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                titles.forEachIndexed { i, t ->
                    val active = tab == i
                    val shape = RoundedCornerShape(14.dp)
                    Box(
                        Modifier.weight(1f).clip(shape)
                            .then(if (active) Modifier.background(Brush.linearGradient(listOf(AruColors.Accent, AruColors.Accent2))) else Modifier.background(AruColors.Card).border(1.dp, AruColors.Border, shape))
                            .clickable { tab = i }.padding(vertical = 10.dp),
                        contentAlignment = Alignment.Center,
                    ) { Text(t, color = if (active) Color.White else Color.White.copy(alpha = 0.6f), fontSize = 12.sp, fontWeight = FontWeight.SemiBold, maxLines = 1) }
                }
            }
            Spacer(Modifier.height(10.dp))

            Box(Modifier.weight(1f).fillMaxWidth()) {
                when (tab) {
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
                    else -> CommentsTab(season = season, onOpenProfile = onOpenProfile)
                }
            }
        }
    }

    if (showSettings) {
        PlayerSettingsSheet(
            episode = current, currentUrl = currentUrl,
            speed = player.speed, autoNext = autoNext, autoSkipIntro = autoSkipIntro, sleepEndAt = sleepEndAt,
            onQuality = { q ->
                chosenQuality = q
                current?.let { e -> play(e, quality = q, resumeAt = player.positionMs, resumePlaying = player.playing || !player.ready) }
            },
            onSpeed = { player.setSpeed(it) },
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
