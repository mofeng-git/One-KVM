#!/usr/bin/env python3
from __future__ import annotations

import argparse
import asyncio
import getpass
import inspect
import json
import lzma
import os
import re
import socket
import struct
import sys
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from okvm_media import HDMI_COLOR_SEQUENCE, closest_hdmi_color, hex_to_rgb, jpeg_rgb_stats, percentile, rgb_error
from okvm_report import Reporter

try:
    import httpx
except ImportError:  # Keep --help usable before dependencies are installed.
    httpx = None  # type: ignore[assignment]


HID_KEY_USAGE: dict[str, tuple[int, int]] = {
    **{chr(ord("a") + i): (0x04 + i, 0x00) for i in range(26)},
    **{chr(ord("A") + i): (0x04 + i, 0x02) for i in range(26)},
    "1": (0x1E, 0x00),
    "2": (0x1F, 0x00),
    "3": (0x20, 0x00),
    "4": (0x21, 0x00),
    "5": (0x22, 0x00),
    "6": (0x23, 0x00),
    "7": (0x24, 0x00),
    "8": (0x25, 0x00),
    "9": (0x26, 0x00),
    "0": (0x27, 0x00),
    "-": (0x2D, 0x00),
    "_": (0x2D, 0x02),
    " ": (0x2C, 0x00),
    "\n": (0x28, 0x00),
}

HID_ALNUM_KEYS: list[tuple[str, int, int]] = [
    *[(chr(ord("A") + i), 0x04 + i, 0x41 + i) for i in range(26)],
    *[(str((i + 1) % 10), 0x1E + i, 0x31 + i if i < 9 else 0x30) for i in range(10)],
]
HID_ALNUM_TEXT = "abcdefghijklmnopqrstuvwxyz1234567890"
HID_FUNCTION_KEYS: list[tuple[str, int, int]] = [
    (f"F{i}", 0x39 + i, 0x6F + i) for i in range(1, 13)
]

MOD_LEFT_CTRL = 0x01
MOD_LEFT_SHIFT = 0x02

HID_SAFE_COMBOS: list[tuple[str, int, int, int]] = [
    ("Ctrl+F9", 0x42, 0x78, MOD_LEFT_CTRL),
    ("Shift+F11", 0x44, 0x7A, MOD_LEFT_SHIFT),
    ("Ctrl+Shift+F12", 0x45, 0x7B, MOD_LEFT_CTRL | MOD_LEFT_SHIFT),
]
HID_LATENCY_KEY = ("F8", 0x41, 0x77)
VIDEO_LATENCY_OUTPUT_MODES = ("mjpeg", "h264", "h265")
CHROME_BROWSER_CHANNEL = "chrome"
CHROME_BROWSER_NAME = "Chrome"
CHROME_INSTALL_COMMAND = "python -m playwright install chrome"
CHROMIUM_WINDOWS_ARGS = [
    "--enable-features=WebRtcAllowH265Receive",
    "--force-fieldtrials=WebRTC-Video-H26xPacketBuffer/Enabled",
    "--enable-gpu",
    "--ignore-gpu-blocklist",
    "--use-angle=d3d11",
]

@dataclass
class VideoInputCase:
    label: str
    input_class: str
    device: str
    fmt: str
    width: int
    height: int
    fps: float

class SSHRunner:
    def __init__(self, host: str, user: str, password: str | None, port: int = 22):
        self.host = host
        self.user = user
        self.password = password
        self.port = port

    def connect(self) -> Any:
        try:
            import paramiko
        except ImportError as exc:
            raise RuntimeError("paramiko is required for SSH; run pip install -r requirements.txt") from exc

        client = paramiko.SSHClient()
        client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
        client.connect(
            self.host,
            port=self.port,
            username=self.user,
            password=self.password,
            look_for_keys=self.password is None,
            allow_agent=self.password is None,
            timeout=15,
        )
        return client

    def run(self, command: str, timeout: int = 60) -> tuple[int, str, str]:
        client = self.connect()
        try:
            _, stdout, stderr = client.exec_command(command, timeout=timeout)
            out = stdout.read().decode("utf-8", errors="replace")
            err = stderr.read().decode("utf-8", errors="replace")
            code = stdout.channel.recv_exit_status()
            return code, out, err
        finally:
            client.close()


class ApiClient:
    def __init__(self, target: str, port: int, timeout: float = 15.0):
        if httpx is None:
            raise RuntimeError("httpx is required; run pip install -r requirements.txt")
        self.base = f"http://{target}:{port}"
        self.client = httpx.Client(base_url=self.base, timeout=timeout, follow_redirects=True)

    def close(self) -> None:
        self.client.close()

    def cookie_header(self) -> str:
        return "; ".join(f"{cookie.name}={cookie.value}" for cookie in self.client.cookies.jar)

    def request(self, method: str, path: str, **kwargs: Any) -> Any:
        response = self.client.request(method, f"/api{path}", **kwargs)
        content_type = response.headers.get("content-type", "")
        data: Any
        if "json" in content_type:
            data = response.json()
        else:
            try:
                data = response.json()
            except Exception:
                data = response.text
        if response.status_code >= 400:
            raise RuntimeError(f"{method} {path} -> HTTP {response.status_code}: {data}")
        if isinstance(data, dict) and data.get("success") is False:
            raise RuntimeError(f"{method} {path} failed: {data}")
        return data

    def get(self, path: str) -> Any:
        return self.request("GET", path)

    def post(self, path: str, payload: dict[str, Any] | None = None) -> Any:
        return self.request("POST", path, json=payload or {})

    def delete(self, path: str) -> Any:
        return self.request("DELETE", path)

    def patch(self, path: str, payload: dict[str, Any]) -> Any:
        return self.request("PATCH", path, json=payload)

    def wait_health(self, timeout: int = 60) -> dict[str, Any]:
        deadline = time.monotonic() + timeout
        last_error: Exception | None = None
        while time.monotonic() < deadline:
            try:
                return self.get("/health")
            except Exception as exc:
                last_error = exc
                time.sleep(1)
        raise TimeoutError(f"One-KVM health check did not pass in {timeout}s: {last_error}")


class AgentClient:
    def __init__(self, host: str, port: int, reporter: Reporter):
        self.host = host
        self.port = port
        self.reporter = reporter
        self.websocket: Any = None
        self.hello: dict[str, Any] | None = None
        self.connected = asyncio.Event()
        self.lock = asyncio.Lock()

    async def stop(self) -> None:
        if self.websocket:
            await self.websocket.close()
            self.websocket = None
        self.connected.clear()

    async def wait_connected(self, timeout: int = 180) -> bool:
        deadline = time.monotonic() + timeout
        last_error: Exception | None = None
        while time.monotonic() < deadline:
            try:
                await self._connect_once()
                return True
            except Exception as exc:
                last_error = exc
                await asyncio.sleep(1)
        self.reporter.add("windows_agent", "FAIL", f"failed to connect Windows agent: {last_error}")
        return False

    async def _connect_once(self) -> None:
        if self.websocket and self.connected.is_set():
            return
        try:
            import websockets
        except ImportError as exc:
            raise RuntimeError("websockets is required for agent support; run pip install -r requirements.txt") from exc

        uri = f"ws://{self.host}:{self.port}/agent"
        self.websocket = await websockets.connect(uri, max_size=8 * 1024 * 1024)
        raw = await asyncio.wait_for(self.websocket.recv(), 10)
        data = json.loads(raw)
        if data.get("type") != "hello":
            raise RuntimeError(f"unexpected agent hello: {data}")
        self.hello = data
        self.connected.set()
        self.reporter.add("windows_agent", "PASS", "connected to Windows agent", hello=data)

    async def command(self, name: str, payload: dict[str, Any] | None = None, timeout: int = 30) -> dict[str, Any]:
        if not self.websocket or not self.connected.is_set():
            raise RuntimeError("Windows agent is not connected")
        msg_id = str(uuid.uuid4())
        async with self.lock:
            await self.websocket.send(json.dumps({"id": msg_id, "command": name, "payload": payload or {}}))
            response = json.loads(await asyncio.wait_for(self.websocket.recv(), timeout))
        if not response.get("ok"):
            raise RuntimeError(response.get("error") or f"agent command failed: {name}")
        return response.get("payload") or {}


class DeviceSelector:
    CSI_HINTS = ("rkcif", "rk_hdmirx", "mipi", "csi", "platform", "hdmirx")

    def __init__(self, lsusb_tree: str, devices: dict[str, Any]):
        self.lsusb_tree = lsusb_tree
        self.devices = devices

    def classify(self, device: dict[str, Any]) -> str:
        haystack = " ".join(
            str(device.get(k, "")) for k in ("name", "driver", "path")
        ).lower()
        if any(h in haystack for h in self.CSI_HINTS) or not device.get("usb_bus"):
            return "csi_mipi"
        if re.search(r"\b(5000|10000|20000)M\b", self.lsusb_tree):
            return "usb3"
        return "usb2"

    @staticmethod
    def _find_format(device: dict[str, Any], fmt: str) -> dict[str, Any] | None:
        want = fmt.upper()
        for item in device.get("formats", []):
            got = str(item.get("format", "")).upper()
            if want in got or got in want:
                return item
        return None

    @staticmethod
    def _pick_exact(fmt: dict[str, Any], width: int, height: int, fps: float) -> tuple[int, int, float] | None:
        for res in fmt.get("resolutions", []):
            if int(res.get("width", 0)) == width and int(res.get("height", 0)) == height:
                fps_values = [float(x) for x in res.get("fps", [])]
                if not fps_values:
                    continue
                best = min(fps_values, key=lambda x: abs(x - fps))
                if best >= fps * 0.9:
                    return width, height, best
        return None

    @staticmethod
    def _pick_highest_1080(fmt: dict[str, Any]) -> tuple[int, int, float] | None:
        candidates: list[tuple[int, int, float]] = []
        for res in fmt.get("resolutions", []):
            width = int(res.get("width", 0))
            height = int(res.get("height", 0))
            if width > 1920 or height > 1080:
                continue
            for fps in res.get("fps", []):
                candidates.append((width, height, float(fps)))
        if not candidates:
            return None
        return max(candidates, key=lambda x: (x[0] * x[1], x[2]))

    def select(self) -> list[VideoInputCase]:
        video_devices = self.devices.get("video", [])
        if not video_devices:
            return []
        ordered = sorted(video_devices, key=lambda d: (not bool(d.get("has_signal")), d.get("path", "")))
        device = ordered[0]
        input_class = self.classify(device)
        path = str(device["path"])
        cases: list[VideoInputCase] = []

        if input_class == "csi_mipi":
            nv12 = self._find_format(device, "NV12")
            if nv12:
                picked = self._pick_exact(nv12, 1920, 1080, 60) or self._pick_highest_1080(nv12)
                if picked:
                    w, h, f = picked
                    cases.append(VideoInputCase("csi_mipi_nv12", input_class, path, "NV12", w, h, f))
            return cases

        mjpeg = self._find_format(device, "MJPEG")
        yuyv = self._find_format(device, "YUYV")
        target_fps = 60 if input_class == "usb3" else 30
        if mjpeg:
            picked = self._pick_exact(mjpeg, 1920, 1080, target_fps) or self._pick_highest_1080(mjpeg)
            if picked:
                w, h, f = picked
                cases.append(VideoInputCase(f"{input_class}_mjpeg", input_class, path, "MJPEG", w, h, f))
        if yuyv:
            picked = self._pick_highest_1080(yuyv)
            if picked:
                w, h, f = picked
                cases.append(VideoInputCase(f"{input_class}_yuyv", input_class, path, "YUYV", w, h, f))
        return cases


