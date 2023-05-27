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


from xml.sax.saxutils import escape

from ....apps.otg.hid.keyboard import make_keyboard_hid
from ....apps.otg.hid.mouse import make_mouse_hid


# =====
def make_sdp_record(manufacturer: str, product: str, description: str) -> str:
    manufacturer = escape(manufacturer)
    product = escape(product)
    description = escape(description)

    keyboard_descriptor = make_keyboard_hid(0x01).report_descriptor.hex().upper()
    mouse_descriptor = make_mouse_hid(False, False, 0x02).report_descriptor.hex().upper()

    return f"""
        <?xml version="1.0" encoding="UTF-8" ?>
        <record>
            <attribute id="0x0001">
                <sequence>
                    <uuid value="0x1124" />
                </sequence>
            </attribute>
            <attribute id="0x0004">
                <sequence>
                    <sequence>
                        <uuid value="0x0100" />
                        <uint16 value="0x0011" />
                    </sequence>
                    <sequence>
                        <uuid value="0x0011" />
                    </sequence>
                </sequence>
            </attribute>
            <attribute id="0x0005">
                <sequence>
                    <uuid value="0x1002" />
                </sequence>
            </attribute>
            <attribute id="0x0006">
                <sequence>
                    <uint16 value="0x656E" />
                    <uint16 value="0x006A" />
                    <uint16 value="0x0100" />
                </sequence>
            </attribute>
            <attribute id="0x0009">
                <sequence>
                    <sequence>
                        <uuid value="0x1124" />
                        <uint16 value="0x0100" />
                    </sequence>
                </sequence>
            </attribute>
            <attribute id="0x000D">
                <sequence>
                    <sequence>
                        <sequence>
                            <uuid value="0x0100" />
                            <uint16 value="0x0013" />
                        </sequence>
                        <sequence>
                            <uuid value="0x0011" />
                        </sequence>
                    </sequence>
                </sequence>
            </attribute>
            <attribute id="0x0100">
                <text value="{product}" />
            </attribute>
            <attribute id="0x0101">
                <text value="{description}" />
            </attribute>
            <attribute id="0x0102">
                <text value="{manufacturer}" />
            </attribute>
            <attribute id="0x0200">
                <uint16 value="0x0100" />
            </attribute>
            <attribute id="0x0201">
                <uint16 value="0x0111" />
            </attribute>
            <attribute id="0x0202">
                <uint8 value="0xC0" />
            </attribute>
            <attribute id="0x0203">
                <uint8 value="0x00" />
            </attribute>
            <attribute id="0x0204">
                <boolean value="false" />
            </attribute>
            <attribute id="0x0205">
                <boolean value="false" />
            </attribute>
            <attribute id="0x0206">
                <sequence>
                    <sequence>
                        <uint8 value="0x22" />
                        <text encoding="hex" value="{keyboard_descriptor}{mouse_descriptor}" />
                    </sequence>
                </sequence>
            </attribute>
            <attribute id="0x0207">
                <sequence>
                    <sequence>
                        <uint16 value="0x0409" />
                        <uint16 value="0x0100" />
                    </sequence>
                </sequence>
            </attribute>
            <attribute id="0x020B">
                <uint16 value="0x0100" />
            </attribute>
            <attribute id="0x020C">
                <uint16 value="0x0C80" />
            </attribute>
            <attribute id="0x020D">
                <boolean value="false" />
            </attribute>
            <attribute id="0x020E">
                <boolean value="false" />
            </attribute>
            <attribute id="0x020F">
                <uint16 value="0x0640" />
            </attribute>
            <attribute id="0x0210">
                <uint16 value="0x0320" />
            </attribute>
        </record>
    """
