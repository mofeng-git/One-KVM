#!/usr/bin/env python3


from setuptools import setup


# =====
def main() -> None:
    with open("requirements.txt") as requirements_file:
        install_requires = list(filter(None, requirements_file.read().splitlines()))

    setup(
        name="kvmd",
        version="0.10",
        url="https://github.com/mdevaev/pi-kvm",
        license="GPLv3",
        author="Maxim Devaev",
        author_email="mdevaev@gmail.com",
        description="The main Pi-KVM daemon",
        platforms="any",

        packages=[
            "kvmd",
            "kvmd.extras",
            "kvmd.extras.cleanup",
            "kvmd.extras.wscli",
        ],

        entry_points={
            "console_scripts": [
                "kvmd = kvmd:main",
                "kvmd-cleanup = kvmd.extras.cleanup:main",
                "kvmd-wscli = kvmd.extras.wscli:main",
            ],
        },

        install_requires=install_requires,

        classifiers=[
            "License :: OSI Approved :: GNU General Public License v3 or later (GPLv3+)",
            "Development Status :: 3 - Alpha",
            "Programming Language :: Python :: 3.6",
            "Topic :: System :: Systems Administration",
            "Operating System :: POSIX :: Linux",
            "Intended Audience :: System Administrators",
            "Intended Audience :: End Users/Desktop",
            "Intended Audience :: Telecommunications Industry",
        ],
    )


if __name__ == "__main__":
    main()
