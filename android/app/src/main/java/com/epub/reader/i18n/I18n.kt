package com.zhongbai233.epub.reader.i18n

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import org.json.JSONObject
import com.zhongbai233.epub.reader.RustBridge
import java.util.Locale

object I18n {
    var currentCode: String = "zh_cn"
        private set

    var isAuto: Boolean = true
        private set

    private var translations: Map<String, String> = emptyMap()

    var version: Int by mutableIntStateOf(0)
        private set

    val availableLanguages: List<Pair<String, String>> = listOf(
        "auto" to "自动 / Auto",
        "zh_cn" to "中文",
        "en" to "English"
    )

    init {
        load(detectSystemLanguage())
    }

    fun setLanguage(code: String) {
        isAuto = (code == "auto")
        val resolved = if (isAuto) detectSystemLanguage() else code
        currentCode = resolved
        load(resolved)
    }

    fun detectSystemLanguage(): String {
        val lang = Locale.getDefault().language
        return if (lang == "zh") "zh_cn" else "en"
    }

    fun t(key: String): String = translations[key] ?: key

    fun tf1(key: String, arg: String): String = t(key).replaceFirst("{}", arg)

    fun tf2(key: String, arg1: String, arg2: String): String =
        t(key).replaceFirst("{}", arg1).replaceFirst("{}", arg2)

    private fun load(code: String) {
        try {
            val jsonStr = RustBridge.getTranslations(code) ?: "{}"
            val obj = JSONObject(jsonStr)
            val map = mutableMapOf<String, String>()
            val keys = obj.keys()
            while (keys.hasNext()) {
                val k = keys.next()
                map[k] = obj.getString(k)
            }
            translations = map
        } catch (e: Exception) {
            e.printStackTrace()
            translations = emptyMap()
        }
        version++
    }
}
