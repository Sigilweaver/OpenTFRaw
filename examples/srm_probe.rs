/// Dump TSQ/SRM scan index entries, scan params, and raw data bytes for Q1 analysis.
///
/// Usage: cargo run --example srm_probe -- <file.raw> [n_scans]
use opentfraw::RawFileReader;
use std::{fs::File, io::{Read, Seek, SeekFrom}, process};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: srm_probe <file.raw> [n_scans]");
        process::exit(1);
    });
    let n = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(9usize);

    let raw = RawFileReader::open_path(&path).unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        process::exit(1);
    });

    println!("version={} model={} scan_format={:?} data_addr=0x{:x}",
        raw.version,
        raw.instrument_model.as_deref().unwrap_or("?"),
        raw.scan_format,
        raw.data_addr,
    );
    println!("RunHeader addresses:");
    println!("  scan_index_addr:   0x{:x}", raw.run_header.scan_index_addr);
    println!("  inst_log_addr:     0x{:x}", raw.run_header.inst_log_addr);
    println!("  error_log_addr:    0x{:x}", raw.run_header.error_log_addr);
    println!("  unk_addr:          0x{:x}", raw.run_header.unk_addr);
    println!("  scan_trailer_addr: 0x{:x}", raw.run_header.scan_trailer_addr);
    println!("  scan_params_addr:  0x{:x}", raw.run_header.scan_params_addr);
    println!("  ntrailer={} nparams={} nsegs={}", raw.run_header.ntrailer, raw.run_header.nparams, raw.run_header.nsegs);    println!("inst_log: {} entries, {} fields", raw.inst_log.len(), raw.inst_log_header.fields.len());
    for f in &raw.inst_log_header.fields {
        println!("  inst_log field: {:?}", f.label);
    }
    if let Some(first_entry) = raw.inst_log.first() {
        println!("  inst_log[0] values:");
        for (label, val) in &first_entry.values {
            println!("    {:?} = {:?}", label, val);
        }
    }

    // Print first 5 scan index entries to verify low_mz/high_mz
    for i in 0..5.min(raw.scan_index.len()) {
        let e = &raw.scan_index[i];
        let q1 = raw.srm_q1_by_event.get(&e.scan_event).copied().unwrap_or(0.0);
        println!("scan_index[{}]: event={} data_size={} low_mz={:.4} high_mz={:.4} Q1={:.4} TIC={:.1}",
            i, e.scan_event, e.data_size, e.low_mz, e.high_mz, q1, e.total_current);
    }
    println!("srm_q1_by_event: {:?}", raw.srm_q1_by_event);

    let mut src = File::open(&path).unwrap();

    for idx in 0..n.min(raw.scan_index.len()) {
        let entry = &raw.scan_index[idx];
        let abs = raw.data_addr + entry.offset;
        println!("\nScan {} (idx={}): scan_event={} data_size={} offset=0x{:x} abs=0x{:x}",
            idx + 1, entry.index, entry.scan_event, entry.data_size, entry.offset, abs);

        // Read the full record
        src.seek(SeekFrom::Start(abs)).unwrap();
        let mut buf = vec![0u8; entry.data_size as usize];
        src.read_exact(&mut buf).unwrap();

        // Decode n_peaks
        let n_peaks = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
        println!("  n_peaks={}", n_peaks);

        // Header bytes 4-31
        print!("  header[4-31]:");
        for i in (4..32).step_by(4) {
            let v = u32::from_le_bytes(buf[i..i+4].try_into().unwrap());
            print!(" {:08x}", v);
        }
        println!();

        // Q3 window table: bytes 32..32+n_peaks*8
        let win_start = 32;
        let win_end = win_start + n_peaks * 8;
        println!("  Q3 windows (lo_mz, hi_mz):");
        for i in 0..n_peaks {
            let lo = f32::from_le_bytes(buf[win_start + i*8..win_start + i*8 + 4].try_into().unwrap());
            let hi = f32::from_le_bytes(buf[win_start + i*8 + 4..win_start + i*8 + 8].try_into().unwrap());
            println!("    channel {}: {:.4} - {:.4}", i, lo, hi);
        }

        // Peak records: bytes win_end..win_end+n_peaks*12
        let pk_start = win_end;
        let pk_end = pk_start + n_peaks * 12;
        println!("  Peak records (ch_idx, mz, intensity):");
        for i in 0..n_peaks {
            let ch = u32::from_le_bytes(buf[pk_start + i*12..pk_start + i*12 + 4].try_into().unwrap());
            let mz = f32::from_le_bytes(buf[pk_start + i*12 + 4..pk_start + i*12 + 8].try_into().unwrap());
            let ab = f32::from_le_bytes(buf[pk_start + i*12 + 8..pk_start + i*12 + 12].try_into().unwrap());
            println!("    peak {}: ch={} mz={:.4} intensity={:.4}", i, ch, mz, ab);
        }

        // Trailing bytes as f32 values
        let trail_start = pk_end;
        println!("  Trailing ({} bytes from byte {}):", buf.len() - trail_start, trail_start);
        for i in (0..buf.len() - trail_start).step_by(4) {
            let v = f32::from_le_bytes(buf[trail_start + i..trail_start + i + 4].try_into().unwrap());
            println!("    [{}] {:?}", trail_start + i, v);
        }

        // Dump scan params
        let scan_number = raw.run_header.sample_info.first_scan_number + idx as u32;
        if let Some(params) = raw.scan_params(scan_number) {
            let rec = params.record();
            println!("  Scan params ({} fields):", rec.values.len());
            for (label, value) in &rec.values {
                println!("    {:?} = {:?}", label, value);
            }
        } else {
            println!("  Scan params: NONE");
        }
    }
}
