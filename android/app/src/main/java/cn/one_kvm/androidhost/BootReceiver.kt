package cn.one_kvm.androidhost

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED && HostSettings.getAutoStart(context)) {
            OneKvmService.start(context)
        }
    }
}
