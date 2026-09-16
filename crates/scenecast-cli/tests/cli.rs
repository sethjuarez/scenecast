use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use scenecast_core::{
    BundleManifest, GuideMarkId, GuideMarkStyle, Hotspot, HotspotId, InteractionTrigger, Rect,
    Scene, SceneGraph, SceneId, SceneKind, SourceArtifactKind, manifest_path, write_manifest,
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
            "--scroll-direction",
            "down",
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
    assert_eq!(manifest.sources.len(), 1);
    assert_eq!(manifest.sources[0].id, "clip-source");
    assert_eq!(manifest.sources[0].kind, SourceArtifactKind::Video);
    assert_eq!(manifest.sources[0].label, "demo.mp4");
    assert_eq!(manifest.sources[0].media_type.as_deref(), Some("video/mp4"));
    assert_eq!(
        manifest.graph.start_scene,
        SceneId::new("clip-0001").unwrap()
    );
    assert_eq!(imported.kind, SceneKind::VideoFrame);
    assert_eq!(
        imported.assets.screenshot.as_deref(),
        Some("captures/clip-0001.png")
    );
    let provenance = imported.provenance.as_ref().unwrap();
    assert_eq!(provenance.source_id, "clip-source");
    assert_eq!(provenance.timestamp_ms, Some(0));
    assert_eq!(provenance.confidence, Some(1.0));
    assert_eq!(provenance.evidence[0].id, "clip-frame-0001");
}

#[test]
fn import_video_links_multiple_frames_as_clickthrough_sequence() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");
    let output = temp.path().join("player");
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
    assert_eq!(manifest.sources[0].id, "clip-source");
    assert_eq!(first.provenance.as_ref().unwrap().source_id, "clip-source");
    assert_eq!(first.provenance.as_ref().unwrap().timestamp_ms, Some(0));
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

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["validate", bundle.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Valid"));

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
    assert!(html.contains("\"startScene\":\"clip-0001\""));
    assert!(html.contains("\"sceneOrder\":[\"clip-0001\",\"clip-0002\"]"));
    assert!(html.contains("\"clip-0001\":"));
    assert!(html.contains("\"clip-0002\":"));
    assert!(html.contains("\"target\":\"clip-0002\""));
    assert!(html.contains("\"trigger\":\"scroll\""));
    assert!(html.contains("\"kind\":\"frame-sequence\""));
    assert!(html.contains("\"sources\":[{\"id\":\"clip-source\""));
    assert!(html.contains("\"sourceId\":\"clip-source\""));
    assert!(html.contains("\"timestampMs\":0"));
    assert!(output.join("captures").join("clip-0001.png").is_file());
    assert!(output.join("captures").join("clip-0002.png").is_file());
}

