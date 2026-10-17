package uz.arumediatv.com

import android.content.Context
import uz.arumediatv.com.core.RustCore
import uz.arumediatv.com.core.SecureStore
import uz.arumediatv.com.data.AuthService
import uz.arumediatv.com.data.ImageStore
import uz.arumediatv.com.data.LibraryStore
import uz.arumediatv.com.data.MyStatsStore
import uz.arumediatv.com.data.SyncQueue
import uz.arumediatv.com.data.BillingStore
import uz.arumediatv.com.data.SeasonRepository
import uz.arumediatv.com.data.WatchProgress
import uz.arumediatv.com.data.PublicRepository
import uz.arumediatv.com.data.VideoCache
import uz.arumediatv.com.data.UnreadBadge
import uz.arumediatv.com.data.TrafficMeter
import uz.arumediatv.com.data.ChannelGate
import uz.arumediatv.com.data.PackStore
import uz.arumediatv.com.data.AdminApi
import uz.arumediatv.com.data.DownloadManager
import uz.arumediatv.com.data.DownloadsIndex
import uz.arumediatv.com.data.SeasonsRepository
import uz.arumediatv.com.data.SeasonsStore
import uz.arumediatv.com.data.TelegramService

/** Ilova bo'ylab yagona xizmatlar (qo'lda "dependency injection"): `AruApplication` yaratadi. */
object AppServices {
    lateinit var secure: SecureStore
        private set
    lateinit var auth: AuthService
        private set
    lateinit var telegram: TelegramService
        private set
    lateinit var images: ImageStore
    lateinit var thumbs: uz.arumediatv.com.data.ThumbStore
    lateinit var offlineLib: uz.arumediatv.com.data.OfflineLibrary
        private set
    lateinit var seasons: SeasonsStore
        private set
    lateinit var library: LibraryStore
        private set
    lateinit var sync: SyncQueue
        private set
    lateinit var billing: BillingStore
        private set
    lateinit var seasonData: SeasonRepository
        private set
    lateinit var progress: WatchProgress
        private set
    lateinit var publicData: PublicRepository
        private set
    lateinit var videoCache: VideoCache
        private set
    lateinit var unread: UnreadBadge
        private set
    lateinit var traffic: TrafficMeter
        private set
    lateinit var gate: ChannelGate
        private set
    lateinit var packs: PackStore
        private set
    lateinit var admin: AdminApi
        private set

    fun trafficOrNull(): TrafficMeter? = if (::traffic.isInitialized) traffic else null
    lateinit var downloads: DownloadManager
        private set
    lateinit var downloadsIndex: DownloadsIndex
        private set
    lateinit var appContext: Context
        private set
    lateinit var myStats: MyStatsStore
        private set

    @Volatile
    var ready = false
        private set

    /** Asosiy kalit (shifrlash) Keystore ichida saqlanadi; Rust yadrosiga ilova ochilganda beriladi. */
    fun init(app: Context) {
        if (ready) return
        val ctx = app.applicationContext
        uz.arumediatv.com.core.Perf.init(ctx)
        secure = SecureStore(ctx)
        if (RustCore.load()) {
            RustCore.setAppVersion(BuildConfig.APP_VERSION)
            // Kalit HECH QACHON ustidan yozilmaydi: o'qib bo'lmasa (Keystore vaqtincha ishlamadi) yangisi yaratilmaydi —
            // aks holda Telegram sessiyasi va boshqa shifrlangan fayllar butunlay yo'qolardi.
            var hex = secure.getString("master_key_v1")
            if ((hex == null || hex.length != 64) && !secure.has("master_key_v1")) {
                hex = RustCore.generateMasterKey()
                if (hex.length == 64 && !secure.putString("master_key_v1", hex)) hex = ""
            }
            if (hex != null && hex.length == 64) RustCore.setMasterKey(hex)
        }
        auth = AuthService(secure)
        telegram = TelegramService(ctx, auth, secure)
        images = ImageStore(ctx, telegram)
        seasons = SeasonsStore(SeasonsRepository(ctx))
        library = LibraryStore(auth)
        sync = SyncQueue(ctx, auth)
        myStats = MyStatsStore(auth)
        billing = BillingStore(ctx, auth)
        seasonData = SeasonRepository(ctx, auth, { sync }, { library })
        progress = WatchProgress(ctx)
        publicData = PublicRepository(ctx, auth)
        videoCache = VideoCache(ctx)
        unread = UnreadBadge(auth)
        traffic = TrafficMeter(ctx, auth)
        gate = ChannelGate(ctx, auth, telegram)
        thumbs = uz.arumediatv.com.data.ThumbStore(ctx, telegram, videoCache)
        packs = PackStore(ctx, auth, telegram, { sync })
        admin = AdminApi(auth)
        sync.trafficPending = { traffic.pendingBytes }
        sync.onTrafficReported = { traffic.markReported(it) }
        traffic.onNeedFlush = { sync.flushAsync(force = true) }
        traffic.start()
        downloads = DownloadManager(ctx, telegram, videoCache)
        downloadsIndex = DownloadsIndex(ctx, downloads)
        offlineLib = uz.arumediatv.com.data.OfflineLibrary(downloadsIndex)
        appContext = ctx
        auth.onSettings = { hidden, consent -> sync.putSettings(hidden, consent) }
        auth.hasPendingSettings = { sync.hasPendingSettings }
        ready = true
    }
}
