import argparse

from ...msd import explore_device
from ...msd import locate_by_bind


# =====
def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("-d", "--device", default="/dev/sda")
    options = parser.parse_args()

    info = explore_device(options.device)
    print("Path:        ", info.path)
    print("Bind:        ", info.bind)
    print("Size:        ", info.size)
    print("Manufacturer:", info.manufacturer)
    print("Product:     ", info.product)
    print("Serial:      ", info.serial)
    print("Image name:  ", info.image_name)
    assert locate_by_bind(info.bind), "WTF?! Can't locate device file using bind %r" % (info.bind)
