use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use scenecast_core::{
    BundleManifest, Hotspot, HotspotId, MANIFEST_FILE_NAME, Rect, Scene, SceneId, ValidationReport,
    manifest_path, read_manifest, validate_referenced_files, write_manifest,
};
use tracing::{info, instrument};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(author, version, about = "Scenecast bundle authoring CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new .scenecast bundle directory.
    New(NewArgs),
    /// Initialize a .scenecast bundle directory. Alias for `new`.
    Init(NewArgs),
    /// Print a summary of a bundle manifest.
    Inspect {
        /// Path to a .scenecast bundle directory.
        bundle: PathBuf,
    },
    /// Validate a bundle manifest.
    Validate {
        /// Path to a .scenecast bundle directory.
        bundle: PathBuf,
    },
    /// Add a screenshot-backed scene to an existing bundle.
    AddScene {
        /// Path to a .scenecast bundle directory.
        bundle: PathBuf,
        /// Stable scene identifier, such as `pricing` or `checkout.success`.
        id: String,
        /// Human-readable scene title.
        title: String,
        /// Optional screenshot path relative to the bundle root.
        #[arg(long)]
        screenshot: Option<String>,
    },
    /// Add a click-through hotspot to a scene.
    AddHotspot {
        /// Path to a .scenecast bundle directory.
        bundle: PathBuf,
        /// Source scene that owns the hotspot.
        scene: String,
        /// Stable hotspot identifier unique within the source scene.
        id: String,
        /// Human-readable label for the hotspot.
        label: String,
        /// Target scene to navigate to.
        target: String,
        /// Hotspot x coordinate in source capture pixels.
        #[arg(long)]
        x: f32,
        /// Hotspot y coordinate in source capture pixels.
        #[arg(long)]
        y: f32,
        /// Hotspot width in source capture pixels.
        #[arg(long)]
        width: f32,
        /// Hotspot height in source capture pixels.
        #[arg(long)]
        height: f32,
    },
    /// Import a video by extracting frames with ffmpeg and adding them as scenes.
    ImportVideo {
        /// Path to a .scenecast bundle directory.
        bundle: PathBuf,
        /// Path to the source video file.
        input: PathBuf,
        /// Scene ID and capture filename prefix. Defaults to the input file stem.
        #[arg(long)]
        scene_prefix: Option<String>,
        /// Extract one frame every N seconds.
        #[arg(long, default_value_t = 5.0)]
        every_seconds: f32,
        /// Crop source video before extracting frames, as x,y,width,height.
        #[arg(long)]
        crop: Option<String>,
        /// ffmpeg executable to run. Defaults to SCENECAST_FFMPEG or ffmpeg.
        #[arg(long)]
        ffmpeg: Option<PathBuf>,
    },
    /// Export a .scenecast bundle to a static HTML click-through player.
    ExportHtml {
        /// Path to a .scenecast bundle directory.
        bundle: PathBuf,
        /// Output directory for index.html.
        output: PathBuf,
    },
}

#[derive(Debug, Clone, Parser)]
struct NewArgs {
    /// Path to the .scenecast bundle directory to create.
    path: PathBuf,
    /// Bundle title. Defaults to the directory name.
    #[arg(long)]
    title: Option<String>,
}

fn main() -> Result<()> {
    init_tracing();
    run(Cli::parse())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .compact()
        .init();
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::New(args) | Command::Init(args) => new_bundle(args),
        Command::Inspect { bundle } => inspect_bundle(&bundle),
        Command::Validate { bundle } => validate_bundle(&bundle),
        Command::AddScene {
            bundle,
            id,
            title,
            screenshot,
        } => add_scene(&bundle, id, title, screenshot),
        Command::AddHotspot {
            bundle,
            scene,
            id,
            label,
            target,
            x,
            y,
            width,
            height,
        } => add_hotspot(
            &bundle,
            AddHotspotArgs {
                scene,
                id,
                label,
                target,
                x,
                y,
                width,
                height,
            },
        ),
        Command::ImportVideo {
            bundle,
            input,
            scene_prefix,
            every_seconds,
            crop,
            ffmpeg,
        } => import_video(
            &bundle,
            ImportVideoArgs {
                input,
                scene_prefix,
                every_seconds,
                crop,
                ffmpeg,
            },
        ),
        Command::ExportHtml { bundle, output } => export_html(&bundle, &output),
    }
}

