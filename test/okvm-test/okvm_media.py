from __future__ import annotations

import io
from typing import Any


HDMI_COLOR_SEQUENCE: tuple[tuple[str, str], ...] = (
    ("black", "#000000"),
    ("white", "#ffffff"),
    ("gray16", "#101010"),
    ("gray128", "#808080"),
    ("gray235", "#ebebeb"),
    ("red", "#ff0000"),
    ("green", "#00ff00"),
    ("blue", "#0000ff"),
)


def hex_to_rgb(value: str) -> tuple[int, int, int]:
    s = value.strip().lstrip("#")
    if len(s) != 6:
        raise ValueError(f"invalid RGB color: {value}")
    return int(s[0:2], 16), int(s[2:4], 16), int(s[4:6], 16)


def rgb_error(expected: tuple[int, int, int], measured: tuple[float, float, float]) -> dict[str, float]:
    errors = [abs(float(measured[i]) - float(expected[i])) for i in range(3)]
    return {
        "mean_abs_error": sum(errors) / 3,
        "max_abs_error": max(errors),
    }


def closest_hdmi_color(measured: tuple[float, float, float]) -> tuple[str, float]:
    best_name = ""
    best_error = 999.0
    for name, color in HDMI_COLOR_SEQUENCE:
        err = rgb_error(hex_to_rgb(color), measured)["mean_abs_error"]
        if err < best_error:
            best_name = name
            best_error = err
    return best_name, best_error


def jpeg_rgb_stats(frame: bytes) -> dict[str, Any]:
    try:
        from PIL import Image, ImageStat
    except ImportError as exc:
        raise RuntimeError("Pillow is required for HDMI color/latency tests; run pip install -r requirements.txt") from exc

    with Image.open(io.BytesIO(frame)) as image:
        rgb = image.convert("RGB")
        width, height = rgb.size
        left = max(0, int(width * 0.35))
        top = max(0, int(height * 0.35))
        right = min(width, int(width * 0.65))
        bottom = min(height, int(height * 0.65))
        crop = rgb.crop((left, top, right, bottom))
        stat = ImageStat.Stat(crop)
        return {
            "width": width,
            "height": height,
            "sample_box": [left, top, right, bottom],
            "mean_rgb": tuple(round(float(v), 2) for v in stat.mean),
            "stddev_rgb": tuple(round(float(v), 2) for v in stat.stddev),
        }


def percentile(values: list[float], pct: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    rank = (len(ordered) - 1) * (pct / 100)
    lower = int(rank)
    upper = min(lower + 1, len(ordered) - 1)
    weight = rank - lower
    return ordered[lower] * (1 - weight) + ordered[upper] * weight
