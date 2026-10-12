//! Read-only access to the streams of a compound file.
//!
//! The embedded instrument method is a Compound File Binary container, the
//! public format of Microsoft's [MS-CFB] open specification. A stream's bytes
//! lie in fixed-size sectors that need not be adjacent or in order. They are
//! found by following the stream's chain in the file allocation table, for
//! the length its directory entry records. The rest of the last sector is
//! slack and is not part of the stream.
//!
//! Only what a stream read needs is implemented. Every offset is checked
//! against the input and every chain walk is bounded, so malformed input
//! gives `None`, never a panic, a long loop or a large allocation.
//!
//! [MS-CFB]: https://learn.microsoft.com/openspecs/windows_protocols/ms-cfb/

const SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const HEADER_LEN: usize = 512;
const HEADER_TABLE_LIST_OFFSET: usize = 0x4C;
const DIR_ENTRY_LEN: usize = 128;
const MINI_SECTOR_LEN: usize = 64;
/// Streams shorter than this live in the mini stream.
const MINI_STREAM_CUTOFF: u32 = 4096;
/// The largest sector number. Values above it are chain markers.
const MAX_SECTOR: u32 = 0xFFFF_FFFA;
const END_OF_CHAIN: u32 = 0xFFFF_FFFE;
const OBJECT_STREAM: u8 = 2;
const OBJECT_ROOT: u8 = 5;

/// A stream's directory entry: its name, first sector and recorded length.
pub(crate) struct Stream {
    pub(crate) name: String,
    start: u32,
    len: u64,
}

pub(crate) struct CompoundFile<'a> {
    data: &'a [u8],
    sector_len: usize,
    fat: Vec<u32>,
    mini_fat: Vec<u32>,
    /// The root entry's stream, which holds the mini sectors.
    mini_stream: Stream,
    streams: Vec<Stream>,
}

impl<'a> CompoundFile<'a> {
    /// The compound file that starts at the first signature in `window`.
    pub(crate) fn find(window: &'a [u8]) -> Option<Self> {
        let at = window
            .windows(SIGNATURE.len())
            .position(|bytes| bytes == SIGNATURE)?;
        Self::parse(&window[at..])
    }

    fn parse(data: &'a [u8]) -> Option<Self> {
        let header = data.get(..HEADER_LEN)?;
        if header[..SIGNATURE.len()] != SIGNATURE || u16_at(header, 0x1C)? != 0xFFFE {
            return None;
        }
        // The version fixes the sector size: 512 bytes in 3, 4096 in 4.
        let (version, sector_len) = match (u16_at(header, 0x1A)?, u16_at(header, 0x1E)?) {
            (3, 9) => (3, 512),
            (4, 12) => (4, 4096),
            _ => return None,
        };
        // Both are constants of the format.
        if u16_at(header, 0x20)? != 6 || u32_at(header, 0x38)? != MINI_STREAM_CUTOFF {
            return None;
        }
        let mut file = Self {
            data,
            sector_len,
            fat: Vec::new(),
            mini_fat: Vec::new(),
            mini_stream: Stream {
                name: String::new(),
                start: END_OF_CHAIN,
                len: 0,
            },
            streams: Vec::new(),
        };
        file.fat = file.read_fat(header)?;
        file.mini_fat = file.read_mini_fat(u32_at(header, 0x3C)?)?;

        let mut entries: Vec<&[u8; DIR_ENTRY_LEN]> = Vec::new();
        for sector in chain(&file.fat, u32_at(header, 0x30)?)? {
            let sector = file.sector(sector, sector_len)?;
            entries.extend(sector.as_chunks::<DIR_ENTRY_LEN>().0);
        }
        let root = entries.first().filter(|entry| entry[0x42] == OBJECT_ROOT)?;
        file.mini_stream = stream_entry(root, version)?;
        file.streams = entries
            .iter()
            .filter(|entry| entry[0x42] == OBJECT_STREAM)
            .filter_map(|entry| stream_entry(entry, version))
            .collect();
        Some(file)
    }

    /// Every stream in the directory, whichever storage holds it.
    pub(crate) fn streams(&self) -> &[Stream] {
        &self.streams
    }

    /// The bytes of `stream`, exactly as many as its directory entry records.
    pub(crate) fn read(&self, stream: &Stream) -> Option<Vec<u8>> {
        if stream.len >= u64::from(MINI_STREAM_CUTOFF) {
            return self.read_sectors(stream);
        }
        let mini_stream = self.read_sectors(&self.mini_stream)?;
        gather(
            stream.len,
            MINI_SECTOR_LEN,
            &chain(&self.mini_fat, stream.start)?,
            |sector, len| {
                let start = usize::try_from(sector).ok()?.checked_mul(MINI_SECTOR_LEN)?;
                mini_stream.get(start..start.checked_add(len)?)
            },
        )
    }

