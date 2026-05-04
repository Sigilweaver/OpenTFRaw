# OpenTFRaw Validation Corpus

The test corpus covers every major Thermo RAW format variant the parser
needs to handle:

- All supported format versions (8, 47, 57, 60, 62, 63, 64, 66)
- Both scan-data encodings (PacketHeader and the two Flat variants)
- Each major instrument family (ion trap, Orbitrap hybrid, Q-Orbitrap,
  Tribrid, single-stage Orbitrap, Astral, triple quadrupole)

One representative file per instrument line is enough to exercise every
format path while keeping total corpus size to roughly 6-10 GB.

## Source: PRIDE Archive

All files come from the EBI PRIDE Archive (https://www.ebi.ac.uk/pride/),
a public proteomics repository hosting hundreds of thousands of Thermo RAW
files contributed by academic and commercial labs.

Access is via HTTPS from the PRIDE FTP mirror:

    https://ftp.pride.ebi.ac.uk/pride/data/archive/YYYY/MM/\<PXD_ACCESSION\>/

PRIDE datasets are published under CC-BY or equivalent open licences.

## Source List

The file `scripts/sources.json` records exactly which PRIDE file to
download for each instrument:

    [
      {
        "instrument": "LCQ Classic",
        "accession": "PXD044152",
        "pride_filename": "Ex250122_K50ng_60m2.raw"
      },
      {
        "instrument": "Orbitrap Fusion Lumos",
        "mode": "DIA",
        "accession": "PXD031322",
        "pride_filename": "OFL001513-YLL-GPF-15K-1.raw"
      },
      ...
    ]

The optional `mode` field distinguishes additional files for the same
instrument that cover a different acquisition mode (DIA, EThcD, PRM, MS3,
etc.).  When present, the manifest key is `"Instrument (mode)"` instead
of just `"Instrument"`, so both files are tracked independently.

To add or replace an entry, edit `sources.json` directly and re-run the
fetcher.  The manifest (`corpus/manifest.json`) records what is
currently on disk; the fetcher skips any key already present there.

## Running the Fetcher

    python scripts/fetch_corpus.py          # download missing files
    python scripts/fetch_corpus.py --dry-run # report without downloading

The script resolves each download URL through the PRIDE REST API
(https://www.ebi.ac.uk/pride/ws/archive/v2/files/byProject\) and saves
files as `{accession}_{instrument_label}_{original_filename}` under
`corpus/`.  If the API returns an empty response (an intermittent server
behaviour observed in 2026), the script falls back to constructing the
FTP URL directly from the project publication date.

## Provenance Record

`corpus/manifest.json` records which PRIDE project each local
file came from:

    {
      "LTQ Orbitrap XL": {
        "accession": "PXD055201",
        "filename": "PXD055201_LTQ_Orbitrap_XL_20170427_..._2.raw",
        "size_bytes": 396954554
      },
      ...
    }

To trace any file back to its source, use the PXD accession:

    https://www.ebi.ac.uk/pride/archive/projects/\<PXD_ACCESSION\>

## Target Instruments and Acquisition Modes

The corpus is organised in two tiers:

**Tier 1 - one file per instrument line** (covers every format version
and scan-data encoding path):

| Family                    | Instruments                                                   |
| ------------------------- | ------------------------------------------------------------- |
| Ion traps (LCQ/LTQ)       | LCQ Classic, LTQ, LTQ XL, LTQ Velos, LTQ FT                  |
| LTQ Orbitrap hybrids      | LTQ Orbitrap, XL, XL ETD, Velos, Velos Pro, Elite             |
| Q-Orbitrap                | Q Exactive, Plus, HF, HF-X, UHMR                              |
| Tribrid Orbitrap          | Fusion, Fusion Lumos, Eclipse, Ascend                         |
| Single-stage Orbitrap     | Exploris 120, 240, 480, Astral (DIA)                          |
| Triple quadrupole         | TSQ Vantage, Quantiva, Altis                                  |

**Tier 2 - additional files per instrument covering distinct modes**:

| Entry                            | Mode   | What it exercises                          |
| -------------------------------- | ------ | ------------------------------------------ |
| Orbitrap Fusion Lumos (DIA)      | DIA    | Multiple isolation windows per scan cycle  |
| Orbitrap Fusion Lumos (MS3)      | MS3    | Three-stage fragmentation / XL-MS workflow |
| Orbitrap Eclipse (EThcD)         | EThcD  | Electron-transfer + supplemental HCD       |
| Q Exactive HF (DIA)              | DIA    | Fixed-window SWATH-like DIA on Q Exactive  |
| Orbitrap Exploris 480 (DDA-2)    | DDA    | Second firmware vintage for regression     |
| TSQ Altis (SRM-2)                | SRM-2  | Second SRM file from a different dataset   |
| Q Exactive HF-X (PRM)            | PRM    | Parallel reaction monitoring: 42 targets,  |
|                                  |        | 7-minute gradient, SARS-CoV-2 peptides     |

### Multi-controller coverage

Several Tier 1 files carry `controller_count > 1` in their
`RawFileInfoPreamble`, meaning the RAW file contains a UV/analog chromatogram
channel alongside the MS data stream.  The parser exercises the
multi-controller selection path (reader.rs `select_ms_run_header`) for these:

| File (Tier 1 instrument)  | `controller_count` | Confirmed year |
| ------------------------- | :----------------: | -------------- |
| Orbitrap Fusion           | 2                  | 2016-12        |
| Orbitrap Fusion Lumos     | 2                  | 2016-03        |
| LTQ Orbitrap (PXD069348)  | 3                  | 2014-02        |

The selection heuristic — `ntrailer > 0` (v64+) or `nsegs > 0 && first_scan
<= last_scan` (v63) — correctly identifies the MS controller in every case.

## Limitations

- PRIDE's metadata lists declared instrument names; a few submitters
  mislabel files.  Device detection in the parser is therefore best-effort.
- Some instrument lines (Astral, top-down ETD workflows) have few small
  files on PRIDE.  The entries in sources.json were chosen to be the
  smallest available representative files at the time the corpus was built.
