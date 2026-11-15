# Third-party components

- AirFerry by UR-SillyB, MIT. Pinned to
  `8a72ab86dee9d8f19b74bfec56c270101c5980ba` (v1.2.8 source).
  https://github.com/UR-SillyB/AirFerry
  Rust protocol/transfer crates and native ZXing adapter.
  Pinned native sources and their LICENSE are included in libs/airferry-core/native/vendor.
- ZXing-C++, Apache-2.0. Pinned to
  `8dd1cf5c4fd6fb6211bb96713db926ac6f2cf825`, matching AirFerry's build.
  https://github.com/zxing-cpp/zxing-cpp
  Pinned native sources and their LICENSE are included in libs/airferry-core/native/vendor.
- raptorq 2.0.1 by Christopher Berner, Apache-2.0, vendored under libs/airferry-core/vendor/raptorq.
  https://github.com/cberner/raptorq
  ARM32 compilation fix documented in [PATCHES.md — raptorq](PATCHES.md#raptorq).
- libjpeg-turbo, provided by the target system. See its installed licenses.

The built-in adapter, receiver added in this repository are AGPL-3.0-only, as is One-KVM.

The native-only transfer-engine packaging is documented in [PATCHES.md — transfer-engine](PATCHES.md#transfer-engine).

The receiver-only qr-protocol feature boundary is documented in [PATCHES.md — qr-protocol](PATCHES.md#qr-protocol).
