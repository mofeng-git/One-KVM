# https://docs.platformio.org/en/latest/projectconf/advanced_scripting.html


from os import rename
from os import symlink
from os.path import exists
from os.path import join

import platform

Import("env")


# =====
def _get_tool_path() -> str:
    path = env.PioPlatform().get_package_dir("tool-avrdude")
    assert exists(path)
    return path


def _fix_ld_arm() -> None:
    tool_path = _get_tool_path()
    flag_path = join(tool_path, ".fix-ld-arm.done")

    if not exists(flag_path):
        def patch(*_, **__) -> None:
            symlink("/usr/lib/libtinfo.so.6", join(tool_path, "libtinfo.so.5"))
            open(flag_path, "w").close()

        env.Execute(patch)


def _replace_to_system(new_path: str) -> None:
    tool_path = _get_tool_path()
    flag_path = join(tool_path, ".replace-to-system.done")

    if not exists(flag_path):
        def patch(*_, **__) -> None:
            old_path = join(tool_path, "avrdude")
            bak_path = join(tool_path, "_avrdude_bak")
            rename(old_path, bak_path)
            symlink(new_path, old_path)
            open(flag_path, "w").close()

        env.Execute(patch)


# =====
if "arm" in platform.machine():
    _fix_ld_arm()

_path = "/usr/bin/avrdude"
if exists(_path):
    _replace_to_system(_path)
