package cn.one_kvm.androidhost

import android.content.Context

object ServiceStatusStore {
    private const val PREFS = "one_kvm_android_status"
    private const val KEY_STATE = "state"
    private const val KEY_MESSAGE = "message"
    private const val KEY_UPDATED_AT = "updated_at"

    const val STATE_STOPPED = "stopped"
    const val STATE_STARTING = "starting"
    const val STATE_RUNNING = "running"
    const val STATE_STOPPING = "stopping"
    const val STATE_ERROR = "error"

    data class Snapshot(
        val state: String,
        val message: String,
        val updatedAt: Long,
    ) {
        fun labelText(): String {
            return when (state) {
                STATE_STARTING -> "启动中"
                STATE_RUNNING -> "运行中"
                STATE_STOPPING -> "停止中"
                STATE_ERROR -> "错误"
                else -> "已停止"
            }
        }

        fun displayText(): String {
            val label = labelText()
            return if (message.isBlank()) label else "$label：$message"
        }
    }

    fun setStarting(context: Context, message: String = "正在启动服务") {
        write(context, STATE_STARTING, message)
    }

    fun setRunning(context: Context, message: String) {
        write(context, STATE_RUNNING, message)
    }

    fun setStopping(context: Context, message: String = "正在停止服务") {
        write(context, STATE_STOPPING, message)
    }

    fun setStopped(context: Context, message: String = "服务已停止") {
        write(context, STATE_STOPPED, message)
    }

    fun setError(context: Context, message: String) {
        write(context, STATE_ERROR, message)
    }

    fun snapshot(context: Context): Snapshot {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        return Snapshot(
            state = prefs.getString(KEY_STATE, STATE_STOPPED) ?: STATE_STOPPED,
            message = prefs.getString(KEY_MESSAGE, "") ?: "",
            updatedAt = prefs.getLong(KEY_UPDATED_AT, 0L),
        )
    }

    private fun write(context: Context, state: String, message: String) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(KEY_STATE, state)
            .putString(KEY_MESSAGE, message)
            .putLong(KEY_UPDATED_AT, System.currentTimeMillis())
            .apply()
    }
}
