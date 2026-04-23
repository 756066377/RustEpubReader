package com.zhongbai233.epub.reader.csc

import ai.onnxruntime.*
import android.util.Log
import java.io.BufferedReader
import java.io.File
import java.io.FileReader
import java.nio.LongBuffer
import kotlin.math.exp
import kotlin.math.ln

/**
 * Chinese Spelling Correction engine for Android.
 * Uses ONNX Runtime with MacBERT-CSC int8 model.
 */
class CscEngine {

    companion object {
        private const val TAG = "CscEngine"
        private const val MAX_SEQ_LEN = 128
        private const val MODEL_FILENAME = "csc-macbert-int8.onnx"
        private const val VOCAB_FILENAME = "csc-vocab.txt"
        const val MODEL_URL = "https://dl.zhongbai233.com/models/csc-macbert-int8.onnx"
        const val VOCAB_URL = "https://dl.zhongbai233.com/models/csc-vocab.txt"

        fun modelDir(dataDir: String) = File(dataDir, "models")
        fun modelPath(dataDir: String) = File(modelDir(dataDir), MODEL_FILENAME)
        fun vocabPath(dataDir: String) = File(modelDir(dataDir), VOCAB_FILENAME)
        fun isModelAvailable(dataDir: String) =
            modelPath(dataDir).exists() && vocabPath(dataDir).exists()
    }

    private var session: OrtSession? = null
    private var env: OrtEnvironment? = null
    private var vocab: List<String> = emptyList()
    private var tokenToId: Map<String, Int> = emptyMap()
    private var unkId: Int = 100

    val isReady: Boolean get() = session != null && vocab.isNotEmpty()

    /**
     * Load ONNX model and vocabulary.
     * Call from a background thread.
     */
    fun load(dataDir: String): Result<Unit> = runCatching {
        val modelFile = modelPath(dataDir)
        val vocabFile = vocabPath(dataDir)
        require(modelFile.exists()) { "Model file not found" }
        require(vocabFile.exists()) { "Vocab file not found" }

        // Load vocabulary
        vocab = BufferedReader(FileReader(vocabFile)).use { reader ->
            reader.readLines()
        }
        tokenToId = vocab.mapIndexed { idx, token -> token to idx }.toMap()
        unkId = tokenToId["[UNK]"] ?: 100

        // Create ONNX session
        val ortEnv = OrtEnvironment.getEnvironment()
        env = ortEnv
        val opts = OrtSession.SessionOptions().apply {
            setOptimizationLevel(OrtSession.SessionOptions.OptLevel.ALL_OPT)
            setIntraOpNumThreads(2)
        }
        session = ortEnv.createSession(modelFile.absolutePath, opts)
        Log.i(TAG, "CSC model loaded: vocab=${vocab.size}")
    }

    fun release() {
        session?.close()
        session = null
        env?.close()
        env = null
    }

    /**
     * Check text for potential spelling corrections.
     * Returns a list of [CorrectionInfo].
     */
    fun check(text: String, threshold: Float = 0.95f): List<CorrectionInfo> {
        if (!isReady) return emptyList()

        val corrections = mutableListOf<CorrectionInfo>()
        var cumulativeOffset = 0
        var current = StringBuilder()
        var segCharStart = 0

        for (ch in text) {
            current.append(ch)
            if (ch in "。！？；\n") {
                val trimmed = current.toString().trim()
                if (trimmed.isNotEmpty()) {
                    val leadingWs = current.toString().takeWhile { it.isWhitespace() }.length
                    try {
                        val results = inferSentence(trimmed, threshold)
                        for (c in results) {
                            corrections.add(c.copy(charOffset = c.charOffset + segCharStart + leadingWs))
                        }
                    } catch (e: Exception) {
                        Log.w(TAG, "inferSentence failed", e)
                    }
                }
                cumulativeOffset += current.toString().toList().size
                segCharStart = cumulativeOffset
                current = StringBuilder()
            }
        }
        // Last segment
        val trimmed = current.toString().trim()
        if (trimmed.isNotEmpty()) {
            val leadingWs = current.toString().takeWhile { it.isWhitespace() }.length
            try {
                val results = inferSentence(trimmed, threshold)
                for (c in results) {
                    corrections.add(c.copy(charOffset = c.charOffset + segCharStart + leadingWs))
                }
            } catch (e: Exception) {
                Log.w(TAG, "inferSentence failed", e)
            }
        }

        return corrections
    }

