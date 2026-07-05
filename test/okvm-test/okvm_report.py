from __future__ import annotations

import os
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


@dataclass
class CheckResult:
    name: str
    status: str
    message: str = ""
    data: dict[str, Any] = field(default_factory=dict)


@dataclass
class Metric:
    name: str
    value: Any
    unit: str = ""
    labels: dict[str, Any] = field(default_factory=dict)


@dataclass
class Evidence:
    category: str
    title: str
    path: str


STATUS_TEXT = {
    "PASS": "通过",
    "FAIL": "失败",
    "WARN": "警告",
    "SKIP": "跳过",
}

STATUS_COLOR = {
    "PASS": "\033[32m",
    "FAIL": "\033[31m",
    "WARN": "\033[33m",
    "SKIP": "\033[33m",
    "INFO": "\033[36m",
}

CATEGORIES_WITH_DATA_TABLE = {
    "初始化与环境",
    "视频性能",
    "HDMI 画面与颜色",
    "HID / MSD / ATX",
}

CATEGORIES_WITHOUT_DATA_TABLE: set[str] = set()

REPORT_HIDDEN_RESULTS = {
    "log_collection",
    "screenshot",
}

CATEGORY_RULES = (
    ("初始化与环境", ("target_", "setup_", "login", "network_", "target_inventory", "stream_codecs", "windows_agent", "ventoy_resources", "msd_restart")),
    ("视频性能", ("video_", "config_video_")),
    ("HDMI 画面与颜色", ("hdmi_", "config_video_hdmi_probe")),
    ("HID / MSD / ATX", ("hid_", "msd", "atx_")),
)

DISPLAY_NAMES = {
    "target_reset": "目标机重置",
    "setup_init": "初始化账号",
    "login": "登录",
    "login_after_restart": "重启后登录",
    "network_latency": "网络延迟",
    "target_inventory": "目标机设备清单",
    "hid_msd_config": "HID/MSD 配置",
    "ventoy_resources": "Ventoy 资源",
    "msd_restart": "MSD 启用后重启",
    "video_input_select": "视频输入选择",
    "windows_agent": "Windows 配套程序连接",
    "config_video_hdmi_probe": "HDMI 采集配置",
    "hdmi_identity": "HDMI 画面来源确认",
    "hdmi_color_range": "HDMI 颜色偏差",
    "hdmi_latency_mjpeg": "MJPEG 画面延迟",
    "hid_status": "HID 状态",
    "hid_input": "HID 输入",
    "hid_latency": "HID 延迟",
    "msd": "MSD 虚拟盘",
    "atx_api": "ATX API",
    "log_collection": "日志采集",
    "screenshot": "网页截图",
}


