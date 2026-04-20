"""
Fetch one representative RAW file per instrument type from PRIDE FTP.

Reads known_projects.json for pub_dates, then walks each instrument's
eligible projects (sorted by raw_file_count ascending — smaller projects
tend to have shorter runs and smaller files) until a file under the cap
is found.

Usage:
    python scripts/fetch_corpus.py [--dry-run] [--max-mb N]
"""

from __future__ import annotations

import argparse
import json
import re
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

CORPUS_DIR = Path(__file__).parent.parent / "samples" / "corpus"
MANIFEST = CORPUS_DIR / "manifest.json"
KNOWN_PROJECTS = Path(__file__).parent.parent.parent / "TFRaw-Sources" / "data" / "known_projects.json"
USER_AGENT = "TFRaw-CorpusFetcher/1.0"
PRIDE_FTP = "https://ftp.pride.ebi.ac.uk/pride/data/archive"
DEFAULT_MAX_MB = 400

# ── Target instrument classes ───────────────────────────────────────────────
# Instrument name as it appears in known_projects.json["instruments"]
# We scan ALL eligible projects for each instrument (smallest first) until
# we find a file under the cap.
TARGETS: list[str] = [
    # — Ion traps (LCQ/LTQ family) —
    "LCQ Classic",
    "LTQ",
    "LTQ XL",
    "LTQ Velos",
    "LTQ FT",
    # — LTQ Orbitrap hybrids —
    "LTQ Orbitrap",
    "LTQ Orbitrap XL",
    "LTQ Orbitrap XL ETD",
    "LTQ Orbitrap Velos",
    "LTQ Orbitrap Velos Pro",
    "LTQ Orbitrap Elite",
    # — Q-Orbitrap —
    "Q Exactive",
    "Q Exactive Plus",
    "Q Exactive HF",
    "Q Exactive HF-X",
    "Q Exactive UHMR",
    # — Tribrid Orbitrap —
    "Orbitrap Fusion",
    "Orbitrap Fusion Lumos",
    "Orbitrap Eclipse",
    "Orbitrap Ascend",
    # — Single-stage Orbitrap (Exploris / Astral) —
    "Orbitrap Exploris 120",
    "Orbitrap Exploris 240",
    "Orbitrap Exploris 480",
    "Orbitrap Astral",
    # — Triple quads —
    "TSQ Vantage",
    "TSQ Quantiva",
    "TSQ Altis",
]


def _req(url: str) -> urllib.request.Request:
    return urllib.request.Request(url, headers={"User-Agent": USER_AGENT})


def ftp_list_raw(accession: str, pub_date: str) -> list[tuple[int, str]]:
    """
    List (size_bytes, filename) for .raw files in the project's FTP dir.
    Returns [] on failure.
    """
    if not pub_date or len(pub_date) < 7:
        return []
    year, month = pub_date[:4], pub_date[5:7]
    base = f"{PRIDE_FTP}/{year}/{month}/{accession}"
    try:
        with urllib.request.urlopen(_req(base + "/"), timeout=20) as r:
            html = r.read().decode("utf-8", errors="replace")
    except Exception:
        return []

    # Find filenames
    links = re.findall(r'href="([^"]+\.raw)"', html, re.IGNORECASE)
    filenames = [Path(lnk).name for lnk in links]
    if not filenames:
        return []

    # Get sizes (HEAD requests)
    results: list[tuple[int, str]] = []
    for fname in filenames:
        url = f"{base}/{urllib.parse.quote(fname, safe='')}"
        try:
            with urllib.request.urlopen(_req(url), timeout=10) as r:
                # Some servers return size in Content-Length only for HEAD
                # but urlopen for GET also works — read 0 bytes
                cl = r.headers.get("Content-Length")
                size = int(cl) if cl else 0
                r.close()
        except Exception:
            # Try HEAD
            try:
                req_h = urllib.request.Request(url, method="HEAD",
                                               headers={"User-Agent": USER_AGENT})
                with urllib.request.urlopen(req_h, timeout=10) as r:
                    cl = r.headers.get("Content-Length")
                    size = int(cl) if cl else 0
            except Exception:
                size = 0
        results.append((size, fname))
        time.sleep(0.15)
    return results


