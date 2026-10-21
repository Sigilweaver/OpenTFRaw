//! Shared helpers for building hand-crafted binary fixtures in unit tests.
//!
//! These construct byte-for-byte valid (or deliberately broken) encodings of
//! the on-disk structures this crate decodes, without needing real `.raw`
//! files. Per the project's clean-room policy, none of this is derived from
//! vendor sources - it mirrors the encodings already documented and
//! implemented in this crate's own read functions.

#![cfg(test)]

/// Encode `s` as UTF-16LE, zero-padded (or truncated) to exactly `byte_len` bytes.
pub(crate) fn utf16_fixed(s: &str, byte_len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(byte_len);
    for u in s.encode_utf16() {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out.resize(byte_len, 0);
    out
}

/// Encode a PascalStringWin32: UInt32 char count, then that many UTF-16LE code units.
pub(crate) fn pascal_string(s: &str) -> Vec<u8> {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut out = Vec::with_capacity(4 + units.len() * 2);
    out.extend_from_slice(&(units.len() as u32).to_le_bytes());
    for u in units {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out
}

/// Encode a Windows FILETIME for the given Unix-epoch seconds value.
/// Inverse of `BinaryReader::read_windows_filetime`.
pub(crate) fn windows_filetime(unix_seconds: f64) -> u64 {
    if unix_seconds == 0.0 {
        return 0;
    }
    (((unix_seconds + 11_644_473_600.0) * 10_000_000.0) as i64) as u64
}

/// A 112-byte AuditTag blob: FILETIME(8) + tag1 utf16(50) + tag2 utf16(50) + u32(4).
pub(crate) fn audit_tag_bytes(time: f64, tag1: &str, tag2: &str, unknown_long: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(112);
    out.extend_from_slice(&windows_filetime(time).to_le_bytes());
    out.extend_from_slice(&utf16_fixed(tag1, 50));
    out.extend_from_slice(&utf16_fixed(tag2, 50));
    out.extend_from_slice(&unknown_long.to_le_bytes());
    assert_eq!(out.len(), 112);
    out
}

/// A minimal valid 1356-byte FileHeader blob for the given supported version.
pub(crate) fn file_header_bytes(version: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0xa101u16.to_le_bytes()); // magic
    out.extend_from_slice(&utf16_fixed("Finnigan", 18)); // signature
    out.extend_from_slice(&[0u8; 16]); // unknown_long[1..4]
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&audit_tag_bytes(
        0.0,
        "Xcalibur_System",
        "Test Instrument",
        0,
    ));
    out.extend_from_slice(&audit_tag_bytes(
        0.0,
        "Xcalibur_System",
        "Test Instrument",
        0,
    ));
    out.extend_from_slice(&[0u8; 4]); // unknown_long[5]
    out.extend_from_slice(&[0u8; 60]); // unknown_area
    out.extend_from_slice(&utf16_fixed("tag", 1028)); // Tag
    assert_eq!(out.len(), 1356);
    out
}

/// Encode `s` as UTF-16LE.
pub(crate) fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

const CFB_FREE: u32 = 0xFFFF_FFFF;
const CFB_END_OF_CHAIN: u32 = 0xFFFF_FFFE;
const CFB_TABLE_SECTOR: u32 = 0xFFFF_FFFD;
const CFB_DIR_ENTRY_LEN: usize = 128;
const CFB_MINI_SECTOR_LEN: usize = 64;

/// A hand-laid-out compound file for tests that depend on where a stream's
/// sectors lie. It follows the public MS-CFB specification, like the reader
/// in `compound_file.rs`.
///
/// Sectors are numbered in the order the calls add them, so the caller
/// decides what lies next to what. Sector 0 is the allocation table. The mini
/// stream and its table are placed last. Directory entries are linked as one
/// chain of siblings under the root, which a reader can walk; the balanced
/// tree a writer keeps is not reproduced.
pub(crate) struct CompoundFileBuilder {
    version: u16,
    sector_len: usize,
    sectors: Vec<Vec<u8>>,
    fat: Vec<u32>,
    /// Name, first sector and length of each stream, in directory order.
    streams: Vec<(String, u32, u64)>,
    directory: Vec<u32>,
    mini_stream: Vec<u8>,
    mini_fat: Vec<u32>,
}

impl CompoundFileBuilder {
    /// Version 3 has 512-byte sectors and version 4 has 4096-byte sectors.
    pub(crate) fn new(version: u16) -> Self {
        Self {
            version,
            sector_len: if version == 3 { 512 } else { 4096 },
            sectors: vec![Vec::new()],
            fat: vec![CFB_TABLE_SECTOR],
            streams: Vec::new(),
            directory: Vec::new(),
            mini_stream: Vec::new(),
            mini_fat: Vec::new(),
        }
    }

