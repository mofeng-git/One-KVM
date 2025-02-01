# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2023-2025  SilentWind <mofeng654321@hotmail.com>         #
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
import hid

VENDOR_ID = 0x5131
PRODUCT_ID = 0x2007

def find_usbrelay():
    for device in hid.enumerate():
        if device.get("vendor_id") == VENDOR_ID and device.get("product_id") == PRODUCT_ID:
            return device
    return None

def send_command(device_info, channel, onoff):
    device = hid.device()
    device.open(device_info['vendor_id'], device_info['product_id'])
    if device is None:
        print("Failed to open device.")
        return

    try:
        cmd = [0xA0, channel, onoff, 0xA0 + channel + onoff]
        device.write(bytearray(cmd))
    finally:
        device.close()

def main():
    if len(sys.argv) != 3:
        print("Usage:\n"
              "\tpython script.py id on|off")
        return
    
    try:
        id = int(sys.argv[1])
        if sys.argv[2].lower() == 'on':
            onoff = 1
        elif sys.argv[2].lower() == 'off':
            onoff = 0
        else:
            raise ValueError
    except ValueError:
        print("Invalid command, use 'on' or 'off'")
        return
    
    device_info = find_usbrelay()
    if device_info is None:
        print("USB relay not found")
    else:
        send_command(device_info, id, onoff)
        print(f"Sent command to channel {id}: {'ON' if onoff else 'OFF'}")

if __name__ == "__main__":
    main()
