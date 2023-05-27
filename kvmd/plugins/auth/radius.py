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


import io

import pyrad.client
import pyrad.packet
import pyrad.dictionary

from ...yamlconf import Option

from ...validators.net import valid_port
from ...validators.net import valid_ip_or_host
from ...validators.basic import valid_int_f1

from ...logging import get_logger

from ... import aiotools

from . import BaseAuthService


# =====
_FREERADUIS_DICT = """
# https://github.com/pyradius/pyrad/raw/master/example/dictionary

#
#	Following are the proper new names. Use these.
#
ATTRIBUTE	User-Name		1	string
ATTRIBUTE	User-Password		2	string
ATTRIBUTE	CHAP-Password		3	octets
ATTRIBUTE	NAS-IP-Address		4	ipaddr
ATTRIBUTE	NAS-Port		5	integer
ATTRIBUTE	Service-Type		6	integer
ATTRIBUTE	Framed-Protocol		7	integer
ATTRIBUTE	Framed-IP-Address	8	ipaddr
ATTRIBUTE	Framed-IP-Netmask	9	ipaddr
ATTRIBUTE	Framed-Routing		10	integer
ATTRIBUTE	Filter-Id		11	string
ATTRIBUTE	Framed-MTU		12	integer
ATTRIBUTE	Framed-Compression	13	integer
ATTRIBUTE	Login-IP-Host		14	ipaddr
ATTRIBUTE	Login-Service		15	integer
ATTRIBUTE	Login-TCP-Port		16	integer
ATTRIBUTE	Reply-Message		18	string
ATTRIBUTE	Callback-Number		19	string
ATTRIBUTE	Callback-Id		20	string
ATTRIBUTE	Framed-Route		22	string
ATTRIBUTE	Framed-IPX-Network	23	ipaddr
ATTRIBUTE	State			24	octets
ATTRIBUTE	Class			25	octets
ATTRIBUTE	Vendor-Specific		26	octets
ATTRIBUTE	Session-Timeout		27	integer
ATTRIBUTE	Idle-Timeout		28	integer
ATTRIBUTE	Termination-Action	29	integer
ATTRIBUTE	Called-Station-Id	30	string
ATTRIBUTE	Calling-Station-Id	31	string
ATTRIBUTE	NAS-Identifier		32	string
ATTRIBUTE	Proxy-State		33	octets
ATTRIBUTE	Login-LAT-Service	34	string
ATTRIBUTE	Login-LAT-Node		35	string
ATTRIBUTE	Login-LAT-Group		36	octets
ATTRIBUTE	Framed-AppleTalk-Link	37	integer
ATTRIBUTE	Framed-AppleTalk-Network 38	integer
ATTRIBUTE	Framed-AppleTalk-Zone	39	string

ATTRIBUTE	Acct-Status-Type	40	integer
ATTRIBUTE	Acct-Delay-Time		41	integer
ATTRIBUTE	Acct-Input-Octets	42	integer
ATTRIBUTE	Acct-Output-Octets	43	integer
ATTRIBUTE	Acct-Session-Id		44	string
ATTRIBUTE	Acct-Authentic		45	integer
ATTRIBUTE	Acct-Session-Time	46	integer
ATTRIBUTE       Acct-Input-Packets	47	integer
ATTRIBUTE       Acct-Output-Packets	48	integer
ATTRIBUTE	Acct-Terminate-Cause	49	integer
ATTRIBUTE	Acct-Multi-Session-Id	50	string
ATTRIBUTE	Acct-Link-Count		51	integer
ATTRIBUTE	Acct-Input-Gigawords    52      integer
ATTRIBUTE	Acct-Output-Gigawords   53      integer
ATTRIBUTE	Event-Timestamp         55      date

ATTRIBUTE	CHAP-Challenge		60	string
ATTRIBUTE	NAS-Port-Type		61	integer
ATTRIBUTE	Port-Limit		62	integer
ATTRIBUTE	Login-LAT-Port		63	integer

ATTRIBUTE	Acct-Tunnel-Connection	68	string

ATTRIBUTE	ARAP-Password           70      string
ATTRIBUTE	ARAP-Features           71      string
ATTRIBUTE	ARAP-Zone-Access        72      integer
ATTRIBUTE	ARAP-Security           73      integer
ATTRIBUTE	ARAP-Security-Data      74      string
ATTRIBUTE	Password-Retry          75      integer
ATTRIBUTE	Prompt                  76      integer
ATTRIBUTE	Connect-Info		77	string
ATTRIBUTE	Configuration-Token	78	string
ATTRIBUTE	EAP-Message		79	string
ATTRIBUTE	Message-Authenticator	80	octets
ATTRIBUTE	ARAP-Challenge-Response	84	string	# 10 octets
ATTRIBUTE	Acct-Interim-Interval   85      integer
ATTRIBUTE	NAS-Port-Id		87	string
ATTRIBUTE	Framed-Pool		88	string
ATTRIBUTE	NAS-IPv6-Address	95	octets	# really IPv6
ATTRIBUTE	Framed-Interface-Id	96	octets	# 8 octets
ATTRIBUTE	Framed-IPv6-Prefix	97	ipv6prefix	# stupid format
ATTRIBUTE	Login-IPv6-Host		98	octets	# really IPv6
ATTRIBUTE	Framed-IPv6-Route	99	string
ATTRIBUTE	Framed-IPv6-Pool	100	string
ATTRIBUTE   Delegated-IPv6-Prefix   123     ipv6prefix


ATTRIBUTE	Digest-Response		206	string
ATTRIBUTE	Digest-Attributes	207	octets	# stupid format

#
#	Experimental Non Protocol Attributes used by Cistron-Radiusd
#

# 	These attributes CAN go in the reply item list.
ATTRIBUTE	Fall-Through		500	integer
ATTRIBUTE	Exec-Program		502	string
ATTRIBUTE	Exec-Program-Wait	503	string

#	These attributes CANNOT go in the reply item list.
ATTRIBUTE	User-Category		1029	string
ATTRIBUTE	Group-Name		1030	string
ATTRIBUTE	Huntgroup-Name		1031	string
ATTRIBUTE	Simultaneous-Use	1034	integer
ATTRIBUTE	Strip-User-Name		1035	integer
ATTRIBUTE	Hint			1040	string
ATTRIBUTE	Pam-Auth		1041	string
ATTRIBUTE	Login-Time		1042	string
ATTRIBUTE	Stripped-User-Name	1043	string
ATTRIBUTE	Current-Time		1044	string
ATTRIBUTE	Realm			1045	string
ATTRIBUTE	No-Such-Attribute	1046	string
ATTRIBUTE	Packet-Type		1047	integer
ATTRIBUTE	Proxy-To-Realm		1048	string
ATTRIBUTE	Replicate-To-Realm	1049	string
ATTRIBUTE	Acct-Session-Start-Time	1050	date
ATTRIBUTE	Acct-Unique-Session-Id  1051	string
ATTRIBUTE	Client-IP-Address	1052	ipaddr
ATTRIBUTE	Ldap-UserDn		1053	string
ATTRIBUTE	NS-MTA-MD5-Password	1054	string
ATTRIBUTE	SQL-User-Name	 	1055	string
ATTRIBUTE	LM-Password		1057	octets
ATTRIBUTE	NT-Password		1058	octets
ATTRIBUTE	SMB-Account-CTRL	1059	integer
ATTRIBUTE	SMB-Account-CTRL-TEXT	1061	string
ATTRIBUTE	User-Profile		1062	string
ATTRIBUTE	Digest-Realm		1063	string
ATTRIBUTE	Digest-Nonce		1064	string
ATTRIBUTE	Digest-Method		1065	string
ATTRIBUTE	Digest-URI		1066	string
ATTRIBUTE	Digest-QOP		1067	string
ATTRIBUTE	Digest-Algorithm	1068	string
ATTRIBUTE	Digest-Body-Digest	1069	string
ATTRIBUTE	Digest-CNonce		1070	string
ATTRIBUTE	Digest-Nonce-Count	1071	string
ATTRIBUTE	Digest-User-Name	1072	string
ATTRIBUTE	Pool-Name		1073	string
ATTRIBUTE	Ldap-Group		1074	string
ATTRIBUTE	Module-Success-Message	1075	string
ATTRIBUTE	Module-Failure-Message	1076	string
#		X99-Fast		1077	integer

#
#	Non-Protocol Attributes
#	These attributes are used internally by the server
#
ATTRIBUTE	Auth-Type		1000	integer
ATTRIBUTE	Menu			1001	string
ATTRIBUTE	Termination-Menu	1002	string
ATTRIBUTE	Prefix			1003	string
ATTRIBUTE	Suffix			1004	string
ATTRIBUTE	Group			1005	string
ATTRIBUTE	Crypt-Password		1006	string
ATTRIBUTE	Connect-Rate		1007	integer
ATTRIBUTE	Add-Prefix		1008	string
ATTRIBUTE	Add-Suffix		1009	string
ATTRIBUTE	Expiration		1010	date
ATTRIBUTE	Autz-Type		1011	integer

#
#	Integer Translations
#

#	User Types

VALUE		Service-Type		Login-User		1
VALUE		Service-Type		Framed-User		2
VALUE		Service-Type		Callback-Login-User	3
VALUE		Service-Type		Callback-Framed-User	4
VALUE		Service-Type		Outbound-User		5
VALUE		Service-Type		Administrative-User	6
VALUE		Service-Type		NAS-Prompt-User		7
VALUE		Service-Type		Authenticate-Only	8
VALUE		Service-Type		Callback-NAS-Prompt	9
VALUE		Service-Type		Call-Check		10
VALUE		Service-Type		Callback-Administrative	11

#	Framed Protocols

VALUE		Framed-Protocol		PPP			1
VALUE		Framed-Protocol		SLIP			2
VALUE		Framed-Protocol		ARAP			3
VALUE		Framed-Protocol		Gandalf-SLML		4
VALUE		Framed-Protocol		Xylogics-IPX-SLIP	5
VALUE		Framed-Protocol		X.75-Synchronous	6

#	Framed Routing Values

VALUE		Framed-Routing		None			0
VALUE		Framed-Routing		Broadcast		1
VALUE		Framed-Routing		Listen			2
VALUE		Framed-Routing		Broadcast-Listen	3

#	Framed Compression Types

VALUE		Framed-Compression	None			0
VALUE		Framed-Compression	Van-Jacobson-TCP-IP	1
VALUE		Framed-Compression	IPX-Header-Compression	2
VALUE		Framed-Compression	Stac-LZS		3

#	Login Services

VALUE		Login-Service		Telnet			0
VALUE		Login-Service		Rlogin			1
VALUE		Login-Service		TCP-Clear		2
VALUE		Login-Service		PortMaster		3
VALUE		Login-Service		LAT			4
VALUE		Login-Service		X25-PAD			5
VALUE		Login-Service		X25-T3POS		6
VALUE		Login-Service		TCP-Clear-Quiet		8

#	Login-TCP-Port		(see /etc/services for more examples)

VALUE		Login-TCP-Port		Telnet			23
VALUE		Login-TCP-Port		Rlogin			513
VALUE		Login-TCP-Port		Rsh			514

#	Status Types

VALUE		Acct-Status-Type	Start			1
VALUE		Acct-Status-Type	Stop			2
VALUE		Acct-Status-Type	Interim-Update		3
VALUE		Acct-Status-Type	Alive			3
VALUE		Acct-Status-Type	Accounting-On		7
VALUE		Acct-Status-Type	Accounting-Off		8
#	RFC 2867 Additional Status-Type Values
VALUE		Acct-Status-Type	Tunnel-Start		9
VALUE		Acct-Status-Type	Tunnel-Stop		10
VALUE		Acct-Status-Type	Tunnel-Reject		11
VALUE		Acct-Status-Type	Tunnel-Link-Start	12
VALUE		Acct-Status-Type	Tunnel-Link-Stop	13
VALUE		Acct-Status-Type	Tunnel-Link-Reject	14

#	Authentication Types

VALUE		Acct-Authentic		RADIUS			1
VALUE		Acct-Authentic		Local			2

#	Termination Options

VALUE		Termination-Action	Default			0
VALUE		Termination-Action	RADIUS-Request		1

#	NAS Port Types

VALUE		NAS-Port-Type		Async			0
VALUE		NAS-Port-Type		Sync			1
VALUE		NAS-Port-Type		ISDN			2
VALUE		NAS-Port-Type		ISDN-V120		3
VALUE		NAS-Port-Type		ISDN-V110		4
VALUE		NAS-Port-Type		Virtual			5
VALUE		NAS-Port-Type		PIAFS			6
VALUE		NAS-Port-Type		HDLC-Clear-Channel	7
VALUE		NAS-Port-Type		X.25			8
VALUE		NAS-Port-Type		X.75			9
VALUE		NAS-Port-Type		G.3-Fax			10
VALUE		NAS-Port-Type		SDSL			11
VALUE		NAS-Port-Type		ADSL-CAP		12
VALUE		NAS-Port-Type		ADSL-DMT		13
VALUE		NAS-Port-Type		IDSL			14
VALUE		NAS-Port-Type		Ethernet		15
VALUE		NAS-Port-Type		xDSL			16
VALUE		NAS-Port-Type		Cable			17
VALUE		NAS-Port-Type		Wireless-Other		18
VALUE		NAS-Port-Type		Wireless-802.11		19

#	Acct Terminate Causes, available in 3.3.2 and later

VALUE           Acct-Terminate-Cause    User-Request            1
VALUE           Acct-Terminate-Cause    Lost-Carrier            2
VALUE           Acct-Terminate-Cause    Lost-Service            3
VALUE           Acct-Terminate-Cause    Idle-Timeout            4
VALUE           Acct-Terminate-Cause    Session-Timeout         5
VALUE           Acct-Terminate-Cause    Admin-Reset             6
VALUE           Acct-Terminate-Cause    Admin-Reboot            7
VALUE           Acct-Terminate-Cause    Port-Error              8
VALUE           Acct-Terminate-Cause    NAS-Error               9
VALUE           Acct-Terminate-Cause    NAS-Request             10
VALUE           Acct-Terminate-Cause    NAS-Reboot              11
VALUE           Acct-Terminate-Cause    Port-Unneeded           12
VALUE           Acct-Terminate-Cause    Port-Preempted          13
VALUE           Acct-Terminate-Cause    Port-Suspended          14
VALUE           Acct-Terminate-Cause    Service-Unavailable     15
VALUE           Acct-Terminate-Cause    Callback                16
VALUE           Acct-Terminate-Cause    User-Error              17
VALUE           Acct-Terminate-Cause    Host-Request            18

#VALUE		Tunnel-Type		L2TP			3
#VALUE		Tunnel-Medium-Type	IP			1

VALUE		Prompt			No-Echo			0
VALUE		Prompt			Echo			1

#
#	Non-Protocol Integer Translations
#

VALUE		Auth-Type		Local			0
VALUE		Auth-Type		System			1
VALUE		Auth-Type		SecurID			2
VALUE		Auth-Type		Crypt-Local		3
VALUE		Auth-Type		Reject			4
VALUE		Auth-Type		ActivCard		5
VALUE		Auth-Type		EAP			6
VALUE		Auth-Type		ARAP			7

#
#	Cistron extensions
#
VALUE		Auth-Type		Ldap			252
VALUE		Auth-Type		Pam			253
VALUE		Auth-Type		Accept			254

VALUE		Auth-Type		PAP			1024
VALUE		Auth-Type		CHAP			1025
VALUE		Auth-Type		LDAP			1026
VALUE		Auth-Type		PAM			1027
VALUE		Auth-Type		MS-CHAP			1028
VALUE		Auth-Type		Kerberos		1029
VALUE		Auth-Type		CRAM			1030
VALUE		Auth-Type		NS-MTA-MD5		1031
VALUE		Auth-Type		CRAM			1032
VALUE		Auth-Type		SMB			1033

#
#	Authorization type, too.
#
VALUE		Autz-Type		Local			0

#
#	Experimental Non-Protocol Integer Translations for Cistron-Radiusd
#
VALUE		Fall-Through		No			0
VALUE		Fall-Through		Yes			1

VALUE		Packet-Type	Access-Request			1
VALUE		Packet-Type	Access-Accept			2
VALUE		Packet-Type	Access-Reject			3
VALUE		Packet-Type	Accounting-Request		4
VALUE		Packet-Type	Accounting-Response		5
VALUE		Packet-Type	Accounting-Status		6
VALUE		Packet-Type	Password-Request		7
VALUE		Packet-Type	Password-Accept			8
VALUE		Packet-Type	Password-Reject			9
VALUE		Packet-Type	Accounting-Message		10
VALUE		Packet-Type	Access-Challenge		11
VALUE		Packet-Type	Status-Server			12
VALUE		Packet-Type	Status-Client			13
"""


