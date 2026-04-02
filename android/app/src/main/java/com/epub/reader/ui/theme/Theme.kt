package com.zhongbai233.epub.reader.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

// 强调色 — 与 PC 版一致 (56, 132, 255)
val AccentBlue = Color(0xFF3884FF)
val AccentBlueLight = Color(0xFF5A9CFF)

// 深色主题
private val DarkColorScheme = darkColorScheme(
    primary = AccentBlue,
    onPrimary = Color.White,
    primaryContainer = Color(0xFF1A3A6B),
    onPrimaryContainer = Color(0xFFD6E3FF),
    secondary = AccentBlueLight,
    background = Color(0xFF1A1A2E),
    onBackground = Color(0xFFE0E0E0),
    surface = Color(0xFF16213E),
    onSurface = Color(0xFFE0E0E0),
    surfaceVariant = Color(0xFF222640),
    onSurfaceVariant = Color(0xFFC0C0C0),
    outline = Color(0xFF444866),
)

// 浅色主题
private val LightColorScheme = lightColorScheme(
    primary = AccentBlue,
    onPrimary = Color.White,
    primaryContainer = Color(0xFFD6E3FF),
    onPrimaryContainer = Color(0xFF001A41),
    secondary = AccentBlue,
    background = Color(0xFFF8F8FC),
    onBackground = Color(0xFF1A1A1A),
    surface = Color.White,
    onSurface = Color(0xFF1A1A1A),
    surfaceVariant = Color(0xFFF0F0F5),
    onSurfaceVariant = Color(0xFF444444),
    outline = Color(0xFFCCCCCC),
)

@Composable
fun EpubReaderTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    dynamicColor: Boolean = false,
    content: @Composable () -> Unit
) {
    val colorScheme = when {
        dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
            val context = LocalContext.current
            if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        }
        darkTheme -> DarkColorScheme
        else -> LightColorScheme
    }

    MaterialTheme(
        colorScheme = colorScheme,
        typography = Typography(),
        content = content
    )
}
