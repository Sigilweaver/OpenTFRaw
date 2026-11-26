//! Convert a Thermo RAW file to mzML.
//!
//! Usage: to_mzml [--indexed] [--include-profile] [--skip-unreadable]
//!        [--allow <check>]... <input.raw> [output.mzML]
//!
//! If no output path is given, writes to stdout.
//! Pass --indexed to produce an indexed mzML file with spectrum byte offsets
//! and a SHA-1 file checksum.
//! Pass --include-profile to export raw profile signal for profile-mode scans
//! (e.g. Orbitrap MS1) instead of the centroid peak list.
//! By default a scan whose data cannot be decoded stops the conversion with
//! an error. Pass --skip-unreadable to leave such scans out instead; the
//! number left out is printed.
//! The writer rejects a spectrum whose collision energy has no stated unit
//! (`unknown_collision_energy_unit`), a precursor that references a spectrum
//! not in the output (`dangling_precursor_ref`) and one whose m/z values are
//! not ascending (no check). Pass `--allow <check>` (repeatable) to write
//! spectra of the first two kinds anyway; each is marked with a warning in the
//! mzML and counted in the summary. `unknown_scan_mode` is also accepted; a
//! RAW file always states a scan mode, so it never applies.

use openmassspec_core::{
    write_indexed_mzml_with_options, write_mzml_with_options, Check, Leniency, RecordErrorPolicy,
    WriteOptions, WriteSummary,
};
use opentfraw::{mzml::OpenTfRawSource, RawFileReader};
use std::{
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::Path,
};

const FLAGS: [&str; 3] = ["--indexed", "--include-profile", "--skip-unreadable"];

fn main() {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let indexed = raw_args.iter().any(|a| a == "--indexed");
    let include_profile = raw_args.iter().any(|a| a == "--include-profile");
    let skip_unreadable = raw_args.iter().any(|a| a == "--skip-unreadable");
    let mut leniency = Leniency::strict();
    let mut positional: Vec<&String> = Vec::new();
    let mut args = raw_args.iter();
    while let Some(arg) = args.next() {
        if arg == "--allow" {
            let Some(name) = args.next() else {
                eprintln!("--allow needs a check name");
                std::process::exit(1);
            };
            match name.parse::<Check>() {
                Ok(check) => leniency = leniency.with(check),
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }
        } else if !FLAGS.contains(&arg.as_str()) {
            positional.push(arg);
        }
    }

    if positional.is_empty() {
        eprintln!(
            "Usage: to_mzml [--indexed] [--include-profile] [--skip-unreadable] \
             [--allow <check>]... <input.raw> [output.mzML]"
        );
        std::process::exit(1);
    }
    let raw_path = Path::new(positional[0]);
    let raw_filename = raw_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.raw");

    let raw = match RawFileReader::open_path(raw_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error opening {}: {e}", raw_path.display());
            std::process::exit(1);
        }
    };

    let mut source = match File::open(raw_path) {
        Ok(f) => BufReader::with_capacity(2 << 20, f), // 2 MB buffer
        Err(e) => {
            eprintln!("Error reopening {}: {e}", raw_path.display());
            std::process::exit(1);
        }
    };

    let mut opts = WriteOptions::new().with_leniency(leniency);
    if skip_unreadable {
        opts = opts.with_record_error_policy(RecordErrorPolicy::Skip);
    }
    let mut src = OpenTfRawSource::new(&raw, &mut source, raw_filename, include_profile);
    let mut write = |mut out: &mut dyn Write| -> openmassspec_core::Result<WriteSummary> {
        if indexed {
            write_indexed_mzml_with_options(&mut src, &mut out, &opts)
        } else {
            write_mzml_with_options(&mut src, &mut out, &opts)
        }
    };

    let t0 = std::time::Instant::now();
    let (result, target) = match positional.get(1) {
        Some(&out_path) => {
            let out_file = match File::create(out_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error creating {out_path}: {e}");
                    std::process::exit(1);
                }
            };
            let mut out = BufWriter::new(out_file);
            let result = write(&mut out).and_then(|s| Ok(out.flush().map(|_| s)?));
            (result, out_path.to_string())
        }
        None => {
            let stdout = std::io::stdout();
            let mut out = BufWriter::new(stdout.lock());
            let result = write(&mut out).and_then(|s| Ok(out.flush().map(|_| s)?));
            (result, "stdout".to_string())
        }
    };

    match result {
        Ok(summary) => {
            let mut flags = Vec::new();
            if indexed {
                flags.push("indexed");
            }
            if include_profile {
                flags.push("with profile");
            }
            let suffix = if flags.is_empty() {
                String::new()
            } else {
                format!(" ({})", flags.join(", "))
            };
            eprintln!(
                "Written {} spectra to {target} in {:.1}s{suffix}",
                summary.spectra_written,
                t0.elapsed().as_secs_f64(),
            );
            if summary.skipped() > 0 {
                eprintln!(
                    "warning: {} spectra and {} chromatograms could not be decoded and were left out",
                    summary.spectra_skipped, summary.chromatograms_skipped
                );
            }
            for (code, count) in &summary.warning_counts {
                eprintln!("warning: {count} records written with warning {code}");
            }
        }
        Err(e) => {
            eprintln!("Error writing mzML: {e}");
            if let Some(check) = allow_hint(&e) {
                eprintln!("To write such spectra anyway, pass --allow {check}.");
            }
            std::process::exit(1);
        }
    }
}

/// The `--allow` check that would let the write continue past `error`.
fn allow_hint(error: &openmassspec_core::Error) -> Option<Check> {
    use openmassspec_core::{Error, RecordProblem};
    match error {
        Error::Record { problem, .. } => match problem {
            RecordProblem::UnknownScanMode => Some(Check::UnknownScanMode),
            RecordProblem::CollisionEnergyUnitUnknown => Some(Check::UnknownCollisionEnergyUnit),
            RecordProblem::DanglingPrecursorRef { .. } => Some(Check::DanglingPrecursorRef),
            _ => None,
        },
        Error::SpectrumCountShortfall { .. } => Some(Check::SpectrumCountShortfall),
        _ => None,
    }
}
