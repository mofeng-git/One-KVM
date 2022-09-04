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


import os

import passlib.crypto.des


# =====
def rfb_make_challenge() -> bytes:
    return os.urandom(16)


def rfb_encrypt_challenge(challenge: bytes, passwd: bytes) -> bytes:
    assert len(challenge) == 16
    key = _make_key(passwd)
    return (
        passlib.crypto.des.des_encrypt_block(key, challenge[:8])
        + passlib.crypto.des.des_encrypt_block(key, challenge[8:])
    )


def _make_key(passwd: bytes) -> bytes:
    passwd = (passwd + b"\0" * 8)[:8]
    key: list[int] = []
    for ch in passwd:
        btgt = 0
        for index in range(8):
            if ch & (1 << index):
                btgt = btgt | (1 << 7 - index)
        key.append(btgt)
    return bytes(key)