def load_manifest() -> dict:
    if MANIFEST.exists():
        with open(MANIFEST) as f:
            return json.load(f)
    return {}


def save_manifest(manifest: dict) -> None:
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
        print(f"  [ERROR] download failed: {e}")
        if tmp.exists():
            tmp.unlink()
        return False


def label(inst: str) -> str:
    return inst.replace(" ", "_")


def run(max_mb: int, dry_run: bool) -> None:
    CORPUS_DIR.mkdir(parents=True, exist_ok=True)
    max_bytes = max_mb * 1_000_000
    manifest = load_manifest()

    if not KNOWN_PROJECTS.exists():
        print(f"ERROR: {KNOWN_PROJECTS} not found", flush=True)
        return

    with open(KNOWN_PROJECTS) as f:
        projects: dict = json.load(f)

    for instrument in TARGETS:
        lbl = label(instrument)
        print(f"\n{'='*60}", flush=True)
        print(f"  {instrument}", flush=True)

        # Skip if this instrument is recorded in the manifest
        if instrument in manifest:
            print(f"  Already have: {manifest[instrument]['filename']}  — skipping", flush=True)
            continue

        # Gather candidate projects (prefer fewer files = shorter runs = smaller files)
        candidates = [
            (p.get("raw_file_count", 0), pid, p.get("publication_date", ""))
            for pid, p in projects.items()
            if p.get("eligible") and instrument in p.get("instruments", [])
        ]
        candidates.sort()  # sort by raw_file_count ascending
        print(f"  {len(candidates)} eligible projects to scan", flush=True)

        found = None
        for raw_count, accession, pub_date in candidates:
            print(f"  Scanning {accession} ({raw_count} files, {pub_date[:7]}) …", flush=True)
            files = ftp_list_raw(accession, pub_date)
            if not files:
                print(f"    FTP listing failed or empty", flush=True)
                continue

            under_cap = [(sz, fn) for sz, fn in files if sz <= max_bytes]
            if not under_cap:
                smallest = min(files, key=lambda x: x[0])
                print(f"    {len(files)} .raw files, smallest={smallest[1]} "
                      f"({smallest[0]/1e6:.0f} MB) — all exceed cap", flush=True)
                continue

            under_cap.sort()
            sz, fname = under_cap[0]
            year, month = pub_date[:4], pub_date[5:7]
            url = (f"{PRIDE_FTP}/{year}/{month}/{accession}/"
                   f"{urllib.parse.quote(fname, safe='')}")
            print(f"    Found: {fname} ({sz/1e6:.1f} MB)", flush=True)
            found = (accession, fname, url, sz)
            break

        if found is None:
            print(f"  [WARN] No file found under {max_mb} MB cap for {instrument}", flush=True)
            continue

        accession, fname, url, sz = found
        dest = CORPUS_DIR / f"{accession}_{lbl}_{fname}"

        if dry_run:
            print(f"  [DRY-RUN] → {dest.name}", flush=True)
            continue
        ok = download(url, dest)
        if ok:
            actual = dest.stat().st_size
            print(f"  Done: {actual/1e6:.1f} MB", flush=True)
            manifest[instrument] = {"accession": accession, "filename": dest.name,
                                    "size_bytes": actual}
            if not dry_run:
                save_manifest(manifest)
        time.sleep(1)

    print(f"\n{'='*60}", flush=True)
    files = sorted(CORPUS_DIR.glob("*.[Rr][Aa][Ww]"))
    print(f"Corpus now has {len(files)} file(s):", flush=True)
    for f in files:
        print(f"  {f.name}  ({f.stat().st_size/1e6:.1f} MB)", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--max-mb", type=int, default=DEFAULT_MAX_MB)
    args = parser.parse_args()
    run(max_mb=args.max_mb, dry_run=args.dry_run)
