package uz.arumediatv.com.ui.search

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.data.SeasonQueries
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.SeasonGrid
import uz.arumediatv.com.ui.common.fullSpan
import uz.arumediatv.com.ui.theme.AruColors

/** Qidiruv: nom bo'yicha (katta-kichik harfga sezgirsiz), yozilgan zahoti natija. */
@Composable
fun SearchScreen(onOpenSeason: (Season) -> Unit) {
    val store = AppServices.seasons
    LaunchedEffect(Unit) { store.ensureLoaded() }
    val state by store.state.collectAsStateWithLifecycle()
    var query by rememberSaveable { mutableStateOf("") }
    val results = remember(state.items, query) { SeasonQueries.search(state.items, query) }
    val focus = LocalFocusManager.current

    Column(Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 12.dp)) {
        SearchField(query, onChange = { query = it }, onClear = { query = "" }, onDone = { focus.clearFocus() })
        Spacer(Modifier.size(16.dp))
        SeasonGrid(
            items = results,
            onOpen = {
                focus.clearFocus()
                onOpenSeason(it)
            },
            contentPadding = PaddingValues(bottom = 24.dp),
            emptyContent = {
                fullSpan {
                    when {
                        query.isBlank() -> EmptyState(text = "Anime nomini yozib qidiruv qiling")
                        else -> EmptyState(text = "Qidiruv natijalari topilmadi")
                    }
                }
            },
        )
    }
}

@Composable
private fun SearchField(value: String, onChange: (String) -> Unit, onClear: () -> Unit, onDone: () -> Unit) {
    val shape = RoundedCornerShape(18.dp)
    val requester = remember { FocusRequester() }
    Row(
        Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(AruColors.Card)
            .border(1.dp, AruColors.BorderBright, shape)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Filled.Search, null, tint = Color.White.copy(alpha = 0.7f))
        Spacer(Modifier.width(10.dp))
        Box(Modifier.weight(1f)) {
            if (value.isEmpty()) Text("Anime qidirish...", color = Color.White.copy(alpha = 0.54f), fontSize = 16.sp)
            BasicTextField(
                value = value,
                onValueChange = onChange,
                singleLine = true,
                textStyle = TextStyle(color = Color.White, fontSize = 16.sp),
                cursorBrush = SolidColor(AruColors.Accent2),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { onDone() }),
                modifier = Modifier.fillMaxWidth().focusRequester(requester),
            )
        }
        if (value.isNotEmpty()) {
            Icon(
                Icons.Filled.Close, "Tozalash", tint = Color.White.copy(alpha = 0.7f),
                modifier = Modifier.size(20.dp).clickable(onClick = onClear),
            )
        }
    }
}
