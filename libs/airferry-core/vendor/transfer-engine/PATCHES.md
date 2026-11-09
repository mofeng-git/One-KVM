Source: UR-SillyB/AirFerry, revision 8a72ab86dee9d8f19b74bfec56c270101c5980ba, MIT.

Native-only packaging: retain the protocol/session/sender/receiver/assembler sources unchanged.
Replace the workspace-inherited manifest with explicit native dependencies and emit an rlib.
Omit the optional WASM/JNI/C-ABI binding modules and their dependencies. Their legacy exact
wasm-bindgen/js-sys version pins conflict with One-KVM's HTTP stack during Cargo resolution,
even for native builds. The wire protocol and recovery algorithms are unchanged.

Receiver/development split: expose SenderSession and SenderConfig only with the
`sender` feature (also available to this crate's own tests). The core regression
suite enables it as a dev-dependency; the development CLI opts in explicitly.
Remove the unused ingest_status packing module after deleting its binding callers,
and remove the unused direct crc32fast dependency. CRC remains in qr-protocol.
Protocol parsing, descriptor validation and recovery algorithms are unchanged.
