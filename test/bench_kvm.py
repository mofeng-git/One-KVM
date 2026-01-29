#!/usr/bin/env python3
"""
One-KVM benchmark script (Windows-friendly).

Measures FPS + CPU usage across:
- input pixel formats (capture card formats)
- output codecs (mjpeg/h264/h265/vp8/vp9)
- resolution/FPS matrix
- encoder backends (software/hardware)

Requirements:
  pip install requests websockets playwright
  playwright install
"""

from __future__ import annotations

import argparse
import asyncio
import csv
import json
import sys
import threading
import time
from dataclasses import dataclass
from typing import Dict, Iterable, List, Optional, Tuple

import requests
import websockets
from playwright.async_api import async_playwright


SESSION_COOKIE = "one_kvm_session"
DEFAULT_MATRIX = [
    (1920, 1080, 30),
    (1920, 1080, 60),
    (1280, 720, 30),
    (1280, 720, 60),
]


@dataclass
class Case:
    input_format: str
    output_codec: str
    encoder: Optional[str]
    width: int
    height: int
    fps: int


@dataclass
class Result:
    input_format: str
    output_codec: str
    encoder: str
    width: int
    height: int
    fps: int
    avg_fps: float
    avg_cpu: float
    note: str = ""


class KvmClient:
    def __init__(self, base_url: str, username: str, password: str) -> None:
        self.base = base_url.rstrip("/")
        self.s = requests.Session()
        self.login(username, password)

    def login(self, username: str, password: str) -> None:
        r = self.s.post(f"{self.base}/api/auth/login", json={"username": username, "password": password})
        r.raise_for_status()

    def get_cookie(self) -> str:
        return self.s.cookies.get(SESSION_COOKIE, "")

    def get_video_config(self) -> Dict:
        r = self.s.get(f"{self.base}/api/config/video")
        r.raise_for_status()
        return r.json()

    def get_stream_config(self) -> Dict:
        r = self.s.get(f"{self.base}/api/config/stream")
        r.raise_for_status()
        return r.json()

    def get_devices(self) -> Dict:
        r = self.s.get(f"{self.base}/api/devices")
        r.raise_for_status()
        return r.json()

    def get_codecs(self) -> Dict:
        r = self.s.get(f"{self.base}/api/stream/codecs")
        r.raise_for_status()
        return r.json()

    def patch_video(self, device: Optional[str], fmt: str, w: int, h: int, fps: int) -> None:
        payload: Dict[str, object] = {"format": fmt, "width": w, "height": h, "fps": fps}
        if device:
            payload["device"] = device
        r = self.s.patch(f"{self.base}/api/config/video", json=payload)
        r.raise_for_status()

    def patch_stream(self, encoder: Optional[str]) -> None:
        if encoder is None:
            return
        r = self.s.patch(f"{self.base}/api/config/stream", json={"encoder": encoder})
        r.raise_for_status()

    def set_mode(self, mode: str) -> None:
        r = self.s.post(f"{self.base}/api/stream/mode", json={"mode": mode})
        r.raise_for_status()

    def get_mode(self) -> Dict:
        r = self.s.get(f"{self.base}/api/stream/mode")
        r.raise_for_status()
        return r.json()

    def wait_mode_ready(self, mode: str, timeout_sec: int = 20) -> None:
        deadline = time.time() + timeout_sec
        while time.time() < deadline:
            data = self.get_mode()
            if not data.get("switching") and data.get("mode") == mode:
                return
            time.sleep(0.5)
        raise RuntimeError(f"mode switch timeout: {mode}")

    def start_stream(self) -> None:
        r = self.s.post(f"{self.base}/api/stream/start")
        r.raise_for_status()

    def stop_stream(self) -> None:
        r = self.s.post(f"{self.base}/api/stream/stop")
        r.raise_for_status()

    def cpu_sample(self) -> float:
        r = self.s.get(f"{self.base}/api/info")
        r.raise_for_status()
        return float(r.json()["device_info"]["cpu_usage"])

    def close_webrtc_session(self, session_id: str) -> None:
        if not session_id:
            return
        self.s.post(f"{self.base}/api/webrtc/close", json={"session_id": session_id})


class MjpegStream:
    def __init__(self, url: str, cookie: str) -> None:
        self._stop = threading.Event()
        self._resp = requests.get(url, stream=True, headers={"Cookie": f"{SESSION_COOKIE}={cookie}"})
        self._thread = threading.Thread(target=self._reader, daemon=True)
        self._thread.start()

    def _reader(self) -> None:
        try:
            for chunk in self._resp.iter_content(chunk_size=4096):
                if self._stop.is_set():
                    break
                if not chunk:
                    time.sleep(0.01)
        except Exception:
            pass

    def close(self) -> None:
        self._stop.set()
        try:
            self._resp.close()
        except Exception:
            pass


