#!/usr/bin/env python3
# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2018  Maxim Devaev <mdevaev@gmail.com>                    #
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


from setuptools import setup


# =====
def main() -> None:
    setup(
        name="kvmd",
        version="0.157",
        url="https://github.com/pi-kvm/kvmd",
        license="GPLv3",
        author="Maxim Devaev",
        author_email="mdevaev@gmail.com",
        description="The main Pi-KVM daemon",
        platforms="any",

        packages=[
            "kvmd",
            "kvmd.validators",
            "kvmd.yamlconf",
            "kvmd.plugins",
            "kvmd.plugins.auth",
            "kvmd.apps",
            "kvmd.apps.kvmd",
            "kvmd.apps.htpasswd",
            "kvmd.apps.cleanup",
            "kvmd.apps.ipmi",
        ],

        package_data={
            "kvmd": ["data/*.yaml"],
        },

        entry_points={
            "console_scripts": [
                "kvmd = kvmd.apps.kvmd:main",
                "kvmd-htpasswd = kvmd.apps.htpasswd:main",
                "kvmd-cleanup = kvmd.apps.cleanup:main",
                "kvmd-ipmi = kvmd.apps.ipmi:main",
            ],
        },

        classifiers=[
            "License :: OSI Approved :: GNU General Public License v3 or later (GPLv3+)",
            "Development Status :: 4 - Beta",
            "Programming Language :: Python :: 3.7",
            "Topic :: System :: Systems Administration",
            "Operating System :: POSIX :: Linux",
            "Intended Audience :: System Administrators",
            "Intended Audience :: End Users/Desktop",
            "Intended Audience :: Telecommunications Industry",
        ],
    )


if __name__ == "__main__":
    main()
