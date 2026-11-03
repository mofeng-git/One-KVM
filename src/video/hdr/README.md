# Native HDR capture (10-bit PQ -> SDR) on RKMPP

RK3588 HDMI RX can deliver 10-bit packed YCbCr (`NV20` 4:2:2, `NV15` 4:2:0) when
the receiver driver exposes it (`snps_hdmirx` with the 10-bit/HDR patch and
`prefer_10bit=Y`). RKMPP encodes 8-bit only, so the stream is tone-mapped to
SDR first. This path does that inside One-KVM:

```
V4L2 NV20/NV15 dma-buf --(Vulkan compute)--> NV12 dma-buf --(MPP)--> H.264/H.265
```

No CPU pixel copies, and no per-frame V4L2 format queries. The conversion of
frame N overlaps the encode of frame N-1 (one frame of added latency).

## Scope

- 3840x2160, `NV20` or `NV15`, BT.2020 + SMPTE ST 2084 (PQ) as reported by
  `VIDIOC_G_FMT`. Limited or full range follows the reported quantization.
  Anything else is refused; there is no CPU fallback for packed 10-bit input.
- Output is limited-range BT.709 NV12. SDR white 203 nits, 1000 nit highlight
  roll-off, static dither. These are rendering choices, not mastering metadata.
- The stream is capped at 30 fps. Faster sources (4K60) are decimated before
  the GPU using the driver's capture timestamps.
- A change of colorimetry (HDR on/off) or format re-opens the capture.

## Runtime requirements

| Need | Why |
| --- | --- |
| `libvulkan.so.1` and a Vulkan driver exposing `VK_KHR_external_memory_fd`, `VK_EXT_external_memory_dma_buf`, `VK_EXT_queue_family_foreign` | the shader runs on the GPU; the loader is opened at run time, so images without it still start |
| the GPU render node (`/dev/dri/renderD*`) in the container | Vulkan opens it |
| Mali-G610 (RK3588): Mesa PanVK >= 25.0.2 | first release with conformant G610 support |

Debian 12's own Mesa (22.3) cannot drive the G610, so the arm64 runtime images
(`Dockerfile.runtime` and `Dockerfile.runtime-full`) install `libvulkan1` and
Mesa 25.0.7 from bookworm-backports. glibc stays 2.36; backports also supplies
newer `libdrm2`, `libelf1` and `libwayland-client0`. Only the Mali driver is
kept: the other Mesa Vulkan drivers and the LLVM, ICU and z3 libraries they
alone need are removed in the same layer, so the image grows by about 20 MB
(4 MB compressed) instead of 311 MB. Without a render node it is inert.

The container then needs only the Mali render node, e.g. `--device /dev/dri/renderD128`
(in addition to `/dev/mpp_service` and the Rockchip `card0`). It does not need
`/dev/dma_heap`, extra capabilities, or a writable root; with the usual hardening
(`--read-only --cap-drop ALL --security-opt no-new-privileges`) the 4K PQ
conversion is byte-identical to the reference (21.9 ms per frame) and the
GPU + MPP encode path ran at 29.7 fps from a 60 Hz source.

Output buffers are allocated from `/dev/dma_heap/system` when it is available,
otherwise exported from Vulkan. Both were verified on RK3588.

| Variable | Default | Meaning |
| --- | --- | --- |
| `ONE_KVM_HDR_DMA_HEAP` | `/dev/dma_heap/system` | heap node for NV12 outputs; `none` forces Vulkan export |
| `ONE_KVM_HDR_PIPELINE` | on | `0` encodes each frame right after converting it |

`ONE_KVM_RKMPP_DMABUF=0` disables the DMA worker, and with it HDR input.

## Validation (RK3588, Mesa 26.2.5 PanVK)

- `tonemap.comp`/`tonemap.spv` are unchanged from the previously validated
  host worker (SPIR-V rebuilt byte-identically). The Rust stage, run on a
  captured 4K NV20 frame, produced output identical to that worker's: 0 of
  12,441,600 bytes differ, with both output allocators.
- GPU conversion: 17.1 ms per 4K frame.
- Replay of a captured frame through GPU + MPP HEVC at 8 Mbps:
  60 Hz source -> 29.8 fps out; unpaced ceiling 57.6 fps overlapped versus
  29.75 fps serial. Decodes as HEVC Main yuv420p, 48.4 dB PSNR against the
  reference first frame.

The shader is regenerated with `glslangValidator -V tonemap.comp -o tonemap.spv`.
