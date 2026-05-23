# Rust bindings for V4L2

This is a vendored, One-KVM-specific subset of `v4l2r`.

It keeps only the pieces needed for video capture:

- generated Linux V4L2 bindings,
- safe low-level ioctl wrappers used by capture and device probing,
- memory handle helpers for `MMAP`, `USERPTR`, and `DMABUF`,
- core V4L2 types such as `Format`, `PixelFormat`, and `QueueType`.

The upstream crate also contains high-level device abstractions, stateful
decoder/encoder helpers, stateless codec controls, examples, and C FFI. Those
parts are intentionally removed here so this dependency stays scoped to capture.

## Build options

`cargo build` generates V4L2 bindings from `/usr/include/linux/videodev2.h` by
default. Set `V4L2R_VIDEODEV2_H_PATH` to a directory containing `videodev2.h` to
generate bindings from a different header.

For Android targets, the build script uses the Android NDK sysroot. Set one of
`ANDROID_NDK_HOME`, `ANDROID_NDK_ROOT`, `NDK_HOME`, `ANDROID_HOME`, or
`ANDROID_SDK_ROOT` if the NDK cannot be found automatically.
