//! Fuzz target for `RawFileReader::open`, the parser's main entry point for
//! fully-adversarial input (an arbitrary file on disk).
//!
//! This only exercises crash/panic/hang freedom and internal invariants
//! (bounds checks, no unbounded allocation) - per this project's clean-room
//! policy, it never compares decoded output to any vendor tool.

#![no_main]

use libfuzzer_sys::fuzz_target;
use opentfraw::RawFileReader;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let _ = RawFileReader::open(Cursor::new(data));
});
