package uz.arugram.app.ui.main

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import uz.arugram.app.auth.AuthModel
import uz.arugram.app.ui.components.MenuIcon
import uz.arugram.app.ui.components.SearchIcon
import uz.arugram.app.ui.theme.LocalTg

/**
 * Kirgandan keyingi bosh oyna (DrKLO `DialogsActivity`): tepada 56 dp li panel (menyu, "ARUGRAM", qidiruv), pastida suhbatlar ro'yxati.
 * Menyu tugmasi hozircha hisobdan chiqadi (yon panel keyingi bosqichda).
 */
@Composable
fun HomeScreen(auth: AuthModel) {
    val c = LocalTg.current
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val model = remember { ChatsModel(ctx.applicationContext) }
    var openChat by androidx.compose.runtime.saveable.rememberSaveable { androidx.compose.runtime.mutableStateOf<Long?>(null) }
    val ui by model.ui.collectAsStateWithLifecycle()
    var tab by androidx.compose.runtime.saveable.rememberSaveable { androidx.compose.runtime.mutableIntStateOf(0) }
    var me by remember { androidx.compose.runtime.mutableStateOf<ChatRow?>(null) }
    var meContact by remember { androidx.compose.runtime.mutableStateOf("") }
    LaunchedEffect(Unit) {
        val j = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { uz.arugram.app.core.RustCore.tgMe() }
        if (j.optString("error").isEmpty() && j.has("id")) {
            me = ChatRow(j.optLong("id"), "user", j.optString("name"), false, 0, false, false, 0, "", false, false, false)
            val phone = j.optString("phone").let { if (it.isNotEmpty()) "+$it" else "" }
            val user = j.optString("username").let { if (it.isNotEmpty()) "@$it" else "" }
            meContact = listOf(phone, user).filter { it.isNotEmpty() }.joinToString("  ·  ")
        }
    }
    var searching by androidx.compose.runtime.saveable.rememberSaveable { androidx.compose.runtime.mutableStateOf(false) }
    var query by androidx.compose.runtime.saveable.rememberSaveable { androidx.compose.runtime.mutableStateOf("") }
    androidx.activity.compose.BackHandler(enabled = searching) { searching = false; query = "" }
    LaunchedEffect(ui.chats.map { it.id }, me?.id) { AvatarCache.ensure(ctx, (ui.chats.filter { !it.self }.map { it.id } + listOfNotNull(me?.id)).distinct()) }
    // Yangilanishlar: Rust yadrosi kelgan updates sonini hisoblaydi (`rust_tg_update_seq`); u o'zgarsa ro'yxat darhol yangilanadi.
    // Zaxira sifatida har 60 soniyada ham yangilanadi (yangilanish yo'qolib qolgan holat uchun).
    LaunchedEffect(Unit) {
        var last = -1L
        var sinceLoad = 0
        while (true) {
            val seq = withContext(Dispatchers.IO) { uz.arugram.app.core.RustCore.tgUpdateSeq() }
            // Tez-tez keluvchi updates (onlayn holat, yozmoqda...) serverni ortiqcha yuklamasin: kamida 3 soniya oraliq.
            if (last == -1L || (seq != last && sinceLoad >= 2) || sinceLoad >= 40) {
                last = seq
                sinceLoad = 0
                model.load()
            }
            sinceLoad++
            kotlinx.coroutines.delay(1_500)
        }
    }

    val opened = ui.chats.firstOrNull { it.id == openChat }
    if (opened != null) {
        ChatScreen(opened, onBack = { openChat = null })
        return
    }
    Box(Modifier.fillMaxSize().background(c.bg)) {
    Column(Modifier.fillMaxSize()) {
    if (tab == 0) {
        Column(Modifier.fillMaxWidth().background(c.bar).statusBarsPadding()) {
            Row(Modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
                if (searching) {
                    val focus = remember { androidx.compose.ui.focus.FocusRequester() }
                    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
                    Box(Modifier.size(56.dp).clickable { searching = false; query = "" }, contentAlignment = Alignment.Center) { uz.arugram.app.ui.components.BackArrowIcon(c.text) }
                    Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
                        if (query.isEmpty()) BasicText("Qidirish", style = TextStyle(color = c.hint, fontSize = 18.sp))
                        androidx.compose.foundation.text.BasicTextField(
                            value = query,
                            onValueChange = { query = it },
                            singleLine = true,
                            textStyle = TextStyle(color = c.text, fontSize = 18.sp),
                            cursorBrush = androidx.compose.ui.graphics.SolidColor(c.inputActive),
                            modifier = Modifier.fillMaxWidth().focusRequester(focus),
                        )
                    }
                    if (query.isNotEmpty()) Box(Modifier.size(56.dp).clickable { query = "" }, contentAlignment = Alignment.Center) { uz.arugram.app.ui.components.CloseIcon(c.text) }
                } else {
                    // DrKLO (`hasMainTabs`): sarlavha "logotip" rangida (#168BDB), chapga 4 dp suriladi.
                    BasicText("ARUGRAM", Modifier.weight(1f).padding(start = 20.dp), style = TextStyle(color = androidx.compose.ui.graphics.Color(0xFF168BDB), fontSize = 20.sp, fontWeight = FontWeight.Bold), maxLines = 1)
                }
            }
            if (!searching) {
                // `FragmentSearchField`: 48 dp (3 dp ichki chet) -> 42 dp li pilyuli, radius 20, orqa fon matn rangining 5 % (tunda 7 %).
                val fieldBg = if (c.dark) Color.White.copy(alpha = 0.07f) else Color.Black.copy(alpha = 0.05f)
                Row(
                    Modifier.fillMaxWidth().padding(start = 10.dp, end = 10.dp, bottom = 6.dp).height(42.dp)
                        .clip(androidx.compose.foundation.shape.RoundedCornerShape(20.dp)).background(fieldBg).clickable { searching = true },
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(Modifier.size(48.dp), contentAlignment = Alignment.Center) { SearchIcon(c.text.copy(alpha = 0.6f), size = 20.dp) }
                    BasicText("Qidirish", style = TextStyle(color = c.text.copy(alpha = 0.5f), fontSize = 15.sp))
                }
            }
        }
    }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            if (tab == MainTab.Contacts.ordinal) {
                ContactsTab(bottomPadding = 80.dp)
            } else if (tab == MainTab.Settings.ordinal) {
                SettingsTab(me, meContact, 80.dp) { auth.logout() }
            } else if (tab != 0) {
                TabPlaceholder(MainTab.entries[tab], auth, me, meContact)
            } else when {
                ui.loading -> CircularProgressIndicator(Modifier.align(Alignment.Center).size(32.dp), color = c.inputActive, strokeWidth = 3.dp)
                ui.error != null && ui.chats.isEmpty() -> Column(Modifier.align(Alignment.Center).padding(horizontal = 32.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    BasicText(ui.error.orEmpty(), style = TextStyle(color = c.gray, fontSize = 15.sp, textAlign = TextAlign.Center))
                    BasicText("Qayta urinish", Modifier.clickable { model.load() }.padding(16.dp), style = TextStyle(color = c.value, fontSize = 15.sp))
                }
                ui.chats.isEmpty() -> BasicText("Suhbatlar yo‘q", Modifier.align(Alignment.Center), style = TextStyle(color = c.gray, fontSize = 17.sp))
                else -> {
                    val shown = if (query.isBlank()) ui.chats else ui.chats.filter { it.name.contains(query.trim(), ignoreCase = true) }
                    if (shown.isEmpty()) BasicText("Hech narsa topilmadi", Modifier.align(Alignment.Center), style = TextStyle(color = c.gray, fontSize = 17.sp))
                    else ChatList(shown, bottomPadding = 80.dp, onOpen = { openChat = it.id })
                }
            }
        }
    }
    MainTabsBar(MainTab.entries[tab], { tab = it.ordinal; searching = false; query = "" }, me, Modifier.align(Alignment.BottomCenter).navigationBarsPadding())
    }
}


