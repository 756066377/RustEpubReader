package com.zhongbai233.epub.reader.tts

import android.media.AudioAttributes
import android.media.MediaPlayer
import android.util.Log
import kotlinx.coroutines.*
import okhttp3.*
import okio.ByteString
import java.io.File
import java.io.FileOutputStream
import java.security.MessageDigest
import java.text.SimpleDateFormat
import java.util.*
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Edge TTS engine — implements Microsoft Edge Read Aloud API via WebSocket.
 *
 * Usage:
 *   val engine = EdgeTtsEngine(cacheDir)
 *   engine.synthesize("你好世界", "zh-CN-XiaoxiaoNeural", 0, 0) // returns MP3 bytes
 */
class EdgeTtsEngine(private val cacheDir: File) {

    companion object {
        private const val TAG = "EdgeTTS"
        private const val TRUSTED_CLIENT_TOKEN = "6A5AA1D4EAFF4E9FB37E23D68491D6F4"
        private const val VOICE_LIST_URL =
            "https://speech.platform.bing.com/consumer/speech/synthesize/readaloud/voices/list?trustedclienttoken=$TRUSTED_CLIENT_TOKEN"
        private const val WSS_URL =
            "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1?TrustedClientToken=$TRUSTED_CLIENT_TOKEN"
        private const val AUDIO_OUTPUT_FORMAT = "audio-24khz-48kbitrate-mono-mp3"
    }

    private val client = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .writeTimeout(15, TimeUnit.SECONDS)
        .build()

    /**
     * Synthesize text to MP3 bytes using Edge TTS.
     * This is a suspend function — call from a coroutine.
     *
     * @param text    The text to synthesize.
     * @param voice   Voice short name, e.g. "zh-CN-XiaoxiaoNeural".
     * @param rate    Rate adjustment, e.g. -50, 0, +50, +100.
     * @param volume  Volume adjustment, e.g. -50, 0, +50.
     * @return MP3 audio bytes, or null on failure.
     */
    suspend fun synthesize(
        text: String,
        voice: String,
        rate: Int = 0,
        volume: Int = 0
    ): ByteArray? = withContext(Dispatchers.IO) {
        try {
            synthesizeInternal(text, voice, rate, volume)
        } catch (e: Exception) {
            Log.e(TAG, "synthesize failed", e)
            null
        }
    }

    private suspend fun synthesizeInternal(
        text: String,
        voice: String,
        rate: Int,
        volume: Int
    ): ByteArray? = suspendCancellableCoroutine { cont ->
        val requestId = uuid()
        val rateStr = if (rate >= 0) "+${rate}%" else "${rate}%"
        val volumeStr = if (volume >= 0) "+${volume}%" else "${volume}%"
        val escapedText = escapeXml(text)

        val audioChunks = mutableListOf<ByteArray>()
        val completed = AtomicBoolean(false)

        val request = Request.Builder()
            .url(WSS_URL)
            .header("Origin", "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0")
            .build()

        val ws = client.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                // Step 1: Send speech config
                val configMessage = buildString {
                    append("Content-Type:application/json; charset=utf-8\r\n")
                    append("Path:speech.config\r\n\r\n")
                    append("""{"context":{"synthesis":{"audio":{"metadataoptions":{"sentenceBoundaryEnabled":"false","wordBoundaryEnabled":"false"},"outputFormat":"$AUDIO_OUTPUT_FORMAT"}}}}""")
                }
                webSocket.send(configMessage)

                // Step 2: Send SSML synthesis request
                val ssml = buildString {
                    append("<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='zh-CN'>")
                    append("<voice name='$voice'>")
                    append("<prosody rate='$rateStr' volume='$volumeStr'>")
                    append(escapedText)
                    append("</prosody></voice></speak>")
                }
                val ssmlMessage = buildString {
                    append("X-RequestId:$requestId\r\n")
                    append("Content-Type:application/ssml+xml\r\n")
                    append("Path:ssml\r\n\r\n")
                    append(ssml)
                }
                webSocket.send(ssmlMessage)
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                // Text messages: status updates, turn.start, turn.end
                if (text.contains("Path:turn.end")) {
                    if (completed.compareAndSet(false, true)) {
                        webSocket.close(1000, "done")
                        val result = mergeChunks(audioChunks)
                        cont.resumeWith(Result.success(result))
                    }
                }
            }

            override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
                // Binary messages: audio data with a header
                val data = bytes.toByteArray()
                // The first 2 bytes are header length (big-endian uint16)
                if (data.size > 2) {
                    val headerLen = ((data[0].toInt() and 0xFF) shl 8) or (data[1].toInt() and 0xFF)
                    if (data.size > headerLen + 2) {
                        val audioData = data.copyOfRange(headerLen + 2, data.size)
                        synchronized(audioChunks) {
                            audioChunks.add(audioData)
                        }
                    }
                }
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                Log.e(TAG, "WebSocket failure", t)
                if (completed.compareAndSet(false, true)) {
                    cont.resumeWith(Result.success(null))
                }
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                if (completed.compareAndSet(false, true)) {
                    val result = mergeChunks(audioChunks)
                    cont.resumeWith(Result.success(result))
                }
            }
        })

        cont.invokeOnCancellation {
            ws.cancel()
        }
    }

    private fun mergeChunks(chunks: List<ByteArray>): ByteArray? {
        if (chunks.isEmpty()) return null
        val total = chunks.sumOf { it.size }
        val result = ByteArray(total)
        var offset = 0
        for (chunk in chunks) {
            System.arraycopy(chunk, 0, result, offset, chunk.size)
            offset += chunk.size
        }
        return result
    }

    private fun uuid(): String = UUID.randomUUID().toString().replace("-", "")

    private fun escapeXml(text: String): String = text
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&apos;")

    fun shutdown() {
        client.dispatcher.executorService.shutdown()
    }
}
