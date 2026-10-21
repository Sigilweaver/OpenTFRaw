//! Source fragmentation settings from the embedded, public-file method XML.
//!
//! IDs remain one-based, as written in the XML and the per-scan trailer.
//! No setting is assigned to a scan without explicit matching trailer IDs.
use std::collections::{BTreeMap, BTreeSet};

use crate::compound_file::CompoundFile;

pub(crate) type SourceCidSettings = BTreeMap<(u16, u16), f64>;

/// The method XML as the embedded method file stores it.
///
/// The method file is a compound file, and the XML is a stream named `Data`
/// in it. `window` is the metadata region that holds the method file. The
/// result is `None` when the region has no readable compound file, and when
/// the number of `Data` streams that hold such a document is not exactly one.
pub(crate) fn method_xml(window: &[u8]) -> Option<String> {
    let file = CompoundFile::find(window)?;
    let mut documents = file
        .streams()
        .iter()
        .filter(|stream| stream.name == "Data")
        .filter_map(|stream| utf16le_text(&file.read(stream)?))
        .filter(|text| text.contains("<InstrumentSetupMethod"));
    let document = documents.next()?;
    documents.next().is_none().then_some(document)
}

fn utf16le_text(bytes: &[u8]) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|unit| u16::from_le_bytes(*unit))
        .collect();
    let text = String::from_utf16(&units).ok()?;
    // A byte order mark is not part of the document.
    Some(match text.strip_prefix('\u{feff}') {
        Some(document) => document.to_owned(),
        None => text,
    })
}

pub(crate) fn source_cid_settings(text: &str) -> SourceCidSettings {
    parse_settings(text).unwrap_or_default()
}

fn parse_settings(text: &str) -> Option<SourceCidSettings> {
    let doc = roxmltree::Document::parse(text).ok()?;
    let root = doc.root_element();
    if !root.has_tag_name("InstrumentSetupMethod") {
        return None;
    }
    // The observed method schema has no XML namespace. Unsupported schemas
    // must not be matched using local names alone.
    if root
        .descendants()
        .any(|n| n.is_element() && n.tag_name().namespace().is_some())
    {
        return None;
    }
    let mut out = BTreeMap::new();
    let mut seen_segments = BTreeSet::new();
    let mut seen_events = BTreeSet::new();
    for segments in root.children().filter(|n| n.has_tag_name("Segments")) {
        for segment in segments.children().filter(|n| n.has_tag_name("Segment")) {
            let segment_id = positive_id(segment.attribute("id")?)?;
            if !seen_segments.insert(segment_id) {
                return None;
            }
            for event in segment.children().filter(|n| n.has_tag_name("ScanEvent")) {
                let key = (segment_id, positive_id(event.attribute("id")?)?);
                if !seen_events.insert(key) {
                    return None;
                }
                let mut energies = event
                    .children()
                    .filter(|n| n.has_tag_name("Fragmentation_Source"));
                let Some(energy) = energies.next() else {
                    continue;
                };
                if energies.next().is_some() {
                    return None;
                }
                if energy.attribute("unit") != Some("eV") {
                    continue;
                }
                // A scalar setting must not contain nested XML values.
                if energy.children().any(|n| n.is_element()) {
                    continue;
                }
                if let Some(value) = energy.text().and_then(|v| v.trim().parse::<f64>().ok()) {
                    if value.is_finite() && value >= 0.0 {
                        out.insert(key, value);
                    }
                }
            }
        }
    }
    Some(out)
}

fn positive_id(value: &str) -> Option<u16> {
    value.parse().ok().filter(|&v| v > 0)
}

