// android-template/MainActivity.kt
//
// ═══════════════════════════════════════════════════════════════
//  TOMOSHA TARIXI UCHUN KADR AJRATUVCHI
// ═══════════════════════════════════════════════════════════════
//
// Videodan kadr olishni Flutter uddalay olmaydi — rasm GPU
// qatlamida chiziladi va uni "surat qilib olish" mumkin emas.
// Android'da esa buning uchun tayyor vosita bor:
// `MediaMetadataRetriever`.
//
// ── NEGA TASHQI PAKET EMAS ─────────────────────────────────────
//
// `video_thumbnail` paketi sinab ko'rildi va build'ni YIQITDI:
// u 2023-yildan beri yangilanmagan, Gradle faylida allaqachon
// yopilgan `jcenter()` ombori va eski DSL turibdi. Uni har
// Flutter/Gradle yangilanishida "tuzatib" yurish — qarz.
//
// Kerakli ish esa atigi bir necha qator, shu sabab u ilovaning
// O'ZIDA yozilgan. Tashqi bog'liqlik yo'q, Gradle xavfi yo'q.
//
// ── BU FAYL QAYERGA BORADI ─────────────────────────────────────
//
// `android/` papkasi `.gitignore` da — uni CI `flutter create`
// bilan har safar qaytadan yaratadi. Shu sabab bu fayl shablon
// sifatida repoda turadi va CI uni `MainActivity.kt` ustiga
// ko'chiradi, `__PKG__` o'rniga haqiqiy paket nomini qo'yib
// (build-flutter-apk.yml ga qarang).
//
// Kadr manzili — Rust yadrosidagi "/thumb" yo'li: u butun videoni
// emas, faqat kerakli kadrni beradi (rust/src/mp4.rs).

package __PKG__

import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.media.MediaMetadataRetriever
import android.os.Build
import android.util.Base64
import android.view.WindowManager
import java.security.MessageDigest
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import java.io.ByteArrayOutputStream

class MainActivity : FlutterActivity() {
    // ── TIZIM TUGMALARI ORQASIDA QORA PANEL BO'LMASIN ─────────────
    //
    // TALAB (foydalanuvchi): "ilovaning pastida telefon tugmalari
    // orqasida qora oyna bo'lmasin". Ilova tizim panellari ostigacha
    // chiziladi (edge-to-edge) va Android 10+ ning 3 tugmali navigatsiya
    // ortiga qo'yadigan qoramtir "kontrast" pardasi o'chiriladi — ortida
    // ilovaning o'zi ko'rinadi.
    //
    // TOPILGAN XATO (foydalanuvchi: "telefon tugmasi orqasidagi qora
    // oyna hali ham bor"): oyna tizim panellari ostiga faqat Android 11+
    // da (`setDecorFitsSystemWindows`) cho'zilardi. Android 10 va undan
    // eski (MIUI 12 li Redmi'lar) da ilova tugmalar ustida TUGAR, pastda
    // esa tizimning qora foni qolardi. Bundan tashqari MIUI oyna fokusni
    // qaytarib olganda (pleyer, boshqa ilovadan qaytish) bayroqlarni
    // tiklab yuboradi. Endi:
    //   * eski Android'da ham `LAYOUT_HIDE_NAVIGATION`/`LAYOUT_FULLSCREEN`;
    //   * `FLAG_TRANSLUCENT_NAVIGATION` olib tashlanadi,
    //     `FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS` qo'yiladi (rang ishlashi uchun);
    //   * hammasi `onResume` va fokus qaytganda QAYTA qo'llanadi.
    override fun onCreate(savedInstanceState: android.os.Bundle?) {
        // ── VIDEONING CHETIDAGI YASHIL CHIZIQ ──────────────────────
        //
        // Foydalanuvchi: "bo'yiga cho'zilgan GIFda o'ng tomonda yashil
        // chiziq paydo bo'lyapti". Sabab: Android 10+ da Flutter video
        // yuzasini `ImageReader` orqali beradi va u dekoderning KESISH
        // (crop) ma'lumotini e'tiborsiz qoldiradi
        // (`handlesCropAndRotation() == false`). Apparat dekoder kadr enini
        // 32/64 ga yaxlitlab chiqaradi (masalan 360 -> 384) — ortiqcha
        // bo'sh joy yashil bo'lib, kadr esa siqilib ko'rinadi.
        // `SurfaceTexture` yo'li kesish va burishni o'zgartirish matritsasi
        // bilan to'g'ri qo'llaydi. Ilova Skia (OpenGL) da ishlaydi
        // (Impeller o'chirilgan, build-flutter-apk.yml), ya'ni bu yo'l
        // xavfsiz. Birinchi video yuzasi yaratilishidan OLDIN qo'yiladi.
        io.flutter.embedding.engine.renderer.FlutterRenderer
            .debugForceSurfaceProducerGlTextures = true
        super.onCreate(savedInstanceState)
        applyEdgeToEdge()
    }

