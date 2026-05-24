package cn.one_kvm.androidhost

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import java.io.BufferedReader
import java.io.File
import java.io.InputStreamReader
import java.io.InterruptedIOException
import java.util.concurrent.Executors

class OneKvmService : Service() {
    private var rootProcess: Process? = null
    private val commandExecutor = Executors.newSingleThreadExecutor { runnable ->
        Thread(runnable, "OneKvmServiceCommand")
    }

    override fun onCreate() {
        super.onCreate()
        ensureNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action ?: ACTION_START) {
            ACTION_STOP -> {
                ServiceStatusStore.setStopping(this)
                commandExecutor.execute {
                    stopHostRuntime()
                    stopSelfResult(startId)
                }
                return START_NOT_STICKY
            }
            ACTION_START -> {
                ServiceStatusStore.setStarting(this)
                startForegroundCompat(NOTIFICATION_ID, notification("启动中"))
                commandExecutor.execute {
                    val currentState = ServiceStatusStore.snapshot(this).state
                    if (currentState == ServiceStatusStore.STATE_RUNNING && isPortOpen(8080, 100)) {
                        return@execute
                    }
                    val dataDir = File(getExternalFilesDir(null), "runtime")
                    if (!dataDir.exists()) dataDir.mkdirs()
                    val result = startRustHost(dataDir)
                    if (result.startsWith("Running") && !result.contains("start failed", ignoreCase = true)) {
                        ServiceStatusStore.setRunning(this, "服务已启动")
                        notificationManager().notify(NOTIFICATION_ID, notification("运行中"))
                    } else {
                        ServiceStatusStore.setError(this, "启动失败")
                        notificationManager().notify(NOTIFICATION_ID, notification("启动失败"))
                    }
                }
            }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        stopHostRuntime(updateNotification = false)
        commandExecutor.shutdownNow()
        ServiceStatusStore.setStopped(this)
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun notification(state: String): Notification {
        val intent = Intent(this, MainActivity::class.java)
        val pendingIntent = createContentIntent(intent)
        val builder = createNotificationBuilder()
        return builder
            .setSmallIcon(R.drawable.ic_stat_one_kvm)
            .setContentTitle("One-KVM Android Host")
            .setContentText(state)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .build()
    }

    private fun ensureNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val channel = NotificationChannel(
            CHANNEL_ID,
            "One-KVM Host",
            NotificationManager.IMPORTANCE_LOW,
        )
        notificationManager().createNotificationChannel(channel)
    }

