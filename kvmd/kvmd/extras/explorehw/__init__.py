import argparse

from ... import msd
from ... import streamer


# =====
def _probe_msd(path: str) -> bool:
    info = msd.explore_device(path)
    if info:
        print("It's a mass-storage device")
        print("--------------------------")
        print("Path:        ", info.path)
        print("Bind:        ", info.bind)
        print("Size:        ", info.size)
        print("Manufacturer:", info.manufacturer)
        print("Product:     ", info.product)
        print("Serial:      ", info.serial)
        print("Image name:  ", info.image_name)
        assert msd.locate_by_bind(info.bind), info.bind
    return bool(info)


def _probe_streamer(path: str) -> bool:
    info = streamer.explore_device(path)
    if info:
        print("It's a streamer device")
        print("----------------------")
        print("Path:  ", info.path)
        print("Bind:  ", info.bind)
        print("Driver:", info.driver)
        assert streamer.locate_by_bind(info.bind), info.bind
    return bool(info)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("device")
    options = parser.parse_args()

    for probe in [
        _probe_msd,
        _probe_streamer,
    ]:
        if probe(options.device):
            break
    else:
        raise RuntimeError("Can't recognize device")
