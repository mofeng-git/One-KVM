# raptorq
![CI](https://github.com/cberner/raptorq/actions/workflows/ci.yml/badge.svg)
[![Crates](https://img.shields.io/crates/v/raptorq.svg)](https://crates.io/crates/raptorq)
[![Documentation](https://docs.rs/raptorq/badge.svg)](https://docs.rs/raptorq)
[![PyPI](https://img.shields.io/pypi/v/raptorq.svg)](https://pypi.org/project/raptorq/)
[![License](https://img.shields.io/crates/l/raptorq)](https://crates.io/crates/raptorq)
[![dependency status](https://deps.rs/repo/github/cberner/raptorq/status.svg)](https://deps.rs/repo/github/cberner/raptorq)

### Overview

Rust implementation of RaptorQ (RFC6330)

Recovery properties:
Reconstruction probability after receiving K + h packets = 1 - 1/256^(h + 1). Where K is the number of packets in the
original message, and h is the number of additional packets received.
See "RaptorQ Technical Overview" by Qualcomm

### Examples
See the `examples/` directory for usage.

### Benchmarks

The following were run on a Ryzen 9 9950X3D @ 4.30GHz
```
Symbol size: 1280 bytes (without pre-built plan)
symbol count = 10, encoded 127 MB in 0.046secs, throughput: 22259.3Mbit/s
symbol count = 100, encoded 127 MB in 0.046secs, throughput: 22248.6Mbit/s
symbol count = 250, encoded 127 MB in 0.050secs, throughput: 20459.0Mbit/s
symbol count = 500, encoded 127 MB in 0.038secs, throughput: 26855.5Mbit/s
symbol count = 1000, encoded 126 MB in 0.054secs, throughput: 18807.9Mbit/s
symbol count = 2000, encoded 126 MB in 0.058secs, throughput: 17510.8Mbit/s
symbol count = 5000, encoded 122 MB in 0.067secs, throughput: 14575.6Mbit/s
symbol count = 10000, encoded 122 MB in 0.087secs, throughput: 11224.9Mbit/s
symbol count = 20000, encoded 122 MB in 0.136secs, throughput: 7180.6Mbit/s
symbol count = 50000, encoded 122 MB in 0.267secs, throughput: 3657.5Mbit/s

Symbol size: 1280 bytes (with pre-built plan)
symbol count = 10, encoded 127 MB in 0.063secs, throughput: 16252.8Mbit/s
symbol count = 100, encoded 127 MB in 0.034secs, throughput: 30101.1Mbit/s
symbol count = 250, encoded 127 MB in 0.051secs, throughput: 20057.8Mbit/s
symbol count = 500, encoded 127 MB in 0.038secs, throughput: 26855.5Mbit/s
symbol count = 1000, encoded 126 MB in 0.039secs, throughput: 26041.7Mbit/s
symbol count = 2000, encoded 126 MB in 0.057secs, throughput: 17818.0Mbit/s
symbol count = 5000, encoded 122 MB in 0.061secs, throughput: 16009.2Mbit/s
symbol count = 10000, encoded 122 MB in 0.061secs, throughput: 16009.2Mbit/s
symbol count = 20000, encoded 122 MB in 0.111secs, throughput: 8797.9Mbit/s
symbol count = 50000, encoded 122 MB in 0.180secs, throughput: 5425.3Mbit/s

Symbol size: 1280 bytes
symbol count = 10, decoded 127 MB in 0.174secs using 0.0% overhead, throughput: 5884.6Mbit/s
symbol count = 100, decoded 127 MB in 0.146secs using 0.0% overhead, throughput: 7009.8Mbit/s
symbol count = 250, decoded 127 MB in 0.146secs using 0.0% overhead, throughput: 7006.5Mbit/s
symbol count = 500, decoded 127 MB in 0.143secs using 0.0% overhead, throughput: 7136.4Mbit/s
symbol count = 1000, decoded 126 MB in 0.144secs using 0.0% overhead, throughput: 7053.0Mbit/s
symbol count = 2000, decoded 126 MB in 0.140secs using 0.0% overhead, throughput: 7254.5Mbit/s
symbol count = 5000, decoded 122 MB in 0.159secs using 0.0% overhead, throughput: 6141.9Mbit/s
symbol count = 10000, decoded 122 MB in 0.193secs using 0.0% overhead, throughput: 5059.9Mbit/s
symbol count = 20000, decoded 122 MB in 0.268secs using 0.0% overhead, throughput: 3643.9Mbit/s
symbol count = 50000, decoded 122 MB in 0.391secs using 0.0% overhead, throughput: 2497.6Mbit/s

symbol count = 10, decoded 127 MB in 0.187secs using 5.0% overhead, throughput: 5475.5Mbit/s
symbol count = 100, decoded 127 MB in 0.146secs using 5.0% overhead, throughput: 7009.8Mbit/s
symbol count = 250, decoded 127 MB in 0.140secs using 5.0% overhead, throughput: 7306.8Mbit/s
symbol count = 500, decoded 127 MB in 0.117secs using 5.0% overhead, throughput: 8722.3Mbit/s
symbol count = 1000, decoded 126 MB in 0.120secs using 5.0% overhead, throughput: 8463.5Mbit/s
symbol count = 2000, decoded 126 MB in 0.126secs using 5.0% overhead, throughput: 8060.5Mbit/s
symbol count = 5000, decoded 122 MB in 0.134secs using 5.0% overhead, throughput: 7287.8Mbit/s
symbol count = 10000, decoded 122 MB in 0.172secs using 5.0% overhead, throughput: 5677.7Mbit/s
symbol count = 20000, decoded 122 MB in 0.260secs using 5.0% overhead, throughput: 3756.0Mbit/s
symbol count = 50000, decoded 122 MB in 0.415secs using 5.0% overhead, throughput: 2353.2Mbit/s
```

The following were run on a Raspberry Pi 3 B+ (Cortex-A53 @ 1.4GHz), as of raptorq version 2.0.0

```
Symbol size: 1280 bytes (without pre-built plan)
symbol count = 10, encoded 127 MB in 5.078secs, throughput: 201.6Mbit/s
symbol count = 100, encoded 127 MB in 3.966secs, throughput: 258.1Mbit/s
symbol count = 250, encoded 127 MB in 4.293secs, throughput: 238.3Mbit/s
symbol count = 500, encoded 127 MB in 4.451secs, throughput: 229.3Mbit/s
symbol count = 1000, encoded 126 MB in 4.606secs, throughput: 220.5Mbit/s
symbol count = 2000, encoded 126 MB in 5.127secs, throughput: 198.1Mbit/s
symbol count = 5000, encoded 122 MB in 5.615secs, throughput: 173.9Mbit/s
symbol count = 10000, encoded 122 MB in 6.321secs, throughput: 154.5Mbit/s
symbol count = 20000, encoded 122 MB in 7.450secs, throughput: 131.1Mbit/s
symbol count = 50000, encoded 122 MB in 9.407secs, throughput: 103.8Mbit/s

Symbol size: 1280 bytes (with pre-built plan)
symbol count = 10, encoded 127 MB in 3.438secs, throughput: 297.8Mbit/s
symbol count = 100, encoded 127 MB in 2.476secs, throughput: 413.3Mbit/s
symbol count = 250, encoded 127 MB in 2.908secs, throughput: 351.8Mbit/s
symbol count = 500, encoded 127 MB in 3.085secs, throughput: 330.8Mbit/s
symbol count = 1000, encoded 126 MB in 3.284secs, throughput: 309.3Mbit/s
symbol count = 2000, encoded 126 MB in 3.700secs, throughput: 274.5Mbit/s
symbol count = 5000, encoded 122 MB in 4.045secs, throughput: 241.4Mbit/s
symbol count = 10000, encoded 122 MB in 4.451secs, throughput: 219.4Mbit/s
symbol count = 20000, encoded 122 MB in 4.948secs, throughput: 197.4Mbit/s
symbol count = 50000, encoded 122 MB in 6.078secs, throughput: 160.7Mbit/s

Symbol size: 1280 bytes
symbol count = 10, decoded 127 MB in 6.561secs using 0.0% overhead, throughput: 156.1Mbit/s
symbol count = 100, decoded 127 MB in 4.936secs using 0.0% overhead, throughput: 207.3Mbit/s
symbol count = 250, decoded 127 MB in 5.206secs using 0.0% overhead, throughput: 196.5Mbit/s
symbol count = 500, decoded 127 MB in 5.298secs using 0.0% overhead, throughput: 192.6Mbit/s
symbol count = 1000, decoded 126 MB in 5.565secs using 0.0% overhead, throughput: 182.5Mbit/s
symbol count = 2000, decoded 126 MB in 6.309secs using 0.0% overhead, throughput: 161.0Mbit/s
symbol count = 5000, decoded 122 MB in 6.805secs using 0.0% overhead, throughput: 143.5Mbit/s
symbol count = 10000, decoded 122 MB in 7.517secs using 0.0% overhead, throughput: 129.9Mbit/s
symbol count = 20000, decoded 122 MB in 8.875secs using 0.0% overhead, throughput: 110.0Mbit/s
symbol count = 50000, decoded 122 MB in 11.253secs using 0.0% overhead, throughput: 86.8Mbit/s

symbol count = 10, decoded 127 MB in 6.157secs using 5.0% overhead, throughput: 166.3Mbit/s
symbol count = 100, decoded 127 MB in 4.842secs using 5.0% overhead, throughput: 211.4Mbit/s
symbol count = 250, decoded 127 MB in 5.213secs using 5.0% overhead, throughput: 196.2Mbit/s
symbol count = 500, decoded 127 MB in 5.328secs using 5.0% overhead, throughput: 191.5Mbit/s
symbol count = 1000, decoded 126 MB in 5.630secs using 5.0% overhead, throughput: 180.4Mbit/s
symbol count = 2000, decoded 126 MB in 6.364secs using 5.0% overhead, throughput: 159.6Mbit/s
symbol count = 5000, decoded 122 MB in 7.035secs using 5.0% overhead, throughput: 138.8Mbit/s
symbol count = 10000, decoded 122 MB in 8.165secs using 5.0% overhead, throughput: 119.6Mbit/s
symbol count = 20000, decoded 122 MB in 9.929secs using 5.0% overhead, throughput: 98.4Mbit/s
symbol count = 50000, decoded 122 MB in 14.399secs using 5.0% overhead, throughput: 67.8Mbit/s
```

### Public API
Note that the additional classes exported by the `benchmarking` feature flag are not considered part of this
crate's public API. Breaking changes to those classes may occur without warning. The flag is only provided
so that internal classes can be used in this crate's benchmarks.

## Python bindings

The Python bindings are generated using [pyo3](https://github.com/PyO3/pyo3). 

Some operating systems require additional packages to be installed.
```
$ sudo apt install python3-dev cargo
```

[maturin](https://github.com/PyO3/maturin) is recommended for building the Python bindings in this crate.
```
$ pip install maturin
$ maturin build --features python
```

Alternatively, refer to the [Building and Distribution section](https://pyo3.rs/v0.8.5/building_and_distribution.html) in the [pyo3 user guide](https://pyo3.rs/v0.8.5/).
Note, you must pass the `--cargo-extra-args="--features python"` argument to Maturin when building this crate
to enable the Python binding features.

## License

Licensed under

 * Apache License, Version 2.0 ([LICENSE](LICENSE) or http://www.apache.org/licenses/LICENSE-2.0)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you shall be licensed as above, without any
additional terms or conditions.
