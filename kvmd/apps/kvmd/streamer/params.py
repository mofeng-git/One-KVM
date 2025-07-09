# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
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


import copy


# =====
class Params:
    __DESIRED_FPS = "desired_fps"

    __QUALITY = "quality"

    __RESOLUTION = "resolution"
    __AVAILABLE_RESOLUTIONS = "available_resolutions"

    __H264 = "h264"
    __H264_BITRATE = "h264_bitrate"
    __H264_GOP = "h264_gop"

    def __init__(  # pylint: disable=too-many-arguments
        self,
        quality: int,

        resolution: str,
        available_resolutions: list[str],

        desired_fps: int,
        desired_fps_min: int,
        desired_fps_max: int,

        h264_bitrate: int,
        h264_bitrate_min: int,
        h264_bitrate_max: int,

        h264_gop: int,
        h264_gop_min: int,
        h264_gop_max: int,
    ) -> None:

        self.__has_quality = bool(quality)
        self.__has_resolution = bool(resolution)
        self.__has_h264 = bool(h264_bitrate)

        self.__params: dict = {self.__DESIRED_FPS: min(max(desired_fps, desired_fps_min), desired_fps_max)}
        self.__limits: dict = {self.__DESIRED_FPS: {"min": desired_fps_min, "max": desired_fps_max}}

        if self.__has_quality:
            self.__params[self.__QUALITY] = quality

        if self.__has_resolution:
            self.__params[self.__RESOLUTION] = resolution
            self.__limits[self.__AVAILABLE_RESOLUTIONS] = available_resolutions

        if self.__has_h264:
            self.__params[self.__H264_BITRATE] = min(max(h264_bitrate, h264_bitrate_min), h264_bitrate_max)
            self.__limits[self.__H264_BITRATE] = {"min": h264_bitrate_min, "max": h264_bitrate_max}
            self.__params[self.__H264_GOP] = min(max(h264_gop, h264_gop_min), h264_gop_max)
            self.__limits[self.__H264_GOP] = {"min": h264_gop_min, "max": h264_gop_max}

    def get_features(self) -> dict:
        return {
            self.__QUALITY: self.__has_quality,
            self.__RESOLUTION: self.__has_resolution,
            self.__H264: self.__has_h264,
        }

    def get_limits(self) -> dict:
        limits = copy.deepcopy(self.__limits)
        if self.__has_resolution:
            limits[self.__AVAILABLE_RESOLUTIONS] = list(limits[self.__AVAILABLE_RESOLUTIONS])
        return limits

    def get_params(self) -> dict:
        return dict(self.__params)

    def set_params(self, params: dict) -> None:
        new = dict(self.__params)

        if self.__QUALITY in params and self.__has_quality:
            new[self.__QUALITY] = min(max(params[self.__QUALITY], 1), 100)

        if self.__RESOLUTION in params and self.__has_resolution:
            if params[self.__RESOLUTION] in self.__limits[self.__AVAILABLE_RESOLUTIONS]:
                new[self.__RESOLUTION] = params[self.__RESOLUTION]

        for (key, enabled) in [
            (self.__DESIRED_FPS, True),
            (self.__H264_BITRATE, self.__has_h264),
            (self.__H264_GOP, self.__has_h264),
        ]:
            if key in params and enabled:
                if self.__check_limits_min_max(key, params[key]):
                    new[key] = params[key]

        self.__params = new

    def __check_limits_min_max(self, key: str, value: int) -> bool:
        return (self.__limits[key]["min"] <= value <= self.__limits[key]["max"])
