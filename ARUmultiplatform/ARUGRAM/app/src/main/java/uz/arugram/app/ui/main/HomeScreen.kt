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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
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
    val model = remember { ChatsModel() }
    var openChat by androidx.compose.runtime.saveable.rememberSaveable { androidx.compose.runtime.mutableStateOf<Long?>(null) }
    val ui by model.ui.collectAsStateWithLifecycle()
    // Haqiqiy vaqtdagi yangilanishlar (updates) keyingi bosqichda; hozircha har 20 soniyada ro'yxat yangilanadi.
    LaunchedEffect(Unit) {
        while (true) {
            model.load()
            kotlinx.coroutines.delay(20_000)
        }
    }

    val opened = ui.chats.firstOrNull { it.id == openChat }
    if (opened != null) {
        ChatScreen(opened, onBack = { openChat = null })
        return
    }
    Column(Modifier.fillMaxSize().background(c.bg)) {
        Column(Modifier.fillMaxWidth().background(c.bar).statusBarsPadding()) {
            Row(Modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(56.dp).clickable { auth.logout() }, contentAlignment = Alignment.Center) { MenuIcon(c.text) }
                BasicText("ARUGRAM", Modifier.weight(1f), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Medium), maxLines = 1)
                Box(Modifier.size(56.dp), contentAlignment = Alignment.Center) { SearchIcon(c.text) }
            }
        }
        Box(Modifier.weight(1f).fillMaxWidth().navigationBarsPadding()) {
            when {
                ui.loading -> CircularProgressIndicator(Modifier.align(Alignment.Center).size(32.dp), color = c.inputActive, strokeWidth = 3.dp)
                ui.error != null && ui.chats.isEmpty() -> Column(Modifier.align(Alignment.Center).padding(horizontal = 32.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    BasicText(ui.error.orEmpty(), style = TextStyle(color = c.gray, fontSize = 15.sp, textAlign = TextAlign.Center))
                    BasicText("Qayta urinish", Modifier.clickable { model.load() }.padding(16.dp), style = TextStyle(color = c.value, fontSize = 15.sp))
                }
                ui.chats.isEmpty() -> BasicText("Suhbatlar yo‘q", Modifier.align(Alignment.Center), style = TextStyle(color = c.gray, fontSize = 17.sp))
                else -> ChatList(ui.chats, onOpen = { openChat = it.id })
            }
        }
    }
}