class AcceptanceRunner:
    def __init__(self, args: argparse.Namespace):
        self.args = args
        self.run_id = args.run_id or time.strftime("%Y%m%d-%H%M%S-") + uuid.uuid4().hex[:6]
        self.reporter = Reporter(Path(args.report_dir) / self.run_id, color=not args.no_color)
        self.api = ApiClient(args.target, args.http_port)
        self.ssh_password = args.ssh_password or os.environ.get("OKVM_SSH_PASSWORD")
        self.agent: AgentClient | None = None
        self.lsusb_tree = ""
        self.selected_hid_backend = "none"
        self.screenshot_keys: set[str] = set()

    async def run(self) -> int:
        if self.args.agent_host:
            self.agent = AgentClient(self.args.agent_host, self.args.agent_port, self.reporter)
            print(f"Connecting Windows agent at ws://{self.args.agent_host}:{self.args.agent_port}/agent")

        try:
            if self.args.reset:
                self.reset_target()
            self.api.wait_health(timeout=self.args.health_timeout)
            self.run_network_latency_test()
            self.setup_or_login()
            self.collect_target_inventory()
            devices = self.api.get("/devices")
            self.configure_hid_and_msd(devices)
            video_cases = self.select_video_cases(devices)
            await self.wait_for_agent_if_requested()
            await self.capture_key_screenshot("console_after_login", "登录后控制台首页")
            await self.run_video_matrix(video_cases)
            await self.run_hdmi_capture_tests(video_cases)
            await self.run_hid_test()
            await self.run_msd_test()
            self.run_atx_api_test()
            await self.capture_key_screenshot("console_after_io", "HID/MSD/ATX 后控制台状态")
            self.collect_logs()
        finally:
            if self.agent:
                await self.agent.stop()
            self.api.close()
            self.reporter.write(self.run_id, self.args.target)

        return 1 if any(r.status == "FAIL" for r in self.reporter.results) else 0

    def reset_target(self) -> None:
        if not self.ssh_password and self.args.ssh_password_prompt:
            self.ssh_password = getpass.getpass(f"SSH password for {self.args.ssh_user}@{self.args.target}: ")
        ssh = SSHRunner(self.args.target, self.args.ssh_user, self.ssh_password, self.args.ssh_port)
        data_dir = self.args.data_dir
        cmd = f"""
set -eu
RUN_ID='{shell_quote(self.run_id)}'
DATA_DIR='{shell_quote(data_dir)}'
BACKUP="$DATA_DIR/test-backups/$RUN_ID"
mkdir -p "$BACKUP"
(systemctl stop one-kvm || service one-kvm stop || true)
if [ -f "$DATA_DIR/one-kvm.db" ]; then cp -a "$DATA_DIR/one-kvm.db" "$BACKUP/one-kvm.db"; fi
if [ -f "$DATA_DIR/one-kvm.db-wal" ]; then cp -a "$DATA_DIR/one-kvm.db-wal" "$BACKUP/one-kvm.db-wal"; fi
if [ -f "$DATA_DIR/one-kvm.db-shm" ]; then cp -a "$DATA_DIR/one-kvm.db-shm" "$BACKUP/one-kvm.db-shm"; fi
rm -f "$DATA_DIR/one-kvm.db" "$DATA_DIR/one-kvm.db-wal" "$DATA_DIR/one-kvm.db-shm"
(systemctl start one-kvm || service one-kvm start || nohup /usr/bin/one-kvm >"/tmp/one-kvm-test-$RUN_ID.log" 2>&1 &)
echo "$BACKUP"
"""
        code, out, err = ssh.run(cmd, timeout=60)
        if code != 0:
            self.reporter.add("target_reset", "FAIL", "failed to reset target", stdout=out, stderr=err)
            raise RuntimeError(err or out)
        self.reporter.add("target_reset", "PASS", "database backed up and service restarted", backup=out.strip())

    def setup_or_login(self) -> None:
        setup = self.api.get("/setup")
        if setup.get("needs_setup"):
            self.api.post(
                "/setup/init",
                {
                    "username": self.args.web_user,
                    "password": self.args.web_password,
                    "hid_backend": "none",
                    "msd_enabled": False,
                    "ttyd_enabled": False,
                    "rustdesk_enabled": False,
                },
            )
            self.reporter.add("setup_init", "PASS", "initial setup completed")
        else:
            self.reporter.add("setup_init", "WARN", "target already initialized; using login path")
        self.authenticate("login", "authenticated with One-KVM")

    def authenticate(self, check_name: str, detail: str) -> None:
        self.api.post("/auth/login", {"username": self.args.web_user, "password": self.args.web_password})
        self.reporter.add(check_name, "PASS", detail)

    def run_network_latency_test(self) -> None:
        samples = max(1, int(self.args.network_latency_samples))
        timeout = max(0.1, float(self.args.network_latency_timeout))
        tcp_values: list[float] = []
        http_values: list[float] = []
        errors: list[str] = []

        for _ in range(samples):
            start = time.perf_counter_ns()
            try:
                with socket.create_connection((self.args.target, self.args.http_port), timeout=timeout):
                    pass
                tcp_values.append((time.perf_counter_ns() - start) / 1_000_000)
            except Exception as exc:
                errors.append(f"tcp_connect: {exc}")
            time.sleep(0.05)

        for _ in range(samples):
            start = time.perf_counter_ns()
            try:
                self.api.get("/health")
                http_values.append((time.perf_counter_ns() - start) / 1_000_000)
            except Exception as exc:
                errors.append(f"http_health: {exc}")
            time.sleep(0.05)

        tcp_stats = latency_stats(tcp_values)
        http_stats = latency_stats(http_values)
        if tcp_values:
            self.reporter.metric("network_tcp_connect_p50", round(float(tcp_stats["p50_ms"]), 2), "ms")
            self.reporter.metric("network_tcp_connect_p95", round(float(tcp_stats["p95_ms"]), 2), "ms")
            self.reporter.metric("network_tcp_connect_max", round(float(tcp_stats["max_ms"]), 2), "ms")
        if http_values:
            self.reporter.metric("network_http_health_p50", round(float(http_stats["p50_ms"]), 2), "ms")
            self.reporter.metric("network_http_health_p95", round(float(http_stats["p95_ms"]), 2), "ms")
            self.reporter.metric("network_http_health_max", round(float(http_stats["max_ms"]), 2), "ms")

        if tcp_values and http_values and not errors:
            status = "PASS"
        elif tcp_values or http_values:
            status = "WARN"
        else:
            status = "FAIL"
        self.reporter.add(
            "network_latency",
            status,
            "measured controller-to-target TCP and HTTP latency" if status != "FAIL" else "failed to measure network latency",
            target=self.args.target,
            port=self.args.http_port,
            samples=samples,
            tcp_connect=tcp_stats,
            http_health=http_stats,
            errors=errors[:10],
        )

    def collect_target_inventory(self) -> None:
        if not self.ssh_password and self.args.ssh_password_prompt:
            self.ssh_password = getpass.getpass(f"SSH password for {self.args.ssh_user}@{self.args.target}: ")
        ssh = SSHRunner(self.args.target, self.args.ssh_user, self.ssh_password, self.args.ssh_port)
        commands = {
            "lsusb-tree.txt": "lsusb -t || true",
            "uname.txt": "uname -a || true",
            "systemctl-status.txt": "systemctl status one-kvm --no-pager || true",
        }
        for filename, cmd in commands.items():
            code, out, err = ssh.run(cmd, timeout=30)
            text = out + ("\nSTDERR:\n" + err if err else "")
            path = self.reporter.report_dir / "evidence" / filename
            path.write_text(text, encoding="utf-8")
            self.reporter.add_evidence("目标机清单", filename, path)
            if filename == "lsusb-tree.txt":
                self.lsusb_tree = out
        self.reporter.add("target_inventory", "PASS", "collected lsusb/system evidence")

    def configure_hid_and_msd(self, devices: dict[str, Any]) -> None:
        udc = devices.get("udc", [])
        serial = devices.get("serial", [])
        if udc:
            udc_name = udc[0]["name"]
            self.api.patch(
                "/config/hid",
                {
                    "backend": "otg",
                    "otg_udc": udc_name,
                    "otg_profile": "full",
                    "otg_endpoint_budget": "auto",
                    "otg_keyboard_leds": False,
                    "mouse_absolute": True,
                },
            )
            self.selected_hid_backend = "otg"
            if not self.args.no_ventoy_sync:
                try:
                    self.sync_ventoy_resources()
                except Exception as exc:
                    self.reporter.add("ventoy_resources", "WARN", f"failed to sync Ventoy resources: {exc}")
            try:
                self.api.patch("/config/msd", {"enabled": True})
            except Exception as exc:
                if not self.msd_available():
                    self.reporter.add("hid_msd_config", "WARN", f"configured OTG HID but failed to enable MSD: {exc}", udc=udc_name)
                    return
            self.reporter.add("hid_msd_config", "PASS", "configured OTG HID and enabled MSD", udc=udc_name)
            return
        if serial:
            port = serial[0]["path"]
            self.api.patch(
                "/config/hid",
                {
                    "backend": "ch9329",
                    "ch9329_port": port,
                    "ch9329_baudrate": 9600,
                    "mouse_absolute": True,
                },
            )
            self.selected_hid_backend = "ch9329"
            self.reporter.add("hid_msd_config", "WARN", "configured CH9329 HID; MSD requires OTG and is skipped", port=port)
            return
        self.selected_hid_backend = "none"
        self.reporter.add("hid_msd_config", "FAIL", "no UDC or CH9329 serial HID device found")

    def msd_available(self) -> bool:
        try:
            status = self.api.get("/msd/status")
        except Exception:
            return False
        state = status.get("state") if isinstance(status, dict) else None
        return bool(
            isinstance(status, dict)
            and (
                status.get("available")
                or (isinstance(state, dict) and state.get("available"))
            )
        )

    def sync_ventoy_resources(self) -> None:
        source_dir = Path(self.args.ventoy_resources_dir) if self.args.ventoy_resources_dir else default_ventoy_resources_dir()
        if not source_dir.exists():
            self.reporter.add("ventoy_resources", "WARN", f"local Ventoy resources not found: {source_dir}")
            return

        remote_dir = self.args.data_dir.rstrip("/") + "/ventoy"
        required = ("boot.img", "core.img", "ventoy.disk.img")
        test_cmd = " && ".join(f"test -s {shell_arg(remote_dir + '/' + name)}" for name in required)
        ssh = SSHRunner(self.args.target, self.args.ssh_user, self.ssh_password, self.args.ssh_port)
        code, _, _ = ssh.run(test_cmd, timeout=20)
        if code == 0:
            self.reporter.add("ventoy_resources", "PASS", "Ventoy resources already present on target", path=remote_dir)
            return

        code, out, err = ssh.run(f"mkdir -p {shell_arg(remote_dir)}", timeout=30)
        if code != 0:
            raise RuntimeError(err or out or f"failed to create {remote_dir}")

        client = ssh.connect()
        try:
            sftp = client.open_sftp()
            try:
                for name in required:
                    local_plain = source_dir / name
                    local_xz = source_dir / f"{name}.xz"
                    remote_path = remote_dir + "/" + name
                    if local_plain.exists():
                        sftp.put(str(local_plain), remote_path)
                    elif local_xz.exists():
                        with lzma.open(local_xz, "rb") as src, sftp.open(remote_path, "wb") as dst:
                            while True:
                                chunk = src.read(1024 * 1024)
                                if not chunk:
                                    break
                                dst.write(chunk)
                    else:
                        raise FileNotFoundError(f"{local_plain} or {local_xz}")
            finally:
                sftp.close()
        finally:
            client.close()
        self.reporter.add("ventoy_resources", "PASS", "synced Ventoy resources to target", source=str(source_dir), path=remote_dir)

    def select_video_cases(self, devices: dict[str, Any]) -> list[VideoInputCase]:
        selector = DeviceSelector(self.lsusb_tree, devices)
        cases = selector.select()
        if not cases:
            self.reporter.add("video_input_select", "FAIL", "no suitable video input case found", devices=devices.get("video", []))
            return []
        self.reporter.add("video_input_select", "PASS", "selected video test cases", cases=[c.__dict__ for c in cases])
        return cases

    async def wait_for_agent_if_requested(self) -> None:
        if not self.agent:
            self.reporter.add("windows_agent", "SKIP", "agent host not configured")
            return
        if not await self.agent.wait_connected(timeout=self.args.agent_timeout):
            self.reporter.add("windows_agent", "FAIL", "Windows agent did not connect before timeout")

    async def capture_video_screenshot(self, output_mode: str) -> None:
        titles = {
            "mjpeg": "MJPEG 模式网页截图",
            "h264": "H264 WebRTC 模式网页截图",
            "h265": "H265 WebRTC 尝试网页截图",
        }
        await self.capture_key_screenshot(
            f"video_{output_mode}",
            titles.get(output_mode, f"{output_mode} 网页截图"),
            wait_video=True,
        )

    async def capture_key_screenshot(self, key: str, title: str, path: str = "/", wait_video: bool = False) -> None:
        if self.args.no_screenshots or key in self.screenshot_keys:
            return
        self.screenshot_keys.add(key)
        try:
            from playwright.async_api import async_playwright
        except ImportError:
            self.reporter.add("screenshot", "WARN", "playwright is not installed; screenshot skipped", key=key)
            return

        output_dir = self.reporter.report_dir / "evidence" / "screenshots"
        output_dir.mkdir(parents=True, exist_ok=True)
        output_path = output_dir / f"{key}.png"
        try:
            async with async_playwright() as p:
                browser = await p.chromium.launch(**self.chromium_launch_kwargs())
                context = await browser.new_context(ignore_https_errors=True, viewport={"width": 1440, "height": 960})
                for part in self.api.cookie_header().split("; "):
                    if not part or "=" not in part:
                        continue
                    name, value = part.split("=", 1)
                    await context.add_cookies([{"name": name, "value": value, "url": self.api.base}])
                page = await context.new_page()
                await page.goto(self.api.base + path, wait_until="domcontentloaded", timeout=15000)
                if wait_video:
                    await self.wait_for_video_screenshot_ready(page)
                else:
                    await page.wait_for_timeout(1500)
                await page.screenshot(path=str(output_path), full_page=True)
                await browser.close()
            self.reporter.add_evidence("网页截图", title, output_path)
        except Exception as exc:
            if is_playwright_runtime_error(str(exc).lower()):
                self.reporter.add("screenshot", "WARN", f"Playwright {CHROME_BROWSER_NAME} cannot start; screenshot skipped: {exc}", key=key)
                return
            self.reporter.add("screenshot", "WARN", f"failed to capture screenshot {key}: {exc}", key=key)

    async def wait_for_video_screenshot_ready(self, page: Any) -> None:
        timeout = max(0, int(self.args.screenshot_wait_ms))
        if timeout <= 0:
            return
        try:
            await page.wait_for_function(VIDEO_SCREENSHOT_READY_JS, timeout=timeout)
        except Exception:
            await page.wait_for_timeout(min(timeout, 1500))

    def chromium_launch_kwargs(self) -> dict[str, Any]:
        return {
            "headless": True,
            "args": chromium_launch_args(),
            "channel": CHROME_BROWSER_CHANNEL,
        }

    async def run_video_matrix(self, cases: list[VideoInputCase]) -> None:
        if not cases:
            return
        codecs = self.available_codecs()
        for case in cases:
            self.configure_video_case(case)
            for output_mode in ("mjpeg", "h264", "h265"):
                if output_mode in ("h264", "h265") and output_mode not in codecs:
                    self.reporter.add(
                        f"video_{case.label}_{output_mode}",
                        "SKIP",
                        f"{output_mode} is not available",
                        case=case.__dict__,
                    )
                    continue
                try:
                    self.apply_video_case(case)
                    if output_mode == "mjpeg":
                        motion_started = await self.start_mjpeg_motion()
                        try:
                            result = self.measure_mjpeg(case)
                            if motion_started:
                                result["dynamic_source_fps"] = self.args.mjpeg_motion_fps
                        finally:
                            if motion_started:
                                await self.stop_mjpeg_motion()
                    else:
                        result = await self.measure_webrtc(case, output_mode)
                    status = self.video_result_status(result)
                    self.reporter.add(f"video_{case.label}_{output_mode}", status, result.get("message", ""), **result)
                    await self.capture_video_screenshot(output_mode)
                except Exception as exc:
                    status = "SKIP" if self.is_environment_skip(exc, output_mode) else "FAIL"
                    self.reporter.add(f"video_{case.label}_{output_mode}", status, str(exc), case=case.__dict__)
                    await self.capture_video_screenshot(output_mode)

    async def start_mjpeg_motion(self) -> bool:
        if not self.agent:
            self.reporter.add("mjpeg_motion_source", "WARN", "Windows agent not connected; MJPEG fps may be affected by static-frame suppression")
            return False
        try:
            await self.agent.command("start_dynamic", {"fps": self.args.mjpeg_motion_fps}, timeout=5)
            return True
        except Exception as exc:
            self.reporter.add("mjpeg_motion_source", "WARN", f"failed to start dynamic MJPEG source: {exc}")
            return False

    async def stop_mjpeg_motion(self) -> None:
        if not self.agent:
            return
        try:
            await self.agent.command("stop_dynamic", {}, timeout=5)
        except Exception as exc:
            self.reporter.add("mjpeg_motion_source", "WARN", f"failed to stop dynamic MJPEG source: {exc}")

    def video_result_status(self, result: dict[str, Any]) -> str:
        if result.get("unsupported"):
            return "SKIP"
        if result.get("ok"):
            return "PASS"
        return "FAIL"

    @staticmethod
    def is_environment_skip(exc: Exception, output_mode: str) -> bool:
        text = str(exc).lower()
        if is_playwright_runtime_error(text):
            return True
        return output_mode == "h265" and is_webrtc_codec_unsupported_error(text)

    def set_stream_mode(self, mode: str, timeout: int = 45) -> dict[str, Any]:
        deadline = time.monotonic() + timeout
        last: Any = None
        while time.monotonic() < deadline:
            response = self.api.client.post("/api/stream/mode", json={"mode": mode})
            data = response.json()
            last = data
            if response.status_code >= 400:
                raise RuntimeError(f"POST /stream/mode -> HTTP {response.status_code}: {data}")
            if data.get("success") is False and not data.get("switching"):
                raise RuntimeError(f"POST /stream/mode failed: {data}")
            ready = self.wait_stream_mode_ready(mode, timeout=max(1, int(deadline - time.monotonic())))
            if ready:
                return data
            time.sleep(0.5)
        raise TimeoutError(f"stream mode {mode} did not become ready; last={last}")

    def wait_stream_mode_ready(self, mode: str, timeout: int = 45) -> bool:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                data = self.api.get("/stream/mode")
                current = str(data.get("mode", "")).lower()
                switching = bool(data.get("switching"))
                if not switching and (current == mode or (mode == "webrtc" and current in {"h264", "h265", "vp8", "vp9"})):
                    return True
            except Exception:
                pass
            time.sleep(0.5)
        return False

    def available_codecs(self) -> set[str]:
        try:
            data = self.api.get("/stream/codecs")
            return {c["id"] for c in data.get("codecs", []) if c.get("available")}
        except Exception as exc:
            self.reporter.add("stream_codecs", "WARN", f"failed to list codecs: {exc}")
            return {"mjpeg", "h264"}

    def apply_video_case(self, case: VideoInputCase) -> None:
        self.api.patch(
            "/config/video",
            {
                "device": case.device,
                "format": case.fmt,
                "width": case.width,
                "height": case.height,
                "fps": int(round(case.fps)),
                "quality": self.args.jpeg_quality,
            },
        )

    def configure_video_case(self, case: VideoInputCase) -> None:
        self.apply_video_case(case)
        self.reporter.add(
            f"config_video_{case.label}",
            "PASS",
            f"{case.device} {case.fmt} {case.width}x{case.height}@{case.fps}",
            case=case.__dict__,
        )

    def measure_mjpeg(self, case: VideoInputCase) -> dict[str, Any]:
        self.set_stream_mode("mjpeg")
        # One-KVM may prefer MJPEG capture when entering MJPEG/HTTP mode. Re-apply
        # the requested input after the mode switch so YUYV/NV12 cases measure
        # the intended capture format instead of the automatic MJPEG fallback.
        self.apply_video_case(case)
        self.api.post("/stream/start", {})
        client_id = f"test-{self.run_id}-{case.label}"
        deadline = time.monotonic() + self.args.sample_seconds
        frame_count = 0
        byte_count = 0
        first_frame_s: float | None = None
        for frame, frame_time, _ in self.iter_mjpeg_frames(client_id, timeout=self.args.sample_seconds, video_case=case):
            now = time.monotonic()
            if now >= deadline:
                break
            if first_frame_s is None:
                first_frame_s = frame_time
            frame_count += 1
            byte_count += len(frame)
        if frame_count == 0:
            raise RuntimeError("MJPEG stream did not become available before sample timeout")
        duration = self.args.sample_seconds
        measured_fps = frame_count / duration if duration else 0.0
        self.reporter.metric("mjpeg_fps", round(measured_fps, 2), "fps", case=case.label)
        self.reporter.metric("mjpeg_bytes", byte_count, "bytes", case=case.label)
        min_fps = min(case.fps * 0.8, case.fps - 1) if case.fps >= 5 else case.fps * 0.8
        functional = frame_count > 0
        degraded = self.args.strict_performance and functional and measured_fps < min_fps
        message = f"MJPEG {measured_fps:.1f} fps over {duration}s"
        if degraded:
            message += f" (below expected {min_fps:.1f} fps)"
        result = {
            "ok": functional and not degraded,
            "degraded": degraded,
            "message": message,
            "case": case.__dict__,
            "frames": frame_count,
            "fps": measured_fps,
            "input_requested_fps": case.fps,
            "first_frame_ms": None if first_frame_s is None else max(0, (first_frame_s - (deadline - duration)) * 1000),
            "bytes": byte_count,
        }
        if self.args.strict_performance:
            result["min_expected_fps"] = min_fps
        return result

    async def measure_webrtc(self, case: VideoInputCase, codec: str) -> dict[str, Any]:
        self.set_stream_mode(codec)
        try:
            from playwright.async_api import async_playwright
        except ImportError:
            return {"ok": False, "message": "playwright is not installed", "case": case.__dict__, "codec": codec}

        cookie_header = self.api.cookie_header()
        js = WEBRTC_MEASURE_JS
        async with async_playwright() as p:
            try:
                browser = await p.chromium.launch(**self.chromium_launch_kwargs())
            except Exception as exc:
                text = str(exc).lower()
                if is_playwright_runtime_error(text):
                    return {
                        "ok": False,
                        "unsupported": True,
                        "message": f"Playwright {CHROME_BROWSER_NAME} cannot start; run: {CHROME_INSTALL_COMMAND}",
                        "case": case.__dict__,
                        "codec": codec,
                    }
                raise
            context = await browser.new_context(ignore_https_errors=True)
            for part in cookie_header.split("; "):
                if not part or "=" not in part:
                    continue
                name, value = part.split("=", 1)
                await context.add_cookies([{"name": name, "value": value, "url": self.api.base}])
            try:
                page = await context.new_page()
                await page.goto(self.api.base)
                result = await page.evaluate(js, {"seconds": self.args.sample_seconds})
            except Exception as exc:
                text = str(exc)
                if codec == "h265" and is_webrtc_codec_unsupported_error(text):
                    return {
                        "ok": False,
                        "unsupported": True,
                        "message": "H.265 WebRTC appears unsupported by browser or streamer",
                        "case": case.__dict__,
                        "codec": codec,
                        "error": text,
                    }
                raise
            finally:
                await browser.close()
        fps = float(result.get("avgFps") or 0)
        rtt_ms = float(result.get("maxRtt") or 0) * 1000
        jitter_ms = float(result.get("maxJitter") or 0) * 1000
        self.reporter.metric("webrtc_fps", round(fps, 2), "fps", case=case.label, codec=codec)
        self.reporter.metric("webrtc_rtt_max", round(rtt_ms, 2), "ms", case=case.label, codec=codec)
        self.reporter.metric("webrtc_jitter_max", round(jitter_ms, 2), "ms", case=case.label, codec=codec)
        unsupported = codec == "h265" and int(result.get("framesDecoded") or 0) == 0 and fps == 0
        min_fps = max(1.0, min(case.fps * 0.75, case.fps - 2))
        functional = bool(result.get("connected")) and fps > 0
        degraded = self.args.strict_performance and functional and fps < min_fps
        ok = (not unsupported) and functional and not degraded
        message = "H.265 WebRTC appears unsupported by browser or decoder" if unsupported else f"{codec} WebRTC avg {fps:.1f} fps, max RTT {rtt_ms:.1f} ms"
        if degraded:
            message += f" (below expected {min_fps:.1f} fps)"
        measured = {
            "ok": bool(ok),
            "unsupported": unsupported,
            "degraded": degraded,
            "message": message,
            "case": case.__dict__,
            "codec": codec,
            "input_requested_fps": case.fps,
            **result,
        }
        if self.args.strict_performance:
            measured["min_expected_fps"] = min_fps
        return measured

    async def run_hdmi_capture_tests(self, cases: list[VideoInputCase]) -> None:
        if self.args.no_hdmi_tests:
            return
        if not self.agent:
            for name in ("hdmi_identity", "hdmi_color_range"):
                self.reporter.add(name, "SKIP", "Windows agent not connected")
            for case in cases:
                for output_mode in VIDEO_LATENCY_OUTPUT_MODES:
                    self.reporter.add(
                        self.video_latency_check_name(case, output_mode),
                        "SKIP",
                        "Windows agent not connected",
                        video_case=case.__dict__,
                        output_mode=output_mode,
                    )
            return
        case = self.pick_hdmi_case(cases)
        if not case:
            for name in ("hdmi_identity", "hdmi_color_range"):
                self.reporter.add(name, "SKIP", "no video input case available")
            for output_mode in VIDEO_LATENCY_OUTPUT_MODES:
                self.reporter.add(f"video_latency_{output_mode}", "SKIP", "no video input case available", output_mode=output_mode)
            return

        try:
            self.configure_hdmi_probe_case(case)
        except Exception as exc:
            for name in ("hdmi_identity", "hdmi_color_range"):
                self.reporter.add(name, "FAIL", f"failed to configure HDMI probe video case: {exc}")
            for latency_case in cases:
                for output_mode in VIDEO_LATENCY_OUTPUT_MODES:
                    self.reporter.add(
                        self.video_latency_check_name(latency_case, output_mode),
                        "FAIL",
                        f"failed to configure video case before latency test: {exc}",
                        video_case=latency_case.__dict__,
                        output_mode=output_mode,
                    )
            return

        try:
            await self.run_hdmi_color_test(case)
        except Exception as exc:
            self.reporter.add("hdmi_identity", "FAIL", str(exc))
            self.reporter.add("hdmi_color_range", "FAIL", str(exc))

        codecs = self.available_codecs()
        for latency_case in cases:
            for output_mode in VIDEO_LATENCY_OUTPUT_MODES:
                check_name = self.video_latency_check_name(latency_case, output_mode)
                if output_mode in ("h264", "h265") and output_mode not in codecs:
                    self.reporter.add(
                        check_name,
                        "SKIP",
                        f"{output_mode} is not available",
                        video_case=latency_case.__dict__,
                        output_mode=output_mode,
                    )
                    continue
                try:
                    self.apply_video_case(latency_case)
                    if output_mode == "mjpeg":
                        await self.run_mjpeg_latency_test(latency_case, check_name=check_name, save_evidence=False)
                    else:
                        await self.run_webrtc_latency_test(latency_case, output_mode, check_name=check_name)
                except Exception as exc:
                    status = "SKIP" if self.is_environment_skip(exc, output_mode) else "FAIL"
                    self.reporter.add(check_name, status, str(exc), video_case=latency_case.__dict__, output_mode=output_mode)

    @staticmethod
    def pick_hdmi_case(cases: list[VideoInputCase]) -> VideoInputCase | None:
        if not cases:
            return None
        for case in cases:
            if case.fmt.upper() == "MJPEG" and case.width == 1920 and case.height == 1080:
                return case
        for case in cases:
            if case.fmt.upper() == "MJPEG":
                return case
        return cases[0]

    def configure_hdmi_probe_case(self, case: VideoInputCase) -> None:
        self.api.patch(
            "/config/video",
            {
                "device": case.device,
                "format": case.fmt,
                "width": case.width,
                "height": case.height,
                "fps": int(round(case.fps)),
                "quality": self.args.jpeg_quality,
            },
        )
        self.reporter.add(
            "config_video_hdmi_probe",
            "PASS",
            f"{case.device} {case.fmt} {case.width}x{case.height}@{case.fps}",
            case=case.__dict__,
        )

    @staticmethod
    def video_latency_check_name(case: VideoInputCase, output_mode: str) -> str:
        return f"video_latency_{safe_filename(case.label)}_{safe_filename(output_mode)}"

    async def run_hdmi_color_test(self, case: VideoInputCase) -> None:
        samples: list[dict[str, Any]] = []
        for name, color in HDMI_COLOR_SEQUENCE:
            await self.agent.command("show", {"color": color, "full": True}, timeout=5)
            await asyncio.sleep(self.args.hdmi_settle_ms / 1000)
            expected = hex_to_rgb(color)
            stats = self.capture_mjpeg_rgb_mean(
                f"hdmi_color_{name}",
                timeout=self.args.hdmi_capture_timeout,
                evidence_title=f"HDMI 纯色采集帧 {name}",
                expected_rgb=expected,
                threshold=self.args.hdmi_color_fail,
                video_case=case,
            )
            err = rgb_error(expected, stats["mean_rgb"])
            closest_name, closest_error = closest_hdmi_color(stats["mean_rgb"])
            sample = {
                "name": name,
                "expected_hex": color,
                "expected_rgb": expected,
                "measured_rgb_mean": stats["mean_rgb"],
                "measured_rgb_stddev": stats["stddev_rgb"],
                "width": stats["width"],
                "height": stats["height"],
                "mean_abs_error": err["mean_abs_error"],
                "max_abs_error": err["max_abs_error"],
                "closest_color": closest_name,
                "closest_error": closest_error,
                "target_detected": bool(stats.get("target_detected")),
                "frames_seen": int(stats.get("frames_seen") or 0),
            }
            sample["identity_match"] = self.hdmi_identity_match(sample)
            samples.append(sample)
            self.reporter.metric("hdmi_color_mean_abs_error", round(err["mean_abs_error"], 2), "rgb_level", color=name)
            self.reporter.metric("hdmi_color_max_abs_error", round(err["max_abs_error"], 2), "rgb_level", color=name)

        identity_colors = [s for s in samples if s["name"] in {"red", "green", "blue"}]
        identity_ok = bool(identity_colors) and all(bool(s["identity_match"]) for s in identity_colors)
        identity_status = "PASS" if identity_ok else "FAIL"
        self.reporter.add(
            "hdmi_identity",
            identity_status,
            "HDMI capture matches Windows fullscreen color sequence" if identity_ok else "HDMI capture did not match Windows fullscreen color sequence",
            samples=identity_colors,
            video_case=case.__dict__,
        )

        max_mean_error = max((float(s["mean_abs_error"]) for s in samples), default=999.0)
        max_abs_error = max((float(s["max_abs_error"]) for s in samples), default=999.0)
        if max_mean_error <= self.args.hdmi_color_warn:
            color_status = "PASS"
        elif max_mean_error <= self.args.hdmi_color_fail:
            color_status = "WARN"
        else:
            color_status = "FAIL"
        self.reporter.add(
            "hdmi_color_range",
            color_status,
            f"max mean RGB error {max_mean_error:.1f}, max channel error {max_abs_error:.1f}",
            samples=samples,
            warn_threshold=self.args.hdmi_color_warn,
            fail_threshold=self.args.hdmi_color_fail,
            video_case=case.__dict__,
        )

    @staticmethod
    def hdmi_identity_match(sample: dict[str, Any]) -> bool:
        name = str(sample["name"])
        mean = tuple(float(x) for x in sample["measured_rgb_mean"])
        if name == "red":
            return mean[0] > 120 and mean[0] > mean[1] * 1.8 and mean[0] > mean[2] * 1.8
        if name == "green":
            return mean[1] > 120 and mean[1] > mean[0] * 1.8 and mean[1] > mean[2] * 1.8
        if name == "blue":
            return mean[2] > 120 and mean[2] > mean[0] * 1.8 and mean[2] > mean[1] * 1.8
        return float(sample["mean_abs_error"]) <= 60

    async def run_mjpeg_latency_test(self, case: VideoInputCase, check_name: str = "hdmi_latency_mjpeg", save_evidence: bool = True) -> None:
        if self.args.hdmi_latency_trials <= 0:
            self.reporter.add(check_name, "SKIP", "video latency trials disabled", video_case=case.__dict__, output_mode="mjpeg")
            return
        offset_ns, sync = await self.sync_agent_clock(f"{check_name}_agent_clock_sync_rtt")
        trials: list[dict[str, Any]] = []
        colors = [("#ff0000", "#00ff00"), ("#00ff00", "#0000ff"), ("#0000ff", "#ff0000")]
        for i in range(self.args.hdmi_latency_trials):
            source, target = colors[i % len(colors)]
            await self.agent.command("show", {"color": source, "full": True}, timeout=5)
            await asyncio.sleep(self.args.hdmi_settle_ms / 1000)
            await self.agent.command("schedule_color", {"color": target, "delay_ms": self.args.hdmi_latency_delay_ms}, timeout=5)
            detect_timeout = (self.args.hdmi_latency_delay_ms + self.args.hdmi_latency_timeout_ms) / 1000
            detected = self.detect_mjpeg_color(
                hex_to_rgb(target),
                f"{check_name}_{i}",
                timeout=detect_timeout,
                threshold=self.args.hdmi_color_fail,
                evidence_title=f"HDMI 延迟命中帧 {case.label} #{i + 1}" if save_evidence else None,
                video_case=case,
            )
            display = await self.agent.command("display_state", {}, timeout=5)
            actual_agent_ns = int(display.get("last_change_unix_nano") or 0)
            actual_linux_ns = actual_agent_ns - offset_ns
            latency_ms = (int(detected["wall_ns"]) - actual_linux_ns) / 1_000_000
            trial = {
                "trial": i + 1,
                "source": source,
                "target": target,
                "latency_ms": latency_ms,
                "detected_rgb_mean": detected["mean_rgb"],
                "mean_abs_error": detected["mean_abs_error"],
                "detected_wall_ns": detected["wall_ns"],
                "agent_change_unix_nano": actual_agent_ns,
                "clock_sync": sync,
            }
            trials.append(trial)
            self.reporter.metric(check_name, round(latency_ms, 2), "ms", trial=i + 1, target=target, case=case.label)

        latencies = [float(t["latency_ms"]) for t in trials]
        p50 = percentile(latencies, 50)
        p95 = percentile(latencies, 95)
        max_latency = max(latencies) if latencies else 0.0
        self.reporter.metric(f"{check_name}_p50", round(p50, 2), "ms", case=case.label)
        self.reporter.metric(f"{check_name}_p95", round(p95, 2), "ms", case=case.label)
        self.reporter.metric(f"{check_name}_max", round(max_latency, 2), "ms", case=case.label)
        if not trials:
            status = "FAIL"
        elif p95 <= self.args.hdmi_latency_fail_ms:
            status = "PASS"
        else:
            status = "FAIL"
        self.reporter.add(
            check_name,
            status,
            f"MJPEG visual latency for {case.label}: p50 {p50:.1f} ms, p95 {p95:.1f} ms, max {max_latency:.1f} ms",
            trials=trials,
            p50_ms=p50,
            p95_ms=p95,
            max_ms=max_latency,
            functional_threshold_ms=self.args.hdmi_latency_fail_ms,
            video_case=case.__dict__,
            output_mode="mjpeg",
        )

    async def run_webrtc_latency_test(self, case: VideoInputCase, output_mode: str, check_name: str) -> None:
        if self.args.hdmi_latency_trials <= 0:
            self.reporter.add(check_name, "SKIP", "video latency trials disabled", video_case=case.__dict__, output_mode=output_mode)
            return
        self.set_stream_mode(output_mode)
        self.apply_video_case(case)
        try:
            from playwright.async_api import async_playwright
        except ImportError as exc:
            raise RuntimeError("playwright is required for WebRTC visual latency tests; run pip install -r requirements.txt") from exc

        offset_ns, sync = await self.sync_agent_clock(f"{check_name}_agent_clock_sync_rtt")
        cookie_header = self.api.cookie_header()
        trials: list[dict[str, Any]] = []
        colors = [("#ff0000", "#00ff00"), ("#00ff00", "#0000ff"), ("#0000ff", "#ff0000")]
        async with async_playwright() as p:
            try:
                browser = await p.chromium.launch(**self.chromium_launch_kwargs())
            except Exception as exc:
                text = str(exc).lower()
                if is_playwright_runtime_error(text):
                    raise RuntimeError(f"Playwright {CHROME_BROWSER_NAME} cannot start; run: {CHROME_INSTALL_COMMAND}") from exc
                raise
            context = await browser.new_context(ignore_https_errors=True)
            for part in cookie_header.split("; "):
                if not part or "=" not in part:
                    continue
                name, value = part.split("=", 1)
                await context.add_cookies([{"name": name, "value": value, "url": self.api.base}])
            page = await context.new_page()
            try:
                await page.goto(self.api.base, wait_until="domcontentloaded", timeout=15000)
                setup = await page.evaluate(WEBRTC_LATENCY_SETUP_JS, {"timeoutMs": 15000})
                if not setup.get("connected"):
                    raise RuntimeError(f"{output_mode} WebRTC did not connect: {setup}")
                for i in range(self.args.hdmi_latency_trials):
                    source, target = colors[i % len(colors)]
                    await self.agent.command("show", {"color": source, "full": True}, timeout=5)
                    await asyncio.sleep(self.args.hdmi_settle_ms / 1000)
                    source_seen = await page.evaluate(
                        WEBRTC_COLOR_DETECT_JS,
                        {
                            "targetRgb": hex_to_rgb(source),
                            "timeoutMs": max(1000, self.args.hdmi_settle_ms + 2000),
                            "threshold": self.args.hdmi_color_fail,
                        },
                    )
                    if not source_seen.get("target_detected"):
                        raise RuntimeError(f"{output_mode} WebRTC did not show source color before latency trial: {source_seen}")

                    detect_timeout_ms = self.args.hdmi_latency_delay_ms + self.args.hdmi_latency_timeout_ms
                    detect_task = asyncio.create_task(
                        page.evaluate(
                            WEBRTC_COLOR_DETECT_JS,
                            {
                                "targetRgb": hex_to_rgb(target),
                                "timeoutMs": detect_timeout_ms,
                                "threshold": self.args.hdmi_color_fail,
                            },
                        )
                    )
                    await self.agent.command("schedule_color", {"color": target, "delay_ms": self.args.hdmi_latency_delay_ms}, timeout=5)
                    detected = await detect_task
                    if not detected.get("target_detected"):
                        raise RuntimeError(f"{output_mode} WebRTC target color not detected before timeout: {detected}")
                    display = await self.agent.command("display_state", {}, timeout=5)
                    actual_agent_ns = int(display.get("last_change_unix_nano") or 0)
                    actual_linux_ns = actual_agent_ns - offset_ns
                    latency_ms = (int(detected["wall_ns"]) - actual_linux_ns) / 1_000_000
                    trial = {
                        "trial": i + 1,
                        "source": source,
                        "target": target,
                        "latency_ms": latency_ms,
                        "detected_rgb_mean": detected["mean_rgb"],
                        "mean_abs_error": detected["mean_abs_error"],
                        "detected_wall_ns": detected["wall_ns"],
                        "agent_change_unix_nano": actual_agent_ns,
                        "clock_sync": sync,
                    }
                    trials.append(trial)
                    self.reporter.metric(check_name, round(latency_ms, 2), "ms", trial=i + 1, target=target, case=case.label, codec=output_mode)
            finally:
                try:
                    await page.evaluate(WEBRTC_LATENCY_CLOSE_JS)
                except Exception:
                    pass
                await browser.close()

        latencies = [float(t["latency_ms"]) for t in trials]
        p50 = percentile(latencies, 50)
        p95 = percentile(latencies, 95)
        max_latency = max(latencies) if latencies else 0.0
        self.reporter.metric(f"{check_name}_p50", round(p50, 2), "ms", case=case.label, codec=output_mode)
        self.reporter.metric(f"{check_name}_p95", round(p95, 2), "ms", case=case.label, codec=output_mode)
        self.reporter.metric(f"{check_name}_max", round(max_latency, 2), "ms", case=case.label, codec=output_mode)
        status = "PASS" if trials and p95 <= self.args.hdmi_latency_fail_ms else "FAIL"
        self.reporter.add(
            check_name,
            status,
            f"{output_mode} visual latency for {case.label}: p50 {p50:.1f} ms, p95 {p95:.1f} ms, max {max_latency:.1f} ms",
            trials=trials,
            p50_ms=p50,
            p95_ms=p95,
            max_ms=max_latency,
            functional_threshold_ms=self.args.hdmi_latency_fail_ms,
            video_case=case.__dict__,
            output_mode=output_mode,
        )

    async def sync_agent_clock(self, metric_name: str = "agent_clock_sync_rtt") -> tuple[int, dict[str, Any]]:
        best: tuple[int, int, dict[str, Any]] | None = None
        for _ in range(7):
            t0 = time.time_ns()
            payload = await self.agent.command("ping", {}, timeout=5)
            t1 = time.time_ns()
            rtt = t1 - t0
            midpoint = (t0 + t1) // 2
            offset = int(payload.get("unix_nano") or 0) - midpoint
            detail = {
                "offset_ns": offset,
                "rtt_ns": rtt,
                "offset_ms": offset / 1_000_000,
                "rtt_ms": rtt / 1_000_000,
            }
            if best is None or rtt < best[0]:
                best = (rtt, offset, detail)
            await asyncio.sleep(0.05)
        assert best is not None
        self.reporter.metric(metric_name, round(best[2]["rtt_ms"], 3), "ms")
        return best[1], best[2]

    def capture_mjpeg_rgb_mean(
        self,
        client_label: str,
        timeout: float = 6.0,
        evidence_title: str | None = None,
        expected_rgb: tuple[int, int, int] | None = None,
        threshold: float | None = None,
        video_case: VideoInputCase | None = None,
    ) -> dict[str, Any]:
        threshold = self.args.hdmi_color_fail if threshold is None else threshold
        required_matches = max(1, int(self.args.hdmi_match_frames))
        best: dict[str, Any] | None = None
        best_frame: bytes | None = None
        last: dict[str, Any] | None = None
        last_frame: bytes | None = None
        consecutive_matches = 0
        frames_seen = 0

        for frame, _, wall_ns in self.iter_mjpeg_frames(client_label, timeout, video_case=video_case):
            frames_seen += 1
            stats = jpeg_rgb_stats(frame)
            stats["wall_ns"] = wall_ns
            stats["frames_seen"] = frames_seen
            last = stats
            last_frame = frame
            if expected_rgb is None:
                stats["target_detected"] = True
                if evidence_title:
                    self.save_frame_evidence(client_label, evidence_title, frame)
                return stats

            err = rgb_error(expected_rgb, stats["mean_rgb"])
            stats.update(err)
            if best is None or float(err["mean_abs_error"]) < float(best.get("mean_abs_error", 999.0)):
                best = dict(stats)
                best_frame = frame

            if float(err["mean_abs_error"]) <= threshold:
                consecutive_matches += 1
                if consecutive_matches >= required_matches:
                    stats["target_detected"] = True
                    if evidence_title:
                        self.save_frame_evidence(client_label, evidence_title, frame)
                    return stats
            else:
                consecutive_matches = 0

        chosen = best or last
        chosen_frame = best_frame or last_frame
        if chosen is not None and chosen_frame is not None:
            chosen["target_detected"] = False
            chosen["frames_seen"] = frames_seen
            if evidence_title:
                self.save_frame_evidence(client_label, evidence_title, chosen_frame)
            return chosen
        raise RuntimeError(f"no MJPEG frame captured for {client_label}")

    def detect_mjpeg_color(
        self,
        expected_rgb: tuple[int, int, int],
        client_label: str,
        timeout: float,
        threshold: float,
        evidence_title: str | None = None,
        video_case: VideoInputCase | None = None,
    ) -> dict[str, Any]:
        last: dict[str, Any] | None = None
        last_frame: bytes | None = None
        frames_seen = 0
        for frame, _, wall_ns in self.iter_mjpeg_frames(client_label, timeout, video_case=video_case):
            frames_seen += 1
            stats = jpeg_rgb_stats(frame)
            err = rgb_error(expected_rgb, stats["mean_rgb"])
            stats.update(err)
            stats["wall_ns"] = wall_ns
            stats["frames_seen"] = frames_seen
            last = stats
            last_frame = frame
            if float(err["mean_abs_error"]) <= threshold:
                stats["target_detected"] = True
                if evidence_title:
                    self.save_frame_evidence(client_label, evidence_title, frame)
                return stats
        if last_frame is not None and evidence_title:
            self.save_frame_evidence(f"{client_label}_last", f"{evidence_title}（未命中末帧）", last_frame)
        raise RuntimeError(f"target HDMI color not detected before timeout; last={last}")

    def save_frame_evidence(self, label: str, title: str, frame: bytes) -> None:
        output_dir = self.reporter.report_dir / "evidence" / "hdmi-frames"
        output_dir.mkdir(parents=True, exist_ok=True)
        path = output_dir / f"{safe_filename(label)}.jpg"
        path.write_bytes(frame)
        self.reporter.add_evidence("HDMI 采集帧", title, path)

    def iter_mjpeg_frames(self, client_label: str, timeout: float, video_case: VideoInputCase | None = None):
        self.set_stream_mode("mjpeg")
        if video_case is not None:
            self.apply_video_case(video_case)
        self.api.post("/stream/start", {})
        client_id = f"test-{self.run_id}-{client_label}"
        url = f"{self.api.base}/api/stream/mjpeg?client_id={client_id}"
        deadline = time.monotonic() + timeout
        buf = b""
        while time.monotonic() < deadline:
            try:
                stream_timeout = httpx.Timeout(timeout=max(1.0, timeout), connect=5.0, read=1.0, write=5.0, pool=5.0)
                with self.api.client.stream("GET", url, timeout=stream_timeout) as response:
                    response.raise_for_status()
                    for chunk in response.iter_bytes():
                        if time.monotonic() >= deadline:
                            return
                        if not chunk:
                            continue
                        buf += chunk
                        while True:
                            soi = buf.find(b"\xff\xd8")
                            if soi < 0:
                                buf = buf[-2:]
                                break
                            eoi = buf.find(b"\xff\xd9", soi + 2)
                            if eoi < 0:
                                buf = buf[soi:]
                                if len(buf) > 8 * 1024 * 1024:
                                    buf = buf[-1024 * 1024:]
                                break
                            frame = buf[soi : eoi + 2]
                            buf = buf[eoi + 2 :]
                            yield frame, time.monotonic(), time.time_ns()
            except httpx.ReadTimeout:
                continue
            except httpx.HTTPStatusError as exc:
                if exc.response.status_code != 503:
                    raise
                time.sleep(0.5)
                self.set_stream_mode("mjpeg", timeout=10)
                if video_case is not None:
                    self.apply_video_case(video_case)
                self.api.post("/stream/start", {})

    async def run_hid_test(self) -> None:
        try:
            status = self.api.get("/hid/status")
        except Exception as exc:
            self.reporter.add("hid_status", "FAIL", str(exc))
            return
        if not status.get("available"):
            self.reporter.add("hid_status", "FAIL", "HID is not available", status=status)
            return
        self.reporter.add("hid_status", "PASS", "HID backend is available", status=status)
        if not self.agent:
            self.reporter.add("hid_input", "SKIP", "Windows agent not connected")
            self.reporter.add("hid_latency", "SKIP", "Windows agent not connected")
            return
        try:
            keyboard_events = await self.run_hid_keyboard_matrix()
            keyboard = self.evaluate_hid_keyboard_events(keyboard_events)
            mouse = await self.run_hid_mouse_matrix()
            event_count = len(keyboard_events) + len(mouse.get("events", []))
            ok = bool(keyboard.get("ok")) and bool(mouse.get("ok"))
            message = (
                "HID matrix passed: alphanumeric, function keys, safe combos, absolute/relative mouse"
                if ok
                else "HID matrix failed; see missing key/mouse details"
            )
            sample_events = [*keyboard_events[:60], *(mouse.get("events", [])[:20])]
            self.reporter.add(
                "hid_input",
                "PASS" if ok else "FAIL",
                message,
                event_count=event_count,
                chars=keyboard.get("chars", ""),
                keyboard=keyboard,
                mouse={k: v for k, v in mouse.items() if k != "events"},
                events=sample_events,
            )
        except Exception as exc:
            self.reporter.add("hid_input", "FAIL", str(exc))
        try:
            await self.run_hid_latency_test()
        except Exception as exc:
            self.reporter.add("hid_latency", "FAIL", str(exc))

    async def connect_hid_websocket(self) -> Any:
        try:
            import websockets
        except ImportError as exc:
            raise RuntimeError("websockets is required for HID test") from exc

        headers = {"Cookie": self.api.cookie_header()}
        uri = f"ws://{self.args.target}:{self.args.http_port}/api/ws/hid"
        header_arg = "additional_headers" if "additional_headers" in inspect.signature(websockets.connect).parameters else "extra_headers"
        connect_kwargs = {"max_size": 1024 * 1024, header_arg: headers}
        ws = await websockets.connect(uri, **connect_kwargs)
        initial = await ws.recv()
        if isinstance(initial, str) or not initial or initial[0] != 0:
            await ws.close()
            raise RuntimeError(f"HID WebSocket unavailable: {initial!r}")
        return ws

    async def run_hid_keyboard_matrix(self) -> list[dict[str, Any]]:
        if not self.agent:
            return []
        await self.agent.command("begin_hid_capture", {}, timeout=10)
        await asyncio.sleep(0.3)
        ws = await self.connect_hid_websocket()
        try:
            for ch in HID_ALNUM_TEXT:
                usage, mod = HID_KEY_USAGE[ch]
                await self.send_hid_key(ws, usage, mod)
            for _, usage, _ in HID_FUNCTION_KEYS:
                await self.send_hid_key(ws, usage, 0)
            for _, usage, _, mod in HID_SAFE_COMBOS:
                await self.send_hid_key(ws, usage, mod)
        finally:
            await ws.close()
        await asyncio.sleep(1)
        payload = await self.agent.command("get_hid_events", {}, timeout=10)
        return payload.get("events", [])

    async def run_hid_mouse_matrix(self) -> dict[str, Any]:
        if not self.agent:
            return {"ok": False, "reason": "Windows agent not connected", "events": []}

        screen = ((self.agent.hello or {}).get("screen") or {}) if self.agent else {}
        width = int(screen.get("width") or 0)
        height = int(screen.get("height") or 0)
        if width <= 0 or height <= 0:
            return {"ok": False, "reason": "Windows agent did not report screen size", "events": []}

        await self.agent.command("begin_hid_capture", {}, timeout=10)
        await asyncio.sleep(0.3)
        ws = await self.connect_hid_websocket()
        try:
            await self.send_hid_mouse(ws, 0x01, 8192, 8192, 0)
            await asyncio.sleep(0.2)
            abs_x = 19660
            abs_y = 19660
            await self.send_hid_mouse(ws, 0x01, abs_x, abs_y, 0)
            await asyncio.sleep(0.4)
            payload = await self.agent.command("get_hid_events", {}, timeout=10)
            base_events = payload.get("events", [])
            base_move = self.last_event_of_type(base_events, "mouse_move")

            rel_dx = 32
            rel_dy = 24
            await self.send_hid_mouse(ws, 0x00, rel_dx, rel_dy, 0)
            await asyncio.sleep(0.4)
            payload = await self.agent.command("get_hid_events", {}, timeout=10)
            events = payload.get("events", [])
        finally:
            await ws.close()

        target_x = self.hid_abs_to_pixel(abs_x, width)
        target_y = self.hid_abs_to_pixel(abs_y, height)
        abs_tolerance_x = max(100, int(width * 0.10))
        abs_tolerance_y = max(80, int(height * 0.10))
        abs_ok = bool(
            base_move
            and abs(int(base_move.get("x", -9999)) - target_x) <= abs_tolerance_x
            and abs(int(base_move.get("y", -9999)) - target_y) <= abs_tolerance_y
        )

        relative_events = events[len(base_events) :]
        rel_move = self.last_event_of_type(relative_events, "mouse_move")
        rel_delta_x = int(rel_move.get("x", 0)) - int(base_move.get("x", 0)) if rel_move and base_move else 0
        rel_delta_y = int(rel_move.get("y", 0)) - int(base_move.get("y", 0)) if rel_move and base_move else 0
        rel_ok = bool(
            base_move
            and rel_move
            and 2 <= rel_delta_x <= max(300, int(width * 0.25))
            and 2 <= rel_delta_y <= max(300, int(height * 0.25))
        )

        return {
            "ok": abs_ok and rel_ok,
            "absolute_ok": abs_ok,
            "relative_ok": rel_ok,
            "screen": {"width": width, "height": height},
            "absolute_target": {"x": target_x, "y": target_y},
            "absolute_observed": {"x": base_move.get("x"), "y": base_move.get("y")} if base_move else None,
            "relative_delta": {"x": rel_delta_x, "y": rel_delta_y},
            "events": events,
        }

    async def run_hid_latency_test(self) -> None:
        if not self.agent:
            self.reporter.add("hid_latency", "SKIP", "Windows agent not connected")
            return
        trial_count = max(0, int(self.args.hid_latency_trials))
        if trial_count <= 0:
            self.reporter.add("hid_latency", "SKIP", "HID latency trials disabled")
            return

        key_name, usage, vk = HID_LATENCY_KEY
        offset_ns, sync = await self.sync_agent_clock("hid_agent_clock_sync_rtt")
        trials: list[dict[str, Any]] = []
        missing = 0
        ws = await self.connect_hid_websocket()
        try:
            for i in range(trial_count):
                await self.agent.command("begin_hid_capture", {}, timeout=10)
                await asyncio.sleep(0.1)
                send_ns = time.time_ns()
                await ws.send(bytes([0x01, 0x00, usage, 0x00]))
                await asyncio.sleep(0.02)
                await ws.send(bytes([0x01, 0x01, usage, 0x00]))
                event = await self.wait_hid_key_down(vk, timeout=2.0)
                if not event:
                    missing += 1
                    continue
                event_agent_ns = int(event.get("unix_nano") or 0)
                event_linux_ns = event_agent_ns - offset_ns
                raw_latency_ms = (event_linux_ns - send_ns) / 1_000_000
                latency_ms = max(0.0, raw_latency_ms)
                trial = {
                    "trial": i + 1,
                    "key": key_name,
                    "latency_ms": latency_ms,
                    "raw_latency_ms": raw_latency_ms,
                    "sent_unix_nano": send_ns,
                    "event_agent_unix_nano": event_agent_ns,
                    "event_linux_unix_nano": event_linux_ns,
                    "clock_sync": sync,
                }
                trials.append(trial)
                self.reporter.metric("hid_latency", round(latency_ms, 2), "ms", trial=i + 1, key=key_name)
                await asyncio.sleep(0.08)
        finally:
            await ws.close()

        latencies = [float(t["latency_ms"]) for t in trials]
        p50 = percentile(latencies, 50)
        p95 = percentile(latencies, 95)
        max_latency = max(latencies) if latencies else 0.0
        if latencies:
            self.reporter.metric("hid_latency_p50", round(p50, 2), "ms", key=key_name)
            self.reporter.metric("hid_latency_p95", round(p95, 2), "ms", key=key_name)
            self.reporter.metric("hid_latency_max", round(max_latency, 2), "ms", key=key_name)

        if not trials:
            status = "FAIL"
        elif p95 <= self.args.hid_latency_warn_ms and missing == 0:
            status = "PASS"
        elif p95 <= self.args.hid_latency_fail_ms:
            status = "WARN"
        else:
            status = "FAIL"
        self.reporter.add(
            "hid_latency",
            status,
            f"HID key latency p50 {p50:.1f} ms, p95 {p95:.1f} ms, max {max_latency:.1f} ms",
            key=key_name,
            trials=trials,
            missing_trials=missing,
            p50_ms=p50,
            p95_ms=p95,
            max_ms=max_latency,
            warn_threshold_ms=self.args.hid_latency_warn_ms,
            fail_threshold_ms=self.args.hid_latency_fail_ms,
        )

    async def wait_hid_key_down(self, vk: int, timeout: float) -> dict[str, Any] | None:
        if not self.agent:
            return None
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            payload = await self.agent.command("get_hid_events", {}, timeout=10)
            for event in payload.get("events", []):
                if event.get("type") == "key_down" and int(event.get("code") or 0) == vk:
                    return event
            await asyncio.sleep(0.05)
        return None

    async def send_hid_sequence(self, text: str) -> None:
        ws = await self.connect_hid_websocket()
        try:
            for ch in text:
                if ch not in HID_KEY_USAGE:
                    continue
                usage, mod = HID_KEY_USAGE[ch]
                await self.send_hid_key(ws, usage, mod)
        finally:
            await ws.close()

    async def send_hid_key(self, ws: Any, usage: int, modifier: int = 0) -> None:
        await ws.send(bytes([0x01, 0x00, usage, modifier]))
        await asyncio.sleep(0.02)
        await ws.send(bytes([0x01, 0x01, usage, modifier]))
        await asyncio.sleep(0.02)

    async def send_hid_mouse(self, ws: Any, event_type: int, x: int, y: int, value: int) -> None:
        await ws.send(bytes([0x02, event_type]) + struct.pack("<hhB", x, y, value & 0xFF))

    @staticmethod
    def hid_abs_to_pixel(value: int, span: int) -> int:
        return int(round(value * max(span - 1, 0) / 32767))

    @staticmethod
    def last_event_of_type(events: list[dict[str, Any]], event_type: str) -> dict[str, Any] | None:
        for event in reversed(events):
            if event.get("type") == event_type:
                return event
        return None

    @staticmethod
    def evaluate_hid_keyboard_events(events: list[dict[str, Any]]) -> dict[str, Any]:
        chars = "".join(e.get("char", "") for e in events if e.get("type") == "char")
        down_codes = {int(e.get("code") or 0) for e in events if e.get("type") == "key_down"}
        up_codes = {int(e.get("code") or 0) for e in events if e.get("type") == "key_up"}

        missing_alnum_down = [label for label, _, vk in HID_ALNUM_KEYS if vk not in down_codes]
        missing_alnum_up = [label for label, _, vk in HID_ALNUM_KEYS if vk not in up_codes]
        missing_functions_down = [label for label, _, vk in HID_FUNCTION_KEYS if vk not in down_codes]
        missing_functions_up = [label for label, _, vk in HID_FUNCTION_KEYS if vk not in up_codes]
        missing_combos = [
            name
            for name, _, vk, mod in HID_SAFE_COMBOS
            if not any(
                e.get("type") == "key_down"
                and int(e.get("code") or 0) == vk
                and (int(e.get("modifiers") or 0) & mod) == mod
                for e in events
            )
        ]

        ok = not (
            missing_alnum_down
            or missing_alnum_up
            or missing_functions_down
            or missing_functions_up
            or missing_combos
        )
        return {
            "ok": ok,
            "chars": chars,
            "event_count": len(events),
            "alphanumeric_tested": len(HID_ALNUM_KEYS),
            "function_keys_tested": len(HID_FUNCTION_KEYS),
            "safe_combos_tested": len(HID_SAFE_COMBOS),
            "missing_alphanumeric_down": missing_alnum_down,
            "missing_alphanumeric_up": missing_alnum_up,
            "missing_function_down": missing_functions_down,
            "missing_function_up": missing_functions_up,
            "missing_combos": missing_combos,
        }

    async def run_msd_test(self) -> None:
        if self.selected_hid_backend != "otg":
            self.reporter.add("msd", "SKIP", "MSD requires OTG HID backend")
            return
        if not self.agent:
            self.reporter.add("msd", "SKIP", "Windows agent not connected")
            return
        try:
            status = self.api.get("/msd/status")
            if not status.get("available"):
                self.reporter.add("msd", "FAIL", "MSD controller is unavailable", status=status)
                return
            snapshot = await self.agent.command("msd_snapshot", {}, timeout=10)
            known = [d["root"] for d in snapshot.get("drives", [])]
            self.api.post("/msd/drive/init", {"size_mb": self.args.msd_size_mb})
            self.api.post("/msd/drive/mount", {})
            drive = await self.agent.command("msd_wait_new", {"known": known, "timeout_ms": 60000}, timeout=70)
            root = drive.get("root")
            verify = await self.agent.command(
                "msd_write_read",
                {"root": root, "filename": f"okvm-msd-{self.run_id}.bin", "size_bytes": self.args.msd_probe_bytes},
                timeout=60,
            )
            if "write_mib_s" in verify:
                self.reporter.metric("msd_write_mib_s", round(float(verify["write_mib_s"]), 2), "MiB/s")
            if "read_mib_s" in verify:
                self.reporter.metric("msd_read_mib_s", round(float(verify["read_mib_s"]), 2), "MiB/s")
            if "cached_read_mib_s" in verify:
                self.reporter.metric("msd_cached_read_mib_s", round(float(verify["cached_read_mib_s"]), 2), "MiB/s")
            self.api.delete("/msd/drive/mount")
            await self.agent.command("msd_wait_removed", {"root": root, "timeout_ms": 60000}, timeout=70)
            self.reporter.add("msd", "PASS", "Windows detected virtual drive and read/write verification passed", drive=drive, verify=verify)
        except Exception as exc:
            try:
                self.api.delete("/msd/drive/mount")
            except Exception:
                pass
            status = "SKIP" if is_msd_environment_error(str(exc)) else "FAIL"
            self.reporter.add("msd", status, str(exc))

    def run_atx_api_test(self) -> None:
        try:
            status = self.api.get("/atx/status")
            config = self.api.get("/config/atx")
            wol = self.api.post("/atx/wol", {"mac_address": self.args.wol_mac})
            self.reporter.add("atx_api", "PASS", "ATX status/config/WOL APIs responded", status=status, config=config, wol=wol)
        except Exception as exc:
            self.reporter.add("atx_api", "FAIL", str(exc))

    def collect_logs(self) -> None:
        if not self.ssh_password:
            return
        try:
            ssh = SSHRunner(self.args.target, self.args.ssh_user, self.ssh_password, self.args.ssh_port)
            code, out, err = ssh.run("journalctl -u one-kvm -n 300 --no-pager || true", timeout=30)
            path = self.reporter.report_dir / "evidence" / "journalctl-one-kvm-tail.txt"
            path.write_text(out + err, encoding="utf-8")
            self.reporter.add_evidence("日志", "one-kvm journal tail", path)
            self.reporter.add("log_collection", "PASS", "collected one-kvm journal tail")
        except Exception as exc:
            self.reporter.add("log_collection", "WARN", str(exc))


