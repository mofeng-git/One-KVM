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


# =====
class BaseCtl:
    def get_command(self, direct: bool) -> list[str]:
        raise NotImplementedError


class IfaceUpCtl(BaseCtl):
    def __init__(self, base_cmd: list[str], iface: str) -> None:
        self.__base_cmd = base_cmd
        self.__iface = iface

    def get_command(self, direct: bool) -> list[str]:
        return [*self.__base_cmd, "link", "set", self.__iface, ("up" if direct else "down")]


class IfaceAddIpCtl(BaseCtl):
    def __init__(self, base_cmd: list[str], iface: str, cidr: str) -> None:
        self.__base_cmd = base_cmd
        self.__iface = iface
        self.__cidr = cidr

    def get_command(self, direct: bool) -> list[str]:
        return [*self.__base_cmd, "address", ("add" if direct else "del"), self.__cidr, "dev", self.__iface]


class IptablesAllowEstRelCtl(BaseCtl):
    def __init__(self, base_cmd: list[str], iface: str) -> None:
        self.__base_cmd = base_cmd
        self.__iface = iface

    def get_command(self, direct: bool) -> list[str]:
        return [
            *self.__base_cmd,
            ("-A" if direct else "-D"), "INPUT", "-i", self.__iface,
            "-m", "state", "--state", "ESTABLISHED,RELATED", "-j", "ACCEPT",
        ]


class IptablesDropAllCtl(BaseCtl):
    def __init__(self, base_cmd: list[str], iface: str) -> None:
        self.__base_cmd = base_cmd
        self.__iface = iface

    def get_command(self, direct: bool) -> list[str]:
        return [*self.__base_cmd, ("-A" if direct else "-D"), "INPUT", "-i", self.__iface, "-j", "DROP"]


class IptablesAllowIcmpCtl(BaseCtl):
    def __init__(self, base_cmd: list[str], iface: str) -> None:
        self.__base_cmd = base_cmd
        self.__iface = iface

    def get_command(self, direct: bool) -> list[str]:
        return [
            *self.__base_cmd,
            ("-A" if direct else "-D"), "INPUT", "-i", self.__iface, "-p", "icmp", "-j", "ACCEPT",
        ]


class IptablesAllowPortCtl(BaseCtl):
    def __init__(self, base_cmd: list[str], iface: str, port: int, tcp: bool) -> None:
        self.__base_cmd = base_cmd
        self.__iface = iface
        self.__port = port
        self.__proto = ("tcp" if tcp else "udp")

    def get_command(self, direct: bool) -> list[str]:
        return [
            *self.__base_cmd,
            ("-A" if direct else "-D"), "INPUT", "-i", self.__iface, "-p", self.__proto,
            "--dport", str(self.__port), "-j", "ACCEPT",
        ]


class IptablesForwardOut(BaseCtl):
    def __init__(self, base_cmd: list[str], iface: str) -> None:
        self.__base_cmd = base_cmd
        self.__iface = iface

    def get_command(self, direct: bool) -> list[str]:
        return [
            *self.__base_cmd,
            "--table", "nat",
            ("-A" if direct else "-D"), "POSTROUTING",
            "-o", self.__iface, "-j", "MASQUERADE",
        ]


class IptablesForwardIn(BaseCtl):
    def __init__(self, base_cmd: list[str], iface: str) -> None:
        self.__base_cmd = base_cmd
        self.__iface = iface

    def get_command(self, direct: bool) -> list[str]:
        return [
            *self.__base_cmd,
            ("-A" if direct else "-D"), "FORWARD",
            "-i", self.__iface, "-j", "ACCEPT",
        ]


class CustomCtl(BaseCtl):
    def __init__(
        self,
        direct_cmd: list[str],
        reverse_cmd: list[str],
        placeholders: dict[str, str],
    ) -> None:

        self.__direct_cmd = direct_cmd
        self.__reverse_cmd = reverse_cmd
        self.__placeholders = placeholders

    def get_command(self, direct: bool) -> list[str]:
        return [
            part.format(**self.__placeholders)
            for part in (self.__direct_cmd if direct else self.__reverse_cmd)
        ]
