# ALSA compatibility patch

This directory contains `alsa` 0.11.0 from crates.io, licensed under MIT or Apache-2.0. The upstream crate archive SHA-256 is `812947049edcd670a82cd5c73c3661d2e58468577ba8489de58e1a73c04cbd5d`.

Upstream source: https://github.com/diwic/alsa-rs

In `src/pcm.rs`, `Status::get_htstamp`, `get_trigger_htstamp`, and `get_audio_htstamp` initialize `libc::timespec` with `timespec::default()` instead of a struct literal. Rust libc exposes private padding fields when targeting 32-bit musl with time64 enabled, so the original literals fail to compile. Default initialization also initializes those fields while retaining the existing timestamp behavior on other Linux targets.

The root Cargo patch pins this copy for both direct and transitive ALSA dependencies. Remove the patch when an upstream release includes this fix, and refresh `Cargo.lock` together with the dependency update.

Trailing whitespace in upstream `README.md`, `src/card.rs`, and `src/poll.rs` is normalized for repository whitespace checks.