VIDEO_SCREENSHOT_READY_JS = r"""
() => {
  const text = (document.body?.innerText || '').toLowerCase();
  if (text.includes('connection failed') || text.includes('operation failed')) {
    return true;
  }
  if (text.includes('waiting for first frame') || text.includes('webrtc connected') || text.includes('please wait')) {
    return false;
  }
  const media = Array.from(document.querySelectorAll('video, canvas, img')).filter((el) => {
    const r = el.getBoundingClientRect();
    return r.width >= 320 && r.height >= 180 && r.bottom > 120;
  });
  return media.length > 0;
}
"""


WEBRTC_MEASURE_JS = r"""
async ({seconds}) => {
  const api = async (path, opts = {}) => {
    const response = await fetch('/api' + path, {
      credentials: 'include',
      headers: {'Content-Type': 'application/json', ...(opts.headers || {})},
      ...opts
    });
    const data = await response.json().catch(() => ({}));
    if (!response.ok) throw new Error(path + ' HTTP ' + response.status + ': ' + JSON.stringify(data));
    return data;
  };

  const ice = await api('/webrtc/ice-servers');
  const pc = new RTCPeerConnection({
    iceServers: (ice.ice_servers || []).map(s => ({urls: s.urls, username: s.username, credential: s.credential}))
  });
  pc.addTransceiver('video', {direction: 'recvonly'});
  pc.addTransceiver('audio', {direction: 'recvonly'});
  pc.createDataChannel('hid', {ordered: true, maxRetransmits: 3});

  let sessionId = null;
  const pending = [];
  pc.onicecandidate = (event) => {
    if (!event.candidate) return;
    const item = {
      candidate: event.candidate.candidate,
      sdpMid: event.candidate.sdpMid,
      sdpMLineIndex: event.candidate.sdpMLineIndex,
      usernameFragment: event.candidate.usernameFragment
    };
    if (sessionId) {
      api('/webrtc/ice', {method: 'POST', body: JSON.stringify({session_id: sessionId, candidate: item})}).catch(() => {});
    } else {
      pending.push(item);
    }
  };

  const offer = await pc.createOffer();
  await pc.setLocalDescription(offer);
  const answer = await api('/webrtc/offer', {method: 'POST', body: JSON.stringify({sdp: offer.sdp})});
  if (answer.success === false) {
    throw new Error('webrtc_offer_failed: ' + (answer.message || JSON.stringify(answer)));
  }
  if (typeof answer.sdp !== 'string' || !answer.sdp.trim().startsWith('v=')) {
    throw new Error('webrtc_offer_invalid_sdp: ' + JSON.stringify(answer).slice(0, 500));
  }
  sessionId = answer.session_id;
  await pc.setRemoteDescription({type: 'answer', sdp: answer.sdp});
  for (const c of answer.ice_candidates || []) {
    try { await pc.addIceCandidate(c); } catch {}
  }
  for (const c of pending.splice(0)) {
    await api('/webrtc/ice', {method: 'POST', body: JSON.stringify({session_id: sessionId, candidate: c})}).catch(() => {});
  }

  const waitStart = performance.now();
  while (pc.connectionState !== 'connected' && performance.now() - waitStart < 12000) {
    if (pc.connectionState === 'failed' || pc.connectionState === 'closed') break;
    await new Promise(r => setTimeout(r, 100));
  }

  const samples = [];
  const endAt = performance.now() + seconds * 1000;
  while (performance.now() < endAt) {
    const report = await pc.getStats();
    let sample = {fps: 0, framesDecoded: 0, framesDropped: 0, rtt: 0, jitter: 0, bytes: 0};
    report.forEach(stat => {
      if (stat.type === 'inbound-rtp' && stat.kind === 'video') {
        sample.fps = stat.framesPerSecond || 0;
        sample.framesDecoded = stat.framesDecoded || 0;
        sample.framesDropped = stat.framesDropped || 0;
        sample.jitter = stat.jitter || 0;
        sample.bytes = stat.bytesReceived || 0;
      }
      if (stat.type === 'candidate-pair' && (stat.nominated || stat.selected)) {
        sample.rtt = stat.currentRoundTripTime || 0;
      }
    });
    samples.push(sample);
    await new Promise(r => setTimeout(r, 1000));
  }
  const fpsValues = samples.map(s => s.fps).filter(v => v > 0);
  const avgFps = fpsValues.length ? fpsValues.reduce((a, b) => a + b, 0) / fpsValues.length : 0;
  const maxRtt = Math.max(0, ...samples.map(s => s.rtt || 0));
  const maxJitter = Math.max(0, ...samples.map(s => s.jitter || 0));
  const last = samples[samples.length - 1] || {};
  try { await api('/webrtc/close', {method: 'POST', body: JSON.stringify({session_id: sessionId})}); } catch {}
  pc.close();
  return {
    connected: pc.connectionState === 'connected' || samples.length > 0,
    avgFps,
    maxRtt,
    maxJitter,
    framesDecoded: last.framesDecoded || 0,
    framesDropped: last.framesDropped || 0,
    bytesReceived: last.bytes || 0,
    samples
  };
}
"""


