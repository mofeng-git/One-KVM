package cn.one_kvm.androidhost

import android.app.Activity
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.View
import android.widget.AdapterView
import android.widget.ArrayAdapter
import android.widget.Button
import android.widget.CompoundButton
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Spinner
import android.widget.Switch
import android.widget.TextView
import java.net.Inet4Address
import java.net.InetSocketAddress
import java.net.NetworkInterface
import java.net.Socket
import java.util.Collections

class MainActivity : Activity() {
    private lateinit var statusValue: TextView
    private lateinit var hostActionButton: Button
    private lateinit var logLevelSpinner: Spinner
    private lateinit var autoStartSwitch: Switch
    private lateinit var clearOtgSwitch: Switch
    private val statusHandler = Handler(Looper.getMainLooper())
    private var statusPollsRemaining = 0
    private val statusPoller = object : Runnable {
        override fun run() {
            refreshStatus()
            statusPollsRemaining -= 1
            if (statusPollsRemaining > 0) {
                statusHandler.postDelayed(this, STATUS_POLL_INTERVAL_MS)
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        window.statusBarColor = color("#F8FAFC")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            window.navigationBarColor = color("#F8FAFC")
        }

        val content = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(20.dp(), 24.dp(), 20.dp(), 28.dp())
            background = solid("#F8FAFC")
        }

        content.addView(startCard())
        content.addView(settingsCard())
        content.addView(infoCard())

        setContentView(ScrollView(this).apply {
            isFillViewport = true
            setBackgroundColor(color("#F8FAFC"))
            addView(content)
        })
    }

    override fun onResume() {
        super.onResume()
        reconcilePersistedStatus()
        refreshStatus()
        autoStartSwitch.isChecked = HostSettings.getAutoStart(this)
        clearOtgSwitch.isChecked = HostSettings.getClearExistingOtg(this)
    }

    override fun onPause() {
        statusHandler.removeCallbacks(statusPoller)
        super.onPause()
    }

