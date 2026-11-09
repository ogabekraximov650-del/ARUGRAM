package uz.arumediatv.com.ui.login

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.net.Uri
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uz.arumediatv.com.ui.theme.AruColors

/** Telefonda o'rnatilgan Telegram ilovasi (rasmiy Telegram, Telegram X, Cherrygram, Nekogram va h.k.): `tg://` havolasini ochadiganlar. */
class TgClient(val pkg: String, val label: String, val icon: ImageBitmap?)

object TgClients {
    /** `tg://` sxemasini ochadigan ilovalar (kirish uchun Telegram klientlari). Android 11+ da manifestdagi `<queries>` kerak. */
    fun installed(ctx: Context): List<TgClient> {
        val pm = ctx.packageManager
        val probe = Intent(Intent.ACTION_VIEW, Uri.parse("tg://resolve?domain=telegram"))
        val seen = HashSet<String>()
        return try {
            pm.queryIntentActivities(probe, 0).mapNotNull { ri ->
                val pkg = ri.activityInfo?.packageName ?: return@mapNotNull null
                if (pkg == ctx.packageName || !seen.add(pkg)) return@mapNotNull null
                val icon = try {
                    val d = ri.loadIcon(pm)
                    val w = d.intrinsicWidth.coerceAtLeast(1).coerceAtMost(192)
                    val h = d.intrinsicHeight.coerceAtLeast(1).coerceAtMost(192)
                    val bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
                    d.setBounds(0, 0, w, h)
                    d.draw(Canvas(bmp))
                    bmp.asImageBitmap()
                } catch (_: Exception) {
                    null
                }
                TgClient(pkg, ri.loadLabel(pm).toString(), icon)
            }
        } catch (_: Exception) {
            emptyList()
        }
    }

    /**
     * Bot havolasini TANLANGAN Telegram ilovasida ochadi (`https://t.me/<bot>?start=<token>` -> `tg://resolve?domain=<bot>&start=<token>`).
     * `pkg == null` — tizim o'zi tanlaydi (brauzer ham mumkin). Ochilmasa `false`.
     */
    fun open(ctx: Context, deepLink: String, pkg: String?): Boolean {
        val uri = Uri.parse(deepLink)
        val bot = uri.pathSegments.firstOrNull().orEmpty()
        val start = uri.getQueryParameter("start").orEmpty()
        val tg = if (bot.isNotEmpty()) Uri.parse("tg://resolve?domain=$bot&start=$start") else uri
        for (u in listOf(tg, uri)) {
            try {
                val i = Intent(Intent.ACTION_VIEW, u).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                if (pkg != null) i.setPackage(pkg)
                ctx.startActivity(i)
                return true
            } catch (_: Exception) {
            }
        }
        return false
    }
}

/** «Qaysi Telegram orqali kirasiz?» — o'rnatilgan Telegram ilovalari ro'yxati; foydalanuvchi tanlaydi. */
@Composable
fun TgClientPicker(clients: List<TgClient>, onPick: (TgClient?) -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = AruColors.Surface,
        title = { Text("Qaysi Telegram orqali kirasiz?", color = Color.White, fontSize = 17.sp) },
        text = {
            Column {
                if (clients.isEmpty()) {
                    Text("Telegram ilovasi topilmadi. Havola brauzerda ochiladi.", color = AruColors.TextDim, fontSize = 13.sp)
                }
                clients.forEach { c ->
                    Row(
                        Modifier.fillMaxWidth().clickable { onPick(c) }.padding(vertical = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        val img = remember(c.pkg) { c.icon }
                        if (img != null) Image(img, null, Modifier.size(36.dp))
                        Spacer(Modifier.width(12.dp))
                        Text(c.label, color = Color.White, fontSize = 15.sp)
                    }
                }
            }
        },
        confirmButton = {
            if (clients.isEmpty()) TextButton(onClick = { onPick(null) }) { Text("Brauzerda ochish") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Bekor qilish", color = AruColors.TextDim) } },
    )
}
