use crate::error::Result;
use crate::reader::BinaryReader;
use std::io::{Read, Seek};

/// The critical preamble containing addresses and acquisition date.
#[derive(Debug)]
pub struct RawFileInfoPreamble {
    pub method_file_present: bool,
    pub year: u16,
    pub month: u16,
    pub day_of_week: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub millisecond: u16,
    pub controller_count: u32,
    /// File offset to the scan data stream.
    pub data_addr: u64,
    /// File offset to the first RunHeader (may be a non-MS controller in multi-controller files).
    pub run_header_addr: u64,
    /// All run header addresses (one per controller). Index 0 == run_header_addr.
    pub run_header_addrs: Vec<u64>,
    /// Second RunHeader address (for multi-controller files).
    pub run_header_addr_2: u64,
}

/// Convert a proleptic Gregorian civil date to days since 1970-01-01.
///
/// Howard Hinnant's `days_from_civil` algorithm
/// (<https://howardhinnant.github.io/date_algorithms.html#days_from_civil>,
/// public domain calendar arithmetic, independent of any vendor source).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (if m > 2 { m - 3 } else { m + 9 }) as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Return the number of days in `month` of `year`, or `None` if either
/// value is out of its valid range (month 1-12).
fn days_in_month(year: u16, month: u16) -> Option<u16> {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 => Some(if leap { 29 } else { 28 }),
        _ => None,
    }
}

/// Inverse of [`days_from_civil`]: convert days since 1970-01-01 to a
/// proleptic Gregorian civil date `(year, month, day)`.
///
/// Also Howard Hinnant's `civil_from_days` algorithm
/// (<https://howardhinnant.github.io/date_algorithms.html#civil_from_days>,
/// public domain calendar arithmetic, independent of any vendor source).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Format a Unix-seconds timestamp (as returned by
/// [`RawFileInfoPreamble::acquisition_date`]) as an RFC 3339 string.
///
/// The trailing `Z` is a formatting convention, not a claim about timezone:
/// the source value is instrument-local wall-clock time with no recorded
/// offset (see [`RawFileInfoPreamble::acquisition_date`]'s doc comment), so
/// this should not be read as a true UTC instant.
fn format_rfc3339(unix_seconds: f64) -> String {
    let total_secs = unix_seconds.floor() as i64;
    let millis = ((unix_seconds - unix_seconds.floor()) * 1000.0).round() as i64;
    let days = total_secs.div_euclid(86_400);
    let sec_of_day = total_secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = sec_of_day / 3600;
    let minute = (sec_of_day % 3600) / 60;
    let second = sec_of_day % 60;
    if millis > 0 {
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
    } else {
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
    }
}

/// RawFileInfo: preamble + label strings + computer name.
#[derive(Debug)]
pub struct RawFileInfo {
    pub preamble: RawFileInfoPreamble,
    pub label_headings: Vec<String>,
    pub computer_name: String,
}

impl RawFileInfo {
    pub(crate) fn read<R: Read + Seek>(r: &mut BinaryReader<R>, version: u32) -> Result<Self> {
        let preamble = RawFileInfoPreamble::read(r, version)?;

        // 5 label heading strings + computer name
        let mut label_headings = Vec::with_capacity(5);
        for _ in 0..5 {
            label_headings.push(r.read_pascal_string()?);
        }
        let computer_name = r.read_pascal_string()?;

        Ok(Self {
            preamble,
            label_headings,
            computer_name,
        })
    }
}

impl RawFileInfoPreamble {
    /// Convert the preamble's year/month/day/hour/minute/second/millisecond
    /// fields to a Unix timestamp in seconds, or `None` if no acquisition
    /// date is present (`year == 0`) or any field is out of its valid range
    /// (month 1-12, day 1-days_in_month, hour 0-23, minute 0-59, second 0-59,
    /// millisecond 0-999).
    ///
    /// Like [`crate::audit_tag::AuditTag::time`], this is the instrument's
    /// local wall-clock time with no timezone: interpreting the result as
    /// UTC reproduces the local wall-clock value rather than a true UTC
    /// instant. This is a different decoded timestamp from that audit-tag
    /// FILETIME (surfaced separately by the Python bindings as
    /// `RawFile.created`); the two are expected to agree since they record
    /// the same acquisition event, but come from independently-decoded
    /// fields.
    pub fn acquisition_date(&self) -> Option<f64> {
        let max_day = days_in_month(self.year, self.month)?;
        if self.year == 0
            || self.day == 0
            || self.day > max_day
            || self.hour > 23
            || self.minute > 59
            || self.second > 59
            || self.millisecond > 999
        {
            return None;
        }
        let days = days_from_civil(self.year as i64, self.month as u32, self.day as u32);
        let secs =
            days * 86_400 + self.hour as i64 * 3600 + self.minute as i64 * 60 + self.second as i64;
        Some(secs as f64 + self.millisecond as f64 / 1000.0)
    }

