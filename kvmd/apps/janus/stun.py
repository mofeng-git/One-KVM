import asyncio
import socket
import ipaddress
import struct
import secrets
import dataclasses

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
    ext: (StunAddress | None) = dataclasses.field(default=None)
    src: (StunAddress | None) = dataclasses.field(default=None)
    changed: (StunAddress | None) = dataclasses.field(default=None)


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
class Stun:
    # Partially based on https://github.com/JohnVillalovos/pystun

    def __init__(
        self,
        host: str,
        port: int,
        timeout: float,
        retries: int,
        retries_delay: float,
    ) -> None:

        self.host = host
        self.port = port
        self.__timeout = timeout
        self.__retries = retries
        self.__retries_delay = retries_delay

        self.__sock: (socket.socket | None) = None

    async def get_info(self, src_ip: str, src_port: int) -> tuple[str, str]:
        (family, _, _, _, addr) = socket.getaddrinfo(src_ip, src_port, type=socket.SOCK_DGRAM)[0]
        try:
            with socket.socket(family, socket.SOCK_DGRAM) as self.__sock:
                self.__sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                self.__sock.settimeout(self.__timeout)
                self.__sock.bind(addr)
                (nat_type, response) = await self.__get_nat_type(src_ip)
                return (nat_type, (response.ext.ip if response.ext is not None else ""))
        finally:
            self.__sock = None

    async def __get_nat_type(self, src_ip: str) -> tuple[str, StunResponse]:  # pylint: disable=too-many-return-statements
        first = await self.__make_request("First probe")
        if not first.ok:
            return (StunNatType.BLOCKED, first)

        request = struct.pack(">HHI", 0x0003, 0x0004, 0x00000006)  # Change-Request
        response = await self.__make_request("Change request [ext_ip == src_ip]", request)

        if first.ext is not None and first.ext.ip == src_ip:
            if response.ok:
                return (StunNatType.OPEN_INTERNET, response)
            return (StunNatType.SYMMETRIC_UDP_FW, response)

        if response.ok:
            return (StunNatType.FULL_CONE_NAT, response)

        if first.changed is None:
            raise RuntimeError(f"Changed addr is None: {first}")
        response = await self.__make_request("Change request [ext_ip != src_ip]", addr=first.changed)
        if not response.ok:
            return (StunNatType.CHANGED_ADDR_ERROR, response)

        if response.ext == first.ext:
            request = struct.pack(">HHI", 0x0003, 0x0004, 0x00000002)
            response = await self.__make_request("Change port", request, addr=first.changed.ip)
            if response.ok:
                return (StunNatType.RESTRICTED_NAT, response)
            return (StunNatType.RESTRICTED_PORT_NAT, response)

        return (StunNatType.SYMMETRIC_NAT, response)

    async def __make_request(self, ctx: str, request: bytes=b"", addr: (StunAddress | str | None)=None) -> StunResponse:
        # TODO: Support IPv6 and RFC 5389
        # The first 4 bytes of the response are the Type (2) and Length (2)
        # The 5th byte is Reserved
        # The 6th byte is the Family: 0x01 = IPv4, 0x02 = IPv6
        # The remaining bytes are the IP address. 32 bits for IPv4 or 128 bits for
        # IPv6.
        # More info at: https://tools.ietf.org/html/rfc3489#section-11.2.1
        # And at: https://tools.ietf.org/html/rfc5389#section-15.1

        if isinstance(addr, StunAddress):
            addr_t = (addr.ip, addr.port)
        elif isinstance(addr, str):
            addr_t = (addr, self.port)
        else:
            assert addr is None
            addr_t = (self.host, self.port)

        # https://datatracker.ietf.org/doc/html/rfc5389#section-6
        trans_id = b"\x21\x12\xA4\x42" + secrets.token_bytes(12)
        (response, error) = (b"", "")
        for _ in range(self.__retries):
            (response, error) = await self.__inner_make_request(trans_id, request, addr_t)
            if not error:
                break
            await asyncio.sleep(self.__retries_delay)
        if error:
            get_logger(0).error("%s: Can't perform STUN request after %d retries; last error: %s",
                                ctx, self.__retries, error)
            return StunResponse(ok=False)

        parsed: dict[str, StunAddress] = {}
        offset = 0
        remaining = len(response)
        while remaining > 0:
            (attr_type, attr_len) = struct.unpack(">HH", response[offset : offset + 4])  # noqa: E203
            offset += 4
            field = {
                0x0001: "ext",      # MAPPED-ADDRESS
                0x0020: "ext",      # XOR-MAPPED-ADDRESS
                0x0004: "src",      # SOURCE-ADDRESS
                0x0005: "changed",  # CHANGED-ADDRESS
            }.get(attr_type)
            if field is not None:
                parsed[field] = self.__parse_address(response[offset:], (trans_id if attr_type == 0x0020 else b""))
            offset += attr_len
            remaining -= (4 + attr_len)
        return StunResponse(ok=True, **parsed)

    async def __inner_make_request(self, trans_id: bytes, request: bytes, addr: tuple[str, int]) -> tuple[bytes, str]:
        assert self.__sock is not None

        request = struct.pack(">HH", 0x0001, len(request)) + trans_id + request  # Bind Request

        try:
            await aiotools.run_async(self.__sock.sendto, request, addr)
        except Exception as err:
            return (b"", f"Send error: {tools.efmt(err)}")
        try:
            response = (await aiotools.run_async(self.__sock.recvfrom, 2048))[0]
        except Exception as err:
            return (b"", f"Recv error: {tools.efmt(err)}")

        (response_type, payload_len) = struct.unpack(">HH", response[:4])
        if response_type != 0x0101:
            return (b"", f"Invalid response type: {response_type:#06x}")
        if trans_id != response[4:20]:
            return (b"", "Transaction ID mismatch")

        return (response[20 : 20 + payload_len], "")  # noqa: E203

    def __parse_address(self, data: bytes, trans_id: bytes) -> StunAddress:
        family = data[1]
        port = struct.unpack(">H", self.__trans_xor(data[2:4], trans_id))[0]
        if family == 0x01:
            return StunAddress(str(ipaddress.IPv4Address(self.__trans_xor(data[4:8], trans_id))), port)
        elif family == 0x02:
            return StunAddress(str(ipaddress.IPv6Address(self.__trans_xor(data[4:20], trans_id))), port)
        raise RuntimeError(f"Unknown family; received: {family}")

    def __trans_xor(self, data: bytes, trans_id: bytes) -> bytes:
        if len(trans_id) == 0:
            return data
        assert len(data) <= len(trans_id)
        return bytes(byte ^ trans_id[index] for (index, byte) in enumerate(data))