WEBRTC_LATENCY_SETUP_JS = r"""
async ({timeoutMs}) => {
  const api = async (path, opts = {}) => {
    const response = await fetch('/api' + path, {
      credentials: 'include',
      headers: {'Content-Type': 'application/json', ...(opts.headers || {})},
      ...opts
    });
    const data = await response.json().catch(() => ({}));
    if (!response.ok) throw new Error(path + ' HTTP ' + response.status + ': ' + JSON.stringify(data));
    return data;
  };

  if (window.__okvmLatency) {
    try { await api('/webrtc/close', {method: 'POST', body: JSON.stringify({session_id: window.__okvmLatency.sessionId})}); } catch {}
    try { window.__okvmLatency.pc.close(); } catch {}
    window.__okvmLatency = null;
  }

  const ice = await api('/webrtc/ice-servers');
  const pc = new RTCPeerConnection({
    iceServers: (ice.ice_servers || []).map(s => ({urls: s.urls, username: s.username, credential: s.credential}))
  });
  const media = new MediaStream();
  const video = document.createElement('video');
  video.muted = true;
  video.autoplay = true;
  video.playsInline = true;
  video.style.cssText = 'position:fixed;left:-10000px;top:-10000px;width:640px;height:360px;';
  video.srcObject = media;
  document.body.appendChild(video);
  pc.ontrack = (event) => {
    if (event.track && event.track.kind === 'video') {
      media.addTrack(event.track);
      video.play().catch(() => {});
    }
  };
  pc.addTransceiver('video', {direction: 'recvonly'});
  pc.addTransceiver('audio', {direction: 'recvonly'});
  pc.createDataChannel('hid', {ordered: true, maxRetransmits: 3});

  let sessionId = null;
  const pending = [];
  pc.onicecandidate = (event) => {
    if (!event.candidate) return;
    const item = {
      candidate: event.candidate.candidate,
      sdpMid: event.candidate.sdpMid,
      sdpMLineIndex: event.candidate.sdpMLineIndex,
      usernameFragment: event.candidate.usernameFragment
    };
    if (sessionId) {
      api('/webrtc/ice', {method: 'POST', body: JSON.stringify({session_id: sessionId, candidate: item})}).catch(() => {});
    } else {
      pending.push(item);
    }
  };

  const offer = await pc.createOffer();
  await pc.setLocalDescription(offer);
  const answer = await api('/webrtc/offer', {method: 'POST', body: JSON.stringify({sdp: offer.sdp})});
  if (answer.success === false) {
    throw new Error('webrtc_offer_failed: ' + (answer.message || JSON.stringify(answer)));
  }
  if (typeof answer.sdp !== 'string' || !answer.sdp.trim().startsWith('v=')) {
    throw new Error('webrtc_offer_invalid_sdp: ' + JSON.stringify(answer).slice(0, 500));
  }
  sessionId = answer.session_id;
  await pc.setRemoteDescription({type: 'answer', sdp: answer.sdp});
  for (const c of answer.ice_candidates || []) {
    try { await pc.addIceCandidate(c); } catch {}
  }
  for (const c of pending.splice(0)) {
    await api('/webrtc/ice', {method: 'POST', body: JSON.stringify({session_id: sessionId, candidate: c})}).catch(() => {});
  }

  const deadline = performance.now() + timeoutMs;
  while (performance.now() < deadline) {
    if ((pc.connectionState === 'failed') || (pc.connectionState === 'closed')) break;
    if ((pc.connectionState === 'connected' || pc.iceConnectionState === 'connected') && video.videoWidth > 0 && video.readyState >= 2) break;
    await new Promise(r => setTimeout(r, 50));
  }

  const canvas = document.createElement('canvas');
  const ctx = canvas.getContext('2d', {willReadFrequently: true});
  window.__okvmLatency = {pc, sessionId, video, canvas, ctx};
  return {
    connected: pc.connectionState === 'connected' || pc.iceConnectionState === 'connected' || video.videoWidth > 0,
    connectionState: pc.connectionState,
    iceConnectionState: pc.iceConnectionState,
    videoWidth: video.videoWidth || 0,
    videoHeight: video.videoHeight || 0,
    readyState: video.readyState
  };
}
"""


