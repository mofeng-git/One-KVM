# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
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

from typing import List

import passlib.crypto.des

from OpenSSL import crypto, SSL
from socket import gethostname
from pprint import pprint
from time import gmtime, mktime
import os.path

key_file_name = "private_vnc.key"
cert_file_name = "self_signed_cert.crt"

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
    key: List[int] = []
    for ch in passwd:
        btgt = 0
        for index in range(8):
            if ch & (1 << index):
                btgt = btgt | (1 << 7 - index)
        key.append(btgt)
    return bytes(key)


def create_self_signed_cert_if_nonexistent(key_file, cert_file):
    if os.path.isfile(key_file) and os.path.isfile(cert_file):
        return

    key = crypto.PKey()
    key.generate_key(crypto.TYPE_RSA, 2048)

    cert = crypto.X509()
    cert.get_subject().C = "CA"
    cert.get_subject().ST = "Toronto"
    cert.get_subject().L = "Toronto"
    cert.get_subject().O = "Company Ltd"
    cert.get_subject().OU = "Company Ltd"
    cert.get_subject().CN = gethostname()
    cert.set_serial_number(1000)
    cert.gmtime_adj_notBefore(0)
    cert.gmtime_adj_notAfter(100*365*24*60*60)
    cert.set_issuer(cert.get_subject())
    cert.set_pubkey(key)
    cert.sign(key, 'sha256')

    open(key_file, "wt").write(
        crypto.dump_privatekey(crypto.FILETYPE_PEM, key).decode('utf-8'))
    open(cert_file, "wt").write(
        crypto.dump_certificate(crypto.FILETYPE_PEM, cert).decode('utf-8'))
