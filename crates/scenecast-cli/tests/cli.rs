use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use scenecast_core::{
    BundleManifest, GuideMarkId, GuideMarkStyle, HotspotId, InteractionTrigger, SceneId,
    manifest_path,
};
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
            "--trigger",
            "scroll",
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
    let hotspot = start
        .hotspot(&HotspotId::new("pricing-link").unwrap())
        .unwrap();
    assert_eq!(hotspot.trigger, InteractionTrigger::Scroll);
}

#[test]
fn add_guide_mark_and_section_are_exported_to_static_player() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");
    let output = temp.path().join("player");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap(), "--title", "Demo"])
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
            "--description",
            "Pricing page overview",
            "--notes",
            "Mention annual plan",
        ])
        .assert()
        .success();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "add-guide-mark",
            bundle.to_str().unwrap(),
            "pricing",
            "annual-plan",
            "Annual plan",
            "--x",
            "320",
            "--y",
            "240",
            "--width",
            "180",
            "--height",
            "64",
            "--style",
            "highlight",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Added guide mark annual-plan to scene pricing",
        ));

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "add-section",
            bundle.to_str().unwrap(),
            "overview",
            "Overview",
            "--scenes",
            "start,pricing",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added section overview"));

    let manifest: BundleManifest =
        serde_json::from_str(&fs::read_to_string(manifest_path(&bundle)).unwrap()).unwrap();
    let pricing = manifest
        .graph
        .scene(&SceneId::new("pricing").unwrap())
        .unwrap();
    let guide_mark = pricing
        .guide_marks
        .iter()
        .find(|mark| mark.id == GuideMarkId::new("annual-plan").unwrap())
        .unwrap();
    assert_eq!(guide_mark.style, GuideMarkStyle::Highlight);
    assert_eq!(
        pricing.description.as_deref(),
        Some("Pricing page overview")
    );
    assert_eq!(manifest.sections[1].title, "Overview");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "export-html",
            bundle.to_str().unwrap(),
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let html = fs::read_to_string(output.join("index.html")).unwrap();
    assert!(html.contains("\"sections\""));
    assert!(html.contains("\"guideMarks\""));
    assert!(html.contains("\"annual-plan\""));
    assert!(html.contains("Pricing page overview"));
    assert!(html.contains("routeForScene"));
}

#[test]
fn import_video_extracts_frames_with_ffmpeg_and_adds_scenes() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");
    let input = temp.path().join("demo.mp4");
    fs::write(&input, []).unwrap();
    let fake_ffmpeg = write_fake_ffmpeg(temp.path());

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap()])
        .assert()
        .success();

    let frame_path = bundle.join("captures").join("clip-0001.png");
    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .env("SCENECAST_FAKE_FFMPEG_FRAME", &frame_path)
        .args([
            "import-video",
            bundle.to_str().unwrap(),
            input.to_str().unwrap(),
            "--scene-prefix",
            "clip",
            "--every-seconds",
            "2.5",
            "--ffmpeg",
            fake_ffmpeg.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 1 frame scene(s)"));

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["validate", bundle.to_str().unwrap()])
        .assert()
        .success();

    let manifest: BundleManifest =
        serde_json::from_str(&fs::read_to_string(manifest_path(&bundle)).unwrap()).unwrap();
    let imported = manifest
        .graph
        .scene(&SceneId::new("clip-0001").unwrap())
        .unwrap();
    assert_eq!(
        manifest.graph.start_scene,
        SceneId::new("clip-0001").unwrap()
    );
    assert_eq!(
        imported.assets.screenshot.as_deref(),
        Some("captures/clip-0001.png")
    );
}

#[test]
fn import_video_links_multiple_frames_as_clickthrough_sequence() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");
    let input = temp.path().join("demo.mp4");
    fs::write(&input, []).unwrap();
    let fake_ffmpeg = write_fake_ffmpeg_sequence(temp.path());

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .env(
            "SCENECAST_FAKE_FFMPEG_FRAME_ONE",
            bundle.join("captures").join("clip-0001.png"),
        )
        .env(
            "SCENECAST_FAKE_FFMPEG_FRAME_TWO",
            bundle.join("captures").join("clip-0002.png"),
        )
        .args([
            "import-video",
            bundle.to_str().unwrap(),
            input.to_str().unwrap(),
            "--scene-prefix",
            "clip",
            "--ffmpeg",
            fake_ffmpeg.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 2 frame scene(s)"));

    let manifest: BundleManifest =
        serde_json::from_str(&fs::read_to_string(manifest_path(&bundle)).unwrap()).unwrap();
    assert_eq!(
        manifest.graph.start_scene,
        SceneId::new("clip-0001").unwrap()
    );
    assert!(
        manifest
            .graph
            .scene(&SceneId::new("start").unwrap())
            .is_none()
    );

    let first = manifest
        .graph
        .scene(&SceneId::new("clip-0001").unwrap())
        .unwrap();
    let next = first.hotspot(&HotspotId::new("next").unwrap()).unwrap();
    assert_eq!(next.target, SceneId::new("clip-0002").unwrap());
    assert_eq!(next.trigger, InteractionTrigger::Scroll);
    let transition = next.transition.as_ref().unwrap();
    assert_eq!(transition.default_frame_duration_ms, Some(5000));
    assert_eq!(transition.frames.len(), 1);
    assert_eq!(transition.frames[0].path, "captures/clip-0002.png");
    assert_eq!(next.bounds.width, 1.0);
    assert_eq!(next.bounds.height, 1.0);

    let second = manifest
        .graph
        .scene(&SceneId::new("clip-0002").unwrap())
        .unwrap();
    assert!(second.hotspot(&HotspotId::new("next").unwrap()).is_none());
}

#[test]
fn export_html_writes_clickthrough_player_and_assets() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");
    let output = temp.path().join("player");

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
        .args([
            "add-hotspot",
            bundle.to_str().unwrap(),
            "start",
            "pricing-link",
            "View pricing",
            "pricing",
            "--x",
            "0",
            "--y",
            "0",
            "--width",
            "1",
            "--height",
            "1",
            "--trigger",
            "scroll",
        ])
        .assert()
        .success();
    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "add-transition",
            bundle.to_str().unwrap(),
            "start",
            "pricing-link",
            "--frames",
            "captures/pricing.png",
            "--frame-duration-ms",
            "120",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added transition"));

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "export-html",
            bundle.to_str().unwrap(),
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Exported"));

    let html = fs::read_to_string(output.join("index.html")).unwrap();
    assert!(!html.contains("<header>"));
    assert!(html.contains("pricing-link"));
    assert!(html.contains("captures/pricing.png"));
    assert!(html.contains("addEventListener(\"wheel\""));
    assert!(html.contains("\"trigger\":\"scroll\""));
    assert!(html.contains("\"kind\":\"frame-sequence\""));
    assert!(output.join("captures").join("pricing.png").is_file());
}