def parse_matrix(values: Optional[List[str]]) -> List[Tuple[int, int, int]]:
    if not values:
        return DEFAULT_MATRIX
    result: List[Tuple[int, int, int]] = []
    for item in values:
        # WIDTHxHEIGHT@FPS
        part = item.strip().lower()
        if "@" not in part or "x" not in part:
            raise ValueError(f"invalid matrix item: {item}")
        res_part, fps_part = part.split("@", 1)
        w_str, h_str = res_part.split("x", 1)
        result.append((int(w_str), int(h_str), int(fps_part)))
    return result


def avg(values: Iterable[float]) -> float:
    vals = list(values)
    return sum(vals) / len(vals) if vals else 0.0


def normalize_format(fmt: str) -> str:
    return fmt.strip().upper()


def select_device(devices: Dict, preferred: Optional[str]) -> Optional[Dict]:
    video_devices = devices.get("video", [])
    if preferred:
        for d in video_devices:
            if d.get("path") == preferred:
                return d
    return video_devices[0] if video_devices else None


def build_supported_map(device: Dict) -> Dict[str, Dict[Tuple[int, int], List[int]]]:
    supported: Dict[str, Dict[Tuple[int, int], List[int]]] = {}
    for fmt in device.get("formats", []):
        fmt_name = normalize_format(fmt.get("format", ""))
        res_map: Dict[Tuple[int, int], List[int]] = {}
        for res in fmt.get("resolutions", []):
            key = (int(res.get("width", 0)), int(res.get("height", 0)))
            fps_list = [int(f) for f in res.get("fps", [])]
            res_map[key] = fps_list
        supported[fmt_name] = res_map
    return supported


def is_combo_supported(
    supported: Dict[str, Dict[Tuple[int, int], List[int]]],
    fmt: str,
    width: int,
    height: int,
    fps: int,
) -> bool:
    res_map = supported.get(fmt)
    if not res_map:
        return False
    fps_list = res_map.get((width, height), [])
    return fps in fps_list


async def mjpeg_sample(
    base_url: str,
    cookie: str,
    client_id: str,
    duration_sec: float,
    cpu_sample_fn,
) -> Tuple[float, float]:
    mjpeg_url = f"{base_url}/api/stream/mjpeg?client_id={client_id}"
    stream = MjpegStream(mjpeg_url, cookie)
    ws_url = base_url.replace("http://", "ws://").replace("https://", "wss://") + "/api/ws"

    fps_samples: List[float] = []
    cpu_samples: List[float] = []

    # discard first cpu sample (needs delta)
    cpu_sample_fn()

    try:
        async with websockets.connect(ws_url, extra_headers={"Cookie": f"{SESSION_COOKIE}={cookie}"}) as ws:
            start = time.time()
            while time.time() - start < duration_sec:
                try:
                    msg = await asyncio.wait_for(ws.recv(), timeout=1.0)
                except asyncio.TimeoutError:
                    msg = None

                if msg:
                    data = json.loads(msg)
                    if data.get("type") == "stream.stats_update":
                        clients = data.get("clients_stat", {})
                        if client_id in clients:
                            fps = float(clients[client_id].get("fps", 0))
                            fps_samples.append(fps)

                cpu_samples.append(float(cpu_sample_fn()))
    finally:
        stream.close()

    return avg(fps_samples), avg(cpu_samples)


