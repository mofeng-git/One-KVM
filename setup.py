#!/usr/bin/env python3
# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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


import os
import textwrap

import setuptools.command.easy_install
from setuptools import setup


# =====
class _ScriptWriter(setuptools.command.easy_install.ScriptWriter):
    template = textwrap.dedent("""
        # EASY-INSTALL-ENTRY-SCRIPT: {spec},{group},{name}

        __requires__ = "{spec}"

        from {module} import main

        if __name__ == "__main__":
            main()
    """).strip()

    @classmethod
    def get_args(cls, dist, header=None):  # type: ignore
        if header is None:
            header = cls.get_header()
        spec = str(dist.as_requirement())
        for group_type in ["console", "gui"]:
            group = group_type + "_scripts"
            for (name, ep) in dist.get_entry_map(group).items():
                cls._ensure_safe_name(name)
                script_text = cls.template.format(
                    spec=spec,
                    group=group,
                    name=name,
                    module=ep.module_name,
                )
                yield from cls._get_script_args(group_type, name, header, script_text)


# =====
def main() -> None:
    setuptools.command.easy_install.ScriptWriter = _ScriptWriter
    setuptools.command.easy_install.get_script_args = _ScriptWriter.get_script_args
    setuptools.command.easy_install.get_script_header = _ScriptWriter.get_script_header

    setup(
        name="kvmd",
        version="3.29",
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
            "kvmd.plugins.atx",
            "kvmd.plugins.msd",
            "kvmd.plugins.msd.relay",
            "kvmd.plugins.msd.otg",
            "kvmd.plugins.ugpio",
            "kvmd.clients",
            "kvmd.apps",
            "kvmd.apps.kvmd",
            "kvmd.apps.kvmd.info",
            "kvmd.apps.kvmd.api",
            "kvmd.apps.otg",
            "kvmd.apps.otg.hid",
            "kvmd.apps.otgnet",
            "kvmd.apps.otgmsd",
            "kvmd.apps.htpasswd",
            "kvmd.apps.cleanup",
            "kvmd.apps.ipmi",
            "kvmd.apps.vnc",
            "kvmd.apps.vnc.rfb",
            "kvmd.apps.janus",
            "kvmd.apps.watchdog",
            "kvmd.helpers",
            "kvmd.helpers.otgmsd",
            "kvmd.helpers.otgmsd.unlock",
            "kvmd.helpers.otgmsd.remount",
        ],

        package_data={
            "kvmd.apps.vnc": ["fonts/*.ttf"],
        },

        scripts=[
            os.path.join("scripts", name)
            for name in os.listdir("scripts")
            if not name.startswith(".")
        ],

        entry_points={
            "console_scripts": [
                "kvmd = kvmd.apps.kvmd:main",
                "kvmd-otg = kvmd.apps.otg:main",
                "kvmd-otgnet = kvmd.apps.otgnet:main",
                "kvmd-otgmsd = kvmd.apps.otgmsd:main",
                "kvmd-htpasswd = kvmd.apps.htpasswd:main",
                "kvmd-cleanup = kvmd.apps.cleanup:main",
                "kvmd-ipmi = kvmd.apps.ipmi:main",
                "kvmd-vnc = kvmd.apps.vnc:main",
                "kvmd-janus = kvmd.apps.janus:main",
                "kvmd-watchdog = kvmd.apps.watchdog:main",
                "kvmd-helper-otgmsd-unlock = kvmd.helpers.otgmsd.unlock:main",
                "kvmd-helper-otgmsd-remount = kvmd.helpers.otgmsd.remount:main",
            ],
        },

        classifiers=[
            "License :: OSI Approved :: GNU General Public License v3 or later (GPLv3+)",
            "Development Status :: 5 - Production/Stable",
            "Programming Language :: Python :: 3.7",
            "Programming Language :: Python :: 3.8",
            "Programming Language :: Python :: 3.9",
            "Topic :: System :: Systems Administration",
            "Operating System :: POSIX :: Linux",
            "Intended Audience :: System Administrators",
            "Intended Audience :: End Users/Desktop",
            "Intended Audience :: Telecommunications Industry",
        ],
    )


if __name__ == "__main__":
    main()