#[instrument(skip(args), fields(path = %args.path.display()))]
fn new_bundle(args: NewArgs) -> Result<()> {
    ensure_new_bundle_target(&args.path)?;
    fs::create_dir_all(args.path.join("assets")).with_context(|| {
        format!(
            "failed to create assets directory in `{}`",
            args.path.display()
        )
    })?;
    fs::create_dir_all(args.path.join("captures")).with_context(|| {
        format!(
            "failed to create captures directory in `{}`",
            args.path.display()
        )
    })?;

    let title = args
        .title
        .unwrap_or_else(|| default_title(&args.path))
        .trim()
        .to_owned();
    if title.is_empty() {
        bail!("bundle title must not be empty");
    }
    let manifest = BundleManifest::starter(title);
    reject_validation_errors("starter bundle", &manifest.validate())?;
    write_manifest(&args.path, &manifest)?;
    info!("created scenecast bundle");

    println!("Created {}", args.path.display());
    Ok(())
}

fn ensure_new_bundle_target(path: &Path) -> Result<()> {
    if manifest_path(path).exists() {
        bail!(
            "`{}` already contains a {MANIFEST_FILE_NAME}",
            path.display()
        );
    }

    if path.is_file() {
        bail!("`{}` exists and is not a directory", path.display());
    }

    if path.exists() && path.read_dir()?.next().is_some() {
        bail!("`{}` exists and is not empty", path.display());
    }

    Ok(())
}

fn default_title(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Untitled Scenecast")
        .to_owned()
}

#[instrument(fields(bundle = %bundle.display()))]
fn inspect_bundle(bundle: &Path) -> Result<()> {
    let manifest = read_manifest(bundle)?;
    let report = manifest.validate();

    println!("Title: {}", manifest.title);
    println!("Schema: {}", manifest.schema_version);
    println!("Start scene: {}", manifest.graph.start_scene);
    println!("Scenes: {}", manifest.graph.scenes.len());
    println!("Assets: {}", manifest.assets.len());
    println!("Warnings: {}", report.warnings.len());
    println!("Errors: {}", report.errors.len());

    Ok(())
}

#[instrument(fields(bundle = %bundle.display()))]
fn validate_bundle(bundle: &Path) -> Result<()> {
    let manifest = read_manifest(bundle)?;
    let report = manifest.validate();
    let file_report = validate_referenced_files(bundle, &manifest);

    for warning in &report.warnings {
        eprintln!("warning: {warning}");
    }

    if report.is_valid() && file_report.is_valid() {
        println!("Valid {}", bundle.display());
        Ok(())
    } else {
        for error in &report.errors {
            eprintln!("error: {error}");
        }
        for missing_file in &file_report.missing_files {
            eprintln!("error: {missing_file}");
        }
        bail!(
            "bundle validation failed with {} error(s)",
            report.errors.len() + file_report.missing_files.len()
        );
    }
}

#[instrument(skip(id, title, screenshot), fields(bundle = %bundle.display()))]
fn add_scene(bundle: &Path, id: String, title: String, screenshot: Option<String>) -> Result<()> {
    let mut manifest = read_manifest(bundle)?;
    let id = SceneId::new(id)?;

    if manifest.graph.scene(&id).is_some() {
        bail!("scene `{id}` already exists");
    }

    let before_report = manifest.validate();
    manifest.add_scene(Scene::screenshot(id.clone(), title, screenshot));
    reject_introduced_errors("scene", &before_report, &manifest.validate())?;

    write_manifest(bundle, &manifest)?;
    info!(scene_id = %id, "added scene to bundle");

    println!("Added scene {id}");
    Ok(())
}

