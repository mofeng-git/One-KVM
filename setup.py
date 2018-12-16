#!/usr/bin/env python3


from setuptools import setup


# =====
def main() -> None:
    setup(
        name="kvmd",
        version="0.116",
        url="https://github.com/pi-kvm/pi-kvm",
        license="GPLv3",
        author="Maxim Devaev",
        author_email="mdevaev@gmail.com",
        description="The main Pi-KVM daemon",
        platforms="any",

        packages=[
            "kvmd",
            "kvmd.apps",
            "kvmd.apps.kvmd",
            "kvmd.apps.cleanup",
            "kvmd.apps.wscli",
        ],

        package_data={
            "kvmd": ["data/*.yaml"],
        },

        entry_points={
            "console_scripts": [
                "kvmd = kvmd.apps.kvmd:main",
                "kvmd-cleanup = kvmd.apps.cleanup:main",
                "kvmd-wscli = kvmd.apps.wscli:main",
            ],
        },

        classifiers=[
            "License :: OSI Approved :: GNU General Public License v3 or later (GPLv3+)",
            "Development Status :: 3 - Alpha",
            "Programming Language :: Python :: 3.6",
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