WEBRTC_COLOR_DETECT_JS = r"""
async ({targetRgb, timeoutMs, threshold}) => {
  const state = window.__okvmLatency;
  if (!state || !state.video || !state.ctx) {
    throw new Error('WebRTC latency detector is not initialized');
  }
  const video = state.video;
  const canvas = state.canvas;
  const ctx = state.ctx;
  const outW = 64;
  const outH = 64;
  canvas.width = outW;
  canvas.height = outH;
  const deadline = performance.now() + timeoutMs;
  let framesSeen = 0;
  let best = null;

  const sample = () => {
    const width = video.videoWidth || 0;
    const height = video.videoHeight || 0;
    if (width <= 0 || height <= 0 || video.readyState < 2) return null;
    const sx = Math.max(0, Math.floor(width * 0.35));
    const sy = Math.max(0, Math.floor(height * 0.35));
    const sw = Math.max(1, Math.floor(width * 0.30));
    const sh = Math.max(1, Math.floor(height * 0.30));
    ctx.drawImage(video, sx, sy, sw, sh, 0, 0, outW, outH);
    const data = ctx.getImageData(0, 0, outW, outH).data;
    let r = 0, g = 0, b = 0;
    const pixels = outW * outH;
    for (let i = 0; i < data.length; i += 4) {
      r += data[i];
      g += data[i + 1];
      b += data[i + 2];
    }
    const mean = [r / pixels, g / pixels, b / pixels];
    const errors = [
      Math.abs(mean[0] - targetRgb[0]),
      Math.abs(mean[1] - targetRgb[1]),
      Math.abs(mean[2] - targetRgb[2])
    ];
    return {
      mean_rgb: mean.map(v => Math.round(v * 100) / 100),
      mean_abs_error: (errors[0] + errors[1] + errors[2]) / 3,
      max_abs_error: Math.max(...errors),
      width,
      height
    };
  };

  while (performance.now() < deadline) {
    const stats = sample();
    if (stats) {
      framesSeen += 1;
      stats.frames_seen = framesSeen;
      if (!best || stats.mean_abs_error < best.mean_abs_error) best = {...stats};
      if (stats.mean_abs_error <= threshold) {
        stats.target_detected = true;
        stats.wall_ns = Math.round((performance.timeOrigin + performance.now()) * 1000000);
        return stats;
      }
    }
    await new Promise(r => setTimeout(r, 16));
  }
  const out = best || {mean_rgb: [0, 0, 0], mean_abs_error: 999, max_abs_error: 999, width: 0, height: 0};
  out.target_detected = false;
  out.frames_seen = framesSeen;
  out.wall_ns = Math.round((performance.timeOrigin + performance.now()) * 1000000);
  return out;
}
"""