    private fun startCard(): View {
        return card {
            addView(sectionTitle("启动管理"))
            addView(TextView(this@MainActivity).apply {
                text = "管理本机 One-KVM 服务进程。暂停会停止前台服务并释放运行资源。"
                textSize = 14f
                setTextColor(color("#64748B"))
                setPadding(0, 6.dp(), 0, 14.dp())
            })

            statusValue = TextView(this@MainActivity).apply {
                textSize = 14f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor(color("#0F172A"))
                background = rounded("#EFF6FF", "#BFDBFE", 8)
                setPadding(12.dp(), 8.dp(), 12.dp(), 8.dp())
            }
            addView(statusValue, matchWrap())

            addView(LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                setPadding(0, 14.dp(), 0, 0)
                hostActionButton = actionButton("启动", primary = true) { toggleHost() }
                addView(hostActionButton, matchButton())
            })
            refreshStatus()
        }
    }

    private fun settingsCard(): View {
        return card {
            addView(sectionTitle("运行设置"))

            val (autoStartRow, autoStartControl) = settingSwitchRow(
                title = "开机自启动",
                subtitle = "系统启动完成后自动拉起 One-KVM 前台服务。",
                checked = HostSettings.getAutoStart(this@MainActivity),
            ) { _, checked ->
                HostSettings.setAutoStart(this@MainActivity, checked)
                LogStore.append("Boot auto-start ${if (checked) "enabled" else "disabled"}")
            }
            autoStartSwitch = autoStartControl
            addView(autoStartRow)

            addView(divider())

            val (clearOtgRow, clearOtgControl) = settingSwitchRow(
                title = "清除已有 OTG Gadget",
                subtitle = "启动 root host 前尝试解绑并删除 configfs 中已有的 USB gadget。",
                checked = HostSettings.getClearExistingOtg(this@MainActivity),
            ) { _, checked ->
                HostSettings.setClearExistingOtg(this@MainActivity, checked)
                LogStore.append("Clear existing OTG gadget ${if (checked) "enabled" else "disabled"}")
            }
            clearOtgSwitch = clearOtgControl
            addView(clearOtgRow)

            addView(divider())
            addView(logLevelRow())
        }
    }

    private fun infoCard(): View {
        return card {
            addView(sectionTitle("应用信息"))
            addView(infoRow("软件内核版本", kernelVersion()))
            addView(infoRow("访问地址", accessAddresses(), selectable = true))
            addView(infoRow("日志文件", LogStore.defaultLogFile(this@MainActivity).absolutePath, selectable = true))
        }
    }

    private fun settingSwitchRow(
        title: String,
        subtitle: String,
        checked: Boolean,
        listener: CompoundButton.OnCheckedChangeListener,
    ): Pair<View, Switch> {
        val switch = Switch(this).apply {
            isChecked = checked
            setOnCheckedChangeListener(listener)
        }

        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, 12.dp(), 0, 12.dp())
            addView(LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.VERTICAL
                addView(TextView(this@MainActivity).apply {
                    text = title
                    textSize = 15f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(color("#0F172A"))
                })
                addView(TextView(this@MainActivity).apply {
                    text = subtitle
                    textSize = 13f
                    setTextColor(color("#64748B"))
                    setPadding(0, 4.dp(), 12.dp(), 0)
                })
            }, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
            addView(switch)
        }

        return row to switch
    }

    private fun infoRow(label: String, value: String, selectable: Boolean = false): View {
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, 12.dp(), 0, 12.dp())
            addView(TextView(this@MainActivity).apply {
                text = label
                textSize = 13f
                setTextColor(color("#64748B"))
            })
            addView(TextView(this@MainActivity).apply {
                text = value
                textSize = 15f
                setTextColor(color("#0F172A"))
                setPadding(0, 4.dp(), 0, 0)
                setTextIsSelectable(selectable)
            })
            addView(divider())
        }
    }

    private fun logLevelRow(): View {
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, 12.dp(), 0, 0)
            addView(TextView(this@MainActivity).apply {
                text = "日志级别"
                textSize = 15f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor(color("#0F172A"))
            }, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))

            logLevelSpinner = Spinner(this@MainActivity).apply {
                adapter = ArrayAdapter(
                    this@MainActivity,
                    android.R.layout.simple_spinner_dropdown_item,
                    LogConfig.LEVELS,
                )
                setSelection(LogConfig.LEVELS.indexOf(LogConfig.getLevel(this@MainActivity)).coerceAtLeast(0))
                onItemSelectedListener = object : AdapterView.OnItemSelectedListener {
                    override fun onItemSelected(parent: AdapterView<*>?, view: View?, position: Int, id: Long) {
                        val level = LogConfig.LEVELS[position]
                        if (level != LogConfig.getLevel(this@MainActivity)) {
                            LogConfig.setLevel(this@MainActivity, level)
                            LogStore.append("Log level set to $level; restart service to apply")
                        }
                    }

                    override fun onNothingSelected(parent: AdapterView<*>?) = Unit
                }
            }
            addView(logLevelSpinner)
        }
    }

    private fun card(build: LinearLayout.() -> Unit): View {
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(16.dp(), 16.dp(), 16.dp(), 16.dp())
            background = rounded("#FFFFFF", "#E2E8F0", 10)
            elevation = 1.5f.dpFloat()
            build()
        }.also {
            it.layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { setMargins(0, 0, 0, 14.dp()) }
        }
    }

    private fun sectionTitle(text: String): View {
        return TextView(this).apply {
            this.text = text
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(color("#0F172A"))
        }
    }

    private fun actionButton(text: String, primary: Boolean, action: () -> Unit): Button {
        return Button(this).apply {
            this.text = text
            textSize = 15f
            isAllCaps = false
            minHeight = 44.dp()
            setTextColor(color(if (primary) "#FFFFFF" else "#0F172A"))
            background = if (primary) rounded("#2563EB", "#2563EB", 8) else rounded("#FFFFFF", "#CBD5E1", 8)
            setOnClickListener { action() }
        }
    }

    private fun toggleHost() {
        when (ServiceStatusStore.snapshot(this).state) {
            ServiceStatusStore.STATE_RUNNING -> pauseHost()
            ServiceStatusStore.STATE_STOPPED, ServiceStatusStore.STATE_ERROR -> startHost()
        }
    }

    private fun startHost() {
        ServiceStatusStore.setStarting(this)
        refreshStatus()
        OneKvmService.start(this)
        LogStore.append("Start requested from app UI")
        pollStatusForAWhile()
    }

    private fun pauseHost() {
        ServiceStatusStore.setStopping(this)
        refreshStatus()
        OneKvmService.stop(this)
        LogStore.append("Pause requested from app UI")
        pollStatusForAWhile()
    }

    private fun refreshStatus() {
        if (::statusValue.isInitialized) {
            statusValue.text = "状态：${hostStatusSummary()}"
        }
        updateHostActionButton()
    }

    private fun hostStatusSummary(): String {
        val serviceStatus = ServiceStatusStore.snapshot(this)
        if (serviceStatus.state != ServiceStatusStore.STATE_STOPPED) {
            return serviceStatus.labelText()
        }

        val nativeRunning = runCatching {
            NativeBridge.hostStatus().contains("running", ignoreCase = true)
        }.getOrDefault(false)

        return if (nativeRunning) "运行中" else "已停止"
    }

    private fun reconcilePersistedStatus() {
        val serviceStatus = ServiceStatusStore.snapshot(this)
        if (serviceStatus.state == ServiceStatusStore.STATE_STOPPED) return
        if (
            serviceStatus.state == ServiceStatusStore.STATE_STARTING &&
            System.currentTimeMillis() - serviceStatus.updatedAt < STARTING_RECONCILE_GRACE_MS
        ) {
            return
        }

        Thread {
            val portOpen = isLocalWebPortOpen()
            val nativeRunning = runCatching { NativeBridge.hostStatus().contains("running", ignoreCase = true) }
                .getOrDefault(false)
            if (!portOpen && !nativeRunning) {
                ServiceStatusStore.setStopped(this, "服务未运行")
                runOnUiThread { refreshStatus() }
            }
        }.start()
    }

    private fun isLocalWebPortOpen(): Boolean {
        return runCatching {
            Socket().use { socket ->
                socket.connect(InetSocketAddress("127.0.0.1", 8080), 250)
            }
            true
        }.getOrDefault(false)
    }

    private fun updateHostActionButton() {
        if (!::hostActionButton.isInitialized) return

        when (ServiceStatusStore.snapshot(this).state) {
            ServiceStatusStore.STATE_STARTING -> setHostActionButton("启动中...", enabled = false, primary = true)
            ServiceStatusStore.STATE_RUNNING -> setHostActionButton("停止", enabled = true, primary = false)
            ServiceStatusStore.STATE_STOPPING -> setHostActionButton("停止中...", enabled = false, primary = false)
            else -> setHostActionButton("启动", enabled = true, primary = true)
        }
    }

    private fun setHostActionButton(text: String, enabled: Boolean, primary: Boolean) {
        hostActionButton.text = text
        hostActionButton.isEnabled = enabled
        hostActionButton.alpha = if (enabled) 1f else 0.65f
        hostActionButton.setTextColor(color(if (primary) "#FFFFFF" else "#0F172A"))
        hostActionButton.background = if (primary) {
            rounded("#2563EB", "#2563EB", 8)
        } else {
            rounded("#FFFFFF", "#CBD5E1", 8)
        }
    }

    private fun pollStatusForAWhile() {
        statusPollsRemaining = 20
        statusHandler.removeCallbacks(statusPoller)
        statusHandler.postDelayed(statusPoller, STATUS_POLL_INTERVAL_MS)
    }

    private fun kernelVersion(): String {
        return runCatching { NativeBridge.kernelVersion() }
            .getOrElse { "unknown" }
    }

    private fun accessAddresses(): String {
        val addresses = runCatching {
            Collections.list(NetworkInterface.getNetworkInterfaces())
                .filter { it.isUp && !it.isLoopback }
                .flatMap { iface -> Collections.list(iface.inetAddresses) }
                .filterIsInstance<Inet4Address>()
                .filter { !it.isLoopbackAddress }
                .map { "http://${it.hostAddress}:8080" }
                .distinct()
        }.getOrDefault(emptyList())

        return (addresses.ifEmpty { listOf("http://127.0.0.1:8080") }).joinToString("\n")
    }

    private fun divider(): View {
        return View(this).apply {
            setBackgroundColor(color("#E2E8F0"))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                1,
            ).apply { setMargins(0, 0, 0, 0) }
        }
    }

    private fun matchWrap(): LinearLayout.LayoutParams {
        return LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT,
        )
    }

    private fun matchButton(): LinearLayout.LayoutParams {
        return LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            48.dp(),
        )
    }

    private fun solid(hex: String): GradientDrawable = GradientDrawable().apply {
        setColor(color(hex))
    }

    private fun rounded(fill: String, stroke: String, radiusDp: Int): GradientDrawable {
        return GradientDrawable().apply {
            setColor(color(fill))
            cornerRadius = radiusDp.dpFloat()
            setStroke(1.dp(), color(stroke))
        }
    }

    private fun color(hex: String): Int = Color.parseColor(hex)

    private fun Int.dp(): Int = (this * resources.displayMetrics.density + 0.5f).toInt()

    private fun Int.dpFloat(): Float = this * resources.displayMetrics.density

    private fun Float.dpFloat(): Float = this * resources.displayMetrics.density

    companion object {
        private const val STATUS_POLL_INTERVAL_MS = 500L
        private const val STARTING_RECONCILE_GRACE_MS = 15_000L
    }
}
