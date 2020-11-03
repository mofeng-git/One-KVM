# https://docs.platformio.org/en/latest/projectconf/advanced_scripting.html

from os.path import exists
from os.path import join
from os.path import basename

from typing import Dict

Import("env")


# =====
def _get_libs() -> Dict[str, str]:
    return {
        builder.name: builder.path
        for builder in env.GetLibBuilders()
    }


def _patch_lib(lib_path: str, patch_path: str) -> None:
    assert exists(lib_path)
    flag_path: str = join(lib_path, f".{basename(patch_path)}.done")
    if not exists(flag_path):
        env.Execute(f"patch -p1 -d {lib_path} < {patch_path}")
        env.Execute(lambda *_, **__: open(flag_path, "w").close())


# =====
_libs = _get_libs()
if "HID-Project" in _libs:
    _patch_lib(_libs["HID-Project"], "patches/absmouse.patch")
