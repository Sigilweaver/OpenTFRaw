"""
OpenTFRaw corpus fetcher (Thermo .raw).

Thin wrapper around the shared OpenProteo stack fetcher at
``OpenProteo/scripts/fetch_corpus.py``. Defaults preserve the original
behavior of this script: read ``scripts/sources.json``, write into
``corpus/`` next to it, manifest at ``corpus/manifest.json``.

The shared script must be available at one of:

  - ``$OPENPROTEO_FETCHER`` (env override)
  - sibling checkout ``../OpenProteo/scripts/fetch_corpus.py``
  - ``$OPENPROTEO_DIR/scripts/fetch_corpus.py``

Pass extra args through to the shared script::

    python scripts/fetch_corpus.py --dry-run
    python scripts/fetch_corpus.py --list-files PXD012345
"""

from __future__ import annotations

import os
import runpy
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent
SOURCES = HERE / "sources.json"
CORPUS_DIR = REPO / "corpus"
EXT_PATTERN = r"\.[Rr][Aa][Ww]$"


def _locate_shared() -> Path:
    env = os.environ.get("OPENPROTEO_FETCHER")
    if env:
        p = Path(env).expanduser().resolve()
        if p.is_file():
            return p
    op_dir = os.environ.get("OPENPROTEO_DIR")
    if op_dir:
        p = Path(op_dir).expanduser().resolve() / "scripts" / "fetch_corpus.py"
        if p.is_file():
            return p
    sibling = (REPO.parent / "OpenProteo" / "scripts" / "fetch_corpus.py").resolve()
    if sibling.is_file():
        return sibling
    raise SystemExit(
        "[ERROR] could not locate OpenProteo/scripts/fetch_corpus.py.\n"
        "  Set $OPENPROTEO_FETCHER to the file path, or\n"
        "  set $OPENPROTEO_DIR to the OpenProteo checkout, or\n"
        "  place OpenProteo as a sibling of this repo."
    )


def main() -> int:
    shared = _locate_shared()
    extra = sys.argv[1:]

    if "--list-files" in extra:
        argv = ["fetch_corpus.py", *extra]
        if "--ext-pattern" not in extra:
            argv += ["--ext-pattern", EXT_PATTERN]
    else:
        argv = [
            "fetch_corpus.py",
            "--sources", str(SOURCES),
            "--corpus-dir", str(CORPUS_DIR),
            "--ext-pattern", EXT_PATTERN,
            *extra,
        ]

    sys.argv = argv
    runpy.run_path(str(shared), run_name="__main__")
    return 0


if __name__ == "__main__":
    sys.exit(main())
