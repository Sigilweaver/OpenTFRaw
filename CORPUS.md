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
      ...
    ]

To add or replace an entry, edit `sources.json` directly and re-run the
fetcher.  The manifest (`samples/corpus/manifest.json`) records what is
currently on disk; the fetcher skips any instrument already present there.

## Running the Fetcher

    python scripts/fetch_corpus.py          # download missing files
    python scripts/fetch_corpus.py --dry-run # report without downloading

The script resolves each download URL through the PRIDE REST API
(https://www.ebi.ac.uk/pride/ws/archive/v2/files/byProject\) and saves
files as `{accession}_{instrument_label}_{original_filename}` under
`samples/corpus/`.

## Provenance Record

`samples/corpus/manifest.json` records which PRIDE project each local
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

## Target Instruments

| Family                    | Instruments                                                   |
| ------------------------- | ------------------------------------------------------------- |
| Ion traps (LCQ/LTQ)       | LCQ Classic, LTQ, LTQ XL, LTQ Velos, LTQ FT                  |
| LTQ Orbitrap hybrids      | LTQ Orbitrap, XL, XL ETD, Velos, Velos Pro, Elite             |
| Q-Orbitrap                | Q Exactive, Plus, HF, HF-X, UHMR                              |
| Tribrid Orbitrap          | Fusion, Fusion Lumos, Eclipse, Ascend                         |
| Single-stage Orbitrap     | Exploris 120, 240, 480, Astral                                |
| Triple quadrupole         | TSQ Vantage, Quantiva, Altis                                  |

## Limitations

- PRIDE's metadata lists declared instrument names; a few submitters
  mislabel files.  Device detection in the parser is therefore best-effort.
- Some instrument lines (Astral, top-down ETD workflows) have few small
  files on PRIDE.  The entries in sources.json were chosen to be the
  smallest available representative files at the time the corpus was built.