#[derive(Debug)]
struct AddHotspotArgs {
    scene: String,
    id: String,
    label: String,
    target: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[instrument(skip(args), fields(bundle = %bundle.display(), scene = %args.scene, hotspot = %args.id))]
fn add_hotspot(bundle: &Path, args: AddHotspotArgs) -> Result<()> {
    let mut manifest = read_manifest(bundle)?;
    let scene_id = SceneId::new(args.scene)?;
    let hotspot_id = HotspotId::new(args.id)?;
    let target = SceneId::new(args.target)?;
    let bounds = Rect::new(args.x, args.y, args.width, args.height);
    let hotspot = Hotspot::new(hotspot_id.clone(), args.label, target, bounds);
    let before_report = manifest.validate();

    manifest.add_hotspot(&scene_id, hotspot)?;
    reject_introduced_errors("hotspot", &before_report, &manifest.validate())?;

    write_manifest(bundle, &manifest)?;
    info!(scene_id = %scene_id, hotspot_id = %hotspot_id, "added hotspot to bundle");

    println!("Added hotspot {hotspot_id} to scene {scene_id}");
    Ok(())
}

#[derive(Debug)]
struct ImportVideoArgs {
    input: PathBuf,
    scene_prefix: Option<String>,
    every_seconds: f32,
    crop: Option<String>,
    ffmpeg: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
struct Crop {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl Crop {
    fn parse(value: &str) -> Result<Self> {
        let parts = value
            .split([',', ':'])
            .map(str::trim)
            .map(str::parse::<u32>)
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| format!("invalid crop `{value}`; expected x,y,width,height"))?;

        let [x, y, width, height] = parts.as_slice() else {
            bail!("invalid crop `{value}`; expected x,y,width,height");
        };
        if *width == 0 || *height == 0 {
            bail!("crop width and height must be positive");
        }

        Ok(Self {
            x: *x,
            y: *y,
            width: *width,
            height: *height,
        })
    }

