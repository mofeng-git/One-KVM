import sys
import os
import re
import getpass
import tempfile
import contextlib
import argparse

from typing import Generator

import passlib.apache

from ...yamlconf import Section

from .. import init


# =====
@contextlib.contextmanager
def _get_htpasswd_for_write(config: Section) -> Generator[passlib.apache.HtpasswdFile, None, None]:
    path = config.kvmd.auth.htpasswd
    (tmp_fd, tmp_path) = tempfile.mkstemp(
        prefix=".%s." % (os.path.basename(path)),
        dir=os.path.dirname(path),
    )
    try:
        stat = os.stat(path)
        try:
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


def _valid_user(user: str) -> str:
    stripped = user.strip()
    if re.match(r"^[a-z_][a-z0-9_-]*$", stripped):
        return stripped
    raise SystemExit("Invalid user %r" % (user))


# ====
def _cmd_list(config: Section, _: argparse.Namespace) -> None:
    for user in passlib.apache.HtpasswdFile(config.kvmd.auth.htpasswd).users():
        print(user)


def _cmd_set(config: Section, options: argparse.Namespace) -> None:
    with _get_htpasswd_for_write(config) as htpasswd:
        if options.read_stdin:
            passwd = input()
        else:
            passwd = getpass.getpass("Password: ", stream=sys.stderr)
            if getpass.getpass("Repeat: ", stream=sys.stderr) != passwd:
                raise SystemExit("Sorry, passwords do not match")
        htpasswd.set_password(options.user, passwd)


def _cmd_delete(config: Section, options: argparse.Namespace) -> None:
    with _get_htpasswd_for_write(config) as htpasswd:
        htpasswd.delete(options.user)


# =====
def main() -> None:
    (parent_parser, argv, config) = init(add_help=False)
    parser = argparse.ArgumentParser(
        prog="kvmd-htpasswd",
        description="Manage KVMD users",
        parents=[parent_parser],
    )
    parser.set_defaults(cmd=(lambda *_: parser.print_help()))
    subparsers = parser.add_subparsers()

    cmd_list_parser = subparsers.add_parser("list", help="List users")
    cmd_list_parser.set_defaults(cmd=_cmd_list)

    cmd_set_parser = subparsers.add_parser("set", help="Create user or change password")
    cmd_set_parser.add_argument("user", type=_valid_user)
    cmd_set_parser.add_argument("-i", "--read-stdin", action="store_true", help="Read password from stdin")
    cmd_set_parser.set_defaults(cmd=_cmd_set)

    cmd_delete_parser = subparsers.add_parser("del", help="Delete user")
    cmd_delete_parser.add_argument("user", type=_valid_user)
    cmd_delete_parser.set_defaults(cmd=_cmd_delete)

    options = parser.parse_args(argv[1:])
    options.cmd(config, options)
