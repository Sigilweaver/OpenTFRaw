# OpenTFRaw Validation Corpus

The test corpus covers every major Thermo RAW format variant the parser
needs to handle:

- All supported format versions (8, 47, 57, 60, 62, 63, 64, 66)
- Both scan-data encodings (PacketHeader and the two Flat variants)
- Each major instrument family (ion trap, Orbitrap hybrid, Q-Orbitrap,
  Tribrid, single-stage Orbitrap, Astral, triple quadrupole)

Current size: ~124 GB across 283 files, covering all instrument families
and acquisition modes.  Multiple files per instrument are included to
exercise parameter variation across real-world datasets.

## Source: PRIDE Archive

All files come from the EBI PRIDE Archive (https://www.ebi.ac.uk/pride/),
a public proteomics repository hosting hundreds of thousands of Thermo RAW
files contributed by academic and commercial labs.

Access is via HTTPS from the PRIDE FTP mirror:

    https://ftp.pride.ebi.ac.uk/pride/data/archive/YYYY/MM/\<PXD_ACCESSION\>/

PRIDE datasets are published under CC-BY or equivalent open licences.

## Source List

The file `scripts/sources.json` records which PRIDE projects and files to
download:

    [
      {
        "instrument": "LCQ Classic",
        "accession": "PXD044152",
        "files": ["Ex250122_K50ng_60m2.raw"],
        "count": 6
      },
      {
        "instrument": "Orbitrap Fusion Lumos",
        "mode": "DIA",
        "accession": "PXD031322",
        "files": ["OFL001513-YLL-GPF-15K-1.raw"],
        "count": 5
      },
      ...
    ]

- `files` - specific filenames always downloaded first
- `count` - total target file count from this project; the fetcher
  auto-fills from the FTP directory listing until the count is reached
- `mode` - distinguishes multiple entries for the same instrument
  covering different acquisition modes (DIA, EThcD, PRM, MS3, etc.)

To add or replace an entry, edit `sources.json` directly and re-run the
fetcher.  The manifest (`corpus/manifest.json`) records what is
currently on disk; the fetcher skips any key already present there.

## Running the Fetcher

    python scripts/fetch_corpus.py             # download missing files
    python scripts/fetch_corpus.py --dry-run   # report without downloading
    python scripts/fetch_corpus.py --list-files PXD032800  # discover files

The script resolves each download URL through the PRIDE REST API
(https://www.ebi.ac.uk/pride/ws/archive/v2/files/byProject) and saves
files as `{accession}_{instrument_label}_{original_filename}` under
`corpus/`.  If the API returns an empty response (an intermittent server
behaviour observed in 2026), the script falls back to constructing the
FTP URL directly from the project publication date.

To discover all available files in a PRIDE project before adding it to
`sources.json`:

    python scripts/fetch_corpus.py --list-files PXD032800

## Provenance Record

`corpus/manifest.json` records which PRIDE project each local
file came from.  Keys are `{accession}/{original_filename}`:

    {
      "PXD055201/20170427_CO_0673AnGS_DM_Mix1_R12R13R14_2.raw": {
        "instrument": "LTQ Orbitrap XL",
        "dest_filename": "PXD055201_LTQ_Orbitrap_XL_20170427_..._2.raw",
        "size_bytes": 396954554
      },
      ...
    }

To trace any file back to its source, use the PXD accession:

    https://www.ebi.ac.uk/pride/archive/projects/<PXD_ACCESSION>

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

| Entry                            | Mode   | What it exercises                                      |
| -------------------------------- | ------ | ------------------------------------------------------ |
| Orbitrap Fusion Lumos (DIA)      | DIA    | Multiple isolation windows per scan cycle              |
| Orbitrap Fusion Lumos (MS3)      | MS3    | Three-stage fragmentation / XL-MS workflow             |
| Orbitrap Fusion Lumos (EThcD)    | EThcD  | Supplemental activation on tribrid variable-body scans |
| Orbitrap Eclipse (EThcD)         | EThcD  | Electron-transfer + supplemental HCD, two-clause filter|
| Orbitrap Eclipse (DIA)           | DIA    | DIA on tribrid: checks if reaction np>0 in body        |
| Orbitrap Fusion Lumos (UVPD)     | UVPD   | Ultraviolet photodissociation, tests Activation::Uvpd  |
| Q Exactive HF (DIA)              | DIA    | Fixed-window SWATH-like DIA on Q Exactive              |
| Orbitrap Exploris 480 (DDA-2)    | DDA    | Second firmware vintage for regression                 |
| TSQ Altis (SRM-2)                | SRM-2  | Second SRM file from a different dataset               |
| Q Exactive HF-X (PRM)            | PRM    | Parallel reaction monitoring: 42 targets,              |
|                                  |        | 7-minute gradient, SARS-CoV-2 peptides                 |

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
- Some instrument lines (Astral, top-down ETD workflows) have few publicly
  available files on PRIDE.  The `count` values in `sources.json` are
  capped at the number of files actually present in the FTP directory.

## Open Issues

### DIA isolation window m/z (Orbitrap Exploris 480 and similar)

For the Exploris 480 DIA files in corpus (PXD035500), the isolation window
center m/z is currently absent from filter strings.  Investigation findings:

- **Scan event body format**: DIA MS2 scan events use a uniform 136-byte body
  (event size = 272 bytes total).  The body[8..12] f32 field holds a value in
  the range ~3.8-5.0, which is in instrument frequency space, not m/z.  There
  is no reaction structure (np = 0 at body[4..8]) and no m/z at any body offset.

- **Scan params**: The file has 1004 bytes/scan of scan params data starting at
  `scan_params_addr`, but the GenericDataHeader (GDH) that describes the record
  schema was not found anywhere in the 8 MB window between the error log and
  scan_trailer that `find_forward` searches.  As a result `scan_parameters` is
  empty for all scans in this file and `ScanParams` accessors return `None`.

- **What is needed**: Locate the GDH for Exploris 480 scan params (it may be
  outside the current search window, or use a different header format).  Once
  the schema is found, the calibration coefficients (conversion parameters A, B,
  C) inside the scan params record can be used to convert frequency to m/z and
  recover the isolation window center.

- **Workaround**: For instruments where the GDH is found correctly (Q Exactive,
  Fusion Lumos, Eclipse), `ScanParams::isolation_width_mz()` and the
  `monoisotopic_mz()` family already work.  Eclipse DIA files (PXD038440, once
  downloaded) will clarify whether tribrid instruments store the isolation m/z
  in the reaction structure (np > 0) as DDA scans do, bypassing the calibration
  problem entirely.

### Acquisition modes not yet in corpus

| Mode | Notes |
| ---- | ----- |
| SPS-MS3 (TMT) | Synchronous precursor selection MS3 for isobaric quantification; differs from standard MS3 in the number of simultaneous precursor m/z in the scan event body. |
| ECD / IRMPD | Both enum variants implemented; no corpus files yet. |
