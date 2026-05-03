"""
Fetch one representative RAW file per instrument type from PRIDE.

Reads scripts/sources.json, which lists exactly which file to download for
each instrument (PRIDE accession + original filename).  For each entry not
yet recorded in the corpus manifest the script resolves the download URL via
the PRIDE REST API and downloads the file.

Usage:
    python scripts/fetch_corpus.py [--dry-run]
"""

from __future__ import annotations

import argparse
import json
import time
import urllib.request
from pathlib import Path

CORPUS_DIR = Path(__file__).parent.parent / "corpus"
MANIFEST = CORPUS_DIR / "manifest.json"
SOURCES = Path(__file__).parent / "sources.json"
USER_AGENT = "OpenTFRaw-CorpusFetcher/1.0"
PRIDE_API = "https://www.ebi.ac.uk/pride/ws/archive/v2"


def _req(url: str) -> urllib.request.Request:
    return urllib.request.Request(url, headers={"User-Agent": USER_AGENT})


def _project_pub_date(accession: str) -> str | None:
    """
    Return the publicationDate (YYYY-MM-DD) for a PRIDE project, or None.
    Used to construct FTP paths when the files/byProject API is unavailable.
    """
    try:
        url = f"{PRIDE_API}/projects/{accession}"
        with urllib.request.urlopen(_req(url), timeout=20) as r:
            data = json.loads(r.read())
        return data.get("publicationDate")  # e.g. "2022-07-11"
    except Exception:
        return None


def _ftp_url(accession: str, pride_filename: str, pub_date: str) -> str:
    """Construct the PRIDE FTP HTTPS URL for a file given the publication date."""
    yyyy, mm = pub_date[:4], pub_date[5:7]
    return (
        f"https://ftp.pride.ebi.ac.uk/pride/data/archive/{yyyy}/{mm}"
        f"/{accession}/{pride_filename}"
    )


def pride_download_url(accession: str, pride_filename: str) -> tuple[str, int] | None:
    """
    Return (https_url, size_bytes) for a specific file in a PRIDE project,
    or None if not found.

    Tries the PRIDE REST API first; falls back to constructing the FTP URL
    directly from the project publication date when the API returns nothing
    (the v2 files/byProject endpoint has been observed to return empty bodies).
    """
    page = 0
    page_size = 100
    api_returned_data = False
    while True:
        api_url = (
            f"{PRIDE_API}/files/byProject"
            f"?accession={accession}&pageSize={page_size}&page={page}"
        )
        try:
            with urllib.request.urlopen(_req(api_url), timeout=30) as r:
                raw = r.read()
            if not raw:
                break  # API returned empty body -- fall through to FTP fallback
            data = json.loads(raw)
        except Exception as e:
            print(f"  [ERROR] PRIDE API request failed: {e}", flush=True)
            break

        api_returned_data = True
        for entry in data.get("content", []):
            if entry.get("fileName", "").lower() == pride_filename.lower():
                size = entry.get("fileSizeBytes", 0)
                for loc in entry.get("publicFileLocations", []):
                    val: str = loc.get("value", "")
                    if val.startswith("ftp://"):
                        https_url = val.replace(
                            "ftp://ftp.pride.ebi.ac.uk",
                            "https://ftp.pride.ebi.ac.uk", 1,
                        )
                        return https_url, size
                    if val.startswith("https://"):
                        return val, size
                return None  # found file but no usable location

        page_info = data.get("page", {})
        total = page_info.get("totalElements", 0)
        fetched = (page + 1) * page_size
        if fetched >= total:
            break
        page += 1
        time.sleep(0.2)

    if api_returned_data:
        return None  # API worked but file not found in project

    # FTP fallback: resolve the publication date and construct the URL directly.
    print("  [INFO] files/byProject API returned nothing; using FTP fallback",
          flush=True)
    pub_date = _project_pub_date(accession)
    if not pub_date:
        print(f"  [ERROR] could not determine publication date for {accession}",
              flush=True)
        return None
    url = _ftp_url(accession, pride_filename, pub_date)
    # We do not know the byte size in advance; use 0 and let the download fill it.
    return url, 0


def load_manifest() -> dict:
    if MANIFEST.exists():
        with open(MANIFEST) as f:
            return json.load(f)
    return {}


def save_manifest(manifest: dict) -> None:
    MANIFEST.parent.mkdir(parents=True, exist_ok=True)
    with open(MANIFEST, "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")


def download(url: str, dest: Path) -> bool:
    dest.parent.mkdir(parents=True, exist_ok=True)
    tmp = dest.with_suffix(".part")
    try:
        with urllib.request.urlopen(_req(url), timeout=600) as r, \
             open(tmp, "wb") as f:
            while chunk := r.read(1 << 20):
                f.write(chunk)
        tmp.rename(dest)
        return True
    except Exception as e:
        print(f"  [ERROR] download failed: {e}", flush=True)
        if tmp.exists():
            tmp.unlink()
        return False


def label(inst: str) -> str:
    return inst.replace(" ", "_")


def manifest_key(instrument: str, mode: str | None) -> str:
    """Return the key used to store this entry in the corpus manifest."""
    return f"{instrument} ({mode})" if mode else instrument


def run(dry_run: bool) -> None:
    CORPUS_DIR.mkdir(parents=True, exist_ok=True)
    manifest = load_manifest()

    with open(SOURCES) as f:
        sources: list[dict] = json.load(f)

    for entry in sources:
        instrument: str = entry["instrument"]
        accession: str = entry["accession"]
        pride_filename: str = entry["pride_filename"]
        mode: str | None = entry.get("mode")
        key = manifest_key(instrument, mode)

        print(f"\n{'='*60}", flush=True)
        print(f"  {instrument}  ({accession})", flush=True)

        if key in manifest:
            print(
                f"  Already have: {manifest[key]['filename']}  -- skipping",
                flush=True,
            )
            continue

        print("  Resolving download URL via PRIDE API ...", flush=True)
        result = pride_download_url(accession, pride_filename)
        if result is None:
            print(f"  [WARN] {pride_filename} not found in {accession}", flush=True)
            continue

        url, size = result
        lbl = label(instrument)
        dest = CORPUS_DIR / f"{accession}_{lbl}_{pride_filename}"
        size_str = f"{size / 1e6:.1f} MB" if size else "size unknown"
        print(f"  {pride_filename}  ({size_str})", flush=True)

        if dry_run:
            print(f"  [DRY-RUN] -> {dest.name}", flush=True)
            continue

        ok = download(url, dest)
        if ok:
            actual = dest.stat().st_size
            print(f"  Done: {actual / 1e6:.1f} MB", flush=True)
            manifest[key] = {
                "accession": accession,
                "filename": dest.name,
                "size_bytes": actual,
            }
            save_manifest(manifest)

        time.sleep(1)

    print(f"\n{'='*60}", flush=True)
    files = sorted(CORPUS_DIR.glob("*.[Rr][Aa][Ww]"))
    print(f"Corpus now has {len(files)} file(s):", flush=True)
    for f in files:
        print(f"  {f.name}  ({f.stat().st_size / 1e6:.1f} MB)", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    run(dry_run=args.dry_run)