    private fun inferSentence(sentence: String, threshold: Float): List<CorrectionInfo> {
        val sess = session ?: return emptyList()
        Log.d(TAG, "inferSentence input: '$sentence'")
        val encoded = encode(sentence)
        Log.d(TAG, "encoded inputIds (first 20): ${encoded.inputIds.take(20).mapIndexed { i, id -> "[$i]=$id(${idToToken(id.toInt()) ?: "?"})" }.joinToString()}")

        // Create input tensors
        val inputIdsBuffer = LongBuffer.wrap(encoded.inputIds.toLongArray())
        val attentionMaskBuffer = LongBuffer.wrap(encoded.attentionMask.toLongArray())
        val tokenTypeIdsBuffer = LongBuffer.wrap(encoded.tokenTypeIds.toLongArray())
        val shape = longArrayOf(1, MAX_SEQ_LEN.toLong())

        val ortEnv = env ?: return emptyList()
        val inputIdsTensor = OnnxTensor.createTensor(ortEnv, inputIdsBuffer, shape)
        val attentionMaskTensor = OnnxTensor.createTensor(ortEnv, attentionMaskBuffer, shape)
        val tokenTypeIdsTensor = OnnxTensor.createTensor(ortEnv, tokenTypeIdsBuffer, shape)

        val inputs = mapOf(
            "input_ids" to inputIdsTensor,
            "attention_mask" to attentionMaskTensor,
            "token_type_ids" to tokenTypeIdsTensor
        )

        val outputs = sess.run(inputs)
        val outputTensor = outputs[0] as OnnxTensor
        // Shape: [1, seq_len, vocab_size]
        val logits = outputTensor.floatBuffer
        val outputShape = outputTensor.info.shape
        val vocabSize = outputShape[2].toInt()

        val chars = sentence.toList()
        val corrections = mutableListOf<CorrectionInfo>()
        val minMargin = 0.30f

        for ((pos, inputId) in encoded.inputIds.withIndex()) {
            // Skip special tokens
            if (encoded.offsetMapping[pos] == null || encoded.attentionMask[pos] == 0L) continue
            // Skip [UNK]
            if (inputId == unkId.toLong()) continue

            val base = pos * vocabSize
            var maxLogit = Float.NEGATIVE_INFINITY
            var predictedId = 0
            for (i in 0 until vocabSize) {
                val v = logits.get(base + i)
                if (v > maxLogit) {
                    maxLogit = v
                    predictedId = i
                }
            }

            if (predictedId.toLong() == inputId) continue

            // Softmax for predicted and original
            var expSum = 0f
            for (i in 0 until vocabSize) {
                expSum += exp(logits.get(base + i) - maxLogit)
            }
            val pPredicted = exp(logits.get(base + predictedId) - maxLogit) / expSum
            val pOriginal = exp(logits.get(base + inputId.toInt()) - maxLogit) / expSum

            // Filter 1: confidence
            if (pPredicted < threshold) continue
            // Filter 2: margin
            if (pPredicted - pOriginal < minMargin) continue

            val mapping = encoded.offsetMapping[pos] ?: continue
            val charStart = mapping.first

            val originalChar = if (charStart < chars.size) chars[charStart].toString()
            else idToToken(inputId.toInt()) ?: continue

            Log.d(TAG, "pos=$pos, inputId=$inputId(${idToToken(inputId.toInt())}), predictedId=$predictedId(${idToToken(predictedId)}), charStart=$charStart, chars[$charStart]='${if (charStart < chars.size) chars[charStart] else '?'}'")

            // Filter 3: CJK only
            val origCh = originalChar.firstOrNull() ?: continue
            if (!isCjkChar(origCh)) continue

            // Filter 4: protected chars
            if (isProtectedChar(originalChar)) continue

            val correctedChar = idToToken(predictedId) ?: continue
            if (correctedChar == "[UNK]" || correctedChar == originalChar) continue
            if (correctedChar.startsWith("##")) continue
            if (isConfusedPair(originalChar, correctedChar)) continue

            // Filter 5: duplicate neighbor
            val corrCh = correctedChar.firstOrNull()
            if (corrCh != null) {
                if (charStart > 0 && chars[charStart - 1] == corrCh) continue
                if (charStart + 1 < chars.size && chars[charStart + 1] == corrCh) continue
            }

            corrections.add(
                CorrectionInfo(
                    original = originalChar,
                    corrected = correctedChar,
                    confidence = pPredicted,
                    charOffset = charStart,
                    status = CorrectionStatus.PENDING
                )
            )
        }

        // Debug: log all corrections
        for (c in corrections) {
            Log.d(TAG, "CSC correction: '${c.original}' -> '${c.corrected}' (conf=${c.confidence}, offset=${c.charOffset}), sentence[${c.charOffset}]='${if (c.charOffset < chars.size) chars[c.charOffset] else '?'}'")
        }

        // Clean up tensors
        inputIdsTensor.close()
        attentionMaskTensor.close()
        tokenTypeIdsTensor.close()
        outputs.close()

        return corrections
    }

