//! Convert a Thermo RAW file to mzML.
//!
//! Usage: to_mzml [--indexed] [--include-profile] <input.raw> [output.mzML]
//!
//! If no output path is given, writes to stdout.
//! Pass --indexed to produce an indexed mzML file with spectrum byte offsets
//! and a SHA-1 file checksum.
//! Pass --include-profile to export raw profile signal for profile-mode scans
//! (e.g. Orbitrap MS1) instead of the centroid peak list.

use opentfraw::{write_indexed_mzml, write_mzml, RawFileReader};
use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
};

fn main() {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let indexed = raw_args.iter().any(|a| a == "--indexed");
    let include_profile = raw_args.iter().any(|a| a == "--include-profile");
    let positional: Vec<&String> = raw_args
        .iter()
        .filter(|a| *a != "--indexed" && *a != "--include-profile")
        .collect();

    if positional.is_empty() {
        eprintln!("Usage: to_mzml [--indexed] [--include-profile] <input.raw> [output.mzML]");
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

    if let Some(&out_path) = positional.get(1) {
        let out_file = match File::create(out_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error creating {out_path}: {e}");
                std::process::exit(1);
            }
        };
        let mut out = BufWriter::new(out_file);
        let t0 = std::time::Instant::now();
        let result = if indexed {
            write_indexed_mzml(&raw, &mut source, &mut out, raw_filename, include_profile)
        } else {
            write_mzml(&raw, &mut source, &mut out, raw_filename, include_profile)
        };
        match result {
            Ok(()) => {
                let mut flags = Vec::new();
                if indexed { flags.push("indexed"); }
                if include_profile { flags.push("with profile"); }
                let suffix = if flags.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", flags.join(", "))
                };
                eprintln!(
                    "Written {} spectra to {out_path} in {:.1}s{}",
                    raw.num_scans,
                    t0.elapsed().as_secs_f64(),
                    suffix
                );
            }
            Err(e) => {
                eprintln!("Error writing mzML: {e}");
                std::process::exit(1);
            }
        }
    } else {
        // Write to stdout.
        let stdout = std::io::stdout();
        let mut out = BufWriter::new(stdout.lock());
        let result = if indexed {
            write_indexed_mzml(&raw, &mut source, &mut out, raw_filename, include_profile)
        } else {
            write_mzml(&raw, &mut source, &mut out, raw_filename, include_profile)
        };
        if let Err(e) = result {
            eprintln!("Error writing mzML: {e}");
            std::process::exit(1);
        }
    }
}
