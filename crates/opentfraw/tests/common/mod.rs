//! Shared corpus-fixture lookup for integration tests.
//!
//! The canonical CI fixture is the small public LTQ FT file from PRIDE
//! PXD054004, cached at `corpus/<CORPUS_FIXTURE_NAME>` (repo-root-relative).
//! `.github/workflows/ci.yml` downloads it under this exact name and
//! `tests/conftest.py` caches it under the same name for pytest.
//!
//! Without a fixture, tests that need it skip. Set `REQUIRE_CORPUS=1` (CI
//! does) to turn a missing fixture into a hard failure instead.

use std::path::PathBuf;

/// File name of the shared CI corpus fixture. Keep in sync with
/// `PRIDE_RAW_NAME` in `.github/workflows/ci.yml` and `tests/conftest.py`.
pub const CORPUS_FIXTURE_NAME: &str = "PXD054004_LTQ_FT_20171113_Map_NS1_1to139_4deg_50uM_001.raw";

/// Locate the corpus fixture. Returns `None` (test should skip) when it is
/// absent, and panics instead when `REQUIRE_CORPUS=1` is set.
pub fn fixture() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let candidates = [
        // CI / repo-root corpus dir (gitignored; populated by ci.yml,
        // tests/conftest.py, or scripts/fetch_corpus.py).
        root.join("corpus").join(CORPUS_FIXTURE_NAME),
        // Local dev setups with a sibling SpecLance checkout's corpus/.
        root.join("../SpecLance/corpus/thermo")
            .join(CORPUS_FIXTURE_NAME),
    ];
    let found = candidates.iter().find(|p| p.exists()).cloned();
    if found.is_none() && require_corpus() {
        panic!(
            "REQUIRE_CORPUS=1 but no corpus fixture found; looked for {}",
            candidates
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    found
}

fn require_corpus() -> bool {
    matches!(
        std::env::var("REQUIRE_CORPUS").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}
