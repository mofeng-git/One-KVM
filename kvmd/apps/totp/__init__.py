# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
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


import socket
import argparse

import pyotp
import qrcode

from ...yamlconf import Section

from .. import init


# =====
def _get_secret_path(config: Section) -> str:
    path: str = config.kvmd.auth.totp.secret.file
    if len(path) == 0:
        raise SystemExit("Error: TOTP file path is empty (i.e. it was disabled)")
    return path


def _read_secret(config: Section) -> str:
    with open(_get_secret_path(config)) as file:
        return file.read().strip()


# =====
def _cmd_init(config: Section, options: argparse.Namespace) -> None:
    if not options.force:
        if _read_secret(config):
            raise SystemExit("Error: the TOTP secret already exists")
    with open(_get_secret_path(config), "w") as file:
        file.write(pyotp.random_base32())
    _cmd_show(config, options)


def _cmd_show(config: Section, options: argparse.Namespace) -> None:
    secret = _read_secret(config)
    if len(secret) == 0:
        raise SystemExit("Error: TOTP secret is not configured")
    uri = pyotp.totp.TOTP(secret).provisioning_uri(
        name=(options.name or socket.getfqdn()),
        issuer_name="PiKVM",
    )
    qr = qrcode.QRCode()
    qr.add_data(uri)
    print("\nSecret:", secret, "\n")
    print("URI:", uri, "\n")
    qr.print_ascii(invert=True)
    print()


def _cmd_delete(config: Section, _: argparse.Namespace) -> None:
    with open(_get_secret_path(config), "w") as file:
        file.write("")
    print("TOTP is disabled now")


# =====
def main(argv: (list[str] | None)=None) -> None:
    (parent_parser, argv, config) = init(
        add_help=False,
        cli_logging=True,
        argv=argv,
    )
    parser = argparse.ArgumentParser(
        prog="kvmd-totp",
        description="Manage KVMD TOTP secret",
        parents=[parent_parser],
    )
    parser.set_defaults(cmd=(lambda *_: parser.print_help()))
    subparsers = parser.add_subparsers()

    cmd_setup_parser = subparsers.add_parser("init", help="Generate and show TOTP secret with QR code")
    cmd_setup_parser.add_argument("-f", "--force", action="store_true", help="Overwrite an existing secret")
    cmd_setup_parser.add_argument("-n", "--name", default="", help="The PiKVM instance name, FQDN by default")
    cmd_setup_parser.set_defaults(cmd=_cmd_init)

    cmd_show_parser = subparsers.add_parser("show", help="Show the current TOTP secret with QR code")
    cmd_show_parser.add_argument("-n", "--name", default="", help="The PiKVM instance name, FQDN by default")
    cmd_show_parser.set_defaults(cmd=_cmd_show)

    cmd_delete_parser = subparsers.add_parser("del", help="Remove TOTP secret and disable 2FA auth")
    cmd_delete_parser.set_defaults(cmd=_cmd_delete)

    options = parser.parse_args(argv[1:])
    options.cmd(config, options)
