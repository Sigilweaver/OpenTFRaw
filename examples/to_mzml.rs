//! Convert a Thermo RAW file to mzML.
//!
//! Usage: to_mzml <input.raw> [output.mzML]
//!
//! If no output path is given, writes to stdout.

use opentfraw::{write_mzml, RawFileReader};
use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: to_mzml <input.raw> [output.mzML]");
        std::process::exit(1);
    }
    let raw_path = Path::new(&args[1]);
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

    if let Some(out_path) = args.get(2) {
        let out_file = match File::create(out_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error creating {out_path}: {e}");
                std::process::exit(1);
            }
        };
        let mut out = BufWriter::new(out_file);
        let t0 = std::time::Instant::now();
        match write_mzml(&raw, &mut source, &mut out, raw_filename) {
            Ok(()) => {
                eprintln!(
                    "Written {} spectra to {out_path} in {:.1}s",
                    raw.num_scans,
                    t0.elapsed().as_secs_f64()
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
        if let Err(e) = write_mzml(&raw, &mut source, &mut out, raw_filename) {
            eprintln!("Error writing mzML: {e}");
            std::process::exit(1);
        }
    }
}
