# Contributing to OpenTFRaw

Thanks for your interest in OpenTFRaw. This is a small, single-maintainer
project that ships [Apache-2.0](LICENSE) Rust (and Python where
applicable) tooling for the open mass-spec stack.

Crates / packages in this repo: opentfraw, opentfraw-py.

## Contributing code (pull requests)

PRs are welcome for changes of any size, including large or breaking ones -
there's no requirement to open an issue first. That said, for larger changes
you may want to open an issue before writing code, especially if you're
unsure whether it fits the project's direction: a large PR that conflicts
with the roadmap can still be rejected even if the code itself is solid, and
an issue is a cheap way to check alignment before investing the time.

For any PR:

- Scope it to one logical change.
- Run `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings`
  locally. CI will run them too.
- Run `cargo test --all` (and `pytest` if the change touches Python).
- Update [CHANGELOG.md](CHANGELOG.md) under `## [Unreleased]` with a
  short bullet describing the user-visible change.
- Prefer [Conventional Commits](https://www.conventionalcommits.org/)
  (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`).
- Code is ASCII only and `#![forbid(unsafe_code)]` unless the crate
  explicitly opts in (none of the public crates currently do).

### Exposing a decoded value

A value the Rust core decodes should reach Python and the
`openmassspec_core` adapter (and so mzML) through one path, not be wired
into each by hand:

- If `openmassspec_core::SpectrumRecord` has a field for it, derive it in
  `scan_metadata()` (`crates/opentfraw/src/mzml.rs`), map it in
  `to_msc_record()`, and add the key to Python `scan()`.
- Otherwise, register it in `EXTRA_FIELDS` (`crates/opentfraw/src/extra.rs`)
  under an `opentfraw.*` key. It then reaches `SpectrumRecord::extra`, mzML
  `<userParam>`s and the Python `scan()["extra"]` dict with no further code.

A unit test in `extra.rs` fails when a public `ScanParams` or
`StatusLogEntry` accessor reaches neither.

## Vendor software and clean-room policy

If you are contributing to the Thermo `.raw` reader, please make sure new
format knowledge came from public datasets and your own analysis - **do
not** copy or paste vendor SDK headers, sources, decompiled code, or
proprietary specifications. See [ATTRIBUTION.md](ATTRIBUTION.md) and
[CORPUS.md](CORPUS.md).

This section follows the Sigilweaver [format provenance
policy](https://github.com/Sigilweaver/ops/blob/main/PROVENANCE.md), shared
by every reader in the suite. Where the two differ, the policy wins.

**No vendor software in the project.** Do not depend on the vendor's own
tools, or on anything that reads the format through the vendor SDK/DLLs,
and do not run them in CI or tests. ProteoWizard `msconvert` counts as
vendor software because it reads the raw formats through the vendor
libraries, so it cannot be a dependency or a CI step either. Do not bypass
encryption, license checks, or any other technical protection to read data,
and do not contribute format knowledge from anyone working from
vendor-internal information.

Correctness is argued from open references: public data files, the PSI-MS
mzML schema, published open specifications, roundtrip and self-consistency
invariants, and independent open-source parsers used purely as format
checkers. Every layout must be explainable from those. If you can only
explain a field by having watched what the vendor's software shows for it,
don't write that down - keep digging in the bytes instead, or flag it as
unresolved.

**Cross-checking against vendor software is allowed, with disclosure.** If
you are licensed to run the vendor's software, you may compare values this
project decodes against its output. A comparison can confirm a decoder; it
cannot be the only basis for one. Say so in the pull request: which tool,
which files, and what you compared. Do not paste vendor-tool output into
the repository. If a value's only basis is a vendor comparison (for example
a derived quantity whose formula was matched to vendor output rather than
taken from a public source), document that next to the value. Cross-checks
are recorded in [ATTRIBUTION.md](ATTRIBUTION.md).

If you'd rather not write the fix yourself, or you've found a bug by
comparing against vendor software and don't want to send a pull request,
please open an issue instead. Describe the symptom on the input that
triggers it - what's wrong, and on what file - without pasting vendor tool
output or vendor source. We'll investigate and fix it from public
references. Detailed issue reports are genuinely useful and will be acted
on.

## Security

Please report security vulnerabilities privately via GitHub Security
Advisories - see [SECURITY.md](SECURITY.md). Do not open public issues
for vulnerabilities.

## DCO

By submitting a contribution you certify that you have the right
to submit the work under the project license (Apache-2.0) and
agree to the
[Developer Certificate of Origin](https://developercertificate.org/).

## License

By submitting a PR you agree that your contribution is licensed under
the Apache License 2.0, the same terms as the rest of the project.
