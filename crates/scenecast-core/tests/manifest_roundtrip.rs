use scenecast_core::{BundleManifest, HotspotId, SceneId};

#[test]
fn interactive_manifest_fixture_round_trips_and_validates() {
    let manifest: BundleManifest =
        serde_json::from_str(include_str!("fixtures/interactive.scenecast/manifest.json")).unwrap();

    assert!(manifest.validate().is_valid());
    assert_eq!(manifest.title, "Interactive fixture");

    let start = manifest
        .graph
        .scene(&SceneId::new("start").unwrap())
        .unwrap();
    assert!(
        start
            .hotspot(&HotspotId::new("pricing-link").unwrap())
            .is_some()
    );

    let serialized = serde_json::to_string_pretty(&manifest).unwrap();
    let round_tripped: BundleManifest = serde_json::from_str(&serialized).unwrap();
    assert_eq!(manifest, round_tripped);
}
