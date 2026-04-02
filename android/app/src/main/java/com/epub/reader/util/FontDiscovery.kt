package com.zhongbai233.epub.reader.util

import java.io.File

data class FontItem(val displayName: String, val path: String)

object FontDiscovery {

    private val FONT_DIRS = listOf("/system/fonts", "/system/font", "/data/fonts")

    /** 已知字体文件 stem → 人性化显示名 */
    private val KNOWN_NAMES: Map<String, String> = mapOf(
        // Noto CJK
        "NotoSansCJK-Regular"        to "Noto Sans CJK",
        "NotoSansCJKsc-Regular"      to "Noto Sans CJK SC",
        "NotoSansCJKtc-Regular"      to "Noto Sans CJK TC",
        "NotoSerifCJK-Regular"       to "Noto Serif CJK",
        "NotoSerifCJKsc-Regular"     to "Noto Serif CJK SC",
        "NotoSerifCJKtc-Regular"     to "Noto Serif CJK TC",
        "NotoSansSC-Regular"         to "Noto Sans SC",
        "NotoSerifSC-Regular"        to "Noto Serif SC",
        "NotoSansSC-Light"           to "Noto Sans SC Light",
        // AOSP / 常见 Android 字体
        "DroidSansFallback"          to "Droid Sans Fallback",
        "DroidSans"                  to "Droid Sans",
        "Roboto-Regular"             to "Roboto",
        "RobotoCondensed-Regular"    to "Roboto Condensed",
        "RobotoMono-Regular"         to "Roboto Mono",
        // 小米
        "MiSans-Regular"             to "小米 MiSans",
        "MiSans-Medium"              to "小米 MiSans Medium",
        "MiSans-Light"               to "小米 MiSans Light",
        "MiSans-Thin"                to "小米 MiSans Thin",
        "MiSansSC-Regular"           to "小米 MiSans SC",
        // OPPO / 一加
        "OPPO_Sans_Regular"          to "OPPO Sans",
        "OPPO_Sans_Medium"           to "OPPO Sans Medium",
        "OPPO_Sans_Light"            to "OPPO Sans Light",
        "OPPOSans-R"                 to "OPPO Sans",
        "OPPOSans-M"                 to "OPPO Sans Medium",
        // 华为 HarmonyOS
        "HarmonyOS_Sans_SC_Regular"  to "HarmonyOS Sans SC",
        "HarmonyOS_Sans_SC_Medium"   to "HarmonyOS Sans SC Medium",
        "HarmonyOS_Sans_SC_Light"    to "HarmonyOS Sans SC Light",
        "HarmonyOS_Sans_SC_Bold"     to "HarmonyOS Sans SC Bold",
        // 思源 / Source Han
        "SourceHanSans-Regular"      to "思源黑体",
        "SourceHanSansCN-Regular"    to "思源黑体 CN",
        "SourceHanSerif-Regular"     to "思源宋体",
        "SourceHanSerifCN-Regular"   to "思源宋体 CN",
        // 方正
        "FZLTH"                      to "方正兰亭黑",
        "FZLanTingHeiPro-R"          to "方正兰亭黑 Pro",
    )

    /**
     * 扫描系统字体目录，返回排序后的字体列表。
     * CJK 字体优先排在前面，其余按名称升序。
     */
    fun discoverFonts(): List<FontItem> {
        val seen = mutableMapOf<String, String>() // displayName → path
        for (dirPath in FONT_DIRS) {
            val dir = File(dirPath)
            if (!dir.exists() || !dir.isDirectory) continue
            dir.listFiles()
                ?.filter {
                    it.isFile &&
                        (it.name.endsWith(".ttf", ignoreCase = true) ||
                            it.name.endsWith(".otf", ignoreCase = true))
                }
                ?.forEach { file ->
                    val stem = file.nameWithoutExtension
                    val displayName = KNOWN_NAMES[stem] ?: cleanName(stem)
                    if (displayName.isNotBlank()) {
                        seen.putIfAbsent(displayName, file.absolutePath)
                    }
                }
        }
        return seen.entries
            .map { FontItem(it.key, it.value) }
            .sortedWith(compareBy({ !isCjkPriority(it.displayName) }, { it.displayName.lowercase() }))
    }

    private fun cleanName(stem: String): String =
        stem.replace(Regex("[-_]"), " ")
            .replace(Regex("\\s+"), " ")
            .trim()

    /** 含中文字符或 CJK/SC/TC 标记的字体优先显示 */
    private fun isCjkPriority(name: String): Boolean =
        name.any { it.code in 0x4E00..0x9FFF } ||
            name.contains("CJK", ignoreCase = true) ||
            name.contains(" SC") ||
            name.contains(" TC")
}