#[test]
fn add_transition_attaches_frame_sequence_to_hotspot() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap()])
        .assert()
        .success();
    fs::write(bundle.join("captures").join("scroll-0001.png"), []).unwrap();
    fs::write(bundle.join("captures").join("scroll-0002.png"), []).unwrap();
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
            "scroll-pricing",
            "Scroll pricing",
            "pricing",
            "--x",
            "0",
            "--y",
            "0",
            "--width",
            "100",
            "--height",
            "100",
            "--trigger",
            "scroll",
        ])
        .assert()
        .success();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "add-transition",
            bundle.to_str().unwrap(),
            "start",
            "scroll-pricing",
            "--frames",
            "captures/scroll-0001.png,captures/scroll-0002.png",
            "--frame-duration-ms",
            "85",
        ])
        .assert()
        .success();

    let manifest: BundleManifest =
        serde_json::from_str(&fs::read_to_string(manifest_path(&bundle)).unwrap()).unwrap();
    let start = manifest
        .graph
        .scene(&SceneId::new("start").unwrap())
        .unwrap();
    let hotspot = start
        .hotspot(&HotspotId::new("scroll-pricing").unwrap())
        .unwrap();
    let transition = hotspot.transition.as_ref().unwrap();
    assert_eq!(transition.default_frame_duration_ms, Some(85));
    assert_eq!(transition.frames.len(), 2);
    assert_eq!(transition.frames[0].path, "captures/scroll-0001.png");
    assert_eq!(transition.frames[1].path, "captures/scroll-0002.png");
}

#[test]
fn import_video_reports_missing_ffmpeg() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");
    let input = temp.path().join("demo.mp4");
    fs::write(&input, []).unwrap();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "import-video",
            bundle.to_str().unwrap(),
            input.to_str().unwrap(),
            "--ffmpeg",
            temp.path().join("definitely-not-ffmpeg").to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to run ffmpeg"));
}

#[test]
fn import_video_rejects_invalid_crop() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");
    let input = temp.path().join("demo.mp4");
    fs::write(&input, []).unwrap();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "import-video",
            bundle.to_str().unwrap(),
            input.to_str().unwrap(),
            "--crop",
            "0,0,0,1080",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "crop width and height must be positive",
        ));
}

#[test]
fn import_video_rejects_existing_capture_prefix() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");
    let input = temp.path().join("demo.mp4");
    fs::write(&input, []).unwrap();
    let fake_ffmpeg = write_fake_ffmpeg(temp.path());

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap()])
        .assert()
        .success();
    fs::write(bundle.join("captures").join("clip-0001.png"), []).unwrap();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .env(
            "SCENECAST_FAKE_FFMPEG_FRAME",
            bundle.join("captures").join("clip-0002.png"),
        )
        .args([
            "import-video",
            bundle.to_str().unwrap(),
            input.to_str().unwrap(),
            "--scene-prefix",
            "clip",
            "--ffmpeg",
            fake_ffmpeg.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "capture prefix `clip` already has extracted frames",
        ));
}

fn write_fake_ffmpeg(temp: &Path) -> PathBuf {
    let path = if cfg!(windows) {
        temp.join("fake-ffmpeg.cmd")
    } else {
        temp.join("fake-ffmpeg")
    };

    if cfg!(windows) {
        fs::write(
            &path,
            r#"@echo off
type nul > "%SCENECAST_FAKE_FFMPEG_FRAME%"
exit /b 0
"#,
        )
        .unwrap();
    } else {
        fs::write(
            &path,
            r#"#!/bin/sh
: > "$SCENECAST_FAKE_FFMPEG_FRAME"
"#,
        )
        .unwrap();
        make_executable(&path);
    }

    path
}

fn write_fake_ffmpeg_sequence(temp: &Path) -> PathBuf {
    let path = if cfg!(windows) {
        temp.join("fake-ffmpeg-sequence.cmd")
    } else {
        temp.join("fake-ffmpeg-sequence")
    };

    if cfg!(windows) {
        fs::write(
            &path,
            r#"@echo off
type nul > "%SCENECAST_FAKE_FFMPEG_FRAME_ONE%"
type nul > "%SCENECAST_FAKE_FFMPEG_FRAME_TWO%"
exit /b 0
"#,
        )
        .unwrap();
    } else {
        fs::write(
            &path,
            r#"#!/bin/sh
: > "$SCENECAST_FAKE_FFMPEG_FRAME_ONE"
: > "$SCENECAST_FAKE_FFMPEG_FRAME_TWO"
"#,
        )
        .unwrap();
        make_executable(&path);
    }

    path
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
