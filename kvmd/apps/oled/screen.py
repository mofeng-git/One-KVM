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


from luma.core.device import device as luma_device
from luma.core.render import canvas as luma_canvas

from PIL import Image
from PIL import ImageFont


# =====
class Screen:
    def __init__(
        self,
        device: luma_device,
        font: ImageFont.FreeTypeFont,
        font_spacing: int,
        offset: tuple[int, int],
    ) -> None:

        self.__device = device
        self.__font = font
        self.__font_spacing = font_spacing
        self.__offset = offset

    def draw_text(self, text: str, offset_x: int=0) -> None:
        with luma_canvas(self.__device) as draw:
            offset = list(self.__offset)
            offset[0] += offset_x
            draw.multiline_text(offset, text, font=self.__font, spacing=self.__font_spacing, fill="white")

    def draw_image(self, image_path: str) -> None:
        with luma_canvas(self.__device) as draw:
            draw.bitmap(self.__offset, Image.open(image_path).convert("1"), fill="white")

    def draw_white(self) -> None:
        with luma_canvas(self.__device) as draw:
            draw.rectangle((0, 0, self.__device.width, self.__device.height), fill="white")
