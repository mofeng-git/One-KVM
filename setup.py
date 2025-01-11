#!/usr/bin/env python3
# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
#                                                                            #
#    This program is free software: you can redistribute it and/or modify    #
#    it under the terms of the GNU General Public License as published by    #
#    the Free Software Foundation, either version 3 of the License, or       #
#    (at your option) any later version.                                     #
#                                                                            #
#    This program is distributed in the hope that it will be useful,         #
#    but WITHOUT ANY WARRANTY; without even the implied warranty of          #
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the           #
#    GNU General Public License for more details.                            #
#                                                                            #
#    You should have received a copy of the GNU General Public License       #
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.  #
#                                                                            #
# ========================================================================== #


import textwrap

import setuptools.command.easy_install
from setuptools import setup


# =====
class _Template(str):
    def __init__(self, text: str) -> None:
        self.__text = textwrap.dedent(text).strip()

    def __mod__(self, kv: dict) -> str:
        kv = {"module_name": kv["ep"].module_name, **kv}
        return (self.__text % (kv))


class _ScriptWriter(setuptools.command.easy_install.ScriptWriter):
    template = _Template("""
        # EASY-INSTALL-ENTRY-SCRIPT: %(spec)r,%(group)r,%(name)r

        __requires__ = %(spec)r

        from %(module_name)s import main

        if __name__ == '__main__':
            main()
    """)


# =====
def main() -> None:
    setuptools.command.easy_install.ScriptWriter = _ScriptWriter

    setup(
        name="kvmd",
        version="4.41",
        url="https://github.com/pikvm/kvmd",
        license="GPLv3",
        author="Maxim Devaev",
        author_email="mdevaev@gmail.com",
        description="The main PiKVM daemon",
        platforms="any",

        packages=[
            "kvmd",
            "kvmd.validators",
            "kvmd.yamlconf",
            "kvmd.keyboard",
            "kvmd.plugins",
            "kvmd.plugins.auth",
            "kvmd.plugins.hid",
            "kvmd.plugins.hid._mcu",
            "kvmd.plugins.hid.otg",
            "kvmd.plugins.hid.bt",
            "kvmd.plugins.hid.ch9329",
            "kvmd.plugins.atx",
            "kvmd.plugins.msd",
            "kvmd.plugins.msd.otg",
            "kvmd.plugins.ugpio",
            "kvmd.clients",
            "kvmd.apps",
            "kvmd.apps.kvmd",
            "kvmd.apps.kvmd.switch",
            "kvmd.apps.kvmd.info",
            "kvmd.apps.kvmd.api",
            "kvmd.apps.media",
            "kvmd.apps.pst",
            "kvmd.apps.pstrun",
            "kvmd.apps.otg",
            "kvmd.apps.otg.hid",
            "kvmd.apps.otgnet",
            "kvmd.apps.otgmsd",
            "kvmd.apps.otgconf",
            "kvmd.apps.swctl",
            "kvmd.apps.htpasswd",
            "kvmd.apps.totp",
            "kvmd.apps.edidconf",
            "kvmd.apps.ipmi",
            "kvmd.apps.vnc",
            "kvmd.apps.vnc.rfb",
            "kvmd.apps.ngxmkconf",
            "kvmd.apps.janus",
            "kvmd.apps.watchdog",
            "kvmd.apps.oled",
            "kvmd.helpers",
            "kvmd.helpers.remount",
            "kvmd.helpers.swapfiles",
        ],

        package_data={
            "kvmd.apps.vnc": ["fonts/*.ttf"],
            "kvmd.apps.oled": ["fonts/*.ttf", "pics/*.ppm"],
        },

        entry_points={
            "console_scripts": [
                "kvmd = kvmd.apps.kvmd:main",
                "kvmd-media = kvmd.apps.media:main",
                "kvmd-pst = kvmd.apps.pst:main",
                "kvmd-pstrun = kvmd.apps.pstrun:main",
                "kvmd-otg = kvmd.apps.otg:main",
                "kvmd-otgnet = kvmd.apps.otgnet:main",
                "kvmd-otgmsd = kvmd.apps.otgmsd:main",
                "kvmd-otgconf = kvmd.apps.otgconf:main",
                "kvmd-htpasswd = kvmd.apps.htpasswd:main",
                "kvmd-totp = kvmd.apps.totp:main",
                "kvmd-edidconf = kvmd.apps.edidconf:main",
                "kvmd-ipmi = kvmd.apps.ipmi:main",
                "kvmd-vnc = kvmd.apps.vnc:main",
                "kvmd-nginx-mkconf = kvmd.apps.ngxmkconf:main",
                "kvmd-janus = kvmd.apps.janus:main",
                "kvmd-watchdog = kvmd.apps.watchdog:main",
                "kvmd-oled = kvmd.apps.oled:main",
                "kvmd-helper-pst-remount = kvmd.helpers.remount:main",
                "kvmd-helper-otgmsd-remount = kvmd.helpers.remount:main",
                "kvmd-helper-swapfiles = kvmd.helpers.swapfiles:main",
            ],
        },

        classifiers=[
            "License :: OSI Approved :: GNU General Public License v3 or later (GPLv3+)",
            "Development Status :: 5 - Production/Stable",
            "Programming Language :: Python :: 3.12",
            "Topic :: System :: Systems Administration",
            "Operating System :: POSIX :: Linux",
            "Intended Audience :: System Administrators",
            "Intended Audience :: End Users/Desktop",
            "Intended Audience :: Telecommunications Industry",
        ],
    )


if __name__ == "__main__":
    main()
