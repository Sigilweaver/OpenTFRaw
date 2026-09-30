//! Source fragmentation settings from the embedded, public-file method XML.
//!
//! IDs remain one-based, as written in the XML and the per-scan trailer.
//! No setting is assigned to a scan without explicit matching trailer IDs.
use std::collections::{BTreeMap, BTreeSet};

pub(crate) type SourceCidSettings = BTreeMap<(u16, u16), f64>;

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