    /// Append `bytes` as adjacent sectors chained in order and return their
    /// numbers. `slack` is written after the last byte, as far as it fits in
    /// the last sector, and zeros fill the rest.
    pub(crate) fn sectors(&mut self, bytes: &[u8], slack: &[u8]) -> Vec<u32> {
        assert!(!bytes.is_empty());
        let first = self.sectors.len();
        for chunk in bytes.chunks(self.sector_len) {
            let mut sector = chunk.to_vec();
            let room = self.sector_len - sector.len();
            sector.extend_from_slice(&slack[..slack.len().min(room)]);
            sector.resize(self.sector_len, 0);
            self.fat.push(self.sectors.len() as u32 + 1);
            self.sectors.push(sector);
        }
        let last = self.sectors.len() - 1;
        self.fat[last] = CFB_END_OF_CHAIN;
        (first as u32..=last as u32).collect()
    }

    /// Make sector `to` follow sector `from` in a chain.
    pub(crate) fn link(&mut self, from: u32, to: u32) {
        self.fat[from as usize] = to;
    }

    /// Reserve the next sector for the directory.
    pub(crate) fn directory_sector(&mut self) -> u32 {
        let number = self.sectors.len() as u32;
        self.sectors.push(Vec::new());
        self.fat.push(CFB_END_OF_CHAIN);
        self.directory.push(number);
        number
    }

    /// Add the directory entry of a stream that starts at sector `start`.
    pub(crate) fn stream(&mut self, name: &str, start: u32, len: u64) {
        self.streams.push((name.to_owned(), start, len));
    }

    /// Add a stream shorter than the 4096-byte cutoff, which is stored in the
    /// mini stream.
    pub(crate) fn mini_stream(&mut self, name: &str, bytes: &[u8]) {
        assert!(!bytes.is_empty() && bytes.len() < 4096);
        let first = self.mini_fat.len() as u32;
        for chunk in bytes.chunks(CFB_MINI_SECTOR_LEN) {
            self.mini_stream.extend_from_slice(chunk);
            let len = self.mini_stream.len().next_multiple_of(CFB_MINI_SECTOR_LEN);
            self.mini_stream.resize(len, 0);
            self.mini_fat.push(self.mini_fat.len() as u32 + 1);
        }
        *self.mini_fat.last_mut().expect("a mini sector") = CFB_END_OF_CHAIN;
        self.stream(name, first, bytes.len() as u64);
    }