#[test]
fn import_video_with_srt_sidecar_creates_valid_transcript_draft_and_player_data() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");
    let output = temp.path().join("player");
    let draft_output = temp.path().join("editorial-draft");
    let input = temp.path().join("demo.mp4");
    fs::write(&input, []).unwrap();
    fs::write(
        temp.path().join("demo.srt"),
        include_str!("fixtures/transcript-video.srt"),
    )
    .unwrap();
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

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["validate", bundle.to_str().unwrap()])
        .assert()
        .success();

    let mut manifest: BundleManifest =
        serde_json::from_str(&fs::read_to_string(manifest_path(&bundle)).unwrap()).unwrap();
    assert_eq!(manifest.sources.len(), 2);
    assert_eq!(manifest.sources[1].id, "clip-transcript");
    assert_eq!(manifest.sources[1].kind, SourceArtifactKind::Transcript);
    assert_eq!(manifest.sources[1].label, "demo.srt");
    assert_eq!(manifest.sources[1].transcript_segments.len(), 3);
    assert_eq!(manifest.sources[1].transcript_segments[0].start_ms, 0);
    assert_eq!(manifest.sources[1].transcript_segments[0].end_ms, 2_000);
    assert_eq!(
        manifest.sources[1].transcript_segments[0].text,
        "Welcome to the overview."
    );

    let first = manifest
        .graph
        .scene(&SceneId::new("clip-0001").unwrap())
        .unwrap();
    assert_eq!(first.title, "clip draft 0001");
    assert_eq!(first.provenance.as_ref().unwrap().timestamp_ms, Some(2_750));
    assert_eq!(
        first.provenance.as_ref().unwrap().transcript_segment_ids,
        ["clip-transcript-0001", "clip-transcript-0002"]
    );
    assert_eq!(
        first.description.as_deref(),
        Some("Transcript draft: Welcome to the overview. We begin with the dashboard.")
    );
    assert!(
        first
            .notes
            .as_deref()
            .unwrap()
            .contains("No clickable UI is inferred")
    );
    assert_eq!(
        first
            .provenance
            .as_ref()
            .unwrap()
            .evidence
            .iter()
            .filter(|evidence| evidence.kind == scenecast_core::EvidenceKind::TranscriptSegment)
            .count(),
        2
    );
    assert_eq!(first.hotspots.len(), 1);
    assert_eq!(first.hotspots[0].trigger, InteractionTrigger::Scroll);
    manifest
        .graph
        .scene_mut(&SceneId::new("clip-0001").unwrap())
        .unwrap()
        .provenance
        .as_mut()
        .unwrap()
        .transcript_segments
        .reverse();
    write_manifest(&bundle, &manifest).unwrap();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "export-draft",
            bundle.to_str().unwrap(),
            draft_output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Exported"));

    let draft: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(draft_output.join("draft.json")).unwrap())
            .unwrap();
    assert_eq!(draft["format"], "editorial-draft");
    assert_eq!(draft["schemaVersion"], 1);
    assert_eq!(draft["draft"]["source"]["system"], "scenecast");
    assert_eq!(draft["scenes"].as_array().unwrap().len(), 2);
    assert_eq!(draft["scenes"][0]["id"], "clip-0001");
    assert_eq!(draft["scenes"][0]["startMs"], 0);
    assert_eq!(draft["scenes"][0]["endMs"], 5_500);
    assert!(
        draft["scenes"][0]["sourceSegments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|segment| segment["text"] == "Welcome to the overview.")
    );
    assert_eq!(draft["scenes"][0]["actions"], serde_json::json!([]));
    assert_eq!(draft["assets"][0]["path"], "assets/clip-0001-frame.png");
    assert_eq!(draft["assets"][0]["mimeType"], "image/png");
    assert_eq!(draft["assets"][0]["sha256"].as_str().unwrap().len(), 64);
    assert!(
        draft_output
            .join("assets")
            .join("clip-0001-frame.png")
            .is_file()
    );

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
    assert!(html.contains("\"kind\":\"transcript\""));
    assert!(html.contains("\"id\":\"clip-transcript-0001\""));
    assert!(html.contains("\"startMs\":0"));
    assert!(html.contains("\"endMs\":2000"));
    assert!(html.contains("\"transcriptSegments\":["));
    assert!(html.contains("\"segmentId\":\"clip-transcript-0001\""));
    assert!(html.contains("Welcome to the overview."));
    assert!(html.contains("No clickable UI is inferred from transcript text."));
    assert!(output.join("captures").join("clip-0001.png").is_file());
    assert!(output.join("captures").join("clip-0002.png").is_file());
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
            "--scroll-direction",
            "down",
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
fn export_resolved_demo_writes_cutready_playback_contract() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");
    let output = temp.path().join("resolved");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap(), "--title", "Demo"])
        .assert()
        .success();
    fs::write(bundle.join("captures").join("pricing.png"), []).unwrap();
    fs::write(bundle.join("captures").join("scroll-0001.png"), []).unwrap();
    fs::write(bundle.join("captures").join("scroll-0002.png"), []).unwrap();
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
            "Pricing page.",
        ])
        .assert()
        .success();
    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "add-guide-mark",
            bundle.to_str().unwrap(),
            "pricing",
            "price-guide",
            "Price cards",
            "--x",
            "8",
            "--y",
            "9",
            "--width",
            "10",
            "--height",
            "11",
            "--style",
            "highlight",
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
            "420",
            "--y",
            "240",
            "--width",
            "160",
            "--height",
            "48",
            "--trigger",
            "scroll",
            "--scroll-direction",
            "down",
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
            "captures/scroll-0001.png,captures/scroll-0002.png",
            "--frame-duration-ms",
            "120",
        ])
        .assert()
        .success();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "export-resolved-demo",
            bundle.to_str().unwrap(),
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("resolved-demo.json"));

    let resolved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.join("resolved-demo.json")).unwrap())
            .unwrap();
    assert_eq!(resolved["schemaVersion"], "scenecast.resolved-demo.v1");
    assert_eq!(resolved["startSceneId"], "start");
    assert_eq!(resolved["capturePolicy"]["assetBase"], "bundle-relative");
    assert_eq!(resolved["steps"][0]["id"], "step-start");
    assert_eq!(resolved["steps"][0]["order"], 0);
    assert_eq!(
        resolved["steps"][0]["targets"][0]["targetSceneId"],
        "pricing"
    );
    assert_eq!(resolved["steps"][0]["targets"][0]["trigger"], "scroll");
    assert_eq!(
        resolved["steps"][0]["targets"][0]["scrollDirection"],
        "down"
    );
    assert_eq!(resolved["steps"][1]["guideMarks"][0]["style"], "highlight");
    assert_eq!(resolved["segments"][0]["kind"], "scene");
    assert_eq!(resolved["segments"][1]["kind"], "transition");
    assert_eq!(
        resolved["segments"][1]["frames"][0]["path"],
        "captures/scroll-0001.png"
    );
    assert_eq!(resolved["segments"][1]["frames"][0]["durationMs"], 120);
    assert_eq!(resolved["segments"][1]["interaction"]["id"], "pricing-link");
    assert_eq!(resolved["diagnostics"][0]["severity"], "warning");
    assert_eq!(resolved["diagnostics"][0]["code"], "scene_has_no_capture");
    assert_eq!(resolved["diagnostics"][0]["sceneId"], "start");
}