async def webrtc_sample(
    base_url: str,
    cookie: str,
    duration_sec: float,
    cpu_sample_fn,
    headless: bool,
) -> Tuple[float, float, str]:
    fps_samples: List[float] = []
    cpu_samples: List[float] = []
    session_id = ""

    # discard first cpu sample (needs delta)
    cpu_sample_fn()

    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=headless)
        context = await browser.new_context()
        await context.add_cookies([{
            "name": SESSION_COOKIE,
            "value": cookie,
            "url": base_url,
            "path": "/",
        }])
        page = await context.new_page()
        await page.goto(base_url + "/", wait_until="domcontentloaded")

        await page.evaluate(
            """
            async (base) => {
                const pc = new RTCPeerConnection();
                pc.addTransceiver('video', { direction: 'recvonly' });
                pc.addTransceiver('audio', { direction: 'recvonly' });
                pc.onicecandidate = async (e) => {
                    if (e.candidate && window.__sid) {
                        await fetch(base + "/api/webrtc/ice", {
                            method: "POST",
                            headers: { "Content-Type": "application/json" },
                            body: JSON.stringify({ session_id: window.__sid, candidate: e.candidate })
                        });
                    }
                };
                const offer = await pc.createOffer();
                await pc.setLocalDescription(offer);
                const resp = await fetch(base + "/api/webrtc/offer", {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ sdp: offer.sdp })
                });
                const ans = await resp.json();
                window.__sid = ans.session_id;
                await pc.setRemoteDescription({ type: "answer", sdp: ans.sdp });
                (ans.ice_candidates || []).forEach(c => pc.addIceCandidate(c));
                window.__kvmStats = { pc, lastTs: 0, lastFrames: 0 };
            }
            """,
            base_url,
        )

        try:
            await page.wait_for_function(
                "window.__kvmStats && window.__kvmStats.pc && window.__kvmStats.pc.connectionState === 'connected'",
                timeout=15000,
            )
        except Exception:
            pass

        start = time.time()
        while time.time() - start < duration_sec:
            fps = await page.evaluate(
                """
                async () => {
                    const s = window.__kvmStats;
                    const report = await s.pc.getStats();
                    let fps = 0;
                    for (const r of report.values()) {
                        if (r.type === "inbound-rtp" && r.kind === "video") {
                            if (r.framesPerSecond) {
                                fps = r.framesPerSecond;
                            } else if (r.framesDecoded && s.lastTs) {
                                const dt = (r.timestamp - s.lastTs) / 1000.0;
                                const df = r.framesDecoded - s.lastFrames;
                                fps = dt > 0 ? df / dt : 0;
                            }
                            s.lastTs = r.timestamp;
                            s.lastFrames = r.framesDecoded || s.lastFrames;
                            break;
                        }
                    }
                    return fps;
                }
                """
            )
            fps_samples.append(float(fps))
            cpu_samples.append(float(cpu_sample_fn()))
            await asyncio.sleep(1)

        session_id = await page.evaluate("window.__sid || ''")
        await browser.close()

    return avg(fps_samples), avg(cpu_samples), session_id


async def run_case(
    client: KvmClient,
    device: Optional[str],
    case: Case,
    duration_sec: float,
    warmup_sec: float,
    headless: bool,
) -> Result:
    client.patch_video(device, case.input_format, case.width, case.height, case.fps)

    if case.output_codec != "mjpeg":
        client.patch_stream(case.encoder)

    client.set_mode(case.output_codec)
    client.wait_mode_ready(case.output_codec)

    client.start_stream()
    time.sleep(warmup_sec)

    note = ""
    if case.output_codec == "mjpeg":
        avg_fps, avg_cpu = await mjpeg_sample(
            client.base,
            client.get_cookie(),
            client_id=f"bench-{int(time.time() * 1000)}",
            duration_sec=duration_sec,
            cpu_sample_fn=client.cpu_sample,
        )
    else:
        avg_fps, avg_cpu, session_id = await webrtc_sample(
            client.base,
            client.get_cookie(),
            duration_sec=duration_sec,
            cpu_sample_fn=client.cpu_sample,
            headless=headless,
        )
        if session_id:
            client.close_webrtc_session(session_id)
        else:
            note = "no-session-id"

    client.stop_stream()

    return Result(
        input_format=case.input_format,
        output_codec=case.output_codec,
        encoder=case.encoder or "n/a",
        width=case.width,
        height=case.height,
        fps=case.fps,
        avg_fps=avg_fps,
        avg_cpu=avg_cpu,
        note=note,
    )


def write_csv(results: List[Result], path: str) -> None:
    with open(path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["input_format", "output_codec", "encoder", "width", "height", "fps", "avg_fps", "avg_cpu", "note"])
        for r in results:
            w.writerow([r.input_format, r.output_codec, r.encoder, r.width, r.height, r.fps, f"{r.avg_fps:.2f}", f"{r.avg_cpu:.2f}", r.note])


def write_md(results: List[Result], path: str) -> None:
    lines = [
        "| input_format | output_codec | encoder | width | height | fps | avg_fps | avg_cpu | note |",
        "|---|---|---|---:|---:|---:|---:|---:|---|",
    ]
    for r in results:
        lines.append(
            f"| {r.input_format} | {r.output_codec} | {r.encoder} | {r.width} | {r.height} | {r.fps} | {r.avg_fps:.2f} | {r.avg_cpu:.2f} | {r.note} |"
        )
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))