    pub(crate) fn build(mut self) -> Vec<u8> {
        let (mini_stream_start, mini_fat_start) = if self.mini_stream.is_empty() {
            (CFB_END_OF_CHAIN, CFB_END_OF_CHAIN)
        } else {
            let stream = self.mini_stream.clone();
            let mut table: Vec<u8> = self.mini_fat.iter().flat_map(|n| n.to_le_bytes()).collect();
            table.resize(table.len().next_multiple_of(self.sector_len), 0xFF);
            (self.sectors(&stream, &[])[0], self.sectors(&table, &[])[0])
        };

        // The root entry comes first and owns the mini stream.
        let no_entry = CFB_FREE;
        let first_child = if self.streams.is_empty() { no_entry } else { 1 };
        let mut directory = cfb_dir_entry(
            "Root Entry",
            5,
            no_entry,
            first_child,
            mini_stream_start,
            self.mini_stream.len() as u64,
        );
        for (index, (name, start, len)) in self.streams.iter().enumerate() {
            let next = if index + 1 < self.streams.len() {
                index as u32 + 2
            } else {
                no_entry
            };
            directory.extend(cfb_dir_entry(name, 2, next, no_entry, *start, *len));
        }
        assert!(directory.len() <= self.directory.len() * self.sector_len);
        directory.resize(self.directory.len() * self.sector_len, 0);
        for (index, &sector) in self.directory.iter().enumerate() {
            let entries = &directory[index * self.sector_len..(index + 1) * self.sector_len];
            self.sectors[sector as usize] = entries.to_vec();
            if let Some(&next) = self.directory.get(index + 1) {
                self.fat[sector as usize] = next;
            }
        }

        assert!(self.fat.len() <= self.sector_len / 4);
        let mut table: Vec<u8> = self.fat.iter().flat_map(|n| n.to_le_bytes()).collect();
        table.resize(self.sector_len, 0xFF);
        self.sectors[0] = table;

        let mut header = vec![0u8; 512];
        header[..8].copy_from_slice(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
        header[0x18..0x1A].copy_from_slice(&0x3Eu16.to_le_bytes());
        header[0x1A..0x1C].copy_from_slice(&self.version.to_le_bytes());
        header[0x1C..0x1E].copy_from_slice(&0xFFFEu16.to_le_bytes());
        let sector_shift: u16 = if self.version == 3 { 9 } else { 12 };
        header[0x1E..0x20].copy_from_slice(&sector_shift.to_le_bytes());
        header[0x20..0x22].copy_from_slice(&6u16.to_le_bytes());
        if self.version == 4 {
            header[0x28..0x2C].copy_from_slice(&(self.directory.len() as u32).to_le_bytes());
        }
        header[0x2C..0x30].copy_from_slice(&1u32.to_le_bytes());
        header[0x30..0x34].copy_from_slice(&self.directory[0].to_le_bytes());
        header[0x38..0x3C].copy_from_slice(&4096u32.to_le_bytes());
        header[0x3C..0x40].copy_from_slice(&mini_fat_start.to_le_bytes());
        let mini_fat_sectors = self.mini_fat.len().div_ceil(self.sector_len / 4) as u32;
        header[0x40..0x44].copy_from_slice(&mini_fat_sectors.to_le_bytes());
        header[0x44..0x48].copy_from_slice(&CFB_END_OF_CHAIN.to_le_bytes());
        // The header lists the allocation-table sectors: sector 0 alone.
        header[0x4C..0x50].copy_from_slice(&0u32.to_le_bytes());
        header[0x50..].fill(0xFF);

        // The header takes the place of one sector.
        header.resize(self.sector_len, 0);
        header.extend(self.sectors.concat());
        header
    }
}

fn cfb_dir_entry(name: &str, kind: u8, right: u32, child: u32, start: u32, len: u64) -> Vec<u8> {
    let mut entry = utf16_fixed(name, 64);
    entry.extend_from_slice(&((name.encode_utf16().count() as u16 + 1) * 2).to_le_bytes());
    entry.push(kind);
    entry.push(1); // black
    entry.extend_from_slice(&CFB_FREE.to_le_bytes()); // no left sibling
    entry.extend_from_slice(&right.to_le_bytes());
    entry.extend_from_slice(&child.to_le_bytes());
    entry.extend_from_slice(&[0u8; 36]); // class id, state bits, timestamps
    entry.extend_from_slice(&start.to_le_bytes());
    entry.extend_from_slice(&len.to_le_bytes());
    assert_eq!(entry.len(), CFB_DIR_ENTRY_LEN);
    entry
}

/// A method document of exactly `units` UTF-16 code units. Segment 1 has one
/// scan event per entry of `energies`, each with that source fragmentation
/// energy in eV.
pub(crate) fn method_document(energies: &[&str], units: usize) -> String {
    let events: String = energies
        .iter()
        .enumerate()
        .map(|(index, energy)| {
            format!(
                "<ScanEvent id=\"{}\"><Fragmentation_Source unit=\"eV\">{energy}\
                 </Fragmentation_Source></ScanEvent>",
                index + 1
            )
        })
        .collect();
    let open = format!(
        "<?xml version=\"1.0\" encoding=\"utf-16\" standalone=\"yes\"?>\r\n\
         <InstrumentSetupMethod><Segments><Segment id=\"1\">{events}</Segment></Segments><!--"
    );
    let close = "--></InstrumentSetupMethod>";
    let padding = units
        .checked_sub(open.len() + close.len())
        .expect("room for the document");
    format!("{open}{}{close}", " ".repeat(padding))
}

/// A method file whose `Data` stream holds `document` and is fragmented the
/// way the file behind issue #57 is: its sectors are adjacent except the
/// last, which lies after a directory sector that begins with the entry
/// `AuditData`. A `Text` stream lies directly before it. `slack` is left
/// after the document's recorded end.
pub(crate) fn fragmented_method_file(document: &str, slack: &[u8]) -> Vec<u8> {
    let mut file = CompoundFileBuilder::new(3);
    file.directory_sector();
    let text = utf16(&"Method of the instrument\r\n".repeat(84));
    let text_sectors = file.sectors(&text, &[]);
    let data = utf16(document);
    let tail_len = match data.len() % 512 {
        0 => 512,
        rest => rest,
    };
    let (head, tail) = data.split_at(data.len() - tail_len);
    let head_sectors = file.sectors(head, &[]);
    file.directory_sector();
    let tail_sectors = file.sectors(tail, slack);
    file.link(head_sectors[head_sectors.len() - 1], tail_sectors[0]);
    // Four entries fill the first directory sector, so `AuditData` is the
    // first entry of the second.
    file.stream("Text", text_sectors[0], text.len() as u64);
    file.stream("Data", head_sectors[0], data.len() as u64);
    file.mini_stream("Header", &[7u8; 100]);
    file.mini_stream("AuditData", &[1, 0, 0, 0]);
    file.build()
}