    override fun onResume() {
        super.onResume()
        applyEdgeToEdge()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) applyEdgeToEdge()
    }

    private fun applyEdgeToEdge() {
        window.clearFlags(WindowManager.LayoutParams.FLAG_TRANSLUCENT_NAVIGATION)
        window.clearFlags(WindowManager.LayoutParams.FLAG_TRANSLUCENT_STATUS)
        window.addFlags(WindowManager.LayoutParams.FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            window.setDecorFitsSystemWindows(false)
        } else {
            @Suppress("DEPRECATION")
            val flags = android.view.View.SYSTEM_UI_FLAG_LAYOUT_STABLE or
                android.view.View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION or
                android.view.View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
            @Suppress("DEPRECATION")
            val cur = window.decorView.systemUiVisibility
            // Pleyerning to'liq ekran (yashirish) bayroqlariga tegilmaydi.
            @Suppress("DEPRECATION")
            window.decorView.systemUiVisibility = cur or flags
        }
        @Suppress("DEPRECATION")
        window.navigationBarColor = android.graphics.Color.TRANSPARENT
        @Suppress("DEPRECATION")
        window.statusBarColor = android.graphics.Color.TRANSPARENT
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            @Suppress("DEPRECATION")
            window.navigationBarDividerColor = android.graphics.Color.TRANSPARENT
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            window.isNavigationBarContrastEnforced = false
            window.isStatusBarContrastEnforced = false
        }
    }

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "aru/thumb")
            .setMethodCallHandler { call, result ->
                if (call.method == "frames") {
                    // Video tahrirlash oynasi (to'plamga qo'shish): telefondagi
                    // fayldan teng oraliqdagi kichik kadrlar tasmasi. UI oqimida
                    // emas — dekodlash sekin.
                    val path = call.argument<String>("path") ?: ""
                    val count = call.argument<Int>("count") ?: 8
                    val maxWidth = call.argument<Int>("maxWidth") ?: 160
                    val quality = call.argument<Int>("quality") ?: 60
                    Thread {
                        val frames = grabFrames(path, count, maxWidth, quality)
                        runOnUiThread { result.success(frames) }
                    }.start()
                } else if (call.method != "grab") {
                    result.notImplemented()
                } else {
                    val url = call.argument<String>("url") ?: ""
                    val maxWidth = call.argument<Int>("maxWidth") ?: 640
                    val quality = call.argument<Int>("quality") ?: 72
                    // Manzilda HAR DOIM bitta kadrlik bo'lak turadi
                    // (Rust yadrosining "/thumb" yo'li). Ya'ni
                    // "qaysi kadr" degan savol yo'q — bo'lakda
                    // bittagina kadr bor.
                    //
                    // Dekodlash bir necha yuz millisekund olishi
                    // mumkin — UI oqimida bajarilmaydi, aks holda
                    // ro'yxat sirg'alayotganda ilova qotib qolardi.
                    Thread {
                        val bytes = grabFrame(url, maxWidth, quality)
                        runOnUiThread { result.success(bytes) }
                    }.start()
                }
            }

        // ── ILOVANING HAQIQIYLIGINI TASDIQLASH ────────────────
        //
        // TALAB (foydalanuvchi): "APP_KEY nimaga kerak? ... busiz
        // ishlaydigan qilish kerak, ya'ni worker ilovaning
        // haqiqiyligini tekshirishi kerak".
        //
        // ── NEGA IMZO ─────────────────────────────────────────
        //
        // APK'ga qo'yilgan sir (kalit) — shunchaki APK ichidagi
        // matn: uni ochib o'qish mumkin va u hech narsani
        // isbotlamaydi.
        //
        // Imzo sertifikati esa boshqacha: uni ILOVA O'ZI
        // tanlamaydi, TIZIM beradi. Kimdir ilovani o'zgartirib
        // qayta yig'sa, uni O'Z kaliti bilan imzolashga majbur —
        // bizning kalitimiz unda yo'q. Natijada hash boshqacha
        // chiqadi va server bunday ilovani rad etadi.
        //
        // ── ROSTINI AYTISH KERAK ──────────────────────────────
        //
        // Hash'ning O'ZINI APK'dan o'qib, so'rovni qo'lda yasash
        // mumkin. Ya'ni bu:
        //   * O'ZGARTIRILGAN ILOVANI to'xtatadi (asosiy maqsad);
        //   * brauzer, bot va oddiy skriptlarni to'xtatadi;
        //   * maqsadli hujumchini to'xtatmaydi.
        // Mutlaq yechim (Play Integrity) Play Store'ni talab
        // qiladi, bu ilova esa APK bo'lib tarqatiladi.
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "aru/signature")
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "sha256" -> result.success(signatureSha256())
                    // Telegram'ga o'zini tanitish uchun (`initConnection`):
                    // "Xiaomi Redmi Note 9", "10".
                    "device" -> result.success(mapOf(
                        "model" to "${android.os.Build.MANUFACTURER} ${android.os.Build.MODEL}".trim(),
                        "system" to (android.os.Build.VERSION.RELEASE ?: ""),
                    ))
                    else -> result.notImplemented()
                }
            }

        // ── TIZIM TUGMALARI BALANDLIGI (`lib/services/nav_inset.dart`) ──
        //
        // "Barqaror" qiymat: tugmalar vaqtincha yashirilgan (pleyer to'liq
        // ekran) paytda ham o'zgarmaydi. MIUI ba'zan Flutter'ga 0 beradi —
        // ilova buni zaxira sifatida ishlatadi. Fizik pikselda.
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "aru/insets")
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "navBottom" -> result.success(navBottomPx())
                    else -> result.notImplemented()
                }
            }

        // ── QURILMA XOTIRASI ("Xotiradan foydalanish" oynasi) ──
        //
        // Telegram'dagi `CacheControlActivity` sarlavhasi: "ilova
        // qurilma xotirasining N% ini egallaydi" va uning ostidagi
        // chiziq (ilova / boshqa ilovalar / bo'sh joy). Ilova
        // ma'lumotlari turgan bo'lim o'lchanadi.
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "aru/storage")
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "stats" -> try {
                        val fs = android.os.StatFs(filesDir.absolutePath)
                        result.success(mapOf(
                            "total" to fs.totalBytes,
                            "free" to fs.availableBytes,
                        ))
                    } catch (e: Exception) {
                        result.error("stats", e.message, null)
                    }
                    else -> result.notImplemented()
                }
            }

        // ── FAYL TANLASH: "BARCHA FAYLLARGA RUXSAT" ────────────
        //
        // Telegram uslubidagi fayl tanlagich (lib/widgets/tg_file_browser.dart)
        // jildlarni o'zi ko'rsatadi. Android 11+ da APK, ZIP kabi
        // media bo'lmagan fayllarni ko'rish uchun shu ruxsat kerak.
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "aru/files")
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "hasAll" -> try {
                        val ok = if (Build.VERSION.SDK_INT >= 30) {
                            android.os.Environment.isExternalStorageManager()
                        } else {
                            checkSelfPermission(
                                android.Manifest.permission.READ_EXTERNAL_STORAGE
                            ) == PackageManager.PERMISSION_GRANTED
                        }
                        result.success(ok)
                    } catch (e: Throwable) {
                        result.success(true)
                    }
                    "requestAll" -> try {
                        if (Build.VERSION.SDK_INT >= 30) {
                            val i = android.content.Intent(
                                android.provider.Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                                android.net.Uri.parse("package:" + packageName)
                            )
                            startActivity(i)
                        } else {
                            requestPermissions(
                                arrayOf(android.Manifest.permission.READ_EXTERNAL_STORAGE),
                                9101
                            )
                        }
                        result.success(null)
                    } catch (e: Throwable) {
                        result.error("request", e.message, null)
                    }
                    else -> result.notImplemented()
                }
            }

        // ── SKRINSHOT VA EKRAN YOZIB OLISHNI TAQIQLASH ─────────
        //
        // TALAB (foydalanuvchi): "ilovada video pleyerda va shaxsiy
        // chatda screenshot olish va ekranni yozib olish
        // taqiqlansin".
        //
        // Android'da buning yagona ishonchli yo'li — oynaga
        // `FLAG_SECURE` qo'yish. U bir vaqtning o'zida:
        //   * skrinshotni bloklaydi (tizim "ruxsat yo'q" deydi);
        //   * ekran yozuvida va ekranni uzatishda oynani QORA
        //     qilib ko'rsatadi;
        //   * oxirgi ilovalar ro'yxatida ham tarkibni yashiradi.
        //
        // Bayroq BUTUN oynaga tegishli, ya'ni u faqat kerakli
        // ekran ochilganda yoqiladi va yopilganda o'chiriladi —
        // aks holda butun ilovada skrinshot ishlamay qolardi.
        //
        // NEGA TASHQI PAKET EMAS: bu atigi ikki qator kod.
        // `flutter_windowmanager` esa uzoq vaqtdan beri
        // yangilanmagan va Gradle yangilanishlarida yiqiladi
        // (yuqoridagi `video_thumbnail` bilan bir xil dard).
        // ── GALEREYA: ZAXIRA YO'L (MediaStore'dan to'g'ridan-to'g'ri) ──
        //
        // Foydalanuvchi: biriktirish oynasida "Galereyada rasm yoki video
        // topilmadi (ruxsat: authorized, albomlar: 10)". Albomlar topiladi,
        // lekin `photo_manager` elementlarni o'qiy olmaydi: u har qatorni
        // `File(path).exists()` bilan tekshiradi va barcha ustunlarni
        // (`orientation`, `datetaken`, ...) majburiy o'qiydi — ba'zi
        // telefonlarda (MIUI, scoped storage) shu yerda hammasi tushib qoladi.
        // Bu kanal faqat kerakli ustunlarni, `LIMIT`siz (kursorni o'zimiz
        // suramiz) o'qiydi; kichik rasm va fayl nusxasi `content://` orqali.
        // Ilova faqat `photo_manager` bo'sh qaytarganda shu yerga o'tadi.
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "aru/gallery")
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "list" -> {
                        val offset = call.argument<Int>("offset") ?: 0
                        val limit = call.argument<Int>("limit") ?: 60
                        val bucket = call.argument<String>("bucket")
                        Thread {
                            try {
                                val list = galleryList(offset, limit, bucket)
                                runOnUiThread { result.success(list) }
                            } catch (e: Throwable) {
                                runOnUiThread { result.error("list", e.toString(), null) }
                            }
                        }.start()
                    }
                    "thumb" -> {
                        val id = (call.argument<Any>("id") ?: "0").toString().toLongOrNull() ?: 0L
                        val video = call.argument<Boolean>("video") ?: false
                        val size = call.argument<Int>("size") ?: 300
                        Thread {
                            val bytes = try { galleryThumb(id, video, size) } catch (e: Throwable) { null }
                            runOnUiThread { result.success(bytes) }
                        }.start()
                    }
                    "copy" -> {
                        val id = (call.argument<Any>("id") ?: "0").toString().toLongOrNull() ?: 0L
                        val video = call.argument<Boolean>("video") ?: false
                        val name = call.argument<String>("name") ?: "file"
                        Thread {
                            try {
                                val path = galleryCopy(id, video, name)
                                runOnUiThread { result.success(path) }
                            } catch (e: Throwable) {
                                runOnUiThread { result.error("copy", e.toString(), null) }
                            }
                        }.start()
                    }
                    else -> result.notImplemented()
                }
            }

        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "aru/secure")
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "on" -> {
                        runOnUiThread {
                            window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
                        }
                        result.success(true)
                    }
                    "off" -> {
                        runOnUiThread {
                            window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
                        }
                        result.success(true)
                    }
                    else -> result.notImplemented()
                }
            }

        // ── TRAFIK KANALI OLIB TASHLANGAN ──────────────────────
        //
        // Bu yerda ilgari `TrafficStats.getUidRxBytes` bor edi.
        // U ilovaning UID'i ostidagi HAMMA soketni sanardi —
        // shu jumladan MAHALLIY (`127.0.0.1`) uzatmani ham:
        // pleyer videoni ilovaning o'z kesh-serveridan oladi,
        // ya'ni bitta video IKKI MARTA sanalardi, yuklab olingan
        // videoni oflayn qayta ko'rganda esa trafik yo'q joydan
        // o'sardi.
        //
        // Hisob endi AYNAN tarmoqqa chiqadigan ikki joyda olinadi
        // (Rust yadrosi va ilovaning http klienti) —
        // `lib/services/traffic_service.dart` ga qarang.
        //
        // Keyinroq shu yerda telefon xotirasini o'qiydigan
        // `aru/storage` kanali ham bor edi; profil sahifasidan
        // "telefon xotirasi N% band" qatori olib tashlangach
        // (foydalanuvchi talabi) u ham keraksiz bo'lib qoldi.
    }

    // ── GALEREYA YORDAMCHILARI ("aru/gallery") ──────────────────

    private fun mediaUri(id: Long, video: Boolean): android.net.Uri =
        android.content.ContentUris.withAppendedId(
            if (video) android.provider.MediaStore.Video.Media.EXTERNAL_CONTENT_URI
            else android.provider.MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
            id
        )

    /// Rasm va videolar, yangisi birinchi. Ustun bo'lmasa (eski/o'zgartirilgan
    /// MediaStore) — shu maydon shunchaki 0 bo'ladi, qator tashlanmaydi.
    private fun galleryList(offset: Int, limit: Int, bucket: String?): List<Map<String, Any?>> {
        val uri = android.provider.MediaStore.Files.getContentUri("external")
        val type = android.provider.MediaStore.Files.FileColumns.MEDIA_TYPE
        val image = android.provider.MediaStore.Files.FileColumns.MEDIA_TYPE_IMAGE
        val video = android.provider.MediaStore.Files.FileColumns.MEDIA_TYPE_VIDEO
        var sel = "($type=$image OR $type=$video)"
        val args = ArrayList<String>()
        if (!bucket.isNullOrEmpty()) {
            sel += " AND bucket_id=?"
            args.add(bucket)
        }
        val full = arrayOf("_id", type, "mime_type", "date_added", "width", "height",
            "duration", "_display_name")
        val cursor = try {
            contentResolver.query(uri, full, sel, args.toTypedArray(), "date_added DESC")
        } catch (e: Throwable) {
            // `duration` ustuni yo'q MediaStore — usiz.
            contentResolver.query(uri, full.filter { it != "duration" }.toTypedArray(),
                sel, args.toTypedArray(), "date_added DESC")
        }
        val out = ArrayList<Map<String, Any?>>()
        cursor?.use { c ->
            fun col(n: String) = c.getColumnIndex(n)
            val iId = col("_id"); val iType = col(type); val iMime = col("mime_type")
            val iDate = col("date_added"); val iW = col("width"); val iH = col("height")
            val iDur = col("duration"); val iName = col("_display_name")
            if (offset > 0 && !c.moveToPosition(offset - 1)) return out
            while (out.size < limit && c.moveToNext()) {
                try {
                    val isVideo = iType >= 0 && c.getInt(iType) == video
                    out.add(mapOf(
                        "id" to c.getLong(iId).toString(),
                        "video" to isVideo,
                        "mime" to (if (iMime >= 0) c.getString(iMime) else null),
                        "date" to (if (iDate >= 0) c.getLong(iDate) else 0L),
                        "w" to (if (iW >= 0) c.getInt(iW) else 0),
                        "h" to (if (iH >= 0) c.getInt(iH) else 0),
                        "dur" to (if (iDur >= 0 && isVideo) c.getLong(iDur) else 0L),
                        "name" to (if (iName >= 0) c.getString(iName) else null),
                    ))
                } catch (e: Throwable) {
                    // buzuq qator — o'tkazib yuboriladi, qolganlari chiqadi
                }
            }
        }
        return out
    }

    private fun galleryThumb(id: Long, video: Boolean, size: Int): ByteArray? {
        val bmp: Bitmap? = if (Build.VERSION.SDK_INT >= 29) {
            contentResolver.loadThumbnail(mediaUri(id, video), android.util.Size(size, size), null)
        } else if (video) {
            @Suppress("DEPRECATION")
            android.provider.MediaStore.Video.Thumbnails.getThumbnail(
                contentResolver, id, android.provider.MediaStore.Video.Thumbnails.MINI_KIND, null)
        } else {
            @Suppress("DEPRECATION")
            android.provider.MediaStore.Images.Thumbnails.getThumbnail(
                contentResolver, id, android.provider.MediaStore.Images.Thumbnails.MINI_KIND, null)
        }
        if (bmp == null) return null
        val out = ByteArrayOutputStream()
        bmp.compress(Bitmap.CompressFormat.JPEG, 85, out)
        return out.toByteArray()
    }

    /// Tanlangan faylning nusxasi (ilova keshida; yuklash tugagach
    /// `StorageJanitor.dropPicked` o'chiradi).
    private fun galleryCopy(id: Long, video: Boolean, name: String): String {
        val safe = name.replace(Regex("[^A-Za-z0-9._-]"), "_").takeLast(80)
        val dir = java.io.File(cacheDir, "aru_picked")
        dir.mkdirs()
        val dst = java.io.File(dir, "${id}_$safe")
        contentResolver.openInputStream(mediaUri(id, video)).use { input ->
            if (input == null) throw IllegalStateException("fayl ochilmadi")
            java.io.FileOutputStream(dst).use { input.copyTo(it) }
        }
        return dst.absolutePath
    }

    private fun navBottomPx(): Int {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) return 0
        return try {
            val insets = window.decorView.rootWindowInsets ?: return 0
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                insets.getInsetsIgnoringVisibility(
                    android.view.WindowInsets.Type.navigationBars()
                ).bottom
            } else {
                @Suppress("DEPRECATION")
                insets.stableInsetBottom
            }
        } catch (e: Throwable) {
            0
        }
    }

    /// APK imzo sertifikatining SHA-256 hash'i (base64).
    ///
    /// Bir nechta imzo bo'lsa BIRINCHISI olinadi — Flutter
    /// ilovalarida imzo doim bitta.
    ///
    /// Xato bo'lsa bo'sh satr: server bunday holatda so'rovni
    /// rad etadi, lekin ilova yiqilmaydi.
    private fun signatureSha256(): String {
        return try {
            val pm = packageManager
            val sigs = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                @Suppress("DEPRECATION")
                val info = pm.getPackageInfo(
                    packageName, PackageManager.GET_SIGNING_CERTIFICATES
                )
                info.signingInfo?.apkContentsSigners
            } else {
                @Suppress("DEPRECATION")
                val info = pm.getPackageInfo(
                    packageName, PackageManager.GET_SIGNATURES
                )
                @Suppress("DEPRECATION")
                info.signatures
            }
            val first = sigs?.firstOrNull() ?: return ""
            val digest = MessageDigest.getInstance("SHA-256")
                .digest(first.toByteArray())
            Base64.encodeToString(digest, Base64.NO_WRAP)
        } catch (e: Throwable) {
            ""
        }
    }

    /// Manzildagi videodan OXIRGI kadrni JPEG qilib qaytaradi.
    ///
    /// ── NEGA AYNAN OXIRGI KADR ─────────────────────────────────
    ///
    /// Manzilda (`rust/src/video_cache.rs` -> `/thumb`) kalit
    /// kadrdan foydalanuvchi TO'XTAGAN kadrgacha bo'lgan kichik MP4
    /// turadi. Uning eng oxirgi kadri — aynan kerakli kadr.
    ///
    /// Ilgari bu yerda `OPTION_CLOSEST_SYNC` bilan 0-vaqt
    /// so'ralardi, ya'ni har doim KALIT KADR olinardi va rasm
    /// to'xtagan joydan bir necha soniya oldingi bo'lib chiqardi.
    /// Endi davomiylik o'qilib, uning oxiriga `OPTION_CLOSEST`
    /// bilan boriladi — bu dekoderni kalit kadrdan boshlab
    /// kerakli kadrgacha ochishga majbur qiladi.
    ///
    /// Kadr olinmasa `null` — bu XATO EMAS, oddiy zaxira yo'l:
    /// ilova o'shanda posterni ko'rsatadi.
    /// Bitta urinish: kadr chiqmasa yoki dekoder xato tashlasa —
    /// `null`. Xato YUTILADI: bu zaxira yo'llarning biri, xolos,
    /// va keyingisi baribir sinaladi.
    private fun tryFrame(
        r: MediaMetadataRetriever,
        timeUs: Long,
        option: Int
    ): Bitmap? = try {
        r.getFrameAtTime(timeUs, option)
    } catch (e: Throwable) {
        null
    }

    /**
     * Mahalliy videodan [count] ta teng oraliqdagi kadr (JPEG). Olib bo'lmagani
     * `null`. Hech qachon xato tashlamaydi.
     */
    private fun grabFrames(path: String, count: Int, maxWidth: Int, quality: Int): List<ByteArray?> {
        val out = ArrayList<ByteArray?>()
        if (path.isEmpty() || count <= 0) return out
        val retriever = MediaMetadataRetriever()
        try {
            retriever.setDataSource(path)
            val durationMs = retriever
                .extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)
                ?.toLongOrNull() ?: 0L
            val lastUs = if (durationMs > 1) (durationMs - 1) * 1000L else 0L
            for (i in 0 until count) {
                val timeUs = if (count <= 1) 0L else lastUs * i / (count - 1)
                var frame: Bitmap? = try {
                    retriever.getFrameAtTime(timeUs, MediaMetadataRetriever.OPTION_CLOSEST_SYNC)
                } catch (e: Throwable) {
                    null
                }
                if (frame == null) {
                    out.add(null)
                    continue
                }
                if (frame.width > maxWidth && frame.width > 0) {
                    val h = (frame.height.toLong() * maxWidth / frame.width)
                        .toInt().coerceAtLeast(1)
                    val scaled = Bitmap.createScaledBitmap(frame, maxWidth, h, true)
                    if (scaled !== frame) {
                        frame.recycle()
                        frame = scaled
                    }
                }
                val bytes = ByteArrayOutputStream()
                frame.compress(Bitmap.CompressFormat.JPEG, quality, bytes)
                frame.recycle()
                out.add(bytes.toByteArray())
            }
        } catch (e: Throwable) {
            // Qolgan kadrlar `null` bo'lib qoladi.
            while (out.size < count) out.add(null)
        } finally {
            try {
                retriever.release()
            } catch (e: Throwable) {
            }
        }
        return out
    }

    private fun grabFrame(url: String, maxWidth: Int, quality: Int): ByteArray? {
        if (url.isEmpty()) return null
        val retriever = MediaMetadataRetriever()
        try {
            // ── MANZIL ODATDA MAHALLIY SERVERNIKI ─────────────
            //
            // Ikkala chaqiruvchi ham (tomosha tarixi va
            // yozishmadagi video) Rust yadrosining "/thumb"
            // yo'lini beradi — ya'ni `http://127.0.0.1:...`.
            //
            // Fayl yo'li ham qabul qilinadi: `setDataSource` ning
            // bunday holat uchun ALOHIDA chaqiruvi bor —
            // sarlavhali variant manzil (URI) kutadi va oddiy
            // fayl yo'li berilsa xato beradi.
            if (url.startsWith("http://") || url.startsWith("https://")) {
                retriever.setDataSource(url, HashMap<String, String>())
            } else {
                retriever.setDataSource(url)
            }

            // Davomiyligi (ms). O'qilmasa 0 — pastdagi zaxira yo'l
            // ishlaydi.
            val durationMs = retriever
                .extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)
                ?.toLongOrNull() ?: 0L

            // ── BITTA URINISH YETARLI EMAS ───────────────────
            //
            // TOPILGAN XATO (foydalanuvchi: "support chatda video
            // thumbnail ko'rsatish umuman ishlamayapti", 5-rasm).
            //
            // Telegramdan kelgan ba'zi MP4 fayllarda
            // `getFrameAtTime` HAR DOIM `null` qaytaradi — bu
            // bizning xatomiz emas: TELEFONNING O'Z fayl
            // menejeri ham aynan o'sha fayllarda kadr ko'rsata
            // olmaydi. Sabab odatda kalit kadrlarning joylashuvi
            // yoki tahrir ro'yxati (edit list) bo'ladi.
            //
            // Shu sabab endi bitta emas, BIR NECHTA yo'l ketma-ket
            // sinaladi va BIRINCHI natija beradigani olinadi.
            // Hammasi MAHALLIY ish — tarmoq kerak emas, har bir
            // urinish bir necha o'n millisekund.
            var bmp: Bitmap? = null

            // 1) OXIRGI KADR. Bo'lakda bitta kadr bo'lgani uchun
            //    bu aynan o'sha kadr. Tomosha tarixida bo'lak
            //    to'xtagan joydan olinadi, yozishmada esa
            //    videoning BOSHIDAN — ikkovida ham "bo'lakdagi
            //    yagona kadr" degani.
            if (durationMs > 1) {
                bmp = tryFrame(
                    retriever,
                    (durationMs - 1) * 1000,
                    MediaMetadataRetriever.OPTION_CLOSEST
                )
            }

            // 2) BO'LAKNING BOSHI. Davomiylik o'qilmasa yoki
            //    yuqoridagi chiqmasa — eng ishonchli joy shu.
            if (bmp == null) {
                bmp = tryFrame(retriever, 0, MediaMetadataRetriever.OPTION_CLOSEST_SYNC)
            }
            if (bmp == null) {
                bmp = tryFrame(retriever, 0, MediaMetadataRetriever.OPTION_CLOSEST)
            }

            // 3) "Vakil kadr": tizim o'zi mos kadrni tanlaydi.
            //    Manfiy vaqt AYNAN shuni bildiradi.
            if (bmp == null) {
                bmp = try {
                    retriever.getFrameAtTime()
                } catch (e: Throwable) {
                    null
                }
            }

            // 4) VAQT BO'YICHA EMAS, RAQAM BO'YICHA (Android 9+).
            //
            //    Eng ishonchli yo'l: dekoder hech qanday izlashsiz
            //    (seek) birinchi kadrni ochadi. Yuqoridagilar
            //    yiqilgan fayllarda odatda AYNAN shu ishlaydi —
            //    chunki muammo kadrning o'zida emas, unga
            //    "sakrab borish"da bo'ladi.
            if (bmp == null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                bmp = try {
                    retriever.getFrameAtIndex(0)
                } catch (e: Throwable) {
                    null
                }
            }

            if (bmp == null) return null

            var frame: Bitmap = bmp

            // Ro'yxatdagi qator uchun to'liq o'lcham shart emas —
            // kichraytirish JPEG'ni bir necha barobar kichraytiradi.
            if (frame.width > maxWidth && frame.width > 0) {
                val h = (frame.height.toLong() * maxWidth / frame.width)
                    .toInt().coerceAtLeast(1)
                val scaled = Bitmap.createScaledBitmap(frame, maxWidth, h, true)
                if (scaled !== frame) {
                    frame.recycle()
                    frame = scaled
                }
            }

            val out = ByteArrayOutputStream()
            frame.compress(Bitmap.CompressFormat.JPEG, quality, out)
            frame.recycle()
            return out.toByteArray()
        } catch (e: Throwable) {
            return null
        } finally {
            try {
                retriever.release()
            } catch (e: Throwable) {
            }
        }
    }
}