/** Kontaktlar / Sozlamalar / Profil tablari hozircha sarlavha va qisqa izoh bilan (keyingi bosqichlarda to'ldiriladi). */
@Composable
private fun TabPlaceholder(tab: MainTab, auth: AuthModel, me: ChatRow?, meContact: String) {
    val c = LocalTg.current
    Column(Modifier.fillMaxSize().background(c.grayBg).statusBarsPadding(), horizontalAlignment = Alignment.CenterHorizontally) {
        Box(Modifier.fillMaxWidth().height(56.dp).background(c.bar), contentAlignment = Alignment.CenterStart) {
            BasicText(tab.label, Modifier.padding(start = 16.dp), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Medium))
        }
        if (tab == MainTab.Profile && me != null) {
            Column(Modifier.fillMaxWidth().background(c.bar).padding(vertical = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                ChatAvatar(me, 96.dp)
                BasicText(me.name, Modifier.padding(top = 12.dp), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Medium))
                if (meContact.isNotEmpty()) BasicText(meContact, Modifier.padding(top = 4.dp), style = TextStyle(color = c.gray, fontSize = 15.sp))
            }
        }
        if (tab == MainTab.Settings || tab == MainTab.Profile) {
            Box(Modifier.padding(top = 24.dp).fillMaxWidth().background(c.bar).clickable { auth.logout() }.padding(vertical = 16.dp), contentAlignment = Alignment.Center) {
                BasicText("Hisobdan chiqish", style = TextStyle(color = c.red, fontSize = 16.sp))
            }
        } else {
            BasicText("Tez orada qo‘shiladi", Modifier.padding(top = 48.dp), style = TextStyle(color = c.gray, fontSize = 15.sp))
        }
    }
}