# =====
class Plugin(BaseAuthService):
    def __init__(  # pylint: disable=super-init-not-called
        self,
        host: str,
        port: int,
        secret: str,
        timeout: float,
    ) -> None:

        self.__host = host
        self.__port = port
        self.__secret = secret
        self.__timeout = timeout

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "host":    Option("localhost", type=valid_ip_or_host),
            "port":    Option(1812, type=valid_port),
            "secret":  Option(""),
            "timeout": Option(5, type=valid_int_f1),
        }

    async def authorize(self, user: str, passwd: str) -> bool:
        return (await aiotools.run_async(self.__inner_authorize, user, passwd))

    def __inner_authorize(self, user: str, passwd: str) -> bool:
        assert user == user.strip()
        assert user
        try:
            with io.StringIO(_FREERADUIS_DICT) as file:
                dct = pyrad.dictionary.Dictionary(file)
            client = pyrad.client.Client(
                server=self.__host,
                authport=self.__port,
                secret=self.__secret.encode("ascii"),
                timeout=self.__timeout,
                dict=dct,
            )
            request = client.CreateAuthPacket(code=pyrad.packet.AccessRequest, User_Name=user)
            request["User-Password"] = request.PwCrypt(passwd)
            response = client.SendPacket(request)
            return (response.code == pyrad.packet.AccessAccept)
        except Exception:
            get_logger().exception("Failed RADIUS auth request for user %r", user)
            return False
