use super::*;

#[test]
fn uuid_parse_and_format_roundtrip() {
    // FPC `GuidToString` form: braced, uppercase hex.
    let s = "{00000000-0000-4000-8000-0000000000A1}";
    let u = Uuid::parse(s).expect("valid braced uuid");
    assert_eq!(u.to_dss_string(), s);
    // Bare (brace-less) form parses too — `DoUuidsCmd` wraps before parsing,
    // but the parser itself is tolerant like `TryStringToGUID`.
    let bare = Uuid::parse("00000000-0000-4000-8000-0000000000a1").unwrap();
    assert_eq!(bare, u);
    assert!(Uuid::parse("not-a-uuid").is_none());
}

#[test]
fn create_v4_sets_version_and_variant() {
    // Pascal `CreateUUID4` places a 4 at character 13 and 8/9/A/B at 17.
    let s = Uuid::create_v4().to_dss_string();
    let b: Vec<char> = s.chars().collect();
    assert_eq!(b[15], '4', "version nibble in {s}");
    assert!(matches!(b[20], '8' | '9' | 'A' | 'B'), "variant in {s}");
}

#[test]
fn hashed_list_add_get_persist_reset() {
    let mut cim = CimExporter::default();
    assert!(!cim.is_started());
    cim.start_uuid_list(8);
    assert!(cim.is_started());

    // AddHashedUuid inserts; GetHashedUuid finds it (case-insensitive key).
    cim.add_hashed_uuid(
        "Station=Station=1",
        "{00000000-0000-4000-8000-0000000000D1}",
    )
    .unwrap();
    let u = cim.get_hashed_uuid("station=station=1");
    assert_eq!(u.to_dss_string(), "{00000000-0000-4000-8000-0000000000D1}");
    // GetDevUuid builds the exact `'Station=' + name + '=' + seq` key.
    let d = cim.get_dev_uuid(UuidChoice::Station, "Station", 1);
    assert_eq!(d, u);

    // A missing key is created (random v4) and persists.
    let g1 = cim.get_hashed_uuid("GeoRgn=GeoRgn=1");
    let g2 = cim.get_dev_uuid(UuidChoice::GeoRgn, "GeoRgn", 1);
    assert_eq!(g1, g2);

    // AddHashedUuid overwrites an existing key.
    cim.add_hashed_uuid(
        "Station=Station=1",
        "{11111111-2222-4333-8444-555555555555}",
    )
    .unwrap();
    assert_eq!(
        cim.get_hashed_uuid("Station=Station=1").to_dss_string(),
        "{11111111-2222-4333-8444-555555555555}"
    );

    // WriteHashedUUIDs prints original-case keys in insertion order.
    let mut out = String::new();
    cim.write_hashed_uuids(&mut out);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("Station=Station=1 {11111111-"));
    assert!(lines[1].starts_with("GeoRgn=GeoRgn=1 {"));

    // StartUuidList resets; FreeUuidList unassigns.
    cim.start_uuid_list(4);
    let mut out = String::new();
    cim.write_hashed_uuids(&mut out);
    assert!(out.is_empty());
    cim.free_uuid_list();
    assert!(!cim.is_started());
}

#[test]
fn add_hashed_uuid_rejects_garbage() {
    let mut cim = CimExporter::default();
    cim.start_uuid_list(2);
    // FPC SysUtils `EConvertError` text (no trailing period; "GUID", not
    // "UUID") — `DoUuidsCmd` embeds it verbatim in the error-303 report.
    assert_eq!(
        cim.add_hashed_uuid("k=1", "{zz}").unwrap_err(),
        "\"{zz}\" is not a valid GUID value"
    );
    // The failed add must mutate nothing: the key is not in the list.
    let mut out = String::new();
    cim.write_hashed_uuids(&mut out);
    assert!(out.is_empty(), "failed add left state behind: {out}");
}
