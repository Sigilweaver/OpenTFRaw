"""
Fetch RAW files from PRIDE for the OpenTFRaw corpus.

Each entry in sources.json describes one PRIDE project to draw files from:

  instrument  str      instrument model name
  accession   str      PRIDE accession, e.g. "PXD006062"
  mode        str?     optional acquisition-mode label
  files       list?    specific filenames to always download
  count       int?     total target file count from this project;
                       auto-fills from FTP directory listing when more
                       files are needed beyond those listed in 'files'

Manifest (corpus/manifest.json) keys are "{accession}/{original_filename}",
making them file-level and immune to instrument-label changes.

Usage:
    python scripts/fetch_corpus.py [--dry-run]
    python scripts/fetch_corpus.py --list-files ACCESSION
"""

from __future__ import annotations

import argparse
import json
import re
import time
import urllib.request
from pathlib import Path

CORPUS_DIR = Path(__file__).parent.parent / "corpus"
MANIFEST = CORPUS_DIR / "manifest.json"
SOURCES = Path(__file__).parent / "sources.json"
USER_AGENT = "OpenTFRaw-CorpusFetcher/1.0"
PRIDE_API = "https://www.ebi.ac.uk/pride/ws/archive/v2"
FTP_BASE = "https://ftp.pride.ebi.ac.uk/pride/data/archive"


def _req(url: str) -> urllib.request.Request:
    return urllib.request.Request(url, headers={"User-Agent": USER_AGENT})


def project_pub_date(accession: str) -> str | None:
    """Return publicationDate (YYYY-MM-DD) for a PRIDE project, or None."""
    try:
        with urllib.request.urlopen(
            _req(f"{PRIDE_API}/projects/{accession}"), timeout=20
        ) as r:
            return json.loads(r.read()).get("publicationDate")
    except Exception:
        return None


def ftp_dir_url(accession: str, pub_date: str) -> str:
    yyyy, mm = pub_date[:4], pub_date[5:7]
    return f"{FTP_BASE}/{yyyy}/{mm}/{accession}/"


def ftp_file_url(accession: str, filename: str, pub_date: str) -> str:
    return ftp_dir_url(accession, pub_date) + filename


def list_ftp_raw_files(accession: str, pub_date: str) -> list[str]:
    """Return sorted list of .raw/.RAW filenames in the project FTP directory."""
    url = ftp_dir_url(accession, pub_date)
    try:
        with urllib.request.urlopen(_req(url), timeout=30) as r:
            html = r.read().decode("utf-8", errors="replace")
    except Exception as e:
        print(f"  [ERROR] FTP listing failed ({url}): {e}", flush=True)
        return []
    names = re.findall(r'<a\s+href="([^"/][^"]*\.[Rr][Aa][Ww])"', html)
    return sorted(set(names))


def resolve_url(
    accession: str, filename: str, pub_date: str | None = None
) -> tuple[str, int] | None:
    """
    Return (https_url, size_bytes) for a PRIDE file.
    Tries the PRIDE REST API; falls back to the FTP URL via pub_date.
    """
    page, page_size = 0, 100
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
                break
            data = json.loads(raw)
        except Exception as e:
            print(f"  [ERROR] PRIDE API: {e}", flush=True)
            break

        api_returned_data = True
        for entry in data.get("content", []):
            if entry.get("fileName", "").lower() == filename.lower():
                size = entry.get("fileSizeBytes", 0)
                for loc in entry.get("publicFileLocations", []):
                    val: str = loc.get("value", "")
                    if val.startswith("ftp://"):
                        return (
                            val.replace(
                                "ftp://ftp.pride.ebi.ac.uk",
                                "https://ftp.pride.ebi.ac.uk",
                                1,
                            ),
                            size,
                        )
                    if val.startswith("https://"):
                        return val, size
                return None

        pi = data.get("page", {})
        if (page + 1) * page_size >= pi.get("totalElements", 0):
            break
        page += 1
        time.sleep(0.2)

    if api_returned_data:
        return None

    print("  [INFO] API empty; using FTP fallback", flush=True)
    if pub_date is None:
        pub_date = project_pub_date(accession)
    if pub_date is None:
        print(f"  [ERROR] no pub date for {accession}", flush=True)
        return None
    return ftp_file_url(accession, filename, pub_date), 0


def load_manifest() -> dict:
    if not MANIFEST.exists():
        return {}
    with open(MANIFEST) as f:
        return json.load(f)


def migrate_manifest(manifest: dict, sources: list[dict]) -> dict:
    """
    One-time migration: rekey old instrument-string entries to
    accession/filename format.
    """
    if all("/" in k for k in manifest):
        return manifest

    print("[INFO] Migrating manifest to accession/filename key format.", flush=True)
    instruments = sorted(
        {e["instrument"] for e in sources}, key=len, reverse=True
    )
    new: dict = {}
    for key, val in manifest.items():
        if "/" in key:
            new[key] = val
            continue
        acc = val.get("accession", "")
        dest = val.get("filename", "")
        size = val.get("size_bytes", 0)
        rest = dest.removeprefix(f"{acc}_")
        original = None
        inst_matched = None
        for inst in instruments:
            lbl = inst.replace(" ", "_") + "_"
            if rest.startswith(lbl):
                original = rest[len(lbl):]
                inst_matched = inst
                break
        if original is None:
            # Older downloads used "{accession}_{original_filename}" with no label.
            # In that case rest IS the original filename.
            original = rest
            # Locate the instrument from sources using the accession.
            for s in sources:
                if s.get("accession") == acc:
                    inst_matched = s["instrument"]
                    break
            if inst_matched is None:
                print(f"  [WARN] could not migrate {key!r}, keeping as-is", flush=True)
                new[key] = val
                continue
        new_key = f"{acc}/{original}"
        new[new_key] = {
            "instrument": inst_matched,
            "dest_filename": dest,
            "size_bytes": size,
        }
        print(f"  {key!r} -> {new_key!r}", flush=True)
    return new


