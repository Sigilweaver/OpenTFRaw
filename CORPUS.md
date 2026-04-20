# OpenTFRaw Validation Corpus

This document describes the test corpus used to validate `opentfraw`'s parser
across the Thermo Fisher instrument lineup — **how the files are found,
where they come from, and why each one was chosen**.

## Goal

Build a **minimal but representative sample set** covering every major
Thermo RAW file format variant the parser needs to handle:

- All supported file-format versions (8, 47, 57, 60, 62, 63, 64, 66)
- Both scan data encodings (`PacketHeader` and the two `Flat` variants)
- Each major instrument family (ion trap, Orbitrap hybrid, Q-Orbitrap,
  Tribrid, single-stage Orbitrap, Astral, triple quadrupole)

The corpus is deliberately kept small — one representative file per
instrument class — so it fits in local dev environments (≈ 6–10 GB total)
while still exercising every format path.

## Source: PRIDE Archive

All files come from the **EBI PRIDE Archive**
(<https://www.ebi.ac.uk/pride/>), a public proteomics data repository that
hosts hundreds of thousands of Thermo RAW files contributed by academic
and commercial labs worldwide.

- **Access**: open FTP at `https://ftp.pride.ebi.ac.uk/pride/data/archive/YYYY/MM/<PXD_ACCESSION>/`
- **Licensing**: PRIDE datasets are published under CC-BY or equivalent
  open licences — free to download, redistribute, and use for research
  and software-testing purposes.
- **Discovery**: the sibling repo [`TFRaw-Sources`](../TFRaw-Sources/) runs
  a nightly crawl of PRIDE's submission metadata and emits
  `data/known_projects.json` — a map
  `{PXD_accession → {instruments: [...], raw_file_count, pub_date, ...}}`
  covering every PRIDE project that declared at least one Thermo
  instrument.

## Discovery Pipeline

The corpus fetcher ([scripts/fetch_corpus.py](scripts/fetch_corpus.py))
implements the following algorithm:

1. Read `TFRaw-Sources/data/known_projects.json`.
2. For each target instrument name in the `TARGETS` list:
   1. Filter all projects whose `instruments` array contains that name
      exactly.
   2. Sort the filtered projects by `raw_file_count` **ascending** —
      smaller projects tend to have shorter gradients and therefore
      smaller individual RAW files.
   3. For each project in order, walk the FTP directory listing and
      pick the smallest `.raw` file **under the size cap** (default 400 MB).
   4. Download the first one that fits; skip and try the next project
      otherwise.
3. Rename the file to `<PXD_accession>_<Instrument>_<original_name>.raw`
   on disk and record the provenance in `samples/corpus/manifest.json`.

This deterministic, cap-bounded walk means the corpus is reproducible:
re-running `fetch_corpus.py` on a clean machine produces the same set
of files.

## Running the Fetcher

```bash
cd OpenTFRaw
python3 scripts/fetch_corpus.py             # default 400 MB cap
python3 scripts/fetch_corpus.py --max-mb 800 # raise cap for Astral etc.
python3 scripts/fetch_corpus.py --dry-run   # just report sizes
```

Targets already present in `samples/corpus/` are skipped.

## Target Instruments

`TARGETS` in `fetch_corpus.py` covers 29 instrument names spanning every
format-relevant line Thermo has shipped:

| Family                    | Instruments                                                       |
| ------------------------- | ----------------------------------------------------------------- |
| Ion traps (LCQ/LTQ)       | LCQ Classic · LTQ · LTQ XL · LTQ Velos · LTQ FT                   |
| LTQ Orbitrap hybrids      | LTQ Orbitrap · XL · XL ETD · Velos · Velos Pro · Elite            |
| Q-Orbitrap                | Q Exactive · Plus · HF · HF-X · UHMR                              |
| Tribrid Orbitrap          | Fusion · Fusion Lumos · Eclipse · Ascend                          |
| Single-stage Orbitrap     | Exploris 120 · 240 · 480 · Astral                                 |
| Triple quadrupole         | TSQ Vantage · Quantiva · Altis                                    |

Not every target is guaranteed to have a file under the default cap on
PRIDE (some instrument lines — notably LTQ FT, LTQ Orbitrap XL ETD, and
Astral — predominantly produce multi-GB top-down / intact-protein
datasets). In those cases raising `--max-mb` is required.

## Provenance Record

`samples/corpus/manifest.json` records **exactly which PRIDE project**
each downloaded file came from, so the corpus is fully auditable:

```jsonc
{
  "LTQ Orbitrap XL": {
    "accession": "PXD055201",
    "filename": "PXD055201_LTQ_Orbitrap_XL_20170427_CO_0673AnGS_DM_Mix1_...",
    "size_bytes": 396954554
  },
  // ...
}
```

To trace any file back to its source: the PXD accession links to
`https://www.ebi.ac.uk/pride/archive/projects/<PXD_ACCESSION>`, which
lists the submitters, publication, and licence.

## Why This Approach

- **Open data**: PRIDE files are freely redistributable, so CI can
  re-fetch on demand without licence friction.
- **Diversity**: submissions come from hundreds of labs using many
  different acquisition methods, giving broader coverage than any
  single-lab test set.
- **Real-world**: these are actual research datasets, not synthetic
  test files — any bug surfaced here is a bug users will hit.
- **Cheap to reproduce**: the fetcher + manifest make a clean-room
  rebuild a single command.

## Limitations

- PRIDE's schema lists _declared_ instrument names; a few submitters
  mislabel files (e.g. acquired on an Orbitrap Fusion but tagged as
  something else). Device detection in the parser is therefore
  best-effort.
- Some instrument lines (Astral, LTQ FT top-down, ETD workflows)
  have no published files under 400 MB — the cap may need to be
  raised to cover them.
- The fetcher does **not** attempt to balance multiple acquisition
  modes (DDA vs DIA vs SRM) within an instrument line; it picks
  whichever file is smallest.