class Reporter:
    def __init__(self, report_dir: Path, color: bool | None = None):
        self.report_dir = report_dir
        self.results: list[CheckResult] = []
        self.metrics: list[Metric] = []
        self.evidence: list[Evidence] = []
        self.report_dir.mkdir(parents=True, exist_ok=True)
        (self.report_dir / "evidence").mkdir(exist_ok=True)
        self.use_color = color if color is not None else (sys.stdout.isatty() and not os.environ.get("NO_COLOR"))

    def add(self, check_name: str, outcome: str, detail: str = "", **data: Any) -> None:
        self.results.append(CheckResult(name=check_name, status=outcome, message=detail, data=data))
        prefix = {"PASS": "[PASS]", "FAIL": "[FAIL]", "SKIP": "[SKIP]", "WARN": "[WARN]"}.get(outcome, "[INFO]")
        print(f"{self.colorize(prefix, outcome)} {check_name}: {detail}")

    def info(self, check_name: str, detail: str) -> None:
        print(f"{self.colorize('[INFO]', 'INFO')} {check_name}: {detail}")

    def metric(self, name: str, value: Any, unit: str = "", **labels: Any) -> None:
        self.metrics.append(Metric(name=name, value=value, unit=unit, labels=labels))

    def add_evidence(self, category: str, title: str, path: Path) -> None:
        rel = path.relative_to(self.report_dir) if path.is_relative_to(self.report_dir) else path
        self.evidence.append(Evidence(category=category, title=title, path=str(rel)))

    def write(self, run_id: str, target: str) -> None:
        generated_at = time.strftime("%Y-%m-%d %H:%M:%S %z", time.localtime())
        report_timestamp = time.strftime("%Y%m%d-%H%M%S", time.localtime())
        status = "失败" if any(r.status == "FAIL" for r in self.report_results()) else "通过"
        md_path = self.report_dir / f"report-{report_timestamp}.md"
        user_md_path = self.report_dir / f"user-report-{report_timestamp}.md"
        md_path.write_text(self.render_markdown(run_id, target, generated_at, status), encoding="utf-8")
        user_md_path.write_text(self.render_user_markdown(run_id), encoding="utf-8")
        self.info("markdown_report", str(md_path))
        self.info("user_markdown_report", str(user_md_path))

    def colorize(self, text: str, status: str) -> str:
        if not self.use_color:
            return text
        return f"{STATUS_COLOR.get(status, '')}{text}\033[0m"

    def render_markdown(self, run_id: str, target: str, generated_at: str, status: str) -> str:
        counts = self.status_counts()
        lines = [
            "# One-KVM 自动化测试报告",
            "",
            f"- 运行编号：`{run_id}`",
            f"- 目标设备：`{target}`",
            f"- 生成时间：`{generated_at}`",
            f"- 总体结果：**{status}**",
            f"- 统计：通过 {counts.get('PASS', 0)}，警告 {counts.get('WARN', 0)}，跳过 {counts.get('SKIP', 0)}，失败 {counts.get('FAIL', 0)}",
            "",
        ]

        grouped = self.group_results()
        for category, results in grouped.items():
            if not results:
                continue
            lines.extend([f"## {category}", ""])
            lines.extend(self.render_result_table(category, results))
            lines.append("")

        return "\n".join(lines).rstrip() + "\n"

    def render_user_markdown(self, run_id: str) -> str:
        lines = [
            "# One-KVM 测试摘要",
            "",
            "## 背景",
            "",
            f"- 运行编号：`{run_id}`",
            "- 测试设备：",
            f"- 视频设备：{markdown_inline(self.user_video_device())}",
            f"- HID设备：{markdown_inline(self.user_hid_backend())}",
            f"- 网络延迟：{markdown_inline(self.user_network_latency())}",
            "",
            "## 视频性能",
            "",
            "| 视频输入参数 | 测试帧率 | 延迟统计（中位数 p50 / 95%分位 p95 / 最大值 max） |",
            "| --- | --- | --- |",
        ]
        video_rows = self.user_video_rows()
        if video_rows:
            for params, fps, latency in video_rows:
                lines.append(
                    "| "
                    + " | ".join(
                        (
                            markdown_table_cell(params),
                            markdown_table_cell(fps),
                            markdown_table_cell(latency),
                        )
                    )
                    + " |"
                )
        else:
            lines.append("| 无数据 | - | - |")

        lines.extend(
            [
                "",
                "## HID性能",
                "",
                "| 输入方式 | 延迟统计（中位数 p50 / 95%分位 p95 / 最大值 max） |",
                "| --- | --- |",
            ]
        )
        hid_rows = self.user_hid_rows()
        if hid_rows:
            for method, latency in hid_rows:
                lines.append(f"| {markdown_table_cell(method)} | {markdown_table_cell(latency)} |")
        else:
            lines.append("| 无数据 | - |")

        lines.extend(
            [
                "",
                "## MSD性能",
                "",
                "| 操作 | 数据 |",
                "| --- | --- |",
            ]
        )
        msd_rows = self.user_msd_rows()
        if msd_rows:
            for operation, data in msd_rows:
                lines.append(f"| {markdown_table_cell(operation)} | {markdown_table_cell(data)} |")
        else:
            lines.append("| 无数据 | - |")

        return "\n".join(lines).rstrip() + "\n"

    def find_result(self, name: str) -> CheckResult | None:
        for result in self.results:
            if result.name == name:
                return result
        return None

    def user_network_latency(self) -> str:
        result = self.find_result("network_latency")
        if not result:
            return "无数据"
        tcp = result.data.get("tcp_connect") or {}
        if tcp.get("samples"):
            return format_latency_values(tcp)
        return result.message or "无数据"

    def user_video_device(self) -> str:
        result = self.find_result("video_input_select")
        cases = result.data.get("cases") if result else None
        if not cases:
            return "无数据"
        by_device: dict[str, list[str]] = {}
        for case in cases:
            if not isinstance(case, dict):
                continue
            device = str(case.get("device") or "未知设备")
            by_device.setdefault(device, []).append(format_video_case(case))
        return "；".join(f"{device}：{', '.join(items)}" for device, items in by_device.items()) or "无数据"

    def user_hid_backend(self) -> str:
        config = self.find_result("hid_msd_config")
        if config:
            if config.data.get("udc"):
                return "OTG"
            if config.data.get("port"):
                return "CH9329"
        status = self.find_result("hid_status")
        if status:
            data = status.data.get("status") or {}
            backend = str(data.get("backend") or data.get("hid_backend") or "").strip()
            if backend:
                return backend.upper() if backend.lower() == "otg" else backend
        return "无数据"

    def user_video_rows(self) -> list[tuple[str, str, str]]:
        fps_by_key: dict[tuple[str, str], str] = {}
        for result in self.results:
            if not result.name.startswith("video_") or result.name.startswith("video_latency_"):
                continue
            data = result.data
            case = data.get("case") or {}
            if not isinstance(case, dict):
                continue
            output_mode = str(data.get("codec") or result.name.rsplit("_", 1)[-1]).lower()
            label = str(case.get("label") or "")
            fps = data.get("fps")
            if fps is None:
                fps = data.get("avgFps")
            if label and output_mode and fps is not None:
                fps_by_key[(label, output_mode)] = format_fps(float(fps))

        rows: list[tuple[str, str, str]] = []
        for result in self.results:
            if not result.name.startswith("video_latency_"):
                continue
            data = result.data
            case = data.get("video_case") or {}
            if not isinstance(case, dict):
                case = {}
            output_mode = str(data.get("output_mode") or video_latency_output_from_name(result.name) or "").lower()
            label = str(case.get("label") or "")
            params = format_video_latency_params(case, output_mode)
            fps = fps_by_key.get((label, output_mode), "-")
            latency = format_latency_data(data)
            if not latency:
                latency = f"{STATUS_TEXT.get(result.status, result.status)}：{result.message}" if result.message else STATUS_TEXT.get(result.status, result.status)
            rows.append((params, fps, latency))
        return rows

    def user_hid_rows(self) -> list[tuple[str, str]]:
        result = self.find_result("hid_latency")
        if not result:
            return []
        latency = format_latency_data(result.data)
        if not latency:
            latency = f"{STATUS_TEXT.get(result.status, result.status)}：{result.message}" if result.message else STATUS_TEXT.get(result.status, result.status)
        return [(self.user_hid_backend(), latency)]

    def user_msd_rows(self) -> list[tuple[str, str]]:
        result = self.find_result("msd")
        if not result:
            return []
        verify = result.data.get("verify") or {}
        rows: list[tuple[str, str]] = []
        if "write_mib_s" in verify:
            rows.append(("写", f"{float(verify.get('write_mib_s') or 0):.2f}MiB/s"))
        if "read_mib_s" in verify:
            rows.append(("读", f"{float(verify.get('read_mib_s') or 0):.2f}MiB/s"))
        elif "cached_read_mib_s" in verify:
            rows.append(("读（缓存，仅校验）", f"{float(verify.get('cached_read_mib_s') or 0):.2f}MiB/s"))
        if rows:
            return rows
        return [("MSD", f"{STATUS_TEXT.get(result.status, result.status)}：{result.message}" if result.message else STATUS_TEXT.get(result.status, result.status))]

    def report_results(self) -> list[CheckResult]:
        return [result for result in self.results if result.name not in REPORT_HIDDEN_RESULTS]

    def status_counts(self) -> dict[str, int]:
        counts: dict[str, int] = {}
        for result in self.report_results():
            counts[result.status] = counts.get(result.status, 0) + 1
        return counts

    def group_results(self) -> dict[str, list[CheckResult]]:
        grouped = {name: [] for name, _ in CATEGORY_RULES}
        grouped["其他"] = []
        for result in self.report_results():
            for category, prefixes in CATEGORY_RULES:
                if any(result.name == prefix or result.name.startswith(prefix) for prefix in prefixes):
                    grouped[category].append(result)
                    break
            else:
                grouped["其他"].append(result)
        return grouped

    def render_result(self, result: CheckResult) -> list[str]:
        title = display_name(result.name)
        status = STATUS_TEXT.get(result.status, result.status)
        lines = [f"### {title}", "", f"- 结果：**{status}**"]
        summary = summarize_result(result, self.metrics)
        if summary:
            lines.append(f"- 数据：{summary}")
        if result.status == "FAIL":
            lines.append(f"- 失败日志：`{markdown_inline(result.message)}`")
        elif result.status in {"WARN", "SKIP"} and result.message:
            lines.append(f"- 说明：{markdown_inline(result.message)}")
        return lines

    def render_result_table(self, category: str, results: list[CheckResult]) -> list[str]:
        with_data = category in CATEGORIES_WITH_DATA_TABLE or (
            category not in CATEGORIES_WITHOUT_DATA_TABLE
            and any(has_numeric_data(result, self.metrics) for result in results)
        )
        if with_data:
            lines = ["| 项目 | 结果 | 数据 |", "| --- | --- | --- |"]
            for result in results:
                lines.append(
                    "| "
                    + " | ".join(
                        (
                            markdown_table_cell(display_name(result.name)),
                            markdown_table_cell(STATUS_TEXT.get(result.status, result.status)),
                            markdown_table_cell(summarize_result(result, self.metrics)),
                        )
                    )
                    + " |"
                )
        else:
            lines = ["| 项目 | 结果 |", "| --- | --- |"]
            for result in results:
                lines.append(
                    "| "
                    + " | ".join(
                        (
                            markdown_table_cell(display_name(result.name)),
                            markdown_table_cell(STATUS_TEXT.get(result.status, result.status)),
                        )
                    )
                    + " |"
                )

        sections = (
            ("失败项目", [result for result in results if result.status == "FAIL"]),
            ("跳过项目", [result for result in results if result.status == "SKIP"]),
            ("警告项目", [result for result in results if result.status == "WARN"]),
        )
        for heading, items in sections:
            if not items:
                continue
            lines.extend(["", f"{heading}："])
            for result in items:
                title = display_name(result.name)
                status = STATUS_TEXT.get(result.status, result.status)
                reason = result.message or summarize_result(result, self.metrics) or "无详细日志"
                lines.append(f"- **{markdown_inline(title)}**（{status}）：{markdown_inline(reason)}")
        return lines


