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


from passlib.context import CryptContext
from passlib.apache import HtpasswdFile as _ApacheHtpasswdFile
from passlib.apache import htpasswd_context as _apache_htpasswd_ctx


# =====
_SHA512 = "ldap_salted_sha512"
_SHA256 = "ldap_salted_sha256"


def _make_kvmd_htpasswd_context() -> CryptContext:
    schemes = list(_apache_htpasswd_ctx.schemes())
    for alg in [_SHA256, _SHA512]:
        if alg in schemes:
            schemes.remove(alg)
        schemes.insert(0, alg)
    assert schemes[0] == _SHA512
    return CryptContext(
        schemes=schemes,
        default=_SHA512,
        bcrypt__ident="2y",  # See note in the passlib.apache
    )


_kvmd_htpasswd_ctx = _make_kvmd_htpasswd_context()


# =====
class KvmdHtpasswdFile(_ApacheHtpasswdFile):
    def __init__(self, path: str, new: bool=False) -> None:
        super().__init__(
            path=path,
            default_scheme=_SHA512,
            context=_kvmd_htpasswd_ctx,
            new=new,
        )
