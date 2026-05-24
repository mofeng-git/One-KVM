package cn.one_kvm.androidhost

import android.content.Context

object HostSettings {
    private const val PREFS = "one_kvm_android"
    private const val KEY_AUTO_START = "auto_start"
    private const val KEY_CLEAR_EXISTING_OTG = "clear_existing_otg"

    fun getAutoStart(context: Context): Boolean {
        return context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getBoolean(KEY_AUTO_START, false)
    }

    fun setAutoStart(context: Context, enabled: Boolean) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putBoolean(KEY_AUTO_START, enabled)
            .apply()
    }

    fun getClearExistingOtg(context: Context): Boolean {
        return context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getBoolean(KEY_CLEAR_EXISTING_OTG, false)
    }

    fun setClearExistingOtg(context: Context, enabled: Boolean) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putBoolean(KEY_CLEAR_EXISTING_OTG, enabled)
            .apply()
    }
}
