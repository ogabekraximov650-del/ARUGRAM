package uz.arugram.soft

import android.app.Application
import coil3.ImageLoader
import coil3.SingletonImageLoader
import coil3.PlatformContext
import coil3.disk.DiskCache
import coil3.memory.MemoryCache
import coil3.request.crossfade
import okio.Path.Companion.toOkioPath
import uz.arugram.soft.data.SeasonsRepo

/** Ilova darajasidagi yagona obyektlar (Flutter: `SeasonsRepo.instance` kabi). */
class AruApp : Application(), SingletonImageLoader.Factory {
    val seasons: SeasonsRepo by lazy { SeasonsRepo(this) }

    /**
     * Rasm keshi (Flutter: `AppImageCache`): xotirada ~20%, diskda 256 MB.
     * Animatsiyasiz — kartochkalar ro'yxatda tez chizilsin.
     */
    override fun newImageLoader(context: PlatformContext): ImageLoader =
        ImageLoader.Builder(context)
            .memoryCache { MemoryCache.Builder().maxSizePercent(context, 0.20).build() }
            .diskCache {
                DiskCache.Builder()
                    .directory(cacheDir.resolve("images").toOkioPath())
                    .maxSizeBytes(256L * 1024 * 1024)
                    .build()
            }
            .crossfade(false)
            .build()
}
