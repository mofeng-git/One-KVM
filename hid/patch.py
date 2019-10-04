from os.path import join
from os.path import exists

Import("env")


# =====
env_path = join(env["PROJECTLIBDEPS_DIR"], env["PIOENV"])
flag_path = join(env_path, ".patched")

if not exists(flag_path):
    env.Execute(f"patch -p1 -d {join(env_path, 'HID-Project_ID523')} < {join('patches', 'absmouse.patch')}")

    def touch_flag(*_, **__) -> None:
        with open(flag_path, "w") as flag_file:
            pass

    env.Execute(touch_flag)
