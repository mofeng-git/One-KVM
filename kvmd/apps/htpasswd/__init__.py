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


import sys
import os
import getpass
import tempfile
import contextlib
import textwrap
import argparse

from typing import Generator

import passlib.apache

from ...yamlconf import Section

from ...validators import ValidatorError
from ...validators.auth import valid_user
from ...validators.auth import valid_passwd

from .. import init


# =====
def _get_htpasswd_path(config: Section) -> str:
    if config.kvmd.auth.internal.type != "htpasswd":
        raise SystemExit(f"Error: KVMD internal auth not using 'htpasswd'"
                         f" (now configured {config.kvmd.auth.internal.type!r})")
    return config.kvmd.auth.internal.file


@contextlib.contextmanager
def _get_htpasswd_for_write(config: Section) -> Generator[passlib.apache.HtpasswdFile, None, None]:
    path = _get_htpasswd_path(config)
    (tmp_fd, tmp_path) = tempfile.mkstemp(
        prefix=f".{os.path.basename(path)}.",
        dir=os.path.dirname(path),
    )
    try:
        try:
            st = os.stat(path)
            with open(path, "rb") as file:
                os.write(tmp_fd, file.read())
                os.fchown(tmp_fd, st.st_uid, st.st_gid)
                os.fchmod(tmp_fd, st.st_mode)
        finally:
            os.close(tmp_fd)
        htpasswd = passlib.apache.HtpasswdFile(tmp_path)
        yield htpasswd
        htpasswd.save()
        os.rename(tmp_path, path)
    finally:
        if os.path.exists(tmp_path):
            os.remove(tmp_path)


def _print_invalidate_tip(prepend_nl: bool) -> None:
    if sys.stdout.isatty() and sys.stderr.isatty():
        gray = "\033[30;1m"
        blue = "\033[34m"
        reset = "\033[39m"
    else:
        gray = blue = reset = ""
    if prepend_nl:
        print(file=sys.stderr)
    print(textwrap.dedent(f"""
        {gray}# Note: Users logged in with this username will stay logged in.
        # To invalidate their cookies you need to restart kvmd & kvmd-nginx:
        #    {reset}{blue}systemctl restart kvmd kvmd-nginx{gray}
        # Be careful, this will break your connection to the PiKVM
        # and may affect the GPIO relays state. Also don't forget to edit
        # the files {reset}{blue}/etc/kvmd/{{vncpasswd,ipmipasswd}}{gray} and restart
        # the corresponding services {reset}{blue}kvmd-vnc{gray} & {reset}{blue}kvmd-ipmi{gray} if necessary.{reset}
    """).strip(), file=sys.stderr)


# ====
def _cmd_list(config: Section, _: argparse.Namespace) -> None:
    for user in sorted(passlib.apache.HtpasswdFile(_get_htpasswd_path(config)).users()):
        print(user)


def _cmd_set(config: Section, options: argparse.Namespace) -> None:
    with _get_htpasswd_for_write(config) as htpasswd:
        has_user = (options.user in htpasswd.users())
        if options.read_stdin:
            passwd = valid_passwd(input())
        else:
            passwd = valid_passwd(getpass.getpass("Password: ", stream=sys.stderr))
            if valid_passwd(getpass.getpass("Repeat: ", stream=sys.stderr)) != passwd:
                raise SystemExit("Sorry, passwords do not match")
        htpasswd.set_password(options.user, passwd)
    if has_user and not options.quiet:
        _print_invalidate_tip(True)


def _cmd_delete(config: Section, options: argparse.Namespace) -> None:
    with _get_htpasswd_for_write(config) as htpasswd:
        has_user = (options.user in htpasswd.users())
        htpasswd.delete(options.user)
    if has_user and not options.quiet:
        _print_invalidate_tip(False)


# =====
def main(argv: (list[str] | None)=None) -> None:
    (parent_parser, argv, config) = init(
        add_help=False,
        cli_logging=True,
        argv=argv,
        load_auth=True,
    )
    parser = argparse.ArgumentParser(
        prog="kvmd-htpasswd",
        description="Manage KVMD users (htpasswd auth only)",
        parents=[parent_parser],
    )
    parser.set_defaults(cmd=(lambda *_: parser.print_help()))
    subparsers = parser.add_subparsers()

    cmd_list_parser = subparsers.add_parser("list", help="List users")
    cmd_list_parser.set_defaults(cmd=_cmd_list)

    cmd_set_parser = subparsers.add_parser("set", help="Create user or change password")
    cmd_set_parser.add_argument("user", type=valid_user)
    cmd_set_parser.add_argument("-i", "--read-stdin", action="store_true", help="Read password from stdin")
    cmd_set_parser.add_argument("-q", "--quiet", action="store_true", help="Don't show invalidation note")
    cmd_set_parser.set_defaults(cmd=_cmd_set)

    cmd_delete_parser = subparsers.add_parser("del", help="Delete user")
    cmd_delete_parser.add_argument("user", type=valid_user)
    cmd_delete_parser.add_argument("-q", "--quiet", action="store_true", help="Don't show invalidation note")
    cmd_delete_parser.set_defaults(cmd=_cmd_delete)

    options = parser.parse_args(argv[1:])
    try:
        options.cmd(config, options)
    except ValidatorError as err:
        raise SystemExit(str(err))