def summarize_result(result: CheckResult, metrics: list[Metric]) -> str:
    data = result.data
    name = result.name
    if name == "network_latency":
        tcp = data.get("tcp_connect") or {}
        http = data.get("http_health") or {}
        parts = []
        if tcp.get("samples"):
            parts.append(
                f"TCP p50={float(tcp.get('p50_ms', 0)):.1f}ms，p95={float(tcp.get('p95_ms', 0)):.1f}ms，max={float(tcp.get('max_ms', 0)):.1f}ms"
            )
        if http.get("samples"):
            parts.append(
                f"HTTP p50={float(http.get('p50_ms', 0)):.1f}ms，p95={float(http.get('p95_ms', 0)):.1f}ms，max={float(http.get('max_ms', 0)):.1f}ms"
            )
        errors = data.get("errors") or []
        if errors:
            parts.append(f"异常样本={len(errors)}")
        return "；".join(parts)
    if name.startswith("config_video_") and isinstance(data.get("case"), dict):
        case = data["case"]
        return f"{case.get('device')} {case.get('fmt')} {case.get('width')}x{case.get('height')}@{case.get('fps')}"
    if name.startswith("video_latency_") or name == "hdmi_latency_mjpeg":
        if not any(k in data for k in ("p50_ms", "p95_ms", "max_ms")):
            return ""
        case = data.get("video_case") or {}
        output_mode = str(data.get("output_mode") or video_latency_output_from_name(name) or "").upper()
        parts = []
        if output_mode:
            parts.append(output_mode)
        if isinstance(case, dict) and case.get("fmt"):
            parts.append(f"{case.get('fmt')} {case.get('width')}x{case.get('height')}@{case.get('fps')}")
        parts.extend(
            [
                f"p50={float(data.get('p50_ms', 0)):.1f}ms",
                f"p95={float(data.get('p95_ms', 0)):.1f}ms",
                f"max={float(data.get('max_ms', 0)):.1f}ms",
            ]
        )
        return "，".join(parts)
    if name.startswith("video_"):
        parts = []
        if "fps" in data:
            parts.append(f"fps={float(data['fps']):.1f}")
        if "avgFps" in data:
            parts.append(f"平均fps={float(data['avgFps']):.1f}")
        if "input_requested_fps" in data:
            parts.append(f"请求输入={float(data['input_requested_fps']):.1f}fps")
        if "maxRtt" in data:
            parts.append(f"最大RTT={float(data['maxRtt']) * 1000:.1f}ms")
        if "min_expected_fps" in data:
            parts.append(f"目标下限={float(data['min_expected_fps']):.1f}fps")
        if "dynamic_source_fps" in data:
            parts.append(f"动态源={int(data['dynamic_source_fps'])}fps")
        if data.get("fps_above_requested"):
            parts.append("输出帧率高于请求输入")
        if data.get("unsupported"):
            parts.append("不支持")
        return "，".join(parts)
    if name == "hdmi_identity":
        samples = data.get("samples") or []
        ok = sum(1 for item in samples if item.get("identity_match"))
        parts = [f"颜色匹配 {ok}/{len(samples)}"]
        if samples:
            detail = []
            for item in samples:
                rgb = item.get("measured_rgb_mean")
                status = "匹配" if item.get("identity_match") else "不匹配"
                detail.append(f"{item.get('name')}={rgb}({status})")
            parts.append("；".join(detail))
        return "，".join(parts)
    if name == "hdmi_color_range":
        samples = data.get("samples") or []
        if not samples:
            return result.message
        worst = max(samples, key=lambda item: float(item.get("mean_abs_error") or 0))
        missed = sum(1 for item in samples if not item.get("target_detected"))
        max_mean_error = max((float(item.get("mean_abs_error") or 0) for item in samples), default=0.0)
        max_abs_error = max((float(item.get("max_abs_error") or 0) for item in samples), default=0.0)
        return (
            f"最大平均误差={max_mean_error:.1f}，最大通道误差={max_abs_error:.1f}，"
            f"未命中颜色={missed}/{len(samples)}，"
            f"最差={worst.get('name')} 实测RGB={worst.get('measured_rgb_mean')}，"
            f"采样帧={worst.get('frames_seen', 0)}"
        )
    if name == "hid_input":
        count = int(data.get("event_count") or len(data.get("events") or []))
        keyboard = data.get("keyboard") or {}
        mouse = data.get("mouse") or {}
        parts = [
            f"捕获事件数={count}",
            f"字母数字={keyboard.get('alphanumeric_tested', 0)}",
            f"功能键={keyboard.get('function_keys_tested', 0)}",
            f"安全组合键={keyboard.get('safe_combos_tested', 0)}",
            f"绝对鼠标={'通过' if mouse.get('absolute_ok') else '失败'}",
            f"相对鼠标={'通过' if mouse.get('relative_ok') else '失败'}",
        ]
        missing = []
        for key in (
            "missing_alphanumeric_down",
            "missing_alphanumeric_up",
            "missing_function_down",
            "missing_function_up",
            "missing_combos",
        ):
            values = keyboard.get(key) or []
            if values:
                missing.append(f"{key}={','.join(map(str, values[:10]))}")
        if missing:
            parts.append("缺失：" + "；".join(missing))
        return "，".join(parts)
    if name == "hid_latency":
        if not any(k in data for k in ("p50_ms", "p95_ms", "max_ms")):
            return ""
        parts = [
            f"按键={data.get('key', '')}",
            f"p50={float(data.get('p50_ms', 0)):.1f}ms",
            f"p95={float(data.get('p95_ms', 0)):.1f}ms",
            f"max={float(data.get('max_ms', 0)):.1f}ms",
        ]
        missing = int(data.get("missing_trials") or 0)
        if missing:
            parts.append(f"未捕获={missing}")
        return "，".join(parts)
    if name == "msd":
        verify = data.get("verify") or {}
        size = verify.get("size_bytes")
        if not size:
            return ""
        parts = [f"读写校验大小={size} bytes"]
        if "write_mib_s" in verify:
            parts.append(f"写入={float(verify.get('write_mib_s') or 0):.2f}MiB/s")
        if "read_mib_s" in verify:
            parts.append(f"未缓存读取={float(verify.get('read_mib_s') or 0):.2f}MiB/s")
        elif "cached_read_mib_s" in verify:
            parts.append(f"缓存读取={float(verify.get('cached_read_mib_s') or 0):.2f}MiB/s（仅校验）")
        if "write_ms" in verify and "read_ms" in verify:
            parts.append(f"写耗时={float(verify.get('write_ms') or 0):.1f}ms，读耗时={float(verify.get('read_ms') or 0):.1f}ms")
        if verify.get("read_cached"):
            parts.append("读速未作为真实盘读速")
        return "，".join(parts)
    if name == "atx_api":
        return "status/config/WOL API 均有响应"
    if name == "windows_agent":
        hello = data.get("hello") or {}
        return f"主机={hello.get('hostname', 'unknown')}"
    if name == "target_inventory":
        return "已采集 lsusb、uname、服务状态"
    if name == "target_reset":
        return "数据库已备份，服务已重启"
    if name == "setup_init":
        return "初始化检查完成"
    if name in {"login", "login_after_restart"}:
        return "认证成功"
    if name == "hid_msd_config":
        return "已按设备能力配置 HID/MSD"
    if name == "ventoy_resources":
        return "Ventoy 资源检查完成"
    if name == "msd_restart":
        return "服务重启完成"
    if name == "video_input_select":
        cases = data.get("cases") or []
        return "，".join(f"{c.get('fmt')} {c.get('width')}x{c.get('height')}@{c.get('fps')}" for c in cases if isinstance(c, dict))
    return ""


