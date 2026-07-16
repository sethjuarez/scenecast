use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use scenecast_core::{BundleManifest, HotspotId, SceneId, manifest_path};
use tempfile::tempdir;

#[test]
fn new_creates_valid_starter_bundle() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap(), "--title", "Demo"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["validate", bundle.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Valid"))
        .stderr(predicate::str::contains(
            "warning: scene `start` has no screenshot",
        ));
}

#[test]
fn init_alias_creates_valid_starter_bundle() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["init", bundle.to_str().unwrap(), "--title", "Demo"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["inspect", bundle.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Title: Demo"));
}

#[test]
fn new_rejects_empty_title() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap(), "--title", "   "])
        .assert()
        .failure()
        .stderr(predicate::str::contains("bundle title must not be empty"));
}

#[test]
fn inspect_prints_bundle_summary() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap(), "--title", "Demo"])
        .assert()
        .success();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["inspect", bundle.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Title: Demo"))
        .stdout(predicate::str::contains("Scenes: 1"));
}

#[test]
fn add_scene_persists_scene_and_rejects_duplicates() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "add-scene",
            bundle.to_str().unwrap(),
            "pricing",
            "Pricing",
            "--screenshot",
            "captures/pricing.png",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added scene pricing"));

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["add-scene", bundle.to_str().unwrap(), "pricing", "Pricing"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    let manifest: BundleManifest =
        serde_json::from_str(&fs::read_to_string(manifest_path(&bundle)).unwrap()).unwrap();
    assert!(
        manifest
            .graph
            .scene(&SceneId::new("pricing").unwrap())
            .is_some()
    );
}

#[test]
fn validate_fails_when_referenced_capture_is_missing() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "add-scene",
            bundle.to_str().unwrap(),
            "pricing",
            "Pricing",
            "--screenshot",
            "captures/pricing.png",
        ])
        .assert()
        .success();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["validate", bundle.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "referenced asset `captures/pricing.png` does not exist",
        ));
}

#[test]
fn validate_surfaces_manifest_errors() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");
    fs::create_dir_all(&bundle).unwrap();
    fs::write(
        manifest_path(&bundle),
        r#"{
  "schema_version": "wrong",
  "title": "",
  "graph": {
    "start_scene": "start",
    "scenes": []
  },
  "assets": []
}
"#,
    )
    .unwrap();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["validate", bundle.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported schema version"))
        .stderr(predicate::str::contains("bundle title must not be empty"));
}

#[test]
fn validate_accepts_existing_referenced_capture() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap()])
        .assert()
        .success();
    fs::write(bundle.join("captures").join("pricing.png"), []).unwrap();
    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "add-scene",
            bundle.to_str().unwrap(),
            "pricing",
            "Pricing",
            "--screenshot",
            "captures/pricing.png",
        ])
        .assert()
        .success();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["validate", bundle.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Valid"));
}

#[test]
fn add_scene_rejects_non_portable_capture_paths() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "add-scene",
            bundle.to_str().unwrap(),
            "outside",
            "Outside",
            "--screenshot",
            "../outside.png",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("asset path `../outside.png`"));
}

#[test]
fn add_hotspot_links_scenes_and_rejects_invalid_targets() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["add-scene", bundle.to_str().unwrap(), "pricing", "Pricing"])
        .assert()
        .success();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "add-hotspot",
            bundle.to_str().unwrap(),
            "start",
            "pricing-link",
            "View pricing",
            "pricing",
            "--x",
            "420",
            "--y",
            "240",
            "--width",
            "160",
            "--height",
            "48",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Added hotspot pricing-link to scene start",
        ));

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "add-hotspot",
            bundle.to_str().unwrap(),
            "start",
            "missing-link",
            "Missing",
            "missing",
            "--x",
            "0",
            "--y",
            "0",
            "--width",
            "10",
            "--height",
            "10",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("targets missing scene `missing`"));

    let manifest: BundleManifest =
        serde_json::from_str(&fs::read_to_string(manifest_path(&bundle)).unwrap()).unwrap();
    let start = manifest
        .graph
        .scene(&SceneId::new("start").unwrap())
        .unwrap();
    assert!(
        start
            .hotspot(&HotspotId::new("pricing-link").unwrap())
            .is_some()
    );
}
