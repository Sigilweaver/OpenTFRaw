---
sidebar_position: 2
---

# Install

OpenTFRaw ships as a Rust crate and a Python wheel.

## Rust

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
opentfraw = "0.1"
```

Or from the command line:

```sh
cargo add opentfraw
```

OpenTFRaw needs Rust 1.75 or newer. There are no native or system
dependencies.

## Python

Install the wheel from PyPI:

```sh
pip install opentfraw
```

Pre-built wheels target Python 3.8+ via the stable ABI (`abi3`).

From source (requires a Rust toolchain and `maturin`):

```sh
git clone https://github.com/Sigilweaver/OpenTFRaw
cd OpenTFRaw/python
maturin develop --release
```

## Verifying the install

Rust:

```sh
cargo test
```

Python:

```python
import opentfraw
print(opentfraw.__version__)
```

## Optional: corpus fetcher

The validation corpus is not redistributed. It is pulled on demand from
the [PRIDE Archive](https://www.ebi.ac.uk/pride/):

```sh
python3 scripts/fetch_corpus.py
```

See [`CORPUS.md`](https://github.com/Sigilweaver/OpenTFRaw/blob/main/CORPUS.md)
for the file list and provenance.