def display_name(name: str) -> str:
    if name.startswith("video_latency_"):
        body = name.removeprefix("video_latency_")
        for mode in ("mjpeg", "h264", "h265"):
            suffix = f"_{mode}"
            if body == mode:
                return f"视频延迟 {mode.upper()}"
            if body.endswith(suffix):
                label = body[: -len(suffix)]
                return f"视频延迟 {label} {mode.upper()}"
        return f"视频延迟 {body}"
    return DISPLAY_NAMES.get(name, name)


def format_video_case(case: dict[str, Any]) -> str:
    fmt = str(case.get("fmt") or "").lower()
    resolution = format_resolution(case)
    fps = format_fps_value(case.get("fps"))
    return " ".join(part for part in (f"{resolution}@{fps}" if resolution and fps else resolution or fps, fmt) if part)


def format_video_latency_params(case: dict[str, Any], output_mode: str) -> str:
    fmt = str(case.get("fmt") or "").lower()
    output = output_mode.lower() if output_mode else "unknown"
    resolution = format_resolution(case)
    fps = format_fps_value(case.get("fps"))
    input_part = " ".join(part for part in (f"{resolution}@{fps}" if resolution and fps else resolution or fps, fmt) if part)
    return f"{input_part}-->{output}" if input_part else output


def format_resolution(case: dict[str, Any]) -> str:
    try:
        width = int(case.get("width") or 0)
        height = int(case.get("height") or 0)
    except Exception:
        return ""
    if height > 0 and width in {1280, 1920, 2560, 3840, 4096}:
        return f"{height}p"
    if width > 0 and height > 0:
        return f"{width}x{height}"
    return ""