#[test]
fn export_resolved_demo_orders_steps_from_start_scene() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");
    fs::create_dir_all(bundle.join("captures")).unwrap();
    fs::create_dir_all(bundle.join("assets")).unwrap();
    fs::write(bundle.join("captures").join("intro.png"), []).unwrap();
    fs::write(bundle.join("captures").join("start.png"), []).unwrap();
    fs::write(bundle.join("captures").join("pricing.png"), []).unwrap();

    let intro = Scene::screenshot(
        SceneId::new("intro").unwrap(),
        "Intro",
        Some("captures/intro.png".to_string()),
    );
    let mut start = Scene::screenshot(
        SceneId::new("start").unwrap(),
        "Start",
        Some("captures/start.png".to_string()),
    );
    start.hotspots.push(Hotspot::new(
        HotspotId::new("pricing-link").unwrap(),
        "View pricing",
        SceneId::new("pricing").unwrap(),
        Rect::new(1.0, 2.0, 3.0, 4.0),
    ));
    let pricing = Scene::screenshot(
        SceneId::new("pricing").unwrap(),
        "Pricing",
        Some("captures/pricing.png".to_string()),
    );
    let manifest = BundleManifest {
        schema_version: "scenecast.bundle.v1".to_string(),
        title: "Demo".to_string(),
        sections: Vec::new(),
        graph: SceneGraph {
            start_scene: SceneId::new("start").unwrap(),
            scenes: vec![intro, start, pricing],
        },
        sources: Vec::new(),
        assets: Vec::new(),
    };
    write_manifest(&bundle, &manifest).unwrap();

    let output = temp.path().join("resolved");
    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "export-resolved-demo",
            bundle.to_str().unwrap(),
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let resolved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.join("resolved-demo.json")).unwrap())
            .unwrap();
    assert_eq!(resolved["steps"][0]["sceneId"], "start");
    assert_eq!(resolved["steps"][1]["sceneId"], "pricing");
    assert_eq!(resolved["steps"][2]["sceneId"], "intro");
    assert_eq!(resolved["segments"][0]["stepId"], "step-start");
    assert_eq!(resolved["segments"][1]["interaction"]["id"], "pricing-link");
    assert_eq!(resolved["segments"][2]["stepId"], "step-pricing");
    assert_eq!(resolved["segments"][3]["stepId"], "step-intro");
}

