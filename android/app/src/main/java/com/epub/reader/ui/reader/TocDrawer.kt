package com.zhongbai233.epub.reader.ui.reader

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.MyLocation
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.zhongbai233.epub.reader.i18n.I18n
import com.zhongbai233.epub.reader.ui.theme.AccentBlue
import kotlinx.coroutines.launch

/**
 * 目录侧边栏 — 对应 PC 版的 render_toc()
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TocDrawerContent(
    toc: List<Pair<String, Int>>,
    currentChapter: Int,
    language: String,
    onSelectChapter: (Int) -> Unit,
    onClose: () -> Unit
) {
    // 读取 I18n.version 以确保语言切换时触发重组
    @Suppress("UNUSED_VARIABLE")
    val langVersion = I18n.version

    // Find the index of the current chapter in the TOC list for scrolling
    val currentIndex = toc.indexOfFirst { it.second == currentChapter }.coerceAtLeast(0)
    val listState = rememberLazyListState()
    val scope = rememberCoroutineScope()

    // Auto-scroll to current chapter when the drawer opens
    LaunchedEffect(currentChapter) {
        listState.animateScrollToItem(currentIndex)
    }

    Column(modifier = Modifier.fillMaxSize()) {
        // 标题栏
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 12.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                I18n.t("toc.title"),
                fontSize = 18.sp,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.onSurface
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                // "Locate current chapter" button
                IconButton(onClick = {
                    scope.launch { listState.animateScrollToItem(currentIndex) }
                }) {
                    Icon(Icons.Default.MyLocation, I18n.t("toc.locate_current"))
                }
                IconButton(onClick = onClose) {
                    Icon(Icons.Default.Close, I18n.t("settings.close"))
                }
            }
        }

        HorizontalDivider()

        // 目录列表
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(vertical = 8.dp)
        ) {
            itemsIndexed(toc) { _, (title, chapterIdx) ->
                val isSelected = chapterIdx == currentChapter

                Surface(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { onSelectChapter(chapterIdx) },
                    color = if (isSelected) {
                        AccentBlue.copy(alpha = 0.12f)
                    } else {
                        MaterialTheme.colorScheme.surface
                    }
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        if (isSelected) {
                            Box(
                                modifier = Modifier
                                    .width(3.dp)
                                    .height(20.dp)
                                    .padding(end = 0.dp)
                            ) {
                                Surface(
                                    modifier = Modifier.fillMaxSize(),
                                    color = AccentBlue,
                                    shape = MaterialTheme.shapes.extraSmall
                                ) {}
                            }
                            Spacer(Modifier.width(12.dp))
                        }

                        Text(
                            text = title,
                            fontSize = 14.sp,
                            fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Normal,
                            color = if (isSelected) AccentBlue else MaterialTheme.colorScheme.onSurface,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f)
                        )
                    }
                }
            }
        }
    }
}