/// Only explicit numeric trailer IDs establish the link to a method event.
pub(crate) fn trailer_id(record: &crate::generic_data::GenericRecord, key: &str) -> Option<u16> {
    match record.get(key)? {
        crate::generic_data::GenericValue::Int32(value) => {
            u16::try_from(*value).ok().filter(|&v| v > 0)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generic_data::{GenericRecord, GenericValue};
    use crate::test_util::{fragmented_method_file, method_document, utf16, CompoundFileBuilder};

    /// A compound file with the given streams, all below the mini stream
    /// cutoff.
    fn method_file(streams: &[(&str, &[u8])]) -> Vec<u8> {
        let mut file = CompoundFileBuilder::new(3);
        for _ in 0..(streams.len() + 1).div_ceil(4) {
            file.directory_sector();
        }
        for (name, bytes) in streams {
            file.mini_stream(name, bytes);
        }
        file.build()
    }

    #[test]
    fn method_xml_is_the_data_stream_at_its_recorded_length() {
        // One code unit past nine sectors: the closing `>` is alone in the
        // sector that lies out of line, and stale text follows it there.
        let document = method_document(&["20", "0"], 9 * 256 + 1);
        let file = fragmented_method_file(&document, &utf16("thod>"));
        assert_eq!(method_xml(&file).as_deref(), Some(document.as_str()));
        assert_eq!(
            source_cid_settings(&method_xml(&file).unwrap()),
            BTreeMap::from([((1, 1), 20.0), ((1, 2), 0.0)])
        );

        // The same holds wherever the last sector is cut.
        for units in [8 * 256 + 255, 9 * 256, 9 * 256 + 2, 12 * 256 + 77] {
            let document = method_document(&["20"], units);
            let file = fragmented_method_file(&document, &utf16(">>"));
            assert_eq!(
                method_xml(&file).as_deref(),
                Some(document.as_str()),
                "{units}"
            );
        }
    }

    #[test]
    fn method_xml_reads_a_document_below_the_mini_stream_cutoff() {
        let document = method_document(&["20"], 400);
        let file = method_file(&[
            ("Text", &utf16("Method of the instrument")),
            ("Data", &utf16(&document)),
        ]);
        assert_eq!(method_xml(&file).as_deref(), Some(document.as_str()));
    }

    #[test]
    fn method_xml_drops_a_byte_order_mark() {
        let document = method_document(&["20"], 400);
        let file = method_file(&[("Data", &utf16(&format!("\u{feff}{document}")))]);
        assert_eq!(method_xml(&file).as_deref(), Some(document.as_str()));
    }

    #[test]
    fn method_xml_needs_exactly_one_document() {
        let document = utf16(&method_document(&["20"], 400));
        let other = utf16(&method_document(&["30"], 400));

        // Two `Data` streams hold a method document: neither is chosen.
        let file = method_file(&[("Data", &document), ("Data", &other)]);
        assert_eq!(method_xml(&file), None);

        // `Data` streams that hold something else do not count against it:
        // bytes that are not UTF-16, another document, an odd length.
        let file = method_file(&[
            ("Data", &[0x00, 0xD8, 0x00, 0xD8]),
            ("Data", &utf16("<Method><Pump/></Method>")),
            ("Data", &document[..7]),
            ("Data", &document),
        ]);
        assert!(method_xml(&file).is_some());

        // The document is only taken from a stream named `Data`.
        let file = method_file(&[("Text", &document), ("Data", &utf16("no document"))]);
        assert_eq!(method_xml(&file), None);

        // No compound file, and one that is cut short.
        assert_eq!(method_xml(&document), None);
        let complete = fragmented_method_file(&method_document(&["20"], 9 * 256 + 1), &[]);
        assert!(method_xml(&complete).is_some());
        assert_eq!(method_xml(&complete[..complete.len() - 3 * 512]), None);
    }

    fn method(events: &str) -> String {
        format!("<InstrumentSetupMethod><Segments>{events}</Segments></InstrumentSetupMethod>")
    }

    #[test]
    fn links_events_within_their_segments() {
        let text = method(
            r#"<Segment id="1"><ScanEvent id="1"><Fragmentation_Source unit="eV">20</Fragmentation_Source></ScanEvent><ScanEvent id="2"><Fragmentation_Source unit="eV">0</Fragmentation_Source></ScanEvent></Segment><Segment id="2"><ScanEvent id="1"><Fragmentation_Source unit="eV">35.5</Fragmentation_Source></ScanEvent></Segment>"#,
        );
        assert_eq!(
            source_cid_settings(&text),
            BTreeMap::from([((1, 1), 20.0), ((1, 2), 0.0), ((2, 1), 35.5)])
        );
    }

    #[test]
    fn namespace_is_not_silently_treated_as_supported_method_xml() {
        let text = r#"<InstrumentSetupMethod xmlns="urn:unknown"><Segments><Segment id="1"><ScanEvent id="1"><Fragmentation_Source unit="eV">20</Fragmentation_Source></ScanEvent></Segment></Segments></InstrumentSetupMethod>"#;
        assert!(source_cid_settings(text).is_empty());
    }

    #[test]
    fn invalid_energy_is_not_a_source_setting() {
        for value in ["NaN", "inf", "-1", "", "twenty", "<Nested>20</Nested>"] {
            let text = method(&format!(
                r#"<Segment id="1"><ScanEvent id="1"><Fragmentation_Source unit="eV">{value}</Fragmentation_Source></ScanEvent></Segment>"#
            ));
            assert!(source_cid_settings(&text).is_empty(), "{value}");
        }
        for unit in ["%", "V", "ev", ""] {
            let text = method(&format!(
                r#"<Segment id="1"><ScanEvent id="1"><Fragmentation_Source unit="{unit}">20</Fragmentation_Source></ScanEvent></Segment>"#
            ));
            assert!(source_cid_settings(&text).is_empty(), "{unit}");
        }
    }

    #[test]
    fn rejects_ambiguous_invalid_and_malformed_methods() {
        for text in [
            method(
                r#"<Segment id="1"><ScanEvent id="1"><Fragmentation_Source unit="eV">20</Fragmentation_Source><Fragmentation_Source unit="eV">30</Fragmentation_Source></ScanEvent></Segment>"#,
            ),
            method(
                r#"<Segment id="1"><ScanEvent id="1"><Fragmentation_Source unit="eV">20</Fragmentation_Source></ScanEvent><ScanEvent id="1"/></Segment>"#,
            ),
            method(
                r#"<Segment id="1"><ScanEvent id="1"><Fragmentation_Source unit="eV">20</Fragmentation_Source></ScanEvent></Segment><Segment id="1"/>"#,
            ),
            method(
                r#"<Segment id="0"><ScanEvent id="1"><Fragmentation_Source unit="eV">20</Fragmentation_Source></ScanEvent></Segment>"#,
            ),
            method(
                r#"<Segment><ScanEvent id="1"><Fragmentation_Source unit="eV">20</Fragmentation_Source></ScanEvent></Segment>"#,
            ),
            "<InstrumentSetupMethod><Segments>".into(),
            "<!DOCTYPE x [<!ENTITY energy '20'>]><InstrumentSetupMethod/>".into(),
        ] {
            assert!(source_cid_settings(&text).is_empty(), "{text}");
        }
    }

    #[test]
    fn trailer_identifiers_must_be_positive_typed_integers() {
        for value in [
            GenericValue::Int32(0),
            GenericValue::Int32(-1),
            GenericValue::Int32(65536),
            GenericValue::String("1".into()),
            GenericValue::Float64(1.0),
        ] {
            let record = GenericRecord {
                values: vec![("Scan Event:".into(), value)],
            };
            assert_eq!(trailer_id(&record, "Scan Event:"), None);
        }
        let record = GenericRecord {
            values: vec![("Scan Event:".into(), GenericValue::Int32(1))],
        };
        assert_eq!(trailer_id(&record, "Scan Event:"), Some(1));
        assert_eq!(trailer_id(&record, "Scan Segment:"), None);
    }
}