def main() -> int:
    parser = argparse.ArgumentParser(description="One-KVM benchmark (FPS + CPU)")
    parser.add_argument("--base-url", required=True, help="e.g. http://192.168.1.50")
    parser.add_argument("--username", required=True)
    parser.add_argument("--password", required=True)
    parser.add_argument("--device", help="video device path, e.g. /dev/video0")
    parser.add_argument("--input-formats", help="comma list, e.g. MJPEG,YUYV,NV12")
    parser.add_argument("--output-codecs", help="comma list, e.g. mjpeg,h264,h265,vp8,vp9")
    parser.add_argument("--encoder-backends", help="comma list, e.g. software,auto,vaapi,nvenc,qsv,amf,rkmpp,v4l2m2m")
    parser.add_argument("--matrix", action="append", help="repeatable WIDTHxHEIGHT@FPS, e.g. 1920x1080@30")
    parser.add_argument("--duration", type=float, default=30.0, help="sample duration seconds (default 30)")
    parser.add_argument("--warmup", type=float, default=3.0, help="warmup seconds before sampling")
    parser.add_argument("--csv", default="bench_results.csv")
    parser.add_argument("--md", default="bench_results.md")
    parser.add_argument("--headless", action="store_true", help="run browser headless (default: headful)")

    args = parser.parse_args()

    if sys.platform.startswith("win"):
        asyncio.set_event_loop_policy(asyncio.WindowsSelectorEventLoopPolicy())

    base_url = args.base_url.strip()
    if not base_url.startswith(("http://", "https://")):
        base_url = "http://" + base_url
    client = KvmClient(base_url, args.username, args.password)

    devices = client.get_devices()
    video_cfg = client.get_video_config()
    device_path = args.device or video_cfg.get("device")
    device_info = select_device(devices, device_path)
    if not device_info:
        print("No video device found.", file=sys.stderr)
        return 2
    device_path = device_info.get("path")

    supported_map = build_supported_map(device_info)

    if args.input_formats:
        input_formats = [normalize_format(f) for f in args.input_formats.split(",") if f.strip()]
    else:
        input_formats = list(supported_map.keys())

    matrix = parse_matrix(args.matrix)

    codecs_info = client.get_codecs()
    available_codecs = {c["id"] for c in codecs_info.get("codecs", []) if c.get("available")}
    available_codecs.add("mjpeg")

    if args.output_codecs:
        output_codecs = [c.strip().lower() for c in args.output_codecs.split(",") if c.strip()]
    else:
        output_codecs = sorted(list(available_codecs))

    if args.encoder_backends:
        encoder_backends = [e.strip().lower() for e in args.encoder_backends.split(",") if e.strip()]
    else:
        encoder_backends = ["software", "auto"]

    cases: List[Case] = []
    for fmt in input_formats:
        for (w, h, fps) in matrix:
            if not is_combo_supported(supported_map, fmt, w, h, fps):
                continue
            for codec in output_codecs:
                if codec not in available_codecs:
                    continue
                if codec == "mjpeg":
                    cases.append(Case(fmt, codec, None, w, h, fps))
                else:
                    for enc in encoder_backends:
                        cases.append(Case(fmt, codec, enc, w, h, fps))

    print(f"Total cases: {len(cases)}")
    results: List[Result] = []

    for idx, case in enumerate(cases, 1):
        print(f"[{idx}/{len(cases)}] {case.input_format} {case.output_codec} {case.encoder or 'n/a'} {case.width}x{case.height}@{case.fps}")
        try:
            result = asyncio.run(
                run_case(
                    client,
                    device=device_path,
                    case=case,
                    duration_sec=args.duration,
                    warmup_sec=args.warmup,
                    headless=args.headless,
                )
            )
            results.append(result)
            print(f"  -> avg_fps={result.avg_fps:.2f}, avg_cpu={result.avg_cpu:.2f}")
        except Exception as exc:
            results.append(
                Result(
                    input_format=case.input_format,
                    output_codec=case.output_codec,
                    encoder=case.encoder or "n/a",
                    width=case.width,
                    height=case.height,
                    fps=case.fps,
                    avg_fps=0.0,
                    avg_cpu=0.0,
                    note=f"error: {exc}",
                )
            )
            print(f"  -> error: {exc}")

    write_csv(results, args.csv)
    write_md(results, args.md)
    print(f"Saved: {args.csv}, {args.md}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
