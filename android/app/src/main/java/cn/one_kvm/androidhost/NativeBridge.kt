package cn.one_kvm.androidhost

import android.content.Context

object NativeBridge {
    init {
        System.loadLibrary("one_kvm_android_bootstrap")
    }

    external fun initTlsVerifier(context: Context): Int

    external fun setEnv(name: String, value: String): Int

    external fun startHost(dataDir: String, bindAddress: String, port: Int): String

    external fun stopHost(): String

    external fun hostStatus(): String

    external fun kernelVersion(): String
}