    @Suppress("DEPRECATION")
    private fun createNotificationBuilder(): Notification.Builder {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            Notification.Builder(this)
        }
    }

    private fun createContentIntent(intent: Intent): PendingIntent {
        val flags = pendingIntentFlags()
        return PendingIntent.getActivity(this, 0, intent, flags)
    }

    private fun pendingIntentFlags(): Int {
        var flags = PendingIntent.FLAG_UPDATE_CURRENT
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            flags = flags or pendingIntentImmutableFlag()
        }
        return flags
    }

    private fun pendingIntentImmutableFlag(): Int {
        return try {
            PendingIntent::class.java.getField("FLAG_IMMUTABLE").getInt(null)
        } catch (_: ReflectiveOperationException) {
            0
        }
    }

    private fun notificationManager(): NotificationManager {
        return getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
    }

    private fun stopHostRuntime(updateNotification: Boolean = true) {
        stopRootHost()
        NativeBridge.stopHost()
        waitForPortRelease(8080, 2_000)
        LogStore.flush()
        ServiceStatusStore.setStopped(this)
        if (updateNotification) {
            notificationManager().notify(NOTIFICATION_ID, notification("已停止"))
        }
    }

    private fun startRustHost(dataDir: File): String {
        val logLevel = LogConfig.getLevel(this)
        val rustLog = LogConfig.rustLogFilter(logLevel)
        val appLogFile = LogStore.defaultLogFile(this)
        LogStore.configure(appLogFile)
        val rustLogFile = appLogFile
        LogStore.append("Starting One-KVM Rust host, data_dir=${dataDir.absolutePath}, log_level=$logLevel")
        val executable = extractHostBinary()
        return runCatching {
            val tlsInit = NativeBridge.initTlsVerifier(this)
            if (tlsInit != 0) {
                throw IllegalStateException("rustls platform verifier init failed with code $tlsInit")
            }
            stopRootHost(executable)
            clearExistingOtgGadgetsIfEnabled()
            startRootHost(executable, dataDir, rustLog, rustLogFile, logLevel)
            LogStore.append("Rust host running as root on port 8080")
            "Running as root on port 8080"
        }.getOrElse { rootError ->
            LogStore.append("Root host unavailable: ${rootError.message ?: rootError::class.java.simpleName}")
            configureAlsaEnvironment(executable)
            NativeBridge.setEnv("RUST_LOG", rustLog)
            NativeBridge.setEnv("ONE_KVM_FFMPEG_LOG", ffmpegLogLevel(logLevel))
            NativeBridge.setEnv("ONE_KVM_ANDROID_LOG_FILE", rustLogFile.absolutePath)
            val jniResult = NativeBridge.startHost(dataDir.absolutePath, "0.0.0.0", 8080)
            LogStore.append("Rust host running in app process on port 8080: $jniResult")
            "Running in app process on port 8080 (${rootError.message ?: "root unavailable"}; $jniResult)"
        }
    }

    private fun clearExistingOtgGadgetsIfEnabled() {
        if (!HostSettings.getClearExistingOtg(this)) return

        val command = """
            root=/sys/kernel/config/usb_gadget
            [ -d "${'$'}root" ] || exit 0
            for gadget in "${'$'}root"/*; do
              [ -d "${'$'}gadget" ] || continue
              [ -w "${'$'}gadget/UDC" ] && echo "" > "${'$'}gadget/UDC" 2>/dev/null || true
              find "${'$'}gadget/configs" -type l -delete 2>/dev/null || true
              rm -rf "${'$'}gadget" 2>/dev/null || true
            done
        """.trimIndent()

        runCatching {
            ProcessBuilder("/system/xbin/su", "0", "sh", "-c", command)
                .redirectErrorStream(true)
                .start()
                .waitFor()
        }.onSuccess { exit ->
            LogStore.append("Existing OTG gadget cleanup finished with exit code $exit")
        }.onFailure { err ->
            LogStore.append("Existing OTG gadget cleanup failed: ${err.message ?: err::class.java.simpleName}")
        }
    }

    private fun configureAlsaEnvironment(executable: File) {
        val binDir = executable.parentFile
            ?: throw IllegalStateException("host binary has no parent directory")
        val alsaConfigDir = File(binDir, "alsa")
        val alsaConfigPath = File(alsaConfigDir, "alsa.conf")
        NativeBridge.setEnv("ALSA_CONFIG_DIR", alsaConfigDir.absolutePath)
        NativeBridge.setEnv("ALSA_CONFIG_PATH", alsaConfigPath.absolutePath)
    }

    private fun extractHostBinary(): File {
        val abi = Build.SUPPORTED_ABIS.firstOrNull { it == "arm64-v8a" || it == "armeabi-v7a" }
            ?: throw IllegalStateException("unsupported ABI: ${Build.SUPPORTED_ABIS.joinToString()}")
        val binDir = File(filesDir, "bin/$abi")
        val target = File(binDir, "one-kvm-android-host")
        copyAssetIfChanged("bin/$abi/one-kvm-android-host", target)
        copyAssetIfChanged("bin/$abi/libc++_shared.so", File(binDir, "libc++_shared.so"))
        copyAssetIfChanged("bin/$abi/libasound.so", File(binDir, "libasound.so"))
        copyAssetIfChanged("bin/$abi/libopus.so", File(binDir, "libopus.so"))
        copyAssetDirectoryIfChanged("bin/$abi/alsa", File(binDir, "alsa"))
        if (!target.setExecutable(true, false)) {
            throw IllegalStateException("cannot mark host binary executable")
        }
        return target
    }

    private fun copyAssetIfChanged(assetPath: String, target: File) {
        val stamp = File(target.parentFile, "${target.name}.stamp")
        @Suppress("DEPRECATION")
        val packageInfo = packageManager.getPackageInfo(packageName, 0)
        val expectedStamp = "${packageInfo.lastUpdateTime}:$assetPath"
        if (target.exists() && stamp.exists() && stamp.readText() == expectedStamp) return
        target.parentFile?.mkdirs()
        assets.open(assetPath).use { input ->
            target.outputStream().use { output -> input.copyTo(output) }
        }
        stamp.writeText(expectedStamp)
    }

    private fun copyAssetDirectoryIfChanged(assetDir: String, targetDir: File) {
        @Suppress("DEPRECATION")
        val packageInfo = packageManager.getPackageInfo(packageName, 0)
        val stamp = File(targetDir, ".stamp")
        val expectedStamp = "${packageInfo.lastUpdateTime}:$assetDir"
        if (targetDir.exists() && stamp.exists() && stamp.readText() == expectedStamp) return
        if (targetDir.exists()) targetDir.deleteRecursively()
        copyAssetDirectory(assetDir, targetDir)
        stamp.writeText(expectedStamp)
    }

    private fun copyAssetDirectory(assetDir: String, targetDir: File) {
        targetDir.mkdirs()
        val children = assets.list(assetDir)?.filter { it.isNotEmpty() }.orEmpty()
        for (child in children) {
            val childAsset = "$assetDir/$child"
            val childTarget = File(targetDir, child)
            val grandChildren = assets.list(childAsset)?.filter { it.isNotEmpty() }.orEmpty()
            if (grandChildren.isEmpty()) {
                copyAssetIfChanged(childAsset, childTarget)
            } else {
                copyAssetDirectory(childAsset, childTarget)
            }
        }
    }

    private fun startRootHost(
        executable: File,
        dataDir: File,
        rustLog: String,
        rustLogFile: File,
        logLevel: String,
    ) {
        stopRootHost(executable)
        waitForPortRelease(8080, 2_000)
        val libDir = executable.parentFile?.absolutePath
            ?: throw IllegalStateException("host binary has no parent directory")
        val alsaConfigDir = File(executable.parentFile, "alsa")
        val alsaConfigPath = File(alsaConfigDir, "alsa.conf")
        val command =
            "export LD_LIBRARY_PATH=${shellQuote(libDir)}:\${LD_LIBRARY_PATH:-}; " +
                "export ALSA_CONFIG_DIR=${shellQuote(alsaConfigDir.absolutePath)}; " +
                "export ALSA_CONFIG_PATH=${shellQuote(alsaConfigPath.absolutePath)}; " +
                "export RUST_LOG=${shellQuote(rustLog)}; " +
                "export ONE_KVM_FFMPEG_LOG=${shellQuote(ffmpegLogLevel(logLevel))}; " +
                "export ONE_KVM_ANDROID_LOG_FILE=${shellQuote(rustLogFile.absolutePath)}; " +
                "${shellQuote(executable.absolutePath)} ${shellQuote(dataDir.absolutePath)} 0.0.0.0 8080"
        val process = ProcessBuilder("/system/xbin/su", "0", "sh", "-c", command)
            .redirectErrorStream(true)
            .start()
        rootProcess = process

        Thread {
            val readError = runCatching {
                BufferedReader(InputStreamReader(process.inputStream)).useLines { lines ->
                    lines.forEach {
                        android.util.Log.i("OneKvmService", it)
                    }
                }
            }.exceptionOrNull()
            if (readError != null && readError !is InterruptedIOException) {
                android.util.Log.w("OneKvmService", "Root host log reader stopped", readError)
                LogStore.append("Root host log reader stopped: ${readError.message ?: readError::class.java.simpleName}")
            }
            val exit = runCatching { process.waitFor() }.getOrNull()
            if (rootProcess === process && exit != null) {
                rootProcess = null
                ServiceStatusStore.setError(this, "Root host exited with code $exit")
            }
        }.start()

        Thread.sleep(500)
        val exit = runCatching { process.exitValue() }.getOrNull()
        if (exit != null) {
            rootProcess = null
            throw IllegalStateException("root host exited immediately: $exit")
        }
    }

    private fun startForegroundCompat(id: Int, notification: Notification) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val invoked = runCatching {
                val method = Service::class.java.getMethod(
                    "startForeground",
                    Int::class.javaPrimitiveType,
                    Notification::class.java,
                    Int::class.javaPrimitiveType,
                )
                method.invoke(this, id, notification, foregroundServiceTypeConnectedDevice())
            }.isSuccess
            if (invoked) return
        }
        super.startForeground(id, notification)
    }

    private fun foregroundServiceTypeConnectedDevice(): Int {
        return try {
            Service::class.java.getField("FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE").getInt(null)
        } catch (_: ReflectiveOperationException) {
            0
        }
    }

    private fun stopRootHost(executable: File? = null) {
        rootProcess?.destroy()
        rootProcess = null
        stopRootHostProcess(executable)
    }

    private fun stopRootHostProcess(executable: File? = null) {
        val command = buildString {
            append("pkill -TERM -f '[o]ne-kvm-android-host' 2>/dev/null || true; ")
            append("for pid in $(pidof one-kvm-android-host 2>/dev/null); do kill -TERM \"${'$'}pid\" 2>/dev/null || true; done; ")
            append("sleep 0.2; ")
            append("pkill -KILL -f '[o]ne-kvm-android-host' 2>/dev/null || true; ")
            append("for pid in $(pidof one-kvm-android-host 2>/dev/null); do kill -KILL \"${'$'}pid\" 2>/dev/null || true; done; ")
        }

        runCatching {
            ProcessBuilder("/system/xbin/su", "0", "sh", "-c", command)
                .redirectErrorStream(true)
                .start()
                .waitFor()
        }.onFailure { err ->
            LogStore.append("Failed to stop stale root host: ${err.message ?: err::class.java.simpleName}")
        }
    }

    private fun waitForPortRelease(port: Int, timeoutMs: Long) {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            val inUse = isPortOpen(port, 100)
            if (!inUse) return
            Thread.sleep(100)
        }
    }

    private fun isPortOpen(port: Int, timeoutMs: Int): Boolean {
        return runCatching {
            java.net.Socket().use { socket ->
                socket.connect(java.net.InetSocketAddress("127.0.0.1", port), timeoutMs)
            }
            true
        }.getOrDefault(false)
    }

    private fun shellQuote(value: String): String {
        return "'" + value.replace("'", "'\\''") + "'"
    }

    private fun ffmpegLogLevel(level: String): String {
        return when (level) {
            "trace" -> "trace"
            "debug" -> "debug"
            "info" -> "info"
            "warn" -> "warning"
            else -> "error"
        }
    }

    companion object {
        private const val CHANNEL_ID = "one_kvm_host"
        private const val NOTIFICATION_ID = 1001
        const val ACTION_START = "cn.one_kvm.androidhost.START"
        const val ACTION_STOP = "cn.one_kvm.androidhost.STOP"

        fun start(context: Context) {
            val intent = Intent(context, OneKvmService::class.java).setAction(ACTION_START)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        fun stop(context: Context) {
            context.startService(Intent(context, OneKvmService::class.java).setAction(ACTION_STOP))
        }
    }
}
