package com.zhongbai233.epub.reader.ui.reader

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhongbai233.epub.reader.i18n.I18n

val TTS_VOICES = listOf(
    "zh-CN-XiaoxiaoNeural" to "晓晓 (女)",
    "zh-CN-YunyangNeural" to "云扬 (男)",
    "zh-CN-XiaoyiNeural" to "晓伊 (女)",
    "zh-CN-YunjianNeural" to "云健 (男)",
    "zh-CN-YunxiNeural" to "云希 (男)",
    "zh-CN-XiaochenNeural" to "晓辰 (女)",
    "zh-CN-XiaohanNeural" to "晓涵 (女)",
    "zh-CN-XiaomoNeural" to "晓墨 (女)",
    "zh-CN-XiaoruiNeural" to "晓睿 (女)",
    "zh-CN-XiaoshuangNeural" to "晓双 (女)"
)

val TTS_RATES get() = listOf(
    -50 to "-50%",
    -25 to "-25%",
    0 to I18n.t("tts.rate_normal"),
    25 to "+25%",
    50 to "+50%",
    75 to "+75%",
    100 to "+100%"
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TtsSettingsSheet(
    voiceName: String,
    rate: Int,
    volume: Int,
    onVoiceChange: (String) -> Unit,
    onRateChange: (Int) -> Unit,
    onVolumeChange: (Int) -> Unit,
    onDismiss: () -> Unit
) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(bottom = 32.dp, start = 16.dp, end = 16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text(text = I18n.t("settings.tts_settings"), fontSize = 18.sp, fontWeight = FontWeight.Bold)

            // 发音人
            Text(text = I18n.t("settings.tts_voice"), style = MaterialTheme.typography.titleMedium)
            // A simple grid or flow row for voices would be better, but we can do a Dropdown or chips
            // Let's do a ScrollableRow of FilterChips
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                TTS_VOICES.chunked(3).forEach { rowVoices ->
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        rowVoices.forEach { (id, name) ->
                            FilterChip(
                                selected = voiceName == id,
                                onClick = { onVoiceChange(id) },
                                label = { Text(name) }
                            )
                        }
                    }
                }
            }

            Spacer(Modifier.height(8.dp))

            // 朗读速度
            Text(text = I18n.t("settings.tts_rate"), style = MaterialTheme.typography.titleMedium)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TTS_RATES.take(4).forEach { (v, label) ->
                    FilterChip(
                        selected = rate == v,
                        onClick = { onRateChange(v) },
                        label = { Text(label) }
                    )
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TTS_RATES.drop(4).forEach { (v, label) ->
                    FilterChip(
                        selected = rate == v,
                        onClick = { onRateChange(v) },
                        label = { Text(label) }
                    )
                }
            }

            Spacer(Modifier.height(8.dp))

            // 音量
            Text(text = I18n.t("settings.tts_volume") + ": %", style = MaterialTheme.typography.titleMedium)
            Slider(
                value = volume.toFloat(),
                onValueChange = { onVolumeChange(it.toInt()) },
                valueRange = -50f..50f,
                steps = 19
            )
        }
    }
}
