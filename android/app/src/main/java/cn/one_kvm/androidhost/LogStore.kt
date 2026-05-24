package cn.one_kvm.androidhost

import android.content.Context
import java.io.File
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

object LogStore {
    private const val FLUSH_DELAY_MS = 250L
    private const val MAX_BUFFER_CHARS = 64 * 1024

    private val lock = Any()
    private val buffer = StringBuilder()
    private val executor = Executors.newSingleThreadScheduledExecutor { runnable ->
        Thread(runnable, "OneKvmLogStore").apply { isDaemon = true }
    }

    private var logFile: File? = null
    private var flushScheduled = false

    fun defaultLogFile(context: Context): File {
        return File(File(context.getExternalFilesDir(null), "runtime"), "one-kvm.log")
    }

    fun configure(file: File) {
        synchronized(lock) {
            flushLocked()
            file.parentFile?.mkdirs()
            file.writeText("")
            buffer.clear()
            logFile = file
            flushScheduled = false
        }
    }

    fun append(line: String) {
        synchronized(lock) {
            if (logFile == null) return

            buffer.append(line).append('\n')
            if (buffer.length >= MAX_BUFFER_CHARS) {
                flushLocked()
                return
            }

            if (!flushScheduled) {
                flushScheduled = true
                executor.schedule({ flush() }, FLUSH_DELAY_MS, TimeUnit.MILLISECONDS)
            }
        }
    }

    fun flush() {
        synchronized(lock) {
            flushLocked()
        }
    }

    private fun flushLocked() {
        val file = logFile ?: return
        if (buffer.isEmpty()) {
            flushScheduled = false
            return
        }

        val text = buffer.toString()
        buffer.clear()
        flushScheduled = false
        file.appendText(text)
    }
}