def format_fps(value: float) -> str:
    return f"{value:.0f}fps" if abs(value - round(value)) < 0.05 else f"{value:.1f}fps"


def format_fps_value(value: Any) -> str:
    try:
        return format_fps(float(value))
    except Exception:
        return ""


def format_latency_data(data: dict[str, Any]) -> str:
    if not any(key in data for key in ("p50_ms", "p95_ms", "max_ms")):
        return ""
    return format_latency_values(data)


def format_latency_values(data: dict[str, Any]) -> str:
    return (
        f"p50={float(data.get('p50_ms', 0)):.1f}ms，"
        f"p95={float(data.get('p95_ms', 0)):.1f}ms，"
        f"max={float(data.get('max_ms', 0)):.1f}ms"
    )


def video_latency_output_from_name(name: str) -> str:
    body = name.removeprefix("video_latency_") if name.startswith("video_latency_") else name
    for mode in ("mjpeg", "h264", "h265"):
        if body == mode or body.endswith(f"_{mode}"):
            return mode
    return ""


def has_numeric_data(result: CheckResult, metrics: list[Metric]) -> bool:
    summary = summarize_result(result, metrics)
    return any(ch.isdigit() for ch in summary)


def markdown_inline(value: str) -> str:
    return str(value).replace("\n", " | ").replace("`", "'")


def markdown_table_cell(value: str) -> str:
    return markdown_inline(value).replace("|", "\\|")


def markdown_link(path: str) -> str:
    return path.replace(" ", "%20")
