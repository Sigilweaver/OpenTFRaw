/// Lightweight probe: opens a RAW file and prints header info + first scan events
/// without reading any scan peak data. Fast even on 600MB+ files.
use opentfraw::RawFileReader;
use std::process;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: probe <file.raw>");
        process::exit(1);
    });

    match RawFileReader::open_path(&path) {
        Ok(raw) => {
            println!(
                "OK  v{}  {}  scans={}  events={}",
                raw.version,
                raw.instrument_model
                    .as_deref()
                    .unwrap_or("unknown"),
                raw.scan_index.len(),
                raw.scan_events.len(),
            );
            // Print first 3 scan events
            for (i, ev) in raw.scan_events.iter().take(3).enumerate() {
                println!(
                    "  Event {}: ms_power={:?} dependent={} range=[{:.1}-{:.1}]",
                    i,
                    ev.preamble.ms_power(),
                    ev.preamble.is_dependent(),
                    ev.fraction_collectors
                        .first()
                        .map(|fc| fc.low_mz)
                        .unwrap_or(0.0),
                    ev.fraction_collectors
                        .first()
                        .map(|fc| fc.high_mz)
                        .unwrap_or(0.0),
                );
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    }
}