WEBRTC_LATENCY_CLOSE_JS = r"""
async () => {
  const state = window.__okvmLatency;
  if (!state) return true;
  const api = async (path, opts = {}) => {
    const response = await fetch('/api' + path, {
      credentials: 'include',
      headers: {'Content-Type': 'application/json', ...(opts.headers || {})},
      ...opts
    });
    return response.json().catch(() => ({}));
  };
  try { await api('/webrtc/close', {method: 'POST', body: JSON.stringify({session_id: state.sessionId})}); } catch {}
  try { state.pc.close(); } catch {}
  try { state.video.remove(); } catch {}
  window.__okvmLatency = null;
  return true;
}
"""


def local_ip_for(target: str) -> str:
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.connect((target, 80))
        return sock.getsockname()[0]
    except Exception:
        return "127.0.0.1"
    finally:
        try:
            sock.close()
        except Exception:
            pass


def is_playwright_dependency_error(text: str) -> bool:
    return any(
        marker in text
        for marker in (
            "host system is missing dependencies",
            "missing libraries",
            "error while loading shared libraries",
            "cannot open shared object file",
        )
    )


def is_playwright_runtime_error(text: str) -> bool:
    return (
        "playwright install" in text
        or "playwright is required" in text
        or "executable doesn't exist" in text
        or "browser distribution" in text
        or "is not found at" in text
        or "not found on your system" in text
        or is_playwright_dependency_error(text)
    )


