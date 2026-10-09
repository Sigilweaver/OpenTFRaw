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
| Q Exactive Plus (DDA-2)          | DDA    | Second Q Exactive Plus vintage for regression          |
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

The selection heuristic - `ntrailer > 0` (v64+) or `nsegs > 0 && first_scan
<= last_scan` (v63) - correctly identifies the MS controller in every case.

## Limitations

- PRIDE's metadata lists declared instrument names; a few submitters
  mislabel files.  Device detection in the parser is therefore best-effort.
- Some instrument lines (Astral, top-down ETD workflows) have few publicly
  available files on PRIDE.  The `count` values in `sources.json` are
  capped at the number of files actually present in the FTP directory.

## Open Issues

### Source-CID method fallback (issue #57)

The public GlycoPOST acquisition
[`GPST000122.0/NGlycans_Serum_Fetuin_File1.raw`](https://glycopost.glycosmos.org/data/GPST000122.0/NGlycans_Serum_Fetuin_File1.raw)
provides an additional original-byte regression for a Q Exactive Plus in
negative NSI. The [study entry](https://glycopost.glycosmos.org/entry/GPST000122)
is linked by the [research publication](https://doi.org/10.1038/s41467-023-37365-4).

- Size: 28,102,388 bytes.
- SHA-256: `ea19fe225a38e370f63dcdc7bba4d551706d61f06ec98dc2edc6b8334d7ab819`.
- All 6,674 scan trailers lack a source-CID energy. Two explicitly linked
  embedded method XML scan events each declare 70 eV.
- All 419 MS1 and 6,255 MS2 scans resolve `instrument_method` provenance and
  render `sid=70.00`. Rust extras, Python streamed scans and canonical records
  are checked by the public-fixture tests.

Run the ignored Rust regression and the optional Python regression with
`OPENTFRAW_SOURCE_CID_PLUS_RAW` pointing to this acquisition. The fixture stays
outside the repository; no acquisition bytes or converted vendor output are
committed. This file verifies the reported instrument family, negative
polarity and missing-trailer fallback, but does not reproduce the original
single-SIM 20 eV acquisition. Do not treat that remaining case as verified.

### DIA isolation-window centers (issue #44)

A 2026-09-29 investigation of public PXD035500 Exploris 480 files
`RN_SGLab_210301_DN_vDIA_01.raw` and
`RN_SGLab_210301_DN_vDIA_15.raw`, and PXD031322 Fusion Lumos file
`OFL001513-YLL-GPF-15K-1.raw`, corrected the earlier interpretation:

- The isolation center is an f64 at scan-event body offset 4, not a
  frequency value. Reading only its high four bytes at body[8..12] as f32
  produced the misleading values near 3.8-5.0. No frequency-to-m/z
  conversion is needed for these centers. The existing offset-4 reaction
  decoder recovers the Exploris centers.
- Matching scan-parameter schemas are present close to the error log in
  both Exploris files (332 and 1196 bytes from `error_log_addr`, respectively).
  Each describes a 1004-byte record. The current 4 MiB forward-search cap
  does not block recovery on these files. The Lumos schema describes a
  567-byte record and is also recovered by the existing search.
- The Lumos DIA file uses 232-byte primary events and 288-byte MS2 events,
  rather than the 232/344-byte family used by other tribrid workflows.
  Both size equations can fit its total event-stream length. The reader
  now validates every preamble and the exact stream end before selecting
  a known variable layout.
- Decoding the complete Lumos event metadata yields 2308 MS1 and 150020
  MS2 events, with an isolation center on every MS2. The first centers
  (351.4096, 353.4105, 355.4114) agree with the file's embedded method table.

Evidence came from bounded HTTP byte ranges of the public RAW files,
without vendor software or vendor-generated output. Full-spectrum export
and whole-file decoding still require a complete target fixture.

### Acquisition modes not yet in corpus

| Mode | Notes |
| ---- | ----- |
| Eclipse DIA | DIA on tribrid Orbitrap: needed to confirm whether tribrid instruments store isolation m/z in reaction structure (np>0) as DDA scans do. No confirmed Eclipse DIA PRIDE accession with accessible RAW files identified yet. Fusion Lumos DIA files (PXD031322) carry direct isolation centers at body offset 4; Eclipse DIA remains unverified. |
| SPS-MS3 (TMT) | Synchronous precursor selection MS3 for isobaric quantification; differs from standard MS3 in the number of simultaneous precursor m/z in the scan event body. |
| ECD / IRMPD | Both enum variants implemented; no corpus files yet. |

## Lock-mass count controls (#58)

These intentionally published original acquisitions are used by the opt-in
`public_lock_mass_counts_and_inherited_correction` test. Files remain outside
version control; the test takes local paths through the variables below.

| Variable | Public acquisition | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `OPENTFRAW_LOCK_MASS_PLUS_RAW` | [MTBLS5657/20200612_01_Neg_DOPAEx2_ZH04.RAW](https://ftp.ebi.ac.uk/pub/databases/metabolights/studies/public/MTBLS5657/FILES/RAW_FILES/NEG/20200612_01_Neg_DOPAEx2_ZH04.RAW) | 5902616 | `83b38b0523361a6babcc21bcb2c112250aecd3a3a56fce5ab7f7b3f56a36bf42` |
| `OPENTFRAW_LOCK_MASS_HFX_RAW` | [PXD071477/RS_300825_3.raw](https://ftp.pride.ebi.ac.uk/pride/data/archive/2026/03/PXD071477/RS_300825_3.raw) | 210873724 | `853a4291744a1ec93636073483ecc38fd67862a0e6baa8d2e9bde8eb121c2270` |
| `OPENTFRAW_LOCK_MASS_EXPLORIS_RAW` | [PXD064947/X6212FD_2.raw](https://ftp.pride.ebi.ac.uk/pride/data/archive/2026/04/PXD064947/X6212FD_2.raw) | 655224569 | `7d686792c803e347bad482a733cab93012c75bbefe630dfd8672b06076d656a5` |

MTBLS5657 contains 138 negative-mode Q Exactive Plus scans, each configured
for nine lock masses and reporting one match with a nonzero correction.
Its [study announcement](https://ftp.ebi.ac.uk/pub/databases/metabolights/studies/public/MTBLS5657/MTBLS5657.announcement.json)
specifies EMBL-EBI Terms of Use. The two PRIDE project records specify CC0.

PXD071477 has 27832 Q Exactive HF-X scans, of which 3559 report a match and
24273 report zero matches. PXD064947 has 51254 Exploris 480 scans, of which
16033 report a match and 35221 report zero matches. In both files every
zero-match scan retains the latest matched scan's nonzero correction; their
MS1 event also contains both matched and zero-match scans. These are count
and correction controls, not proof of the exact `lock` filter-token meaning.
No vendor software, SDK, or vendor output was used to validate them.
