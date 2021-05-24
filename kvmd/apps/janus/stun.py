import socket
import struct
import secrets
import dataclasses

from typing import Tuple
from typing import Dict
from typing import Optional

from ... import tools
from ... import aiotools

from ...logging import get_logger


# =====
@dataclasses.dataclass(frozen=True)
class StunAddress:
    ip: str
    port: int


@dataclasses.dataclass(frozen=True)
class StunResponse:
    ok: bool
    ext: Optional[StunAddress] = dataclasses.field(default=None)
    src: Optional[StunAddress] = dataclasses.field(default=None)
    changed: Optional[StunAddress] = dataclasses.field(default=None)


class StunNatType:
    BLOCKED = "Blocked"
    OPEN_INTERNET = "Open Internet"
    SYMMETRIC_UDP_FW = "Symmetric UDP Firewall"
    FULL_CONE_NAT = "Full Cone NAT"
    RESTRICTED_NAT = "Restricted NAT"
    RESTRICTED_PORT_NAT = "Restricted Port NAT"
    SYMMETRIC_NAT = "Symmetric NAT"
    CHANGED_ADDR_ERROR = "Error when testing on Changed-IP and Port"


# =====
async def stun_get_info(
    stun_host: str,
    stun_port: int,
    src_ip: str,
    src_port: int,
    timeout: float,
) -> Tuple[str, str]:

    return (await aiotools.run_async(_stun_get_info, stun_host, stun_port, src_ip, src_port, timeout))


def _stun_get_info(
    stun_host: str,
    stun_port: int,
    src_ip: str,
    src_port: int,
    timeout: float,
) -> Tuple[str, str]:

    # Partially based on https://github.com/JohnVillalovos/pystun

    (family, _, _, _, addr) = socket.getaddrinfo(src_ip, src_port, type=socket.SOCK_DGRAM)[0]
    with socket.socket(family, socket.SOCK_DGRAM) as sock:
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        sock.settimeout(timeout)
        sock.bind(addr)
        (nat_type, response) = _get_nat_type(
            stun_host=stun_host,
            stun_port=stun_port,
            src_ip=src_ip,
            sock=sock,
        )
        return (nat_type, (response.ext.ip if response.ext is not None else ""))


def _get_nat_type(  # pylint: disable=too-many-return-statements
    stun_host: str,
    stun_port: int,
    src_ip: str,
    sock: socket.socket,
) -> Tuple[str, StunResponse]:

    first = _stun_request("First probe", stun_host, stun_port, b"", sock)
    if not first.ok:
        return (StunNatType.BLOCKED, first)
    if first.ext is None:
        raise RuntimeError(f"Ext addr is None: {first}")

    request = struct.pack(">HHI", 0x0003, 0x0004, 0x00000006)  # Change-Request
    response = _stun_request("Change request [ext_ip == src_ip]", stun_host, stun_port, request, sock)

    if first.ext.ip == src_ip:
        if response.ok:
            return (StunNatType.OPEN_INTERNET, response)
        return (StunNatType.SYMMETRIC_UDP_FW, response)

    if response.ok:
        return (StunNatType.FULL_CONE_NAT, response)

    if first.changed is None:
        raise RuntimeError(f"Changed addr is None: {first}")
    response = _stun_request("Change request [ext_ip != src_ip]", first.changed.ip, first.changed.port, b"", sock)
    if not response.ok:
        return (StunNatType.CHANGED_ADDR_ERROR, response)

    if response.ext == first.ext:
        request = struct.pack(">HHI", 0x0003, 0x0004, 0x00000002)
        response = _stun_request("Change port", first.changed.ip, stun_port, request, sock)
        if response.ok:
            return (StunNatType.RESTRICTED_NAT, response)
        return (StunNatType.RESTRICTED_PORT_NAT, response)

    return (StunNatType.SYMMETRIC_NAT, response)


def _stun_request(  # pylint: disable=too-many-locals
    ctx: str,
    host: str,
    port: int,
    request: bytes,
    sock: socket.socket,
) -> StunResponse:

    # TODO: Support IPv6 and RFC 5389
    # The first 4 bytes of the response are the Type (2) and Length (2)
    # The 5th byte is Reserved
    # The 6th byte is the Family: 0x01 = IPv4, 0x02 = IPv6
    # The remaining bytes are the IP address. 32 bits for IPv4 or 128 bits for
    # IPv6.
    # More info at: https://tools.ietf.org/html/rfc3489#section-11.2.1
    # And at: https://tools.ietf.org/html/rfc5389#section-15.1

    trans_id = secrets.token_bytes(16)
    request = struct.pack(">HH", 0x0001, len(request)) + trans_id + request  # Bind Request

    try:
        sock.sendto(request, (host, port))
    except Exception as err:
        get_logger().error("%s: Can't send request: %s", ctx, tools.efmt(err))
        return StunResponse(ok=False)
    try:
        response = sock.recvfrom(2048)[0]
    except Exception as err:
        get_logger().error("%s: Can't recv response: %s", ctx, tools.efmt(err))
        return StunResponse(ok=False)

    (response_type, payload_len) = struct.unpack(">HH", response[:4])
    if response_type != 0x0101:
        get_logger().error("%s: Invalid response type: %#.4x", ctx, response_type)
        return StunResponse(ok=False)
    if trans_id != response[4:20]:
        get_logger().error("%s: Transaction ID mismatch")
        return StunResponse(ok=False)

    parsed: Dict[str, StunAddress] = {}
    base = 20
    remaining = payload_len
    while remaining > 0:
        (attr_type, attr_len) = struct.unpack(">HH", response[base:(base + 4)])
        base += 4
        field = {
            0x0001: "ext",      # MAPPED-ADDRESS
            0x0004: "src",      # SOURCE-ADDRESS
            0x0005: "changed",  # CHANGED-ADDRESS
        }.get(attr_type)
        if field is not None:
            parsed[field] = _parse_address(response[base:])
        base += attr_len
        remaining -= (4 + attr_len)
    return StunResponse(ok=True, **parsed)


def _parse_address(data: bytes) -> StunAddress:
    family = data[1]
    if family == 1:
        parts = struct.unpack(">HBBBB", data[2:8])
        return StunAddress(
            ip=".".join(map(str, parts[1:])),
            port=parts[0],
        )
    raise RuntimeError(f"Only IPv4 supported; received: {family}")