def save_manifest(manifest: dict) -> None:
    MANIFEST.parent.mkdir(parents=True, exist_ok=True)
    with open(MANIFEST, "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")


def downloaded_for(manifest: dict, accession: str) -> list[str]:
    """Return original filenames already recorded for this accession."""
    pfx = f"{accession}/"
    return [k[len(pfx):] for k in manifest if k.startswith(pfx)]


def _download_file(url: str, dest: Path) -> bool:
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


def fetch_one(
    accession: str,
    filename: str,
    instrument: str,
    manifest: dict,
    dry_run: bool,
    pub_date: str | None = None,
) -> None:
    """Resolve URL, download filename, update manifest."""
    result = resolve_url(accession, filename, pub_date)
    if result is None:
        print(f"  [WARN] could not resolve URL for {filename}", flush=True)
        return
    url, size = result
    size_str = f"{size / 1e6:.1f} MB" if size else "size unknown"
    lbl = instrument.replace(" ", "_")
    dest = CORPUS_DIR / f"{accession}_{lbl}_{filename}"
    print(f"  {filename}  ({size_str})", flush=True)
    if dry_run:
        print(f"  [DRY-RUN] would write {dest.name}", flush=True)
        return
    if _download_file(url, dest):
        actual = dest.stat().st_size
        print(f"  Done: {actual / 1e6:.1f} MB", flush=True)
        manifest[f"{accession}/{filename}"] = {
            "instrument": instrument,
            "dest_filename": dest.name,
            "size_bytes": actual,
        }
        save_manifest(manifest)
    time.sleep(1)


def run(dry_run: bool) -> None:
    CORPUS_DIR.mkdir(parents=True, exist_ok=True)
    with open(SOURCES) as f:
        sources: list[dict] = json.load(f)

    manifest = load_manifest()
    manifest = migrate_manifest(manifest, sources)
    save_manifest(manifest)

    for entry in sources:
        instrument: str = entry["instrument"]
        accession: str = entry["accession"]
        mode: str | None = entry.get("mode")
        explicit: list[str] = entry.get("files") or (
            [entry["pride_filename"]] if "pride_filename" in entry else []
        )
        count: int | None = entry.get("count")

        lbl_mode = f" ({mode})" if mode else ""
        print(f"\n{'='*60}", flush=True)
        print(f"  {instrument}{lbl_mode}  ({accession})", flush=True)

        already = downloaded_for(manifest, accession)

        for fname in explicit:
            if fname in already:
                print(f"  Already have: {fname}  -- skipping", flush=True)
                continue
            fetch_one(accession, fname, instrument, manifest, dry_run)
            already = downloaded_for(manifest, accession)

        if count is not None:
            need = count - len(already)
            if need <= 0:
                print(
                    f"  count={count} satisfied ({len(already)} files)  -- skipping",
                    flush=True,
                )
                continue
            pub_date = project_pub_date(accession)
            if not pub_date:
                print(f"  [ERROR] no pub date for {accession}", flush=True)
                continue
            available = list_ftp_raw_files(accession, pub_date)
            if not available:
                print(f"  [WARN] FTP listing empty for {accession}", flush=True)
                continue
            candidates = [f for f in available if f not in already]
            if not candidates:
                print(
                    f"  All {len(available)} available files already downloaded.",
                    flush=True,
                )
                continue
            print(
                f"  Auto-fill: need {need} more, "
                f"{len(candidates)} candidates from {len(available)} total",
                flush=True,
            )
            for fname in candidates[:need]:
                fetch_one(accession, fname, instrument, manifest, dry_run, pub_date)
                already = downloaded_for(manifest, accession)

    print(f"\n{'='*60}", flush=True)
    files = sorted(CORPUS_DIR.glob("*.[Rr][Aa][Ww]"))
    total = sum(f.stat().st_size for f in files)
    print(f"Corpus: {len(files)} file(s), {total / 1e9:.2f} GB total", flush=True)
    for f in files:
        print(f"  {f.name}  ({f.stat().st_size / 1e6:.1f} MB)", flush=True)


def cmd_list_files(accession: str) -> None:
    """Print .raw files available in a PRIDE project's FTP directory."""
    pub_date = project_pub_date(accession)
    if not pub_date:
        print(f"[ERROR] no publication date for {accession}")
        return
    files = list_ftp_raw_files(accession, pub_date)
    print(f"{accession}  published {pub_date}  |  {len(files)} .raw file(s)")
    print(f"  FTP dir: {ftp_dir_url(accession, pub_date)}")
    for fname in files:
        print(f"    {fname}")


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--dry-run", action="store_true",
                   help="resolve URLs but do not download")
    p.add_argument("--list-files", metavar="ACCESSION",
                   help="list available .raw files for a PRIDE project and exit")
    args = p.parse_args()
    if args.list_files:
        cmd_list_files(args.list_files)
    else:
        run(dry_run=args.dry_run)
