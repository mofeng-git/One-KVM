#!/usr/bin/env python3
# ========================================================================== #
#                                                                            #
#    KVMD-OLED - A small OLED daemon for PiKVM.                              #
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


import socket
import functools
import datetime
import time

import netifaces
import psutil


# =====
class Sensors:
    def __init__(self, fahrenheit: bool) -> None:
        self.__fahrenheit = fahrenheit
        self.__sensors = {
            "fqdn":   socket.getfqdn,
            "iface":  self.__get_iface,
            "ip":     self.__get_ip,
            "uptime": self.__get_uptime,
            "temp":   self.__get_temp,
            "cpu":    self.__get_cpu,
            "mem":    self.__get_mem,
        }

    def render(self, text: str) -> str:
        return text.format_map(self)

    def __getitem__(self, key: str) -> str:
        return self.__sensors[key]()  # type: ignore

    # =====

    def __get_iface(self) -> str:
        return self.__get_netconf(round(time.monotonic() / 0.3))[0]

    def __get_ip(self) -> str:
        return self.__get_netconf(round(time.monotonic() / 0.3))[1]

    @functools.lru_cache(maxsize=1)
    def __get_netconf(self, ts: int) -> tuple[str, str]:
        _ = ts
        try:
            gws = netifaces.gateways()
            if "default" in gws:
                for proto in [socket.AF_INET, socket.AF_INET6]:
                    if proto in gws["default"]:
                        iface = gws["default"][proto][1]
                        addrs = netifaces.ifaddresses(iface)
                        return (iface, addrs[proto][0]["addr"])

            for iface in netifaces.interfaces():
                if not iface.startswith(("lo", "docker")):
                    addrs = netifaces.ifaddresses(iface)
                    for proto in [socket.AF_INET, socket.AF_INET6]:
                        if proto in addrs:
                            return (iface, addrs[proto][0]["addr"])
        except Exception:
            # _logger.exception("Can't get iface/IP")
            pass
        return ("<no-iface>", "<no-ip>")

    # =====

    def __get_uptime(self) -> str:
        uptime = datetime.timedelta(seconds=int(time.time() - psutil.boot_time()))
        pl = {"days": uptime.days}
        (pl["hours"], rem) = divmod(uptime.seconds, 3600)
        (pl["mins"], pl["secs"]) = divmod(rem, 60)
        return "{days}d {hours}h {mins}m".format(**pl)

    # =====

    def __get_temp(self) -> str:
        try:
            with open("/sys/class/thermal/thermal_zone0/temp") as file:
                temp = int(file.read().strip()) / 1000
                if self.__fahrenheit:
                    temp = temp * 9 / 5 + 32
                    return f"{temp:.1f}\u00b0F"
                return f"{temp:.1f}\u00b0C"
        except Exception:
            # _logger.exception("Can't read temp")
            return "<no-temp>"

    # =====

    def __get_cpu(self) -> str:
        st = psutil.cpu_times_percent()
        user = st.user - st.guest
        nice = st.nice - st.guest_nice
        idle_all = st.idle + st.iowait
        system_all = st.system + st.irq + st.softirq
        virtual = st.guest + st.guest_nice
        total = max(1, user + nice + system_all + idle_all + st.steal + virtual)
        percent = int(
            st.nice / total * 100
            + st.user / total * 100
            + system_all / total * 100
            + (st.steal + st.guest) / total * 100
        )
        return f"{percent}%"

    def __get_mem(self) -> str:
        return f"{int(psutil.virtual_memory().percent)}%"
