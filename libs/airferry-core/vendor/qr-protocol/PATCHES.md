Source: UR-SillyB/AirFerry revision 8a72ab86dee9d8f19b74bfec56c270101c5980ba, MIT.

Use an explicit native manifest instead of inherited workspace metadata. Make fast_qr
optional and expose qr_render only with feature `generate`. The development CLI
selects that feature and applies the pinned fast_qr patch; the application receiver
does not build a QR generator. Frame layout, CRC, compression and session code are
unchanged. Default builds retain all three supported decompression formats.
