//! Fuzz target covering `RawFileReader::open` followed by
//! `RawFileReader::read_scan_peaks`, the other entry point that takes fully
//! adversarial input: once a file parses far enough to open, every declared
//! scan is read back through whichever scan-data decoder the file format
//! dispatches to (PacketHeader / FlatV63 / FlatV66).
//!
//! Crash/panic/hang freedom and internal invariants only - see `open.rs` and
//! this project's CONTRIBUTING.md clean-room policy.

#![no_main]

use libfuzzer_sys::fuzz_target;
use opentfraw::RawFileReader;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let Ok(raw) = RawFileReader::open(Cursor::new(data)) else {
        return;
    };
    let mut source = Cursor::new(data);
    let first = raw.run_header.sample_info.first_scan_number;
    // Cap the number of scans probed per execution: a legitimately large
    // (but now size-checked) file could still declare thousands of scans,
    // and libFuzzer needs each execution to stay fast. Also probe one scan
    // number past the declared range to exercise the out-of-range path.
    let probe_count = raw.num_scans.min(64);
    for i in 0..=probe_count {
        let scan_number = first.wrapping_add(i);
        let _ = raw.read_scan_peaks(&mut source, scan_number);
    }
});
