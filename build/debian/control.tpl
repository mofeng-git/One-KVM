Source: one-kvm
Section: admin
Priority: optional
Maintainer: SilentWind <admin@mofeng.run>

Package: one-kvm
Architecture: {arch}
Depends: ${{auto}}, ca-certificates{distsuffix}
Description: A open and lightweight IP-KVM solution written in Rust
 Enables BIOS-level remote management of servers and workstations.
 .
 One-KVM provides video capture, HID emulation (keyboard/mouse),
 mass storage device forwarding, and ATX power control for
 remote server management over IP.
 .
 Features:
  * Hardware-accelerated video encoding (VAAPI, QSV, RKMPP)
  * WebRTC and MJPEG streaming with low latency
  * USB HID emulation via OTG gadget
  * Mass storage device for ISO/IMG mounting
  * ATX power control via GPIO or USB relay
Homepage: https://github.com/mofeng-git/One-KVM
