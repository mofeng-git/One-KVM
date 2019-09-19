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


import sys
import os
import getpass
import tempfile
import contextlib
import argparse

from typing import List
from typing import Generator
from typing import Optional

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
            stat = os.stat(path)
            with open(path, "rb") as htpasswd_file:
                os.write(tmp_fd, htpasswd_file.read())
                os.fchown(tmp_fd, stat.st_uid, stat.st_gid)
                os.fchmod(tmp_fd, stat.st_mode)
        finally:
            os.close(tmp_fd)
        htpasswd = passlib.apache.HtpasswdFile(tmp_path)
        yield htpasswd
        htpasswd.save()
        os.rename(tmp_path, path)
    finally:
        if os.path.exists(tmp_path):
            os.remove(tmp_path)


# ====
def _cmd_list(config: Section, _: argparse.Namespace) -> None:
    for user in sorted(passlib.apache.HtpasswdFile(_get_htpasswd_path(config)).users()):
        print(user)


def _cmd_set(config: Section, options: argparse.Namespace) -> None:
    with _get_htpasswd_for_write(config) as htpasswd:
        if options.read_stdin:
            passwd = valid_passwd(input())
        else:
            passwd = valid_passwd(getpass.getpass("Password: ", stream=sys.stderr))
            if valid_passwd(getpass.getpass("Repeat: ", stream=sys.stderr)) != passwd:
                raise SystemExit("Sorry, passwords do not match")
        htpasswd.set_password(options.user, passwd)


def _cmd_delete(config: Section, options: argparse.Namespace) -> None:
    with _get_htpasswd_for_write(config) as htpasswd:
        htpasswd.delete(options.user)


# =====
def main(argv: Optional[List[str]]=None) -> None:
    (parent_parser, argv, config) = init(
        add_help=False,
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
    cmd_set_parser.set_defaults(cmd=_cmd_set)

    cmd_delete_parser = subparsers.add_parser("del", help="Delete user")
    cmd_delete_parser.add_argument("user", type=valid_user)
    cmd_delete_parser.set_defaults(cmd=_cmd_delete)

    options = parser.parse_args(argv[1:])
    try:
        options.cmd(config, options)
    except ValidatorError as err:
        raise SystemExit(str(err))
