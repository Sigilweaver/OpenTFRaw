# Security Policy

## Supported Versions

| Version | Supported |
| ------- | --------- |
| latest  | Yes       |
| older   | No        |

Only the latest published release receives security updates.

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Report privately via [GitHub Security Advisories](https://github.com/Sigilweaver/OpenTFRaw/security/advisories/new).

Include:

- A description of the vulnerability and its potential impact.
- Steps to reproduce or a proof of concept (a small `.raw` file is
  ideal; small synthetic byte sequences are even better).
- The crate version (Rust or Python wheel) and OS / toolchain.

Expect an initial acknowledgment within 7 days.

## Scope

In scope:

- **Parser correctness on malicious `.raw` input.** OpenTFRaw parses
  the Thermo Fisher `.raw` binary format. Panics, out-of-bounds
  reads, undefined behavior, infinite loops, or memory exhaustion
  triggered by a crafted file are in scope.
- **Memory safety**: the crate forbids `unsafe_code`. A demonstrated
  unsafe-code violation reachable from safe API is a security bug.
- **Path-traversal or arbitrary-file-write bugs** in any helper that
  derives output paths from input filenames.
- **Supply-chain integrity** of published artifacts on crates.io and
  PyPI.

Out of scope:

- Denial of service via legitimately large `.raw` files. Thermo
  acquisitions can be hundreds of GB by design.
- Inaccurate decoding of specific Thermo acquisition modes. Those
  are correctness bugs - file them as regular issues.
- Vulnerabilities in third-party crates with no demonstrated exploit
  path through OpenTFRaw.

## Disclosure

We follow coordinated disclosure. Reporters are credited in the
release notes unless they prefer to remain anonymous. We aim to ship
a fix within 30 days of confirming a high or critical issue.

## Note on reverse engineering

OpenTFRaw was developed by clean-room reverse engineering of public
artifacts (PRIDE deposits, published specifications, format
documentation in the public domain). It does not depend on any
Thermo SDK or binary blob, and contains no Thermo proprietary code.
Bug reports about parser accuracy or coverage are welcome but are
not security issues unless they involve one of the categories above.

## Stack context

OpenTFRaw is one of three vendor readers in the
[OpenProteo](https://github.com/Sigilweaver/OpenProteo) stack.
Sibling readers:
[OpenWRaw](https://github.com/Sigilweaver/OpenWRaw) (Waters),
[OpenTimsTDF](https://github.com/Sigilweaver/OpenTimsTDF) (Bruker).
Shared foundation:
[openproteo-core](https://github.com/Sigilweaver/OpenProteoCore).
