use mame_coalesce::domain::{
    CatalogKey, CatalogScope, DocumentKey, ParserInterpretationKey, SnapshotKey,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn persisted_snapshot_keys_parse_without_reconstructing_catalog_metadata() -> TestResult {
    let text = format!("sha256:{}", "ab".repeat(32));
    let key = text.parse::<SnapshotKey>()?;
    assert_eq!(key.as_str(), text);
    assert_eq!(key.to_string(), text);
    Ok(())
}

#[test]
fn snapshot_key_parser_rejects_other_namespaces_and_noncanonical_shapes() {
    for text in [
        String::new(),
        "snapshot-name".into(),
        format!("sha1:{}", "ab".repeat(20)),
        format!("sha256:{}", "ab".repeat(31)),
        format!("sha256:{}", "ab".repeat(33)),
        format!("sha256:{}", "AB".repeat(32)),
        format!("sha256:{}", "ag".repeat(32)),
        format!(" sha256:{}", "ab".repeat(32)),
        format!("sha256:{} ", "ab".repeat(32)),
    ] {
        assert!(
            text.parse::<SnapshotKey>().is_err(),
            "accepted noncanonical key {text:?}"
        );
    }
}

#[test]
fn both_native_snapshot_factories_round_trip_through_the_same_key_parser() -> TestResult {
    let catalog = CatalogKey::new("snapshot-key-witness");
    let document = DocumentKey::from_bytes(b"native source bytes");
    let interpretation =
        ParserInterpretationKey::for_format("no-intro-dat-v3-compatible", &CatalogScope::Complete);
    for key in [
        SnapshotKey::new(&catalog, &document, &interpretation),
        SnapshotKey::new_publication(&catalog, &document, &interpretation),
    ] {
        assert_eq!(key.as_str().parse::<SnapshotKey>()?, key);
    }
    Ok(())
}
