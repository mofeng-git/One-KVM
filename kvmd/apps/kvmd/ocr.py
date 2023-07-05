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


import os
import stat
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

from typing import Generator

from PIL import ImageOps
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


def _load_libtesseract() -> (ctypes.CDLL | None):
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
def _tess_api(data_dir_path: str, langs: list[str]) -> Generator[_TessBaseAPI, None, None]:
    if not _libtess:
        raise OcrError("Tesseract is not available")
    api = _libtess.TessBaseAPICreate()
    try:
        if _libtess.TessBaseAPIInit3(api, data_dir_path.encode(), "+".join(langs).encode()) != 0:
            raise OcrError("Can't initialize Tesseract")
        if not _libtess.TessBaseAPISetVariable(api, b"debug_file", b"/dev/null"):
            raise OcrError("Can't set debug_file=/dev/null")
        yield api
    finally:
        _libtess.TessBaseAPIDelete(api)


_LANG_SUFFIX = ".traineddata"


# =====
class Ocr:
    def __init__(self, data_dir_path: str, default_langs: list[str]) -> None:
        self.__data_dir_path = data_dir_path
        self.__default_langs = default_langs

    def is_available(self) -> bool:
        return bool(_libtess)

    def get_default_langs(self) -> list[str]:
        return list(self.__default_langs)

    def get_available_langs(self) -> list[str]:
        # Это быстрее чем, инициализация либы и TessBaseAPIGetAvailableLanguagesAsVector()
        langs: set[str] = set()
        for lang_name in os.listdir(self.__data_dir_path):
            if lang_name.endswith(_LANG_SUFFIX):
                path = os.path.join(self.__data_dir_path, lang_name)
                if os.access(path, os.R_OK) and stat.S_ISREG(os.stat(path).st_mode):
                    lang = lang_name[:-len(_LANG_SUFFIX)]
                    if lang:
                        langs.add(lang)
        return sorted(langs)

    async def recognize(self, data: bytes, langs: list[str], left: int, top: int, right: int, bottom: int) -> str:
        if not langs:
            langs = self.__default_langs
        return (await aiotools.run_async(self.__inner_recognize, data, langs, left, top, right, bottom))

    def __inner_recognize(self, data: bytes, langs: list[str], left: int, top: int, right: int, bottom: int) -> str:
        with _tess_api(self.__data_dir_path, langs) as api:
            assert _libtess
            with io.BytesIO(data) as bio:
                image = PilImage.open(bio)
                try:
                    if left >= 0 or top >= 0 or right >= 0 or bottom >= 0:
                        left = (0 if left < 0 else min(image.width, left))
                        top = (0 if top < 0 else min(image.height, top))
                        right = (image.width if right < 0 else min(image.width, right))
                        bottom = (image.height if bottom < 0 else min(image.height, bottom))
                        if left < right and top < bottom:
                            image_cropped = image.crop((left, top, right, bottom))
                            image.close()
                            image = image_cropped

                    ImageOps.grayscale(image)
                    image_resized = image.resize((int(image.size[0] * 2), int(image.size[1] * 2)), PilImage.Resampling.BICUBIC)
                    image.close()
                    image = image_resized

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
                finally:
                    image.close()