    // ── Tokenizer (WordPiece) ──

    private data class EncodedInput(
        val inputIds: List<Long>,
        val attentionMask: List<Long>,
        val tokenTypeIds: List<Long>,
        val offsetMapping: List<Pair<Int, Int>?>
    )

    private fun encode(text: String): EncodedInput {
        val chars = text.toList()
        val tokens = mutableListOf<Long>()
        val offsets = mutableListOf<Pair<Int, Int>?>()

        // [CLS]
        tokens.add((tokenToId["[CLS]"] ?: 101).toLong())
        offsets.add(null)

        var charIdx = 0
        while (charIdx < chars.size) {
            val ch = chars[charIdx]
            // Normalize whitespace
            if (ch.isWhitespace()) { charIdx++; continue }

            val lower = ch.lowercaseChar().toString()
            val id = tokenToId[lower]
            if (id != null) {
                tokens.add(id.toLong())
                offsets.add(charIdx to charIdx + 1)
            } else {
                tokens.add(unkId.toLong())
                offsets.add(charIdx to charIdx + 1)
            }
            charIdx++

            // Truncate at MAX_SEQ_LEN - 2 (leave room for [CLS] and [SEP])
            if (tokens.size >= MAX_SEQ_LEN - 1) break
        }

        // [SEP]
        tokens.add((tokenToId["[SEP]"] ?: 102).toLong())
        offsets.add(null)

        // Pad
        val inputIds = tokens.toMutableList()
        val attentionMask = MutableList(tokens.size) { 1L }
        val tokenTypeIds = MutableList(tokens.size) { 0L }
        val offsetMapping = offsets.toMutableList()

        while (inputIds.size < MAX_SEQ_LEN) {
            inputIds.add(0L)
            attentionMask.add(0L)
            tokenTypeIds.add(0L)
            offsetMapping.add(null)
        }

        return EncodedInput(inputIds, attentionMask, tokenTypeIds, offsetMapping)
    }

    private fun idToToken(id: Int): String? =
        if (id in vocab.indices) vocab[id] else null

    // ── Filters (matching Rust implementation) ──

    private fun isCjkChar(ch: Char): Boolean {
        val cp = ch.code
        return cp in 0x4E00..0x9FFF ||
                cp in 0x3400..0x4DBF ||
                cp in 0xF900..0xFAFF
    }

    private fun isProtectedChar(s: String): Boolean {
        val trimmed = s.trim()
        return trimmed in PROTECTED_CHARS
    }

    private fun isConfusedPair(original: String, corrected: String): Boolean {
        val a = original.trim()
        val b = corrected.trim()
        return CONFUSED_GROUPS.any { group -> a in group && b in group }
    }

    private val CONFUSED_GROUPS = listOf(
        setOf("他", "她", "它", "牠", "祂"),
        setOf("的", "得", "地"),
        setOf("做", "作"),
        setOf("哪", "那"),
        setOf("在", "再")
    )

    private val PROTECTED_CHARS = setOf(
        "吗", "吧", "呢", "啊", "呀", "哇", "哦", "嗯", "喔", "噢",
        "啦", "嘛", "咯", "喽", "嘞", "罢", "咧",
        "的", "得", "地",
        "了", "过", "着",
        "么", "个", "们", "这", "那", "就", "都", "也", "又", "才",
        "把", "被", "让", "给", "向", "往", "从", "到", "为",
        "而", "且", "或", "与", "及",
        "我", "你", "他", "她", "它", "谁", "啥",
        "这", "那", "哪", "几", "多", "些",
        "别", "是", "其", "悉", "有", "没", "不", "会", "能",
        "要", "去", "来", "说", "看", "想", "知", "道"
    )
}

data class CorrectionInfo(
    val original: String,
    val corrected: String,
    val confidence: Float,
    val charOffset: Int,
    val status: CorrectionStatus
)

enum class CorrectionStatus {
    PENDING, ACCEPTED, REJECTED, IGNORED
}
