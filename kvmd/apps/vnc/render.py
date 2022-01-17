# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2020  Maxim Devaev <mdevaev@gmail.com>                    #
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
import io
import functools

from PIL import Image as PilImage
from PIL import ImageDraw as PilImageDraw
from PIL import ImageFont as PilImageFont

from ... import aiotools


# =====
async def make_text_jpeg(width: int, height: int, quality: int, text: str) -> bytes:
    return (await aiotools.run_async(_inner_make_text_jpeg, width, height, quality, text))


@functools.lru_cache(maxsize=10)
def _inner_make_text_jpeg(width: int, height: int, quality: int, text: str) -> bytes:
    image = PilImage.new("RGB", (width, height), color=(0, 0, 0))
    draw = PilImageDraw.Draw(image)
    draw.multiline_text((20, 20), text, font=_get_font(), fill=(255, 255, 255))
    with io.BytesIO() as bio:
        image.save(bio, format="jpeg", quality=quality)
        return bio.getvalue()


@functools.lru_cache()
def _get_font() -> PilImageFont.FreeTypeFont:
    module_path = sys.modules[__name__].__file__
    assert module_path is not None
    path = os.path.join(os.path.dirname(module_path), "fonts", "Azbuka04.ttf")
    return PilImageFont.truetype(path, size=20)
