package uz.arudrive.app.data

import android.content.Context
import kotlinx.serialization.json.Json
import java.io.File

/** Worker'dan kelgan ro'yxat telefon xotirasida (`filesDir/list.json`) saqlanadi: ilova ochilishi bilan darhol ko'rinadi, tarmoqsiz ham. */
object Store {
    private val json = Json { ignoreUnknownKeys = true }
    private fun file(ctx: Context) = File(ctx.applicationContext.filesDir, "list.json")

    fun load(ctx: Context): List<DriveFile> = try {
        val f = file(ctx)
        if (f.exists()) json.decodeFromString(DriveList.serializer(), f.readText()).files else emptyList()
    } catch (_: Exception) {
        emptyList()
    }

    fun save(ctx: Context, files: List<DriveFile>) {
        try {
            val tmp = File(ctx.applicationContext.filesDir, "list.json.tmp")
            tmp.writeText(json.encodeToString(DriveList.serializer(), DriveList(files)))
            tmp.renameTo(file(ctx))
        } catch (_: Exception) {
        }
    }
}
