Pinned native sources used without downloads during build:

- zxing/: ZXing-C++ core, revision 8dd1cf5c4fd6fb6211bb96713db926ac6f2cf825, Apache-2.0.
  Source files are unchanged. The parent CMake project builds QR readers only; writers,
  C API and other barcode formats are disabled. See zxing/LICENSE.
- airferry/: AirFerry native ZXing adapter, revision 8a72ab86dee9d8f19b74bfec56c270101c5980ba,
  MIT. Unused single-code and JNI/C-ABI packing entry points are removed; multi-code
  decoding is unchanged. See airferry/LICENSE.

jpeg_qr.cpp in the parent directory is the One-KVM adapter and uses the repository license.
