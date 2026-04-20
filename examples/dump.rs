use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: dump <file.raw> [--max-scans N]");
        std::process::exit(1);
    }

    let path = &args[1];

    // Parse optional --max-scans flag
    let max_scans: Option<u32> = args
        .windows(2)
        .find(|w| w[0] == "--max-scans")
        .and_then(|w| w[1].parse().ok());
    match opentfraw::RawFileReader::open_path(path) {
        Ok(raw) => {
            println!("=== Thermo RAW File ===");
            println!("Version:  {}", raw.version);
            println!(
                "Instrument: {} ({})",
                raw.instrument_model.unwrap_or("unknown model"),
                raw.device_family.display_name()
            );
            println!(
                "Scans:    {} ({} to {})",
                raw.num_scans,
                raw.run_header.sample_info.first_scan_number,
                raw.run_header.sample_info.last_scan_number,
            );
            println!();

            println!("--- Header ---");
            println!("Signature:    {}", raw.header.signature);
            println!(
                "Audit start:  {} (unix ts: {:.0})",
                raw.header.audit_start.tag1, raw.header.audit_start.time
            );
            println!("Audit tag2:   {}", raw.header.audit_start.tag2);
            println!();

            println!("--- Acquisition Date ---");
            let p = &raw.raw_file_info.preamble;
            println!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
                p.year, p.month, p.day, p.hour, p.minute, p.second, p.millisecond
            );
            println!("Controllers:  {}", p.controller_count);
            println!("Data addr:    {:#x}", p.data_addr);
            println!("RunHdr addr:  {:#x}", p.run_header_addr);
            println!();

            println!("--- Sequence Row ---");
            println!("Comment:      {}", raw.seq_row.comment);
            println!("Inst method:  {}", raw.seq_row.inst_method);
            println!("File name:    {}", raw.seq_row.file_name);
            println!();

            println!("--- RawFileInfo ---");
            println!("Computer:     {}", raw.raw_file_info.computer_name);
            for (i, h) in raw.raw_file_info.label_headings.iter().enumerate() {
                if !h.is_empty() {
                    println!("Label[{}]:     {}", i + 1, h);
                }
            }
            println!();

            println!("--- Sample Info ---");
            let si = &raw.run_header.sample_info;
            println!("M/z range:    {:.2} - {:.2}", si.low_mz, si.high_mz);
            println!(
                "RT range:     {:.2} - {:.2} min",
                si.start_time, si.end_time
            );
            println!("Max TIC:      {:.2e}", si.max_ion_current);
            println!("Error log:    {} entries", si.error_log_length);
            println!("Inst log:     {} entries", si.inst_log_length);
            println!();

            println!("--- Run Header ---");
            let rh = &raw.run_header;
            println!("Scan index:   {:#x}", rh.scan_index_addr);
            println!("Data:         {:#x}", rh.data_addr);
            println!("Trailer:      {:#x}", rh.scan_trailer_addr);
            println!("Params:       {:#x}", rh.scan_params_addr);
            println!("Inst log:     {:#x}", rh.inst_log_addr);
            println!("Error log:    {:#x}", rh.error_log_addr);
            println!("Self addr:    {:#x}", rh.own_addr);
            println!("ntrailer:     {}", rh.ntrailer);
            println!("nparams:      {}", rh.nparams);
            println!("nsegs:        {}", rh.nsegs);
            println!();

            // First few scans
            let n_show = std::cmp::min(5, raw.scan_index.len());
            println!("--- First {} Scan Index Entries ---", n_show);
            for entry in &raw.scan_index[..n_show] {
                println!("  Scan {}: RT={:.4} min, TIC={:.2e}, base={:.2} @ {:.4} m/z, range=[{:.2}-{:.2}], offset={:#x}, size={}",
                    entry.index + 1,
                    entry.start_time,
                    entry.total_current,
                    entry.base_intensity,
                    entry.base_mz,
                    entry.low_mz,
                    entry.high_mz,
                    entry.offset,
                    entry.data_size,
                );
            }
            println!();

            // First few scan events
            let n_events = std::cmp::min(3, raw.scan_events.len());
            println!("--- First {} Scan Events ---", n_events);
            for (i, evt) in raw.scan_events[..n_events].iter().enumerate() {
                let p = &evt.preamble;
                println!("  Event {}: analyzer={:?}, polarity={:?}, mode={:?}, ms_power={:?}, dependent={}, ionization={:?}, activation={:?}",
                    i,
                    p.analyzer(),
                    p.polarity(),
                    p.scan_mode(),
                    p.ms_power(),
                    p.is_dependent(),
                    p.ionization(),
                    p.activation(),
                );
                if !evt.reactions.is_empty() {
                    for rx in &evt.reactions {
                        println!("    Precursor: {:.4} @ {:.1}", rx.precursor_mz, rx.energy);
                    }
                }
                println!("    Coefficients: {} params", evt.coefficients.len());
                for fc in &evt.fraction_collectors {
                    println!("    Range: [{:.2}-{:.2}]", fc.low_mz, fc.high_mz);
                }
            }
            println!();

            // Scan parameters header (trailer extra schema)
            println!(
                "--- Scan Parameters Schema ({} fields) ---",
                raw.scan_parameters_header.fields.len()
            );
            for desc in &raw.scan_parameters_header.fields {
                println!(
                    "  {:?}: \"{}\" (len={})",
                    desc.field_type, desc.label, desc.length
                );
            }
            println!();

            // First scan's parameters
            if let Some(first_params) = raw.scan_parameters.first() {
                println!("--- Scan 1 Parameters ---");
                for (label, value) in &first_params.values {
                    match value {
                        opentfraw::generic_data::GenericValue::Gap => {}
                        _ => println!("  {}: {:?}", label, value),
                    }
                }
            }
            println!();

            // Error log
            if !raw.error_log.is_empty() {
                println!("--- Error Log ({} entries) ---", raw.error_log.len());
                for (i, e) in raw.error_log.iter().enumerate().take(5) {
                    println!("  [{}] RT={:.2}: {}", i, e.time, e.message);
                }
            }

            // Instrument log schema
            println!(
                "--- Instrument Log Schema ({} fields) ---",
                raw.inst_log_header.fields.len()
            );
            for desc in &raw.inst_log_header.fields {
                println!("  {:?}: \"{}\"", desc.field_type, desc.label);
            }
            println!();

            // Scan data validation: read first few scans and cross-check against index
            println!(
                "--- Scan Data Validation (device={}, format={}) ---",
                raw.device_family.display_name(),
                raw.scan_format.display_name()
            );
            let mut file = std::fs::File::open(path).expect("reopen file");
            let first_scan = raw.run_header.sample_info.first_scan_number;
            let cap = max_scans.unwrap_or(5);
            let n_validate = std::cmp::min(cap, raw.num_scans);
            for i in 0..n_validate {
                let scan_num = first_scan + i;
                let idx_entry = &raw.scan_index[i as usize];

                if raw.flat_peaks {
                    // Flat-peak (TSQ/SRM) format — use unified router
                    match raw.read_scan_peaks(&mut file, scan_num) {
                        Ok(peaks) => {
                            let n_peaks = peaks.len();
                            let peak_tic: f64 = peaks.iter().map(|p| p.abundance as f64).sum();
                            let nonzero: Vec<_> =
                                peaks.iter().filter(|p| p.abundance != 0.0).collect();
                            println!("  Scan {} (evt={}): {} peaks ({} nonzero) | peak_tic={:.2e} vs index_tic={:.2e}",
                                scan_num, idx_entry.scan_event, n_peaks, nonzero.len(),
                                peak_tic, idx_entry.total_current);
                            if i == 0 {
                                for (j, pk) in peaks.iter().enumerate().take(5) {
                                    println!(
                                        "    Peak {}: m/z={:.4}, abundance={:.4}",
                                        j, pk.mz, pk.abundance
                                    );
                                }
                                println!(
                                    "    Index base peak: m/z={:.4}, intensity={:.2e}",
                                    idx_entry.base_mz, idx_entry.base_intensity
                                );
                            }
                        }
                        Err(e) => {
                            println!("  Scan {}: ERROR - {}", scan_num, e);
                        }
                    }
                } else {
                    // PacketHeader format
                    match raw.read_scan(&mut file, scan_num) {
                        Ok(pkt) => {
                            let h = &pkt.header;
                            let profile_bins: usize = pkt
                                .profile
                                .as_ref()
                                .map(|p| p.chunks.iter().map(|c| c.signal.len()).sum())
                                .unwrap_or(0);
                            let n_peaks = pkt.peaks.len();

                            // Cross-check: scan index says range [low-high]
                            let range_ok = if n_peaks > 0 {
                                let first_mz = pkt.peaks[0].mz;
                                let last_mz = pkt.peaks[n_peaks - 1].mz;
                                // Peaks should be within the declared range (with some tolerance)
                                first_mz >= idx_entry.low_mz * 0.99
                                    && last_mz <= idx_entry.high_mz * 1.01
                            } else {
                                true
                            };

                            // Compute TIC from centroid peaks and compare
                            let peak_tic: f64 = pkt.peaks.iter().map(|p| p.abundance as f64).sum();

                            // Show scan event info
                            let evt = raw.scan_events.get(i as usize);
                            let mode_str = evt
                                .and_then(|e| e.preamble.scan_mode())
                                .map(|m| format!("{:?}", m))
                                .unwrap_or_else(|| "?".into());

                            println!("  Scan {}: {} | profile={} bins, peaks={}, layout={} | mz=[{:.2}-{:.2}] | range_ok={} | peak_tic={:.2e} vs index_tic={:.2e}",
                            scan_num, mode_str, profile_bins, n_peaks, h.layout,
                            h.low_mz, h.high_mz,
                            range_ok, peak_tic, idx_entry.total_current,
                        );

                            // Show top 3 peaks for first scan
                            if i == 0 && !pkt.peaks.is_empty() {
                                let mut sorted: Vec<_> = pkt.peaks.iter().collect();
                                sorted
                                    .sort_by(|a, b| b.abundance.partial_cmp(&a.abundance).unwrap());
                                let n_top = std::cmp::min(3, sorted.len());
                                for (j, pk) in sorted[..n_top].iter().enumerate() {
                                    println!(
                                        "    Top {}: m/z={:.4}, abundance={:.2e}",
                                        j + 1,
                                        pk.mz,
                                        pk.abundance
                                    );
                                }
                                println!(
                                    "    Index base peak: m/z={:.4}, intensity={:.2e}",
                                    idx_entry.base_mz, idx_entry.base_intensity
                                );
                            }

                            // Show profile info for first scan
                            if i == 0 {
                                if let Some(ref prof) = pkt.profile {
                                    println!("    Profile: first_value={:.6e}, step={:.6e}, {} chunks, {} total bins",
                                    prof.first_value, prof.step, prof.chunks.len(), profile_bins);
                                    // Convert and show top signal
                                    let coeffs: Vec<f64> =
                                        evt.map(|e| e.coefficients.clone()).unwrap_or_default();
                                    if !coeffs.is_empty() {
                                        let mz_int = prof.to_mz_intensity(&coeffs);
                                        if let Some(max_pt) = mz_int
                                            .iter()
                                            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                                        {
                                            println!("    Profile max: m/z={:.4}, intensity={:.2e} (coeffs={})",
                                            max_pt.0, max_pt.1, coeffs.len());
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("  Scan {}: ERROR - {}", scan_num, e);
                        }
                    }
                } // end else (PacketHeader)
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
