package uz.arumediatv.com.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.expandVertically
import androidx.compose.runtime.remember
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uz.arumediatv.com.data.SavedAccount
import uz.arumediatv.com.ui.theme.AruColors

/**
 * Profil tepasidagi chap "Accountlarim ⌄" kapsulasi (YouTube'dagidek): bosilsa kapsula eniga biroz kengayadi va pastga
 * hisoblar soniga qarab cho'ziladi; pastida "＋ Account qo'shish". Tashqariga bosilsa yig'iladi.
 */
@Composable
fun AccountsCapsule(
    expanded: Boolean,
    onToggle: (Boolean) -> Unit,
    accounts: List<SavedAccount>,
    activeId: Long,
    limit: Int,
    onSwitch: (SavedAccount) -> Unit,
    onAdd: () -> Unit,
    onRemove: (SavedAccount) -> Unit,
) {
    val now = System.currentTimeMillis()
    Box {
        CapsuleHeader(open = false, modifier = Modifier.clickable { onToggle(true) })
        if (expanded) {
            Popup(
                alignment = Alignment.TopStart,
                onDismissRequest = { onToggle(false) },
                properties = PopupProperties(focusable = true),
            ) {
                val vis = remember { MutableTransitionState(false).apply { targetState = true } }
                Column(
                    Modifier.width(290.dp).clip(RoundedCornerShape(24.dp)).background(AruColors.Surface)
                        .border(1.dp, Color.White.copy(alpha = 0.28f), RoundedCornerShape(24.dp)),
                ) {
                    CapsuleHeader(open = true, modifier = Modifier.clickable { onToggle(false) })
                    AnimatedVisibility(visibleState = vis, enter = expandVertically()) {
                        Column(Modifier.padding(horizontal = 6.dp).padding(bottom = 8.dp)) {
                            accounts.forEach { a ->
                                val u = a.user
                                val active = a.id == activeId
                                Row(
                                    Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp))
                                        .background(if (active) AruColors.Accent.copy(alpha = 0.12f) else Color.Transparent)
                                        .clickable(enabled = !active) { onSwitch(a) }
                                        .padding(horizontal = 8.dp, vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Box(Modifier.size(38.dp).clip(CircleShape).background(AruColors.Accent.copy(alpha = 0.25f)), contentAlignment = Alignment.Center) {
                                        Text(u?.initials ?: "#", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 14.sp)
                                    }
                                    Spacer(Modifier.width(12.dp))
                                    Column(Modifier.weight(1f)) {
                                        Text(u?.fullName ?: "Account ${a.id}", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                        val sub = if (a.subUntil > now) " · Obuna" else ""
                                        Text("ID ${a.id}$sub", color = Color.White.copy(alpha = 0.5f), fontSize = 12.sp, maxLines = 1)
                                    }
                                    if (active) {
                                        Text("✓", color = AruColors.Accent2, fontSize = 18.sp, fontWeight = FontWeight.Bold)
                                    } else {
                                        Text(
                                            "✕", color = Color.White.copy(alpha = 0.45f), fontSize = 16.sp,
                                            modifier = Modifier.clip(CircleShape).clickable { onRemove(a) }.padding(8.dp),
                                        )
                                    }
                                }
                            }
                            Row(
                                Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).clickable(onClick = onAdd).padding(horizontal = 8.dp, vertical = 12.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Box(Modifier.size(38.dp).clip(CircleShape).background(Color.White.copy(alpha = 0.1f)), contentAlignment = Alignment.Center) {
                                    Text("＋", color = Color.White, fontSize = 18.sp)
                                }
                                Spacer(Modifier.width(12.dp))
                                Text("Account qo'shish", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
                            }
                            Text(
                                "${accounts.size} / $limit" + if (limit < 5) " · birorta accountda obuna bo'lsa 5 tagacha" else "",
                                color = Color.White.copy(alpha = 0.45f), fontSize = 12.sp, modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp),
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun CapsuleHeader(open: Boolean, modifier: Modifier = Modifier) {
    Row(
        modifier.height(44.dp).then(if (open) Modifier.width(290.dp) else Modifier)
            .then(if (open) Modifier else Modifier.clip(RoundedCornerShape(22.dp)).border(1.dp, Color.White.copy(alpha = 0.28f), RoundedCornerShape(22.dp)))
            .padding(horizontal = 18.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("Accountlarim", color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.Medium)
        Spacer(Modifier.width(8.dp))
        Text(if (open) "⌃" else "⌄", color = Color.White, fontSize = 16.sp)
    }
}
