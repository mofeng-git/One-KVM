from os.path import join
from os.path import exists

Import("env")


# =====
deps_path = env.get("PROJECT_LIBDEPS_DIR", env.get("PROJECTLIBDEPS_DIR"))
assert deps_path, deps_path
env_path = join(deps_path, env["PIOENV"])
flag_path = join(env_path, ".patched")

if not exists(flag_path):
    env.Execute(f"patch -p1 -d {join(env_path, 'HID-Project_ID523')} < {join('patches', 'absmouse.patch')}")

    def touch_flag(*_, **__) -> None:
        with open(flag_path, "w") as flag_file:
            pass

    env.Execute(touch_flag)