    fn read_sectors(&self, stream: &Stream) -> Option<Vec<u8>> {
        // A stream is no longer than the file that holds it.
        if stream.len > self.data.len() as u64 {
            return None;
        }
        gather(
            stream.len,
            self.sector_len,
            &chain(&self.fat, stream.start)?,
            |sector, len| self.sector(sector, len),
        )
    }

    /// The first `len` bytes of sector `number`. The header takes the place of
    /// one sector, so sector 0 starts one sector length into the file.
    fn sector(&self, number: u32, len: usize) -> Option<&'a [u8]> {
        let start = usize::try_from(number)
            .ok()?
            .checked_add(1)?
            .checked_mul(self.sector_len)?;
        self.data.get(start..start.checked_add(len)?)
    }

    /// The file allocation table: for each sector, the next one in its chain.
    ///
    /// The header lists up to 109 of the table's own sectors, which cover
    /// 7 MB of 512-byte sectors. A larger file continues the list in further
    /// sectors and is not read: a method file is far smaller than that.
    fn read_fat(&self, header: &[u8]) -> Option<Vec<u32>> {
        if u32_at(header, 0x44)? <= MAX_SECTOR {
            return None;
        }
        let mut fat = Vec::new();
        for sector in u32s(&header[HEADER_TABLE_LIST_OFFSET..]).filter(|&s| s <= MAX_SECTOR) {
            fat.extend(u32s(self.sector(sector, self.sector_len)?));
        }
        Some(fat)
    }

    /// The mini sector allocation table, stored as a chain of whole sectors.
    /// A file with no mini stream has none.
    fn read_mini_fat(&self, start: u32) -> Option<Vec<u32>> {
        let mut table = Vec::new();
        if start > MAX_SECTOR {
            return Some(table);
        }
        for sector in chain(&self.fat, start)? {
            table.extend(u32s(self.sector(sector, self.sector_len)?));
        }
        Some(table)
    }
}

/// The sectors of the chain that begins at `start`, in stream order. A chain
/// longer than its table revisits a sector and is rejected.
fn chain(table: &[u32], start: u32) -> Option<Vec<u32>> {
    let mut sectors = Vec::new();
    let mut next = start;
    while next != END_OF_CHAIN {
        if next > MAX_SECTOR || sectors.len() >= table.len() {
            return None;
        }
        sectors.push(next);
        next = *table.get(usize::try_from(next).ok()?)?;
    }
    Some(sectors)
}

/// The first `len` bytes of a chain of `unit`-sized blocks. The last block is
/// read only as far as `len`, which leaves its slack out.
fn gather<'b>(
    len: u64,
    unit: usize,
    chain: &[u32],
    block: impl Fn(u32, usize) -> Option<&'b [u8]>,
) -> Option<Vec<u8>> {
    // The chain has to hold the whole stream.
    let len = usize::try_from(len)
        .ok()
        .filter(|&len| len <= chain.len().saturating_mul(unit))?;
    let mut bytes = Vec::with_capacity(len);
    for &number in chain {
        let wanted = unit.min(len - bytes.len());
        if wanted == 0 {
            break;
        }
        bytes.extend_from_slice(block(number, wanted)?);
    }
    Some(bytes)
}

fn stream_entry(entry: &[u8; DIR_ENTRY_LEN], version: u16) -> Option<Stream> {
    // The name length counts bytes and includes the terminator.
    let name_len = usize::from(u16_at(entry, 0x40)?);
    if !(2..=64).contains(&name_len) || !name_len.is_multiple_of(2) {
        return None;
    }
    let units: Vec<u16> = entry[..name_len - 2]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|unit| u16::from_le_bytes(*unit))
        .collect();
    let len = u64::from_le_bytes(entry.get(0x78..0x80)?.try_into().ok()?);
    Some(Stream {
        name: String::from_utf16(&units).ok()?,
        start: u32_at(entry, 0x74)?,
        // Version 3 lengths are 32 bits wide, and the specification recommends
        // ignoring the upper half, which older writers left uninitialised.
        len: if version == 3 { len & 0xFFFF_FFFF } else { len },
    })
}

fn u32s(bytes: &[u8]) -> impl Iterator<Item = u32> + '_ {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| u32::from_le_bytes(*word))
}

fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    let word = bytes.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_le_bytes(word.try_into().ok()?))
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    crate::bytes::read_u32_le(bytes, offset).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::CompoundFileBuilder;

    /// Bytes that are not text and do not repeat from sector to sector.
    fn payload(len: usize, seed: usize) -> Vec<u8> {
        (0..len).map(|i| ((i + seed) % 251) as u8).collect()
    }

    fn read(file: &[u8], name: &str) -> Option<Vec<u8>> {
        let file = CompoundFile::find(file)?;
        let stream = file.streams().iter().find(|stream| stream.name == name)?;
        file.read(stream)
    }

    /// One stream of nine sectors and two bytes, in adjacent sectors.
    fn plain_file() -> (Vec<u8>, Vec<u8>, Vec<u32>) {
        let mut file = CompoundFileBuilder::new(3);
        file.directory_sector();
        let bytes = payload(9 * 512 + 2, 0);
        let sectors = file.sectors(&bytes, &[0xAA; 512]);
        file.stream("Data", sectors[0], bytes.len() as u64);
        (file.build(), bytes, sectors)
    }

    /// The offset of sector `number`'s entry in the allocation table, which
    /// the builder puts in sector 0.
    fn fat_entry(number: u32) -> usize {
        512 + 4 * number as usize
    }

    #[test]
    fn follows_a_chain_whose_sectors_are_out_of_order() {
        let mut file = CompoundFileBuilder::new(3);
        file.directory_sector();
        let bytes = payload(9 * 512 + 2, 0);
        let other = payload(4096, 100);
        // On disk: the stream's first two sectors, another stream, then the
        // stream's last four sectors ahead of the four in between.
        let head = file.sectors(&bytes[..2 * 512], &[]);
        let other_sectors = file.sectors(&other, &[]);
        let tail = file.sectors(&bytes[6 * 512..], &[]);
        let middle = file.sectors(&bytes[2 * 512..6 * 512], &[]);
        file.link(head[1], middle[0]);
        file.link(middle[3], tail[0]);
        file.stream("Scattered", head[0], bytes.len() as u64);
        file.stream("Other", other_sectors[0], other.len() as u64);
        let file = file.build();
        assert_eq!(read(&file, "Scattered").unwrap(), bytes);
        assert_eq!(read(&file, "Other").unwrap(), other);
        assert_eq!(read(&file, "Missing"), None);
    }

    #[test]
    fn reads_the_recorded_length_and_leaves_the_slack() {
        for len in [4096, 4097, 9 * 512 - 1, 9 * 512, 9 * 512 + 1] {
            let mut file = CompoundFileBuilder::new(3);
            file.directory_sector();
            let bytes = payload(len, 0);
            let sectors = file.sectors(&bytes, &[0xAA; 512]);
            file.stream("Data", sectors[0], len as u64);
            assert_eq!(read(&file.build(), "Data").unwrap(), bytes, "{len}");
        }
    }

    #[test]
    fn a_chain_longer_than_the_recorded_length_is_read_to_that_length() {
        let (file, bytes, _) = plain_file();
        let mut shorter = file.clone();
        let entry = 2 * 512 + 128;
        shorter[entry + 0x78..entry + 0x80].copy_from_slice(&4100u64.to_le_bytes());
        assert_eq!(read(&shorter, "Data").unwrap(), bytes[..4100]);
    }

    #[test]
    fn a_file_that_ends_with_the_stream_needs_no_slack() {
        let (file, bytes, sectors) = plain_file();
        let end = (sectors[sectors.len() - 1] as usize + 1) * 512 + 2;
        assert_eq!(read(&file[..end], "Data").unwrap(), bytes);
        assert_eq!(read(&file[..end - 1], "Data"), None);
    }

    #[test]
    fn ignores_the_upper_half_of_a_version_3_length() {
        let (mut file, bytes, _) = plain_file();
        let entry = 2 * 512 + 128;
        file[entry + 0x7C..entry + 0x80].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        assert_eq!(read(&file, "Data").unwrap(), bytes);
    }

    #[test]
    fn reads_streams_below_the_cutoff_from_the_mini_stream() {
        let mut file = CompoundFileBuilder::new(3);
        file.directory_sector();
        file.directory_sector();
        let streams = [
            ("One", payload(1, 0)),
            ("Sector", payload(64, 10)),
            ("Most", payload(4095, 20)),
            ("Odd", payload(130, 30)),
        ];
        for (name, bytes) in &streams {
            file.mini_stream(name, bytes);
        }
        let large = payload(4096, 40);
        let sectors = file.sectors(&large, &[]);
        file.stream("Large", sectors[0], large.len() as u64);
        let file = file.build();
        for (name, bytes) in &streams {
            assert_eq!(read(&file, name).unwrap(), *bytes, "{name}");
        }
        assert_eq!(read(&file, "Large").unwrap(), large);
    }

    #[test]
    fn reads_version_4_files_with_4096_byte_sectors() {
        let mut file = CompoundFileBuilder::new(4);
        file.directory_sector();
        let bytes = payload(2 * 4096 + 10, 0);
        let head = file.sectors(&bytes[..4096], &[]);
        let small = payload(70, 50);
        file.mini_stream("Small", &small);
        // A second directory sector lies between the stream's two parts.
        file.directory_sector();
        let tail = file.sectors(&bytes[4096..], &[0xAA; 64]);
        file.link(head[0], tail[0]);
        file.stream("Data", head[0], bytes.len() as u64);
        let file = file.build();
        assert_eq!(read(&file, "Data").unwrap(), bytes);
        assert_eq!(read(&file, "Small").unwrap(), small);
    }

    #[test]
    fn finds_the_file_at_the_first_signature() {
        let (file, bytes, _) = plain_file();
        let mut window = vec![0x5Au8; 1357];
        window.extend(&file);
        window.extend([0x5Au8; 99]);
        assert_eq!(read(&window, "Data").unwrap(), bytes);
        assert!(CompoundFile::find(&window[..1357]).is_none());
        assert!(CompoundFile::find(&window[..1357 + 511]).is_none());
        assert!(CompoundFile::find(&[]).is_none());
    }

    #[test]
    fn rejects_a_header_that_is_not_the_format() {
        let (file, _, _) = plain_file();
        let cases: [(&str, usize, &[u8]); 7] = [
            ("byte order", 0x1C, &[0xFF, 0xFE]),
            ("version", 0x1A, &[5, 0]),
            ("sector size of another version", 0x1E, &[12, 0]),
            ("mini sector size", 0x20, &[7, 0]),
            ("mini stream cutoff", 0x38, &[0, 0x20, 0, 0]),
            ("directory start", 0x30, &[0x40, 0x42, 0x0F, 0]),
            (
                "allocation table continued in further sectors",
                0x44,
                &[3, 0, 0, 0],
            ),
        ];
        for (what, offset, bytes) in cases {
            let mut broken = file.clone();
            broken[offset..offset + bytes.len()].copy_from_slice(bytes);
            assert!(CompoundFile::find(&broken).is_none(), "{what}");
        }
        // The first directory entry is the root.
        let mut broken = file.clone();
        broken[2 * 512 + 0x42] = OBJECT_STREAM;
        assert!(CompoundFile::find(&broken).is_none());
    }

    #[test]
    fn rejects_a_chain_that_does_not_hold_the_stream() {
        let (file, _, sectors) = plain_file();
        let last = sectors[sectors.len() - 1];
        let cases: [(&str, u32, u32); 4] = [
            ("returns to its start", last, sectors[0]),
            ("points at itself", sectors[3], sectors[3]),
            ("runs into a free sector", sectors[3], 0xFFFF_FFFF),
            ("leaves the file", sectors[3], 0x000F_4240),
        ];
        for (what, sector, next) in cases {
            let mut broken = file.clone();
            let at = fat_entry(sector);
            broken[at..at + 4].copy_from_slice(&next.to_le_bytes());
            assert!(CompoundFile::find(&broken).is_some(), "{what}");
            assert_eq!(read(&broken, "Data"), None, "{what}");
        }
        // A chain that ends before the recorded length.
        let mut broken = file.clone();
        let at = fat_entry(sectors[3]);
        broken[at..at + 4].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
        assert_eq!(read(&broken, "Data"), None);
    }

    /// Any bytes may follow a signature. Whatever they are, reading ends
    /// without a panic.
    #[test]
    fn survives_arbitrary_damage() {
        let mut file = CompoundFileBuilder::new(3);
        file.directory_sector();
        let large = payload(4200, 0);
        let sectors = file.sectors(&large, &[]);
        file.stream("Large", sectors[0], large.len() as u64);
        file.mini_stream("Small", &payload(200, 60));
        let file = file.build();

        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let mut random = |bound: usize| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as usize % bound
        };
        for _ in 0..20_000 {
            let mut damaged = file.clone();
            for _ in 0..1 + random(4) {
                // The header, allocation table and directory decide every
                // offset, so most of the damage goes there.
                let at = if random(4) == 0 {
                    random(damaged.len())
                } else {
                    random(3 * 512)
                };
                damaged[at] = match random(4) {
                    0 => 0x00,
                    1 => 0xFF,
                    2 => 0xFE,
                    _ => random(256) as u8,
                };
            }
            damaged.truncate(damaged.len() - random(damaged.len() / 4));
            if let Some(file) = CompoundFile::find(&damaged) {
                for stream in file.streams() {
                    if let Some(bytes) = file.read(stream) {
                        assert_eq!(bytes.len() as u64, stream.len);
                    }
                }
            }
        }
    }
}
