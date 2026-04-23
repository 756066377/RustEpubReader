package com.zhongbai233.epub.reader.ui.reader

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.zhongbai233.epub.reader.i18n.I18n

@Composable
fun TtsControlBar(
    playing: Boolean,
    paused: Boolean,
    status: String,
    currentBlockIndex: Int,
    onPlay: () -> Unit,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onStop: () -> Unit,
    onClose: () -> Unit,
    onSettings: () -> Unit,
    modifier: Modifier = Modifier
) {
    val accent = Color(0xFF3884FF)
    val barBg = MaterialTheme.colorScheme.surfaceContainerHigh
    Surface(
        modifier = modifier.padding(horizontal = 16.dp, vertical = 8.dp),
        shape = CircleShape,
        color = barBg,
        shadowElevation = 6.dp
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            if (!playing) {
                FilledIconButton(onClick = onPlay, modifier = Modifier.size(44.dp), colors = IconButtonDefaults.filledIconButtonColors(containerColor = accent)) {
                    Icon(Icons.Default.PlayArrow, contentDescription = "Play", tint = Color.White, modifier = Modifier.size(24.dp))
                }
            } else {
                FilledIconButton(onClick = { if (paused) onResume() else onPause() }, modifier = Modifier.size(44.dp), colors = IconButtonDefaults.filledIconButtonColors(containerColor = accent)) {
                    Icon(if (paused) Icons.Default.PlayArrow else Icons.Default.Pause, contentDescription = if (paused) "Resume" else "Pause", tint = Color.White, modifier = Modifier.size(24.dp))
                }
            }
            if (playing) {
                IconButton(onClick = onStop, modifier = Modifier.size(44.dp)) {
                    Icon(Icons.Default.Stop, contentDescription = "Stop", modifier = Modifier.size(24.dp))
                }
            }
            Text(
                text = when {
                    status.isNotEmpty() -> status
                    playing && paused -> I18n.t("tts.paused")
                    playing -> I18n.t("tts.playing")
                    else -> I18n.t("tts.ready")
                },
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.widthIn(max = 120.dp)
            )
            IconButton(onClick = onSettings, modifier = Modifier.size(40.dp)) {
                Icon(Icons.Default.Settings, contentDescription = "Settings", modifier = Modifier.size(24.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            IconButton(onClick = { onStop(); onClose() }, modifier = Modifier.size(40.dp)) {
                Icon(Icons.Default.Close, contentDescription = "Close", modifier = Modifier.size(22.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}
