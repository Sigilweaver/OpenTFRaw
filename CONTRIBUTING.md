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

## Vendor software and clean-room policy

If you are contributing to the Thermo `.raw` reader, please make sure new
format knowledge came from public datasets and your own analysis - **do
not** copy or paste vendor SDK headers, sources, decompiled code, or
proprietary specifications. See [ATTRIBUTION.md](ATTRIBUTION.md) and
[CORPUS.md](CORPUS.md).

**Never use vendor software.** This is a clean-room project. Do not run,
depend on, or validate against the vendor's own tools, or anything that
reads the format through the vendor SDK/DLLs - not in CI, not in tests, not
in local development. ProteoWizard `msconvert` counts as vendor software
because it reads the raw formats through the vendor libraries. Correctness
is argued only from open references: the PSI-MS mzML schema, published open
specifications, roundtrip and self-consistency invariants, and independent
open-source parsers used purely as format checkers. Comparing, benchmarking,
or tuning output against vendor results is not allowed and would compromise
the clean-room status of the project.

**Pull requests that were written or verified with the help of proprietary
vendor software will not be accepted**, regardless of code quality, since
accepting them would compromise the project's clean-room provenance. If
you've found a bug this way, or you'd simply rather not write the fix
yourself, please open an issue instead. Describe the symptom on the input
that triggers it - what's wrong, and on what file - without pasting vendor
tool output, vendor source, or values you learned by running vendor
software. We'll investigate and fix it from public references. Detailed
issue reports are genuinely useful and will be acted on.

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