    /// [`Self::acquisition_date`] formatted as an RFC 3339 string, or `None`
    /// under the same conditions `acquisition_date` returns `None`.
    ///
    /// Per `acquisition_date`'s doc comment, the source value is the
    /// instrument's local wall-clock time with no recorded timezone offset.
    /// The trailing `Z` here is a formatting convention (consistent with how
    /// this crate's other decoded-but-timezone-less timestamp,
    /// [`crate::audit_tag::AuditTag::time`], would need to be handled) and
    /// should not be read as a claim that this is a true UTC instant.
    pub fn acquisition_date_rfc3339(&self) -> Option<String> {
        self.acquisition_date().map(format_rfc3339)
    }

    pub(crate) fn read<R: Read + Seek>(r: &mut BinaryReader<R>, version: u32) -> Result<Self> {
        let method_file_present = r.read_u32()? != 0;
        let year = r.read_u16()?;
        let month = r.read_u16()?;
        let day_of_week = r.read_u16()?;
        let day = r.read_u16()?;
        let hour = r.read_u16()?;
        let minute = r.read_u16()?;
        let second = r.read_u16()?;
        let millisecond = r.read_u16()?;

        if version >= 64 {
            // Version 66 layout
            let _unk2 = r.read_u32()?;
            let _data_addr_32 = r.read_u32()?; // defunct
            let controller_count = r.read_u32()?;
            let _controller_n2 = r.read_u32()?;
            let _unk5 = r.read_u32()?;
            let _unk6 = r.read_u32()?;
            let _run_header_addr_32 = r.read_u32()?; // defunct

            // Skip unknown_area[1]: 760 bytes
            r.skip(760)?;

            // Controller address table. Layout is:
            //   [data_addr: u64] [u32] [u32]           -- scan data entry
            //   [run_hdr[0]: u64] [u32] [u32]          -- controller 0
            //   [run_hdr[1]: u64] [u32] [u32]          -- controller 1
            //   ... (for every controller beyond 2, each entry is 16 bytes
            //        and lives at the start of what used to be "unknown_area[2]")
            //   [padding zeros to fill 1048-byte region]
            //
            // The total region (from data_addr through the skip) is 1048 bytes for
            // a 2-controller file. Each extra controller takes 16 bytes from the skip.
            let data_addr = r.read_u64()?;
            let _unk7 = r.read_u32()?;
            let _unk8 = r.read_u32()?;

            let mut run_header_addrs = Vec::new();
            // Always read at least 2 entries for compatibility
            let n_read = controller_count.max(2) as usize;

            for _ in 0..n_read {
                let addr = r.read_u64()?;
                let _unk_a = r.read_u32()?;
                let _unk_b = r.read_u32()?;
                run_header_addrs.push(addr);
            }

            let run_header_addr = run_header_addrs[0];
            let run_header_addr_2 = run_header_addrs.get(1).copied().unwrap_or(0);

            // Remaining padding to reach end of address-table region.
            // The region size differs by file version:
            //   v64: 1032 bytes (data entry + up to ~2 controller entries + padding)
            //   v66+: 1048 bytes (one extra slot for the additional controller table)
            let used_bytes = 16usize + n_read * 16;
            let total_region = if version >= 66 { 1048usize } else { 1032usize };
            let remaining = total_region.saturating_sub(used_bytes);
            r.skip(remaining)?;

            Ok(Self {
                method_file_present,
                year,
                month,
                day_of_week,
                day,
                hour,
                minute,
                second,
                millisecond,
                controller_count,
                data_addr,
                run_header_addr,
                run_header_addrs,
                run_header_addr_2,
            })
        } else {
            // Pre-v64 (32-bit addresses)
            let _unk2 = r.read_u32()?;
            let data_addr = r.read_u32()? as u64;
            let controller_count = r.read_u32()?;
            let _controller_n2 = r.read_u32()?;
            let _unk5 = r.read_u32()?;
            let _unk6 = r.read_u32()?;
            let run_header_addr = r.read_u32()? as u64;
            let _unk7 = r.read_u32()?;
            let _unk8 = r.read_u32()?;
            let run_header_addr_2 = r.read_u32()? as u64;

            // Skip unknown_area: 744 bytes
            r.skip(744)?;

            Ok(Self {
                method_file_present,
                year,
                month,
                day_of_week,
                day,
                hour,
                minute,
                second,
                millisecond,
                controller_count,
                data_addr,
                run_header_addr,
                run_header_addrs: vec![run_header_addr, run_header_addr_2],
                run_header_addr_2,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preamble(
        year: u16,
        month: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        millisecond: u16,
    ) -> RawFileInfoPreamble {
        RawFileInfoPreamble {
            method_file_present: false,
            year,
            month,
            day_of_week: 0,
            day,
            hour,
            minute,
            second,
            millisecond,
            controller_count: 1,
            data_addr: 0,
            run_header_addr: 0,
            run_header_addrs: Vec::new(),
            run_header_addr_2: 0,
        }
    }

    #[test]
    fn days_from_civil_epoch() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    #[test]
    fn days_from_civil_reference_value() {
        // Reference value from Hinnant's date algorithms writeup.
        assert_eq!(days_from_civil(2000, 3, 1), 11017);
    }

    #[test]
    fn acquisition_date_matches_known_epoch() {
        let p = preamble(1970, 1, 1, 0, 0, 0, 0);
        assert_eq!(p.acquisition_date(), Some(0.0));
    }

    #[test]
    fn acquisition_date_includes_time_of_day_and_millis() {
        let p = preamble(1970, 1, 1, 1, 1, 1, 500);
        assert_eq!(p.acquisition_date(), Some(3661.5));
    }

    #[test]
    fn acquisition_date_none_when_year_zero() {
        let p = preamble(0, 1, 1, 0, 0, 0, 0);
        assert_eq!(p.acquisition_date(), None);
    }

    #[test]
    fn acquisition_date_none_when_month_out_of_range() {
        let p = preamble(2020, 13, 1, 0, 0, 0, 0);
        assert_eq!(p.acquisition_date(), None);
    }

    #[test]
    fn acquisition_date_none_when_day_out_of_range() {
        let p = preamble(2020, 1, 32, 0, 0, 0, 0);
        assert_eq!(p.acquisition_date(), None);
    }

    #[test]
    fn acquisition_date_none_when_day_exceeds_month_max() {
        // February 29 on a non-leap year.
        let p = preamble(2021, 2, 29, 0, 0, 0, 0);
        assert_eq!(p.acquisition_date(), None);
        // February 29 on a leap year is valid.
        let p = preamble(2024, 2, 29, 0, 0, 0, 0);
        assert!(p.acquisition_date().is_some());
        // April 31 (April has 30 days).
        let p = preamble(2020, 4, 31, 0, 0, 0, 0);
        assert_eq!(p.acquisition_date(), None);
    }

    #[test]
    fn acquisition_date_none_when_hour_out_of_range() {
        let p = preamble(2020, 1, 1, 24, 0, 0, 0);
        assert_eq!(p.acquisition_date(), None);
    }

    #[test]
    fn acquisition_date_none_when_minute_out_of_range() {
        let p = preamble(2020, 1, 1, 0, 60, 0, 0);
        assert_eq!(p.acquisition_date(), None);
    }

    #[test]
    fn acquisition_date_none_when_second_out_of_range() {
        let p = preamble(2020, 1, 1, 0, 0, 60, 0);
        assert_eq!(p.acquisition_date(), None);
    }

    #[test]
    fn acquisition_date_none_when_millisecond_out_of_range() {
        let p = preamble(2020, 1, 1, 0, 0, 0, 1000);
        assert_eq!(p.acquisition_date(), None);
    }

    #[test]
    fn civil_from_days_round_trips_days_from_civil() {
        for &(y, m, d) in &[(1970, 1, 1), (2000, 3, 1), (2024, 2, 29), (1999, 12, 31)] {
            let days = days_from_civil(y, m, d);
            assert_eq!(civil_from_days(days), (y, m, d));
        }
    }

    #[test]
    fn acquisition_date_rfc3339_matches_known_epoch() {
        let p = preamble(1970, 1, 1, 0, 0, 0, 0);
        assert_eq!(
            p.acquisition_date_rfc3339(),
            Some("1970-01-01T00:00:00Z".to_string())
        );
    }

    #[test]
    fn acquisition_date_rfc3339_includes_millis_when_present() {
        let p = preamble(2021, 6, 15, 13, 45, 9, 250);
        assert_eq!(
            p.acquisition_date_rfc3339(),
            Some("2021-06-15T13:45:09.250Z".to_string())
        );
    }

    #[test]
    fn acquisition_date_rfc3339_none_when_year_zero() {
        let p = preamble(0, 1, 1, 0, 0, 0, 0);
        assert_eq!(p.acquisition_date_rfc3339(), None);
    }
}