def is_webrtc_codec_unsupported_error(text: str) -> bool:
    lower = text.lower()
    return any(
        marker in lower
        for marker in (
            "webrtc_offer_failed",
            "webrtc_offer_invalid_sdp",
            "failed to parse sessiondescription",
            "failed to execute 'setremotedescription'",
            "unsupported",
            "not supported",
            "codec",
        )
    )


def is_msd_environment_error(text: str) -> bool:
    lower = text.lower()
    return any(
        marker in lower
        for marker in (
            "resources not initialized",
            "resource not found",
            "boot.img not found",
            "core.img not found",
            "ventoy.disk.img not found",
            "ventoy resources",
        )
    )


def default_ventoy_resources_dir() -> Path:
    return Path(__file__).resolve().parents[2] / "libs" / "ventoy-img-rs" / "resources"


def shell_quote(value: str) -> str:
    return value.replace("'", "'\"'\"'")


def shell_arg(value: str) -> str:
    return "'" + shell_quote(value) + "'"


def safe_filename(value: str) -> str:
    return re.sub(r"[^A-Za-z0-9_.-]+", "_", value).strip("._") or "evidence"


def chromium_launch_args() -> list[str]:
    return list(CHROMIUM_WINDOWS_ARGS)


def is_windows_controller() -> bool:
    return sys.platform.startswith("win")


