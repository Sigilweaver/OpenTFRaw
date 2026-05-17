//! Python bindings for OpenTFRaw.
//!
//! Exposes `opentfraw.RawFile`, an ergonomic Python class that loads a
//! Thermo `.raw` file once and serves per-scan metadata and peak arrays
//! as NumPy arrays suitable for downstream analysis (pandas, pyteomics,
//! scikit-learn, ...).
//!
//! Build with:
//!
//! ```bash
//! cd python
//! maturin develop --release
//! python -c "import opentfraw; print(opentfraw.__doc__)"
//! ```

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Mutex;

use numpy::{PyArray1, ToPyArray};
use pyo3::exceptions::{PyIOError, PyIndexError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use tfraw::{MsPower, Polarity, RawFileReader};

/// Translate an tfraw::Error into a Python exception.
fn to_py_err(e: tfraw::Error) -> PyErr {
    PyIOError::new_err(format!("{e}"))
}

/// Loaded Thermo RAW file.
///
/// Parameters
/// ----------
/// path : str
///     Path to a `.raw` file.
///
/// Attributes
/// ----------
/// num_scans : int
///     Total number of scans.
/// first_scan : int
///     Scan number of the first scan (usually 1).
/// last_scan : int
///     Scan number of the last scan.
/// instrument_model : str | None
///     Detected instrument model name (e.g. `"Orbitrap Fusion Lumos"`).
///
/// Example
/// -------
/// ```python
/// import opentfraw
/// raw = opentfraw.RawFile("experiment.raw")
/// for scan in raw.iter_scans():
///     if scan["ms_level"] == 2:
///         mz, intensity = scan["mz"], scan["intensity"]
///         ...
/// raw.to_mzml("experiment.mzML")
/// ```
#[pyclass(name = "RawFile", module = "opentfraw")]
struct RawFile {
    path: PathBuf,
    reader: RawFileReader,
    // A second handle used for scan-data reads. Wrapped in a Mutex so the
    // class is thread-safe from Python's perspective even though the PyO3
    // runtime already holds the GIL.
    source: Mutex<BufReader<File>>,
}

#[pymethods]
impl RawFile {
    #[new]
    fn new(path: &str) -> PyResult<Self> {
        let p = PathBuf::from(path);
        let reader = RawFileReader::open_path(&p).map_err(to_py_err)?;
        let file = File::open(&p).map_err(|e| PyIOError::new_err(e.to_string()))?;
        Ok(Self {
            path: p,
            reader,
            source: Mutex::new(BufReader::new(file)),
        })
    }

    #[getter]
    fn path(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }

    #[getter]
    fn num_scans(&self) -> u32 {
        self.reader.num_scans
    }

    #[getter]
    fn first_scan(&self) -> u32 {
        self.reader.run_header.sample_info.first_scan_number
    }

    #[getter]
    fn last_scan(&self) -> u32 {
        self.first_scan() + self.num_scans().saturating_sub(1)
    }

    #[getter]
    fn instrument_model(&self) -> Option<String> {
        self.reader.instrument_model.map(|s| s.to_string())
    }

    fn __len__(&self) -> usize {
        self.num_scans() as usize
    }

    fn __repr__(&self) -> String {
        format!(
            "RawFile(path={:?}, num_scans={}, instrument={:?})",
            self.path.to_string_lossy(),
            self.num_scans(),
            self.instrument_model()
        )
    }

    /// Return the canonical Thermo scan filter string for `scan_number`, or
    /// `None` if the scan is out of range.
    fn scan_filter(&self, scan_number: u32) -> Option<String> {
        self.reader.scan_filter(scan_number)
    }

    /// Read centroided peaks for `scan_number` and return
    /// `(mz: numpy.float64[:], intensity: numpy.float32[:])`.
    ///
    /// Profile data is skipped for speed; use :meth:`peaks` to read
    /// centroided peaks on any file type.
    fn peaks<'py>(
        &self,
        py: Python<'py>,
        scan_number: u32,
    ) -> PyResult<(Bound<'py, PyArray1<f64>>, Bound<'py, PyArray1<f32>>)> {
        let mut src = self
            .source
            .lock()
            .map_err(|_| PyIOError::new_err("internal error: source mutex poisoned"))?;
        let peaks = self
            .reader
            .read_peaks_only(&mut *src, scan_number)
            .map_err(to_py_err)?;
        let mz: Vec<f64> = peaks.iter().map(|p| p.mz as f64).collect();
        let intensity: Vec<f32> = peaks.iter().map(|p| p.abundance).collect();
        Ok((mz.to_pyarray_bound(py), intensity.to_pyarray_bound(py)))
    }

    /// Return a dict of per-scan metadata + peak arrays.
    ///
    /// Keys
    /// ----
    /// scan_number : int
    /// ms_level : int
    /// polarity : str  ("+" or "-")
    /// retention_time : float  (minutes)
    /// filter_string : str | None
    /// total_ion_current : float
    /// base_peak_mz : float
    /// base_peak_intensity : float
    /// low_mz : float
    /// high_mz : float
    /// ion_injection_time_ms : float | None
    /// charge : int | None
    /// precursor_mz : float | None
    /// isolation_width : float | None
    /// collision_energy : float | None
    /// mz : numpy.ndarray[float64]
    /// intensity : numpy.ndarray[float32]
    fn scan<'py>(&self, py: Python<'py>, scan_number: u32) -> PyResult<Bound<'py, PyDict>> {
        let first = self.first_scan();
        let idx = scan_number.checked_sub(first).ok_or_else(|| {
            PyIndexError::new_err(format!("scan {scan_number} < first scan {first}"))
        })? as usize;
        if idx >= self.reader.scan_index.len() {
            return Err(PyIndexError::new_err(format!(
                "scan {scan_number} out of range"
            )));
        }
        let entry = &self.reader.scan_index[idx];
        let event = self.reader.scan_events.get(idx);
        let params = self.reader.scan_params(scan_number);

        let ms_level = event
            .and_then(|e| e.preamble.ms_power())
            .map(|p| match p {
                MsPower::Ms1 | MsPower::Undefined => 1u32,
                MsPower::Ms2 => 2,
                MsPower::Ms3 => 3,
                MsPower::Ms4 => 4,
                MsPower::Ms5 => 5,
                MsPower::Ms6 => 6,
                MsPower::Ms7 => 7,
                MsPower::Ms8 => 8,
            })
            .unwrap_or(1);
        let polarity = match event.and_then(|e| e.preamble.polarity()) {
            Some(Polarity::Positive) => "+",
            Some(Polarity::Negative) => "-",
            _ => "",
        };

        let (mz, intensity) = self.peaks(py, scan_number)?;

        let d = PyDict::new_bound(py);
        d.set_item("scan_number", scan_number)?;
        d.set_item("ms_level", ms_level)?;
        d.set_item("polarity", polarity)?;
        d.set_item("retention_time", entry.start_time)?;
        d.set_item("filter_string", self.reader.scan_filter(scan_number))?;
        d.set_item("total_ion_current", entry.total_current)?;
        d.set_item("base_peak_mz", entry.base_mz)?;
        d.set_item("base_peak_intensity", entry.base_intensity)?;
        d.set_item("low_mz", entry.low_mz)?;
        d.set_item("high_mz", entry.high_mz)?;
        d.set_item(
            "ion_injection_time_ms",
            params.as_ref().and_then(|p| p.ion_injection_time_ms()),
        )?;
        d.set_item(
            "charge",
            params
                .as_ref()
                .and_then(|p| p.charge_state())
                .filter(|&z| z > 0),
        )?;
        d.set_item(
            "precursor_mz",
            params
                .as_ref()
                .and_then(|p| p.monoisotopic_mz())
                .filter(|&v| v > 0.0),
        )?;
        d.set_item(
            "isolation_width",
            params.as_ref().and_then(|p| p.isolation_width_mz()),
        )?;
        d.set_item(
            "collision_energy",
            params.as_ref().and_then(|p| p.activation_energy()),
        )?;
        d.set_item("mz", mz)?;
        d.set_item("intensity", intensity)?;
        Ok(d)
    }

    /// Iterate all scans. Yields dicts identical in shape to :meth:`scan`.
    ///
    /// Equivalent to ``(raw.scan(n) for n in range(raw.first_scan, raw.last_scan+1))``
    /// but avoids Python-level arithmetic in the hot loop.
    fn iter_scans<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let first = self.first_scan();
        let n = self.num_scans();
        let list = PyList::empty_bound(py);
        for i in 0..n {
            let d = self.scan(py, first + i)?;
            list.append(d)?;
        }
        Ok(list)
    }

    /// Write the entire file out as mzML 1.1.0 to `out_path`.
    fn to_mzml(&self, out_path: &str) -> PyResult<()> {
        let out_file = File::create(out_path).map_err(|e| PyIOError::new_err(e.to_string()))?;
        let mut out = std::io::BufWriter::new(out_file);
        let mut src = self
            .source
            .lock()
            .map_err(|_| PyIOError::new_err("internal error: source mutex poisoned"))?;
        let raw_filename = self
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PyValueError::new_err("non-UTF8 file name"))?;
        tfraw::write_mzml(&self.reader, &mut *src, &mut out, raw_filename, false)
            .map_err(to_py_err)?;
        Ok(())
    }
}

/// OpenTFRaw - Rust Thermo `.raw` file parser.
#[pymodule]
fn opentfraw(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RawFile>()?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