    fn ffmpeg_filter(self) -> String {
        format!("crop={}:{}:{}:{}", self.width, self.height, self.x, self.y)
    }
}

#[instrument(skip(args), fields(bundle = %bundle.display(), input = %args.input.display()))]
fn import_video(bundle: &Path, args: ImportVideoArgs) -> Result<()> {
    if !args.every_seconds.is_finite() || args.every_seconds <= 0.0 {
        bail!("--every-seconds must be a positive finite number");
    }
    let crop = args.crop.as_deref().map(Crop::parse).transpose()?;

    let mut manifest = read_manifest(bundle)?;
    let before_report = manifest.validate();
    let captures_dir = bundle.join("captures");
    fs::create_dir_all(&captures_dir).with_context(|| {
        format!(
            "failed to create captures directory in `{}`",
            captures_dir.display()
        )
    })?;

    let prefix = args
        .scene_prefix
        .map(|value| slugify_identifier(&value))
        .unwrap_or_else(|| default_scene_prefix(&args.input));
    let output_pattern = captures_dir.join(format!("{prefix}-%04d.png"));
    let ffmpeg = ffmpeg_path(args.ffmpeg);

    let existing_frames = extracted_frames(&captures_dir, &prefix)?;
    if !existing_frames.is_empty() {
        bail!("capture prefix `{prefix}` already has extracted frames");
    }

    let video_filter = video_filter(args.every_seconds, crop);
    let status = ProcessCommand::new(&ffmpeg)
        .args(["-hide_banner", "-nostdin", "-loglevel", "error", "-i"])
        .arg(&args.input)
        .args(["-vf", &video_filter, "-start_number", "1"])
        .arg(&output_pattern)
        .status()
        .with_context(|| {
            format!(
                "failed to run ffmpeg at `{}`; install ffmpeg or pass --ffmpeg",
                ffmpeg.display()
            )
        })?;

    if !status.success() {
        bail!("ffmpeg failed with status {status}");
    }

    let frames = extracted_frames(&captures_dir, &prefix)?;
    if frames.is_empty() {
        bail!("ffmpeg completed but produced no frames for prefix `{prefix}`");
    }

    let frame_scene_ids = frames
        .iter()
        .enumerate()
        .map(|(index, _frame)| SceneId::new(format!("{prefix}-{:04}", index + 1)))
        .collect::<Result<Vec<_>, _>>()?;

    for scene_id in &frame_scene_ids {
        if manifest.graph.scene(scene_id).is_some() {
            bail!("scene `{scene_id}` already exists");
        }
    }

    for (index, frame) in frames.iter().enumerate() {
        let scene_id = frame_scene_ids[index].clone();
        let next_scene_id = frame_scene_ids.get(index + 1).cloned();
        let dimensions = png_dimensions(frame).unwrap_or((1, 1));

        let screenshot = format!("captures/{}", frame.file_name().unwrap().to_string_lossy());
        let mut scene = Scene::screenshot(
            scene_id,
            format!("{} frame {}", prefix, index + 1),
            Some(screenshot),
        );
        if let Some(target) = next_scene_id {
            scene.hotspots.push(Hotspot::new(
                HotspotId::new("next").expect("static hotspot id is valid"),
                "Next frame",
                target,
                Rect::new(0.0, 0.0, dimensions.0 as f32, dimensions.1 as f32),
            ));
        }
        manifest.add_scene(scene);
    }

    if is_untouched_starter(&manifest) {
        manifest
            .graph
            .scenes
            .retain(|scene| scene.id.as_str() != "start");
    }
    manifest.graph.start_scene = frame_scene_ids[0].clone();

    reject_introduced_errors("video import", &before_report, &manifest.validate())?;
    write_manifest(bundle, &manifest)?;
    info!(frame_count = frames.len(), "imported video frames");

    println!("Imported {} frame scene(s)", frames.len());
    Ok(())
}

fn video_filter(every_seconds: f32, crop: Option<Crop>) -> String {
    let fps = format!("fps=1/{every_seconds}");
    match crop {
        Some(crop) => format!("{},{}", crop.ffmpeg_filter(), fps),
        None => fps,
    }
}

fn is_untouched_starter(manifest: &BundleManifest) -> bool {
    manifest.graph.start_scene.as_str() == "start"
        && manifest.graph.scenes.iter().any(|scene| {
            scene.id.as_str() == "start"
                && scene.assets.screenshot.is_none()
                && scene.assets.video.is_none()
                && scene.hotspots.is_empty()
        })
}

#[instrument(fields(bundle = %bundle.display(), output = %output.display()))]
fn export_html(bundle: &Path, output: &Path) -> Result<()> {
    let manifest = read_manifest(bundle)?;
    let report = manifest.validate();
    let file_report = validate_referenced_files(bundle, &manifest);
    reject_validation_errors("bundle", &report)?;
    if !file_report.is_valid() {
        for missing_file in &file_report.missing_files {
            eprintln!("error: {missing_file}");
        }
        bail!(
            "bundle has {} missing referenced file(s)",
            file_report.missing_files.len()
        );
    }

    fs::create_dir_all(output)
        .with_context(|| format!("failed to create output directory `{}`", output.display()))?;
    copy_referenced_assets(bundle, output, &manifest)?;
    let html = render_html_player(&manifest);
    let index_path = output.join("index.html");
    fs::write(&index_path, html)
        .with_context(|| format!("failed to write `{}`", index_path.display()))?;

    println!("Exported {}", index_path.display());
    Ok(())
}

fn copy_referenced_assets(bundle: &Path, output: &Path, manifest: &BundleManifest) -> Result<()> {
    for portable_path in manifest.referenced_asset_paths() {
        let source = bundle.join(portable_path);
        let destination = output.join(portable_path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create `{}`", parent.display()))?;
        }
        fs::copy(&source, &destination).with_context(|| {
            format!(
                "failed to copy `{}` to `{}`",
                source.display(),
                destination.display()
            )
        })?;
    }
    Ok(())
}

fn ffmpeg_path(explicit: Option<PathBuf>) -> PathBuf {
    explicit
        .or_else(|| std::env::var_os("SCENECAST_FFMPEG").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("ffmpeg"))
}

fn default_scene_prefix(input: &Path) -> String {
    let raw = input
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("video");
    slugify_identifier(raw)
}

fn slugify_identifier(value: &str) -> String {
    let mut slug = String::new();

    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if matches!(character, '-' | '_' | '.') {
            slug.push(character);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }

    let slug = slug.trim_matches(['-', '.']).to_owned();
    if slug.is_empty() {
        "video".to_owned()
    } else {
        slug
    }
}

fn extracted_frames(captures_dir: &Path, prefix: &str) -> Result<Vec<PathBuf>> {
    let mut frames = fs::read_dir(captures_dir)
        .with_context(|| format!("failed to read `{}`", captures_dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("failed to list `{}`", captures_dir.display()))?
        .into_iter()
        .filter(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| {
                    name.starts_with(&format!("{prefix}-")) && name.ends_with(".png")
                })
        })
        .collect::<Vec<_>>();

