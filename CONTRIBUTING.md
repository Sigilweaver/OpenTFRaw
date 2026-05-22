# Contributing to OpenTFRaw

Thanks for your interest in OpenTFRaw. This is a small, single-maintainer
project that ships [Apache-2.0](LICENSE) Rust (and Python where
applicable) tooling for the open mass-spec stack.

Crates / packages in this repo: opentfraw, opentfraw-py.

## Before you open a PR

- Open an issue first if the change is non-trivial (new API surface,
  format change, vendor coverage, dependency bump beyond a patch). For
  small fixes - typos, docs, minor bug fixes, additional tests -
  go straight to a PR.
- Run `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` locally.
  CI will run them too.
- Run `cargo test --all` (and `pytest` if the change touches Python).
- Update [CHANGELOG.md](CHANGELOG.md) under `## [Unreleased]` with a
  short bullet describing the user-visible change.
- Keep commits small and prefer [Conventional Commits](https://www.conventionalcommits.org/)
  (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`).
- Code is ASCII only and `#![forbid(unsafe_code)]` unless the crate
  explicitly opts in (none of the public crates currently do).

## Reverse-engineered formats

If you are contributing to a binary-format reader (Thermo `.raw`,
Bruker `.d`, Waters `.raw`), please make sure new format knowledge
came from public datasets and your own analysis - **do not** copy or
paste vendor SDK headers, sources, decompiled code, or proprietary
specifications. See the repo's `ATTRIBUTION.md` and `CORPUS.md`
where present.

## Security

Please report security vulnerabilities privately via GitHub Security
Advisories - see [SECURITY.md](SECURITY.md). Do not open public issues
for vulnerabilities.

## License

By submitting a PR you agree that your contribution is licensed under
the Apache License 2.0, the same terms as the rest of the project.
