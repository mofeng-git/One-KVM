package cn.one_kvm.androidhost

import android.content.Context

object LogConfig {
    private const val PREFS = "one_kvm_android"
    private const val KEY_LOG_LEVEL = "log_level"
    const val DEFAULT_LEVEL = "info"
    val LEVELS = arrayOf("error", "warn", "info", "debug", "trace")

    fun getLevel(context: Context): String {
        val value = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getString(KEY_LOG_LEVEL, DEFAULT_LEVEL)
            ?: DEFAULT_LEVEL
        return if (LEVELS.contains(value)) value else DEFAULT_LEVEL
    }

    fun setLevel(context: Context, level: String) {
        val safeLevel = if (LEVELS.contains(level)) level else DEFAULT_LEVEL
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(KEY_LOG_LEVEL, safeLevel)
            .apply()
    }

    fun rustLogFilter(level: String): String {
        val safeLevel = if (LEVELS.contains(level)) level else DEFAULT_LEVEL
        return "one_kvm=$safeLevel,hwcodec=$safeLevel,tower_http=$safeLevel,webrtc_sctp=warn"
    }
}
