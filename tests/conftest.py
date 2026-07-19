"""Shared fixtures for the Python-bindings pytest suite.

Downloads (and caches) the same small public Thermo RAW file that
ci.yml's ``validate-mzml`` job already uses for mzML structural
validation, so Rust and Python CI share one canonical test fixture
instead of each maintaining their own.

The URL/filename can be overridden via the ``PRIDE_RAW_URL`` /
``PRIDE_RAW_NAME`` environment variables (the same names ci.yml sets),
but default to the values baked into ci.yml so this also works for a
plain local `pytest` run with no environment set up.
"""

from __future__ import annotations

import os
import urllib.request
from pathlib import Path

import pytest

# Small LTQ FT file (~15 MB) from PRIDE PXD054004. Keep this in sync with
# PRIDE_RAW_URL / PRIDE_RAW_NAME in .github/workflows/ci.yml.
DEFAULT_PRIDE_RAW_URL = (
    "https://ftp.pride.ebi.ac.uk/pride/data/archive/2025/05/PXD054004/"
    "20171113_Map_NS1_1to139_4deg_50uM_001.raw"
)
DEFAULT_PRIDE_RAW_NAME = "test.raw"

PRIDE_RAW_URL = os.environ.get("PRIDE_RAW_URL", DEFAULT_PRIDE_RAW_URL)
PRIDE_RAW_NAME = os.environ.get("PRIDE_RAW_NAME", DEFAULT_PRIDE_RAW_NAME)

# corpus/ is already gitignored repo-wide as the canonical out-of-tree
# fixture location (see CORPUS.md), so cache the downloaded fixture there
# too rather than introducing a second scratch directory.
REPO_ROOT = Path(__file__).resolve().parent.parent
CACHE_DIR = REPO_ROOT / "corpus"


@pytest.fixture(scope="session")
def raw_file_path() -> Path:
    """Path to a small real Thermo RAW file, downloaded once per session
    and cached under corpus/ so repeated local runs don't re-download."""
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    dest = CACHE_DIR / PRIDE_RAW_NAME
    if not dest.exists() or dest.stat().st_size == 0:
        req = urllib.request.Request(
            PRIDE_RAW_URL, headers={"User-Agent": "OpenTFRaw-CI/1.0"}
        )
        tmp = dest.with_suffix(dest.suffix + ".part")
        with urllib.request.urlopen(req, timeout=180) as r, open(tmp, "wb") as f:
            while chunk := r.read(1 << 20):
                f.write(chunk)
        tmp.rename(dest)
    return dest


@pytest.fixture(scope="session")
def raw_file(raw_file_path: Path):
    """A opened opentfraw.RawFile for the shared test fixture."""
    import opentfraw

    return opentfraw.RawFile(str(raw_file_path))