def latency_stats(values: list[float]) -> dict[str, Any]:
    if not values:
        return {"samples": 0}
    return {
        "samples": len(values),
        "p50_ms": percentile(values, 50),
        "p95_ms": percentile(values, 95),
        "max_ms": max(values),
        "min_ms": min(values),
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="One-KVM automated acceptance test controller")
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run", help="run acceptance test")
    run.add_argument("--target", required=True, help="One-KVM target IP")
    run.add_argument("--http-port", type=int, default=8080)
    run.add_argument("--ssh-user", default="root")
    run.add_argument("--ssh-port", type=int, default=22)
    run.add_argument("--ssh-password", default=None)
    run.add_argument("--ssh-password-prompt", action="store_true")
    run.add_argument("--data-dir", default="/etc/one-kvm")
    run.add_argument("--reset", action="store_true", help="backup and remove One-KVM database before testing")
    run.add_argument("--web-user", default="okvmtest")
    run.add_argument("--web-password", default="okvmtest1234")
    run.add_argument("--run-id", default=None)
    run.add_argument("--report-dir", default="reports")
    run.add_argument("--health-timeout", type=int, default=60)
    run.add_argument("--network-latency-samples", type=int, default=7, help="controller-to-target network latency samples collected during setup")
    run.add_argument("--network-latency-timeout", type=float, default=3.0, help="per-sample TCP/HTTP latency timeout in seconds")
    run.add_argument("--sample-seconds", type=int, default=30)
    run.add_argument("--jpeg-quality", type=int, default=80)
    run.add_argument("--mjpeg-motion-fps", type=int, default=60, help="Windows agent dynamic source fps used during MJPEG/HTTP tests")
    run.add_argument("--agent-host", default=None, help="Windows agent IP/hostname; omit to skip Windows-side checks")
    run.add_argument("--agent-port", type=int, default=8765)
    run.add_argument("--agent-timeout", type=int, default=180)
    run.add_argument("--msd-size-mb", type=int, default=256)
    run.add_argument("--msd-probe-bytes", type=int, default=1024 * 1024)
    run.add_argument("--ventoy-resources-dir", default=None, help="local Ventoy resource directory; defaults to repo libs/ventoy-img-rs/resources")
    run.add_argument("--no-ventoy-sync", action="store_true", help="do not copy Ventoy resources to the target before MSD testing")
    run.add_argument("--strict-performance", action="store_true", help="fail video results below the expected FPS threshold; default only requires fps > 0")
    run.add_argument("--no-color", action="store_true", help="disable colored terminal output")
    run.add_argument("--no-screenshots", action="store_true", help="skip Playwright webpage screenshots")
    run.add_argument("--screenshot-wait-ms", type=int, default=6000, help="wait up to this long for video page screenshots to reach a stable state")
    run.add_argument("--hid-latency-trials", type=int, default=5)
    run.add_argument("--hid-latency-warn-ms", type=float, default=80.0)
    run.add_argument("--hid-latency-fail-ms", type=float, default=200.0)
    run.add_argument("--no-hdmi-tests", action="store_true", help="skip HDMI identity/color and video visual latency tests")
    run.add_argument("--hdmi-settle-ms", type=int, default=600)
    run.add_argument("--hdmi-capture-timeout", type=float, default=8.0)
    run.add_argument("--hdmi-match-frames", type=int, default=1, help="consecutive matching MJPEG frames required for HDMI color detection")
    run.add_argument("--hdmi-color-warn", type=float, default=30.0)
    run.add_argument("--hdmi-color-fail", type=float, default=60.0)
    run.add_argument("--video-latency-trials", "--hdmi-latency-trials", dest="hdmi_latency_trials", type=int, default=5)
    run.add_argument("--video-latency-delay-ms", "--hdmi-latency-delay-ms", dest="hdmi_latency_delay_ms", type=int, default=2000)
    run.add_argument("--video-latency-timeout-ms", "--hdmi-latency-timeout-ms", dest="hdmi_latency_timeout_ms", type=int, default=3000)
    run.add_argument("--video-latency-warn-ms", "--hdmi-latency-warn-ms", dest="hdmi_latency_warn_ms", type=float, default=3000.0, help=argparse.SUPPRESS)
    run.add_argument("--video-latency-fail-ms", "--hdmi-latency-fail-ms", dest="hdmi_latency_fail_ms", type=float, default=3000.0)
    run.add_argument("--wol-mac", default="02:00:00:00:00:01")
    return parser


async def async_main(argv: list[str]) -> int:
    args = build_parser().parse_args(argv)
    if args.command == "run":
        if not is_windows_controller():
            print("okvm_testctl.py run is supported only on native Windows. Run it from Windows PowerShell, not WSL/Linux.", file=sys.stderr)
            return 2
        runner = AcceptanceRunner(args)
        return await runner.run()
    raise AssertionError(args.command)


def main() -> None:
    try:
        raise SystemExit(asyncio.run(async_main(sys.argv[1:])))
    except KeyboardInterrupt:
        raise SystemExit(130)


if __name__ == "__main__":
    main()