#[test]
fn export_resolved_demo_keeps_branch_targets_out_of_linear_segments() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");
    fs::create_dir_all(bundle.join("captures")).unwrap();
    fs::create_dir_all(bundle.join("assets")).unwrap();
    fs::write(bundle.join("captures").join("start.png"), []).unwrap();
    fs::write(bundle.join("captures").join("pricing.png"), []).unwrap();
    fs::write(bundle.join("captures").join("docs.png"), []).unwrap();

    let mut start = Scene::screenshot(
        SceneId::new("start").unwrap(),
        "Start",
        Some("captures/start.png".to_string()),
    );
    start.hotspots.push(Hotspot::new(
        HotspotId::new("pricing-link").unwrap(),
        "View pricing",
        SceneId::new("pricing").unwrap(),
        Rect::new(1.0, 2.0, 3.0, 4.0),
    ));
    start.hotspots.push(Hotspot::new(
        HotspotId::new("docs-link").unwrap(),
        "Read docs",
        SceneId::new("docs").unwrap(),
        Rect::new(5.0, 6.0, 7.0, 8.0),
    ));
    let pricing = Scene::screenshot(
        SceneId::new("pricing").unwrap(),
        "Pricing",
        Some("captures/pricing.png".to_string()),
    );
    let docs = Scene::screenshot(
        SceneId::new("docs").unwrap(),
        "Docs",
        Some("captures/docs.png".to_string()),
    );
    let manifest = BundleManifest {
        schema_version: "scenecast.bundle.v1".to_string(),
        title: "Demo".to_string(),
        sections: Vec::new(),
        graph: SceneGraph {
            start_scene: SceneId::new("start").unwrap(),
            scenes: vec![start, pricing, docs],
        },
        sources: Vec::new(),
        assets: Vec::new(),
    };
    write_manifest(&bundle, &manifest).unwrap();

    let output = temp.path().join("resolved");
    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "export-resolved-demo",
            bundle.to_str().unwrap(),
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let resolved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.join("resolved-demo.json")).unwrap())
            .unwrap();
    assert_eq!(resolved["steps"][0]["targets"].as_array().unwrap().len(), 2);
    assert_eq!(resolved["steps"][0]["targets"][0]["id"], "pricing-link");
    assert_eq!(resolved["steps"][0]["targets"][1]["id"], "docs-link");
    let segment_interactions = resolved["segments"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|segment| segment["interaction"]["id"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(segment_interactions, vec!["pricing-link"]);
}

#[test]
fn export_resolved_demo_expands_required_interaction_defaults() {
    let temp = tempdir().unwrap();
    let bundle = temp.path().join("demo.scenecast");
    let output = temp.path().join("resolved");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["new", bundle.to_str().unwrap()])
        .assert()
        .success();
    fs::write(bundle.join("captures").join("pricing.png"), []).unwrap();
    fs::write(bundle.join("captures").join("transition.png"), []).unwrap();
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
            "1",
            "--y",
            "2",
            "--width",
            "3",
            "--height",
            "4",
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
            "captures/transition.png",
        ])
        .assert()
        .success();

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args([
            "export-resolved-demo",
            bundle.to_str().unwrap(),
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let resolved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.join("resolved-demo.json")).unwrap())
            .unwrap();
    assert_eq!(resolved["steps"][0]["targets"][0]["trigger"], "click");
    assert_eq!(resolved["steps"][0]["targets"][0]["scrollDirection"], "any");
    assert_eq!(resolved["segments"][1]["frames"][0]["durationMs"], 90);
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
            "--scroll-direction",
            "down",
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
    assert_eq!(
        hotspot.scroll_direction,
        scenecast_core::ScrollDirection::Down
    );
    assert_eq!(transition.default_frame_duration_ms, Some(85));
    assert_eq!(transition.frames.len(), 2);
    assert_eq!(transition.frames[0].path, "captures/scroll-0001.png");
    assert_eq!(transition.frames[1].path, "captures/scroll-0002.png");
}

#[test]
fn checked_in_scroll_sample_validates_and_exports() {
    let temp = tempdir().unwrap();
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let bundle = repo_root.join("examples").join("scroll.scenecast");
    let output = temp.path().join("scroll-player");

    Command::cargo_bin("scenecast-cli")
        .unwrap()
        .args(["validate", bundle.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Valid"));

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
    assert!(html.contains("scroll-down"));
    assert!(html.contains("\"trigger\":\"scroll\""));
    assert!(html.contains("\"scrollDirection\":\"down\""));
    assert!(html.contains("\"scrollDirection\":\"up\""));
    assert!(html.contains("\"kind\":\"frame-sequence\""));
    assert!(html.contains("back-to-top"));
    assert!(output.join("captures").join("home-top.svg").is_file());
    assert!(output.join("captures").join("scroll-0001.svg").is_file());
    assert!(output.join("captures").join("scroll-0002.svg").is_file());
    assert!(output.join("captures").join("home-bottom.svg").is_file());
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
