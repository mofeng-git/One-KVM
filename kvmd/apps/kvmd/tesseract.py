# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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


import io
import ctypes
import ctypes.util
import contextlib
import warnings

from ctypes import POINTER
from ctypes import Structure
from ctypes import c_int
from ctypes import c_bool
from ctypes import c_char_p
from ctypes import c_void_p
from ctypes import c_char

from typing import List
from typing import Set
from typing import Generator
from typing import Optional

from PIL import Image as PilImage

from ...errors import OperationError

from ... import libc
from ... import aiotools


# =====
class OcrError(OperationError):
    pass


# =====
class _TessBaseAPI(Structure):
    pass


def _load_libtesseract() -> Optional[ctypes.CDLL]:
    try:
        path = ctypes.util.find_library("tesseract")
        if not path:
            raise RuntimeError("Can't find libtesseract")
        lib = ctypes.CDLL(path)
        for (name, restype, argtypes) in [
            ("TessBaseAPICreate", POINTER(_TessBaseAPI), []),
            ("TessBaseAPIInit3", c_int, [POINTER(_TessBaseAPI), c_char_p, c_char_p]),
            ("TessBaseAPISetImage", None, [POINTER(_TessBaseAPI), c_void_p, c_int, c_int, c_int, c_int]),
            ("TessBaseAPIGetUTF8Text", POINTER(c_char), [POINTER(_TessBaseAPI)]),
            ("TessBaseAPISetVariable", c_bool, [POINTER(_TessBaseAPI), c_char_p, c_char_p]),
            ("TessBaseAPIGetAvailableLanguagesAsVector", POINTER(POINTER(c_char)), [POINTER(_TessBaseAPI)]),
        ]:
            func = getattr(lib, name)
            if not func:
                raise RuntimeError(f"Can't find libtesseract.{name}")
            setattr(func, "restype", restype)
            setattr(func, "argtypes", argtypes)
        return lib
    except Exception as err:
        warnings.warn(f"Can't load libtesseract: {err}", RuntimeWarning)
        return None


_libtess = _load_libtesseract()


@contextlib.contextmanager
def _tess_api(langs: List[str]) -> Generator[_TessBaseAPI, None, None]:
    if not _libtess:
        raise OcrError("Tesseract is not available")
    api = _libtess.TessBaseAPICreate()
    try:
        if _libtess.TessBaseAPIInit3(api, None, "+".join(langs).encode()) != 0:
            raise OcrError("Can't initialize Tesseract")
        if not _libtess.TessBaseAPISetVariable(api, b"debug_file", b"/dev/null"):
            raise OcrError("Can't set debug_file=/dev/null")
        yield api
    finally:
        _libtess.TessBaseAPIDelete(api)


# =====
class TesseractOcr:
    def __init__(self, default_langs: List[str]) -> None:
        self.__default_langs = default_langs

    def is_available(self) -> bool:
        return bool(_libtess)

    async def get_default_langs(self) -> List[str]:
        return list(self.__default_langs)

    async def get_available_langs(self) -> List[str]:
        return (await aiotools.run_async(self.__inner_get_available_langs))

    def __inner_get_available_langs(self) -> List[str]:
        with _tess_api(["osd"]) as api:
            assert _libtess
            langs: Set[str] = set()
            langs_ptr = _libtess.TessBaseAPIGetAvailableLanguagesAsVector(api)
            if langs_ptr is not None:
                index = 0
                while langs_ptr[index]:
                    lang = ctypes.cast(langs_ptr[index], c_char_p).value
                    if lang is not None:
                        langs.add(lang.decode())
                        libc.free(langs_ptr[index])
                    index += 1
                libc.free(langs_ptr)
            return sorted(langs)

    async def recognize(self, data: bytes, langs: List[str], left: int, top: int, right: int, bottom: int) -> str:
        if not langs:
            langs = self.__default_langs
        return (await aiotools.run_async(self.__inner_recognize, data, langs, left, top, right, bottom))

    def __inner_recognize(self, data: bytes, langs: List[str], left: int, top: int, right: int, bottom: int) -> str:
        with _tess_api(langs) as api:
            assert _libtess
            with io.BytesIO(data) as bio:
                with PilImage.open(bio) as image:
                    if left >= 0 or top >= 0 or right >= 0 or bottom >= 0:
                        left = (0 if left < 0 else min(image.width, left))
                        top = (0 if top < 0 else min(image.height, top))
                        right = (image.width if right < 0 else min(image.width, right))
                        bottom = (image.height if bottom < 0 else min(image.height, bottom))
                        if left < right and top < bottom:
                            image.crop((left, top, right, bottom))

                    _libtess.TessBaseAPISetImage(api, image.tobytes("raw", "RGB"), image.width, image.height, 3, image.width * 3)
                    text_ptr = None
                    try:
                        text_ptr = _libtess.TessBaseAPIGetUTF8Text(api)
                        text = ctypes.cast(text_ptr, c_char_p).value
                        if text is None:
                            raise OcrError("Can't recognize image")
                        return text.decode("utf-8")
                    finally:
                        if text_ptr is not None:
                            libc.free(text_ptr)