    frames.sort();
    Ok(frames)
}

fn png_dimensions(path: &Path) -> Option<(u32, u32)> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() < 24 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }

    Some((
        u32::from_be_bytes(bytes[16..20].try_into().ok()?),
        u32::from_be_bytes(bytes[20..24].try_into().ok()?),
    ))
}

fn render_html_player(manifest: &BundleManifest) -> String {
    let mut scenes = String::new();
    for scene in &manifest.graph.scenes {
        let screenshot = scene.assets.screenshot.as_deref().unwrap_or("");
        let hotspots = scene
            .hotspots
            .iter()
            .map(|hotspot| {
                format!(
                    "{{id:\"{}\",label:\"{}\",target:\"{}\",x:{},y:{},width:{},height:{}}}",
                    escape_js(hotspot.id.as_str()),
                    escape_js(&hotspot.label),
                    escape_js(hotspot.target.as_str()),
                    hotspot.bounds.x,
                    hotspot.bounds.y,
                    hotspot.bounds.width,
                    hotspot.bounds.height
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        scenes.push_str(&format!(
            "\"{}\":{{title:\"{}\",screenshot:\"{}\",hotspots:[{}]}},",
            escape_js(scene.id.as_str()),
            escape_js(&scene.title),
            escape_js(screenshot),
            hotspots
        ));
    }

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>{title}</title>
<style>
html {{ margin: 0; padding: 0; width: 100vw; height: 100vh; overflow: hidden; background: #000; }}
body {{ margin: 0; padding: 0; width: 100vw; height: 100vh; min-width: 100vw; min-height: 100vh; overflow: hidden; background: #000; }}
#capture {{ position: fixed; inset: 0; display: block; width: 100vw; height: 100vh; max-width: none; max-height: none; object-fit: fill; }}
</style>
</head>
<body>
<img id="capture" alt="">
<script>
const bundleBase = new URL("./", location.href);
const startScene = "{start}";
const scenes = {{{scenes}}};
const image = document.getElementById("capture");
const cacheKey = new URLSearchParams(location.search).get("v") || String(Date.now());
function show(sceneId) {{
  const scene = scenes[sceneId];
  if (!scene) return;
  location.hash = sceneId;
  if (scene.screenshot) {{
    const frame = new URL(scene.screenshot, bundleBase);
    frame.searchParams.set("scenecast-cache", cacheKey);
    image.src = frame.href;
  }}
}}
document.body.addEventListener("click", () => {{
  const scene = scenes[location.hash.slice(1) || startScene];
  const next = scene && scene.hotspots[0];
  if (next) show(next.target);
}});
show(location.hash.slice(1) || startScene);
</script>
</body>
</html>
"#,
        title = escape_html(&manifest.title),
        start = escape_js(manifest.graph.start_scene.as_str()),
        scenes = scenes
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_js(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn reject_introduced_errors(
    action: &str,
    before: &ValidationReport,
    after: &ValidationReport,
) -> Result<()> {
    let introduced_errors = after
        .errors
        .iter()
        .filter(|error| !before.errors.contains(error))
        .collect::<Vec<_>>();

    if introduced_errors.is_empty() {
        return Ok(());
    }

    for error in &introduced_errors {
        eprintln!("error: {error}");
    }
    if !before.is_valid() {
        eprintln!(
            "note: manifest also has {} pre-existing validation error(s)",
            before.errors.len()
        );
    }

    bail!(
        "{action} would introduce {} validation error(s)",
        introduced_errors.len()
    );
}

fn reject_validation_errors(context: &str, report: &ValidationReport) -> Result<()> {
    if report.is_valid() {
        return Ok(());
    }

    for error in &report.errors {
        eprintln!("error: {error}");
    }
    bail!("{context} is invalid");
}
