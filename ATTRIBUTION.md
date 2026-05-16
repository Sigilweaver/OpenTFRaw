# Credits

## Prior art

### unfinnigan

Gene Selkov, 2010-2012. Perl and Python reverse-engineering of the Thermo RAW binary format.
The most thorough prior independent analysis of the format, covering versions 57, 62, 63, 64,
and 66. Field names and layout notes from unfinnigan were cross-referenced when validating
field offsets.

Source: https://github.com/prvst/unfinnigan

## Standards

The mzML output follows the [HUPO-PSI mzML 1.1.0 specification](https://www.psidev.info/mzML)
and uses CV terms from the PSI-MS ontology (psi-ms.obo):

    Deutsch EW et al. "A guided tour of the Trans-Proteomic Pipeline."
    Proteomics. 2010;10(6):1150-9. doi:10.1002/pmic.200900375

Instrument CV accessions were cross-referenced against the PSI-MS ontology instrument
model branch (MS:1000031).

## Validation corpus

Corpus files were downloaded from the [PRIDE Archive](https://www.ebi.ac.uk/pride/):

    Perez-Riverol Y et al. "The PRIDE database and related tools and resources in 2019:
    improving support for quantification data." Nucleic Acids Res. 2019;47(D1):D442-D450.
    doi:10.1093/nar/gky1106

## Rust dependencies

- [thiserror](https://github.com/dtolnay/thiserror) -- derive macro for Error impls (David Tolnay, MIT/Apache-2.0)
- [pyo3](https://github.com/PyO3/pyo3) -- Rust/Python bindings (PyO3 contributors, MIT/Apache-2.0)
- [numpy](https://github.com/PyO3/rust-numpy) -- PyO3 numpy integration (PyO3 contributors, BSD-2-Clause)
- [maturin](https://github.com/PyO3/maturin) -- Python wheel build tool (PyO3 contributors, MIT/Apache-2.0)
