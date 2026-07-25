use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use scenecast_core::{
    BundleManifest, EvidenceKind, EvidenceRef, GuideMark, GuideMarkId, GuideMarkStyle, Hotspot,
    HotspotId, InteractionTrigger, MANIFEST_FILE_NAME, Rect, Scene, SceneId, SceneKind,
    SceneProvenance, ScrollDirection, Section, SourceArtifact, SourceArtifactKind,
    TranscriptSegment, TranscriptSegmentRef, Transition, TransitionFrame, TransitionKind,
    ValidationReport, manifest_path, read_manifest, validate_referenced_files, write_manifest,
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
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
        /// Optional authored description for search, narration, and presenter surfaces.
        #[arg(long)]
        description: Option<String>,
        /// Optional presenter or authoring notes.
        #[arg(long)]
        notes: Option<String>,
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
        /// Interaction trigger for this hotspot.
        #[arg(long, value_enum, default_value_t = TriggerArg::Click)]
        trigger: TriggerArg,
        /// Wheel direction for scroll hotspots.
        #[arg(long, value_enum, default_value_t = ScrollDirectionArg::Any)]
        scroll_direction: ScrollDirectionArg,
    },
    /// Add a visible guide mark to a scene without changing navigation.
    AddGuideMark {
        /// Path to a .scenecast bundle directory.
        bundle: PathBuf,
        /// Scene that owns the guide mark.
        scene: String,
        /// Stable guide mark identifier unique within the source scene.
        id: String,
        /// Human-readable label for the guide mark.
        label: String,
        /// Guide mark x coordinate in source capture pixels.
        #[arg(long)]
        x: f32,
        /// Guide mark y coordinate in source capture pixels.
        #[arg(long)]
        y: f32,
        /// Guide mark width in source capture pixels.
        #[arg(long)]
        width: f32,
        /// Guide mark height in source capture pixels.
        #[arg(long)]
        height: f32,
        /// Visual style for the guide mark.
        #[arg(long, value_enum, default_value_t = GuideMarkStyleArg::Pulse)]
        style: GuideMarkStyleArg,
    },
    /// Add a table-of-contents section for a sequence of scenes.
    AddSection {
        /// Path to a .scenecast bundle directory.
        bundle: PathBuf,
        /// Stable section identifier.
        id: String,
        /// Human-readable section title.
        title: String,
        /// Comma-separated scene IDs included in this section, in playback order.
        #[arg(long, value_delimiter = ',')]
        scenes: Vec<String>,
    },
    /// Attach a frame-sequence transition to an existing hotspot.
    AddTransition {
        /// Path to a .scenecast bundle directory.
        bundle: PathBuf,
        /// Source scene that owns the hotspot.
        scene: String,
        /// Hotspot identifier.
        hotspot: String,
        /// Comma-separated transition frame paths relative to the bundle root.
        #[arg(long, value_delimiter = ',')]
        frames: Vec<String>,
        /// Default frame duration in milliseconds when per-frame duration is not set.
        #[arg(long, default_value_t = 90)]
        frame_duration_ms: u32,
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
        /// Optional SRT or WebVTT transcript. Defaults to a same-basename .srt or .vtt sidecar.
        #[arg(long)]
        transcript: Option<PathBuf>,
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
    /// Export a portable editorial draft from transcript-backed scenes.
    ExportDraft {
        /// Path to a .scenecast bundle directory.
        bundle: PathBuf,
        /// Output directory for draft.json and its referenced screenshots.
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

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TriggerArg {
    Click,
    Scroll,
}

impl From<TriggerArg> for InteractionTrigger {
    fn from(value: TriggerArg) -> Self {
        match value {
            TriggerArg::Click => InteractionTrigger::Click,
            TriggerArg::Scroll => InteractionTrigger::Scroll,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ScrollDirectionArg {
    Any,
    Down,
    Up,
}

impl From<ScrollDirectionArg> for ScrollDirection {
    fn from(value: ScrollDirectionArg) -> Self {
        match value {
            ScrollDirectionArg::Any => ScrollDirection::Any,
            ScrollDirectionArg::Down => ScrollDirection::Down,
            ScrollDirectionArg::Up => ScrollDirection::Up,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GuideMarkStyleArg {
    Pulse,
    Ring,
    Highlight,
}

impl From<GuideMarkStyleArg> for GuideMarkStyle {
    fn from(value: GuideMarkStyleArg) -> Self {
        match value {
            GuideMarkStyleArg::Pulse => GuideMarkStyle::Pulse,
            GuideMarkStyleArg::Ring => GuideMarkStyle::Ring,
            GuideMarkStyleArg::Highlight => GuideMarkStyle::Highlight,
        }
    }
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
            description,
            notes,
        } => add_scene(&bundle, id, title, screenshot, description, notes),
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
            trigger,
            scroll_direction,
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
                trigger,
                scroll_direction,
            },
        ),
        Command::AddGuideMark {
            bundle,
            scene,
            id,
            label,
            x,
            y,
            width,
            height,
            style,
        } => add_guide_mark(
            &bundle,
            AddGuideMarkArgs {
                scene,
                id,
                label,
                x,
                y,
                width,
                height,
                style,
            },
        ),
        Command::AddSection {
            bundle,
            id,
            title,
            scenes,
        } => add_section(&bundle, AddSectionArgs { id, title, scenes }),
        Command::AddTransition {
            bundle,
            scene,
            hotspot,
            frames,
            frame_duration_ms,
        } => add_transition(
            &bundle,
            AddTransitionArgs {
                scene,
                hotspot,
                frames,
                frame_duration_ms,
            },
        ),
        Command::ImportVideo {
            bundle,
            input,
            scene_prefix,
            every_seconds,
            crop,
            transcript,
            ffmpeg,
        } => import_video(
            &bundle,
            ImportVideoArgs {
                input,
                scene_prefix,
                every_seconds,
                crop,
                transcript,
                ffmpeg,
            },
        ),
        Command::ExportHtml { bundle, output } => export_html(&bundle, &output),
        Command::ExportDraft { bundle, output } => export_draft(&bundle, &output),
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
    println!("Sections: {}", section_count(&manifest));
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

#[instrument(skip(id, title, screenshot, description, notes), fields(bundle = %bundle.display()))]
fn add_scene(
    bundle: &Path,
    id: String,
    title: String,
    screenshot: Option<String>,
    description: Option<String>,
    notes: Option<String>,
) -> Result<()> {
    let mut manifest = read_manifest(bundle)?;
    let id = SceneId::new(id)?;

    if manifest.graph.scene(&id).is_some() {
        bail!("scene `{id}` already exists");
    }

    let before_report = manifest.validate();
    let mut scene = Scene::screenshot(id.clone(), title, screenshot);
    scene.description = description.filter(|value| !value.trim().is_empty());
    scene.notes = notes.filter(|value| !value.trim().is_empty());
    manifest.add_scene(scene);
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
    trigger: TriggerArg,
    scroll_direction: ScrollDirectionArg,
}

#[derive(Debug)]
struct AddTransitionArgs {
    scene: String,
    hotspot: String,
    frames: Vec<String>,
    frame_duration_ms: u32,
}

#[derive(Debug)]
struct AddGuideMarkArgs {
    scene: String,
    id: String,
    label: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: GuideMarkStyleArg,
}

#[derive(Debug)]
struct AddSectionArgs {
    id: String,
    title: String,
    scenes: Vec<String>,
}

#[instrument(skip(args), fields(bundle = %bundle.display(), scene = %args.scene, hotspot = %args.id))]
fn add_hotspot(bundle: &Path, args: AddHotspotArgs) -> Result<()> {
    let mut manifest = read_manifest(bundle)?;
    let scene_id = SceneId::new(args.scene)?;
    let hotspot_id = HotspotId::new(args.id)?;
    let target = SceneId::new(args.target)?;
    let bounds = Rect::new(args.x, args.y, args.width, args.height);
    let hotspot = Hotspot::new(hotspot_id.clone(), args.label, target, bounds)
        .with_trigger(args.trigger.into())
        .with_scroll_direction(args.scroll_direction.into());
    let before_report = manifest.validate();

    manifest.add_hotspot(&scene_id, hotspot)?;
    reject_introduced_errors("hotspot", &before_report, &manifest.validate())?;

    write_manifest(bundle, &manifest)?;
    info!(scene_id = %scene_id, hotspot_id = %hotspot_id, "added hotspot to bundle");

    println!("Added hotspot {hotspot_id} to scene {scene_id}");
    Ok(())
}

#[instrument(skip(args), fields(bundle = %bundle.display(), scene = %args.scene, guide_mark = %args.id))]
fn add_guide_mark(bundle: &Path, args: AddGuideMarkArgs) -> Result<()> {
    let mut manifest = read_manifest(bundle)?;
    let scene_id = SceneId::new(args.scene)?;
    let guide_mark_id = GuideMarkId::new(args.id)?;
    let bounds = Rect::new(args.x, args.y, args.width, args.height);
    let before_report = manifest.validate();
    let scene = manifest
        .graph
        .scene_mut(&scene_id)
        .ok_or_else(|| anyhow::anyhow!("scene `{scene_id}` does not exist"))?;

    if scene
        .guide_marks
        .iter()
        .any(|candidate| candidate.id == guide_mark_id)
    {
        bail!("scene `{scene_id}` already contains guide mark `{guide_mark_id}`");
    }

    let mut guide_mark = GuideMark::new(guide_mark_id.clone(), args.label, bounds);
    guide_mark.style = args.style.into();
    scene.guide_marks.push(guide_mark);
    reject_introduced_errors("guide mark", &before_report, &manifest.validate())?;

    write_manifest(bundle, &manifest)?;
    info!(scene_id = %scene_id, guide_mark_id = %guide_mark_id, "added guide mark to bundle");

    println!("Added guide mark {guide_mark_id} to scene {scene_id}");
    Ok(())
}

#[instrument(skip(args), fields(bundle = %bundle.display(), section = %args.id))]
fn add_section(bundle: &Path, args: AddSectionArgs) -> Result<()> {
    if args.scenes.is_empty() {
        bail!("--scenes must include at least one scene id");
    }

    let mut manifest = read_manifest(bundle)?;
    if manifest
        .sections
        .iter()
        .any(|section| section.id == args.id)
    {
        bail!("section `{}` already exists", args.id);
    }

    let before_report = manifest.validate();
    let scenes = args
        .scenes
        .into_iter()
        .map(SceneId::new)
        .collect::<Result<Vec<_>, _>>()?;
    manifest.sections.push(Section {
        id: args.id.clone(),
        title: args.title,
        scenes,
    });
    reject_introduced_errors("section", &before_report, &manifest.validate())?;

    write_manifest(bundle, &manifest)?;
    info!(section_id = %args.id, "added section to bundle");

    println!("Added section {}", args.id);
    Ok(())
}

#[instrument(skip(args), fields(bundle = %bundle.display(), scene = %args.scene, hotspot = %args.hotspot))]
fn add_transition(bundle: &Path, args: AddTransitionArgs) -> Result<()> {
    if args.frames.is_empty() {
        bail!("--frames must include at least one transition frame path");
    }
    if args.frame_duration_ms == 0 {
        bail!("--frame-duration-ms must be greater than 0");
    }

    let mut manifest = read_manifest(bundle)?;
    let scene_id = SceneId::new(args.scene)?;
    let hotspot_id = HotspotId::new(args.hotspot)?;
    let before_report = manifest.validate();
    let scene = manifest
        .graph
        .scene_mut(&scene_id)
        .ok_or_else(|| anyhow::anyhow!("scene `{scene_id}` does not exist"))?;
    let hotspot = scene
        .hotspots
        .iter_mut()
        .find(|candidate| candidate.id == hotspot_id)
        .ok_or_else(|| {
            anyhow::anyhow!("scene `{scene_id}` does not contain hotspot `{hotspot_id}`")
        })?;
    hotspot.transition = Some(Transition {
        kind: TransitionKind::FrameSequence,
        frames: args
            .frames
            .into_iter()
            .map(|path| TransitionFrame {
                path: path.replace('\\', "/"),
                duration_ms: None,
            })
            .collect(),
        default_frame_duration_ms: Some(args.frame_duration_ms),
    });

    reject_introduced_errors("transition", &before_report, &manifest.validate())?;
    write_manifest(bundle, &manifest)?;
    info!(scene_id = %scene_id, hotspot_id = %hotspot_id, "attached hotspot transition");

    println!("Added transition to hotspot {hotspot_id} in scene {scene_id}");
    Ok(())
}

#[derive(Debug)]
struct ImportVideoArgs {
    input: PathBuf,
    scene_prefix: Option<String>,
    every_seconds: f32,
    crop: Option<String>,
    transcript: Option<PathBuf>,
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
    let source_id = format!("{prefix}-source");
    if manifest.sources.iter().any(|source| source.id == source_id) {
        bail!("source `{source_id}` already exists");
    }
    let transcript = discover_transcript_sidecar(&args.input, args.transcript.as_deref())?
        .map(|path| {
            parse_transcript(&path, &prefix).map(|cues| (path, group_transcript_cues(cues)))
        })
        .transpose()?;
    if transcript.as_ref().is_some_and(|_| {
        manifest
            .sources
            .iter()
            .any(|source| source.id == format!("{prefix}-transcript"))
    }) {
        bail!("source `{}-transcript` already exists", prefix);
    }
    let output_pattern = captures_dir.join(format!("{prefix}-%04d.png"));
    let ffmpeg = ffmpeg_path(args.ffmpeg);

    let existing_frames = extracted_frames(&captures_dir, &prefix)?;
    if !existing_frames.is_empty() {
        bail!("capture prefix `{prefix}` already has extracted frames");
    }

    if let Some((_, candidates)) = &transcript {
        extract_transcript_candidate_frames(
            &ffmpeg,
            &args.input,
            crop,
            candidates,
            &captures_dir,
            &prefix,
        )?;
    } else {
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

    manifest.sources.push(SourceArtifact {
        id: source_id.clone(),
        kind: SourceArtifactKind::Video,
        label: source_label(&args.input),
        media_type: media_type_for_path(&args.input).map(str::to_owned),
        transcript_segments: Vec::new(),
    });
    if let Some((transcript_path, candidates)) = &transcript {
        manifest.sources.push(SourceArtifact {
            id: format!("{prefix}-transcript"),
            kind: SourceArtifactKind::Transcript,
            label: source_label(transcript_path),
            media_type: transcript_media_type(transcript_path).map(str::to_owned),
            transcript_segments: candidates
                .iter()
                .flat_map(|candidate| candidate.cues.iter().cloned())
                .collect(),
        });
    }

    for (index, frame) in frames.iter().enumerate() {
        let scene_id = frame_scene_ids[index].clone();
        let next_scene_id = frame_scene_ids.get(index + 1).cloned();
        let next_frame = frames.get(index + 1);
        let dimensions = png_dimensions(frame).unwrap_or((1, 1));

        let screenshot = format!("captures/{}", frame.file_name().unwrap().to_string_lossy());
        let candidate = transcript
            .as_ref()
            .map(|(_, candidates)| &candidates[index]);
        let mut scene = Scene::screenshot(
            scene_id,
            candidate
                .map(|_| format!("{prefix} draft {:04}", index + 1))
                .unwrap_or_else(|| format!("{prefix} frame {}", index + 1)),
            Some(screenshot),
        );
        scene.kind = SceneKind::VideoFrame;
        if let Some(candidate) = candidate {
            scene.description = Some(format!("Transcript draft: {}", candidate.text));
            scene.notes = Some(format!(
                "Transcript draft. No clickable UI is inferred from transcript text.\n\n{}",
                candidate.text
            ));
        }
        let mut evidence = vec![EvidenceRef {
            kind: EvidenceKind::Frame,
            id: format!("{}-frame-{:04}", prefix, index + 1),
            source_id: Some(source_id.clone()),
            label: Some(
                candidate
                    .map(|candidate| {
                        format!(
                            "Representative frame at {}",
                            format_timestamp_ms(candidate.representative_timestamp_ms())
                        )
                    })
                    .unwrap_or_else(|| format!("Sampled frame {}", index + 1)),
            ),
        }];
        if let Some(candidate) = candidate {
            evidence.extend(candidate.cues.iter().map(|cue| EvidenceRef {
                kind: EvidenceKind::TranscriptSegment,
                id: cue.id.clone(),
                source_id: Some(format!("{prefix}-transcript")),
                label: Some(cue.text.clone()),
            }));
        }
        scene.provenance = Some(SceneProvenance {
            source_id: source_id.clone(),
            timestamp_ms: Some(
                candidate
                    .map(TranscriptCandidate::representative_timestamp_ms)
                    .unwrap_or_else(|| sample_timestamp_ms(index, args.every_seconds)),
            ),
            transcript_segment_ids: candidate
                .map(|candidate| candidate.cues.iter().map(|cue| cue.id.clone()).collect())
                .unwrap_or_default(),
            transcript_segments: candidate
                .map(|candidate| {
                    candidate
                        .cues
                        .iter()
                        .map(|cue| TranscriptSegmentRef {
                            source_id: format!("{prefix}-transcript"),
                            segment_id: cue.id.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            evidence,
            confidence: Some(if candidate.is_some() { 0.6 } else { 1.0 }),
        });
        if let (Some(target), Some(next_frame)) = (next_scene_id, next_frame) {
            let transition_frame_path = format!(
                "captures/{}",
                next_frame.file_name().unwrap().to_string_lossy()
            );
            scene.hotspots.push(
                Hotspot::new(
                    HotspotId::new("next").expect("static hotspot id is valid"),
                    if candidate.is_some() {
                        "Next draft scene"
                    } else {
                        "Next frame"
                    },
                    target,
                    Rect::new(0.0, 0.0, dimensions.0 as f32, dimensions.1 as f32),
                )
                .with_trigger(InteractionTrigger::Scroll)
                .with_transition(Transition {
                    kind: TransitionKind::FrameSequence,
                    frames: vec![TransitionFrame {
                        path: transition_frame_path,
                        duration_ms: None,
                    }],
                    default_frame_duration_ms: Some(default_transition_duration_ms(
                        args.every_seconds,
                    )),
                }),
            );
        }
        manifest.add_scene(scene);
    }

    if is_untouched_starter(&manifest) {
        manifest
            .graph
            .scenes
            .retain(|scene| scene.id.as_str() != "start");
        manifest.sections = vec![Section {
            id: "main".to_owned(),
            title: "Main".to_owned(),
            scenes: frame_scene_ids.clone(),
        }];
    }
    manifest.graph.start_scene = frame_scene_ids[0].clone();

    reject_introduced_errors("video import", &before_report, &manifest.validate())?;
    write_manifest(bundle, &manifest)?;
    info!(frame_count = frames.len(), "imported video frames");

    println!("Imported {} frame scene(s)", frames.len());
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TranscriptCandidate {
    cues: Vec<TranscriptSegment>,
    start_ms: u64,
    end_ms: u64,
    text: String,
}

impl TranscriptCandidate {
    fn representative_timestamp_ms(&self) -> u64 {
        self.start_ms + (self.end_ms - self.start_ms) / 2
    }
}

const TRANSCRIPT_PAUSE_BOUNDARY_MS: u64 = 1_500;
const TRANSCRIPT_MAX_DURATION_MS: u64 = 15_000;
const TRANSCRIPT_MAX_TEXT_LENGTH: usize = 280;

fn discover_transcript_sidecar(input: &Path, explicit: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(path) = explicit {
        if !path.is_file() {
            bail!(
                "transcript `{}` does not exist or is not a file",
                path.display()
            );
        }
        return Ok(Some(path.to_owned()));
    }

    for extension in ["srt", "vtt"] {
        let candidate = input.with_extension(extension);
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn parse_transcript(path: &Path, prefix: &str) -> Result<Vec<TranscriptSegment>> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read transcript `{}`", path.display()))?;
    let is_vtt = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("vtt"));
    let mut cues = Vec::new();

    for block in contents.replace('\r', "").split("\n\n") {
        let lines = block.lines().collect::<Vec<_>>();
        if lines.is_empty() || (is_vtt && lines[0].trim_start().starts_with("WEBVTT")) {
            continue;
        }
        let Some(timing_index) = lines.iter().position(|line| line.contains("-->")) else {
            continue;
        };
        let Some((start, end)) = lines[timing_index].split_once("-->") else {
            continue;
        };
        let start_ms = parse_timestamp_ms(start.trim()).with_context(|| {
            format!(
                "invalid transcript start timestamp `{}` in `{}`",
                start.trim(),
                path.display()
            )
        })?;
        let end_ms =
            parse_timestamp_ms(end.split_whitespace().next().unwrap_or("")).with_context(|| {
                format!(
                    "invalid transcript end timestamp `{}` in `{}`",
                    end.trim(),
                    path.display()
                )
            })?;
        if end_ms <= start_ms {
            bail!(
                "transcript cue in `{}` must end after it starts",
                path.display()
            );
        }
        let text = lines[timing_index + 1..].join("\n");
        if text.trim().is_empty() {
            continue;
        }
        cues.push(TranscriptSegment {
            id: String::new(),
            start_ms,
            end_ms,
            text,
        });
    }

    if cues.is_empty() {
        bail!(
            "transcript `{}` contains no timed text cues",
            path.display()
        );
    }
    for (index, cue) in cues.iter_mut().enumerate() {
        cue.id = format!("{prefix}-transcript-{:04}", index + 1);
    }
    Ok(cues)
}

fn parse_timestamp_ms(value: &str) -> Result<u64> {
    let normalized = value.replace(',', ".");
    let parts = normalized.split(':').collect::<Vec<_>>();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [hours, minutes, seconds] => (
            hours.parse::<u64>()?,
            minutes.parse::<u64>()?,
            seconds.parse::<f64>()?,
        ),
        [minutes, seconds] => (0, minutes.parse::<u64>()?, seconds.parse::<f64>()?),
        _ => bail!("expected HH:MM:SS.mmm or MM:SS.mmm"),
    };
    if minutes >= 60 || !seconds.is_finite() || !(0.0..60.0).contains(&seconds) {
        bail!("timestamp values are out of range");
    }
    Ok((hours * 3_600_000) + (minutes * 60_000) + (seconds * 1000.0).round() as u64)
}

fn group_transcript_cues(cues: Vec<TranscriptSegment>) -> Vec<TranscriptCandidate> {
    let mut candidates = Vec::new();
    let mut current = Vec::new();
    for cue in cues {
        let should_split = current.last().is_some_and(|previous: &TranscriptSegment| {
            cue.start_ms.saturating_sub(previous.end_ms) >= TRANSCRIPT_PAUSE_BOUNDARY_MS
                || cue.end_ms.saturating_sub(current[0].start_ms) > TRANSCRIPT_MAX_DURATION_MS
                || transcript_text_length(&current) + cue.text.len() > TRANSCRIPT_MAX_TEXT_LENGTH
                || (is_topic_cue(&cue.text) && !current.is_empty())
        });
        if should_split {
            candidates.push(transcript_candidate(std::mem::take(&mut current)));
        }
        current.push(cue);
    }
    if !current.is_empty() {
        candidates.push(transcript_candidate(current));
    }
    candidates
}

fn transcript_candidate(cues: Vec<TranscriptSegment>) -> TranscriptCandidate {
    TranscriptCandidate {
        start_ms: cues.first().expect("candidate has cues").start_ms,
        end_ms: cues.last().expect("candidate has cues").end_ms,
        text: cues
            .iter()
            .map(|cue| cue.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        cues,
    }
}

fn transcript_text_length(cues: &[TranscriptSegment]) -> usize {
    cues.iter().map(|cue| cue.text.len()).sum::<usize>() + cues.len().saturating_sub(1)
}

fn is_topic_cue(text: &str) -> bool {
    let normalized = text.trim_start().to_ascii_lowercase();
    [
        "next ", "now ", "first ", "then ", "finally ", "let's ", "we'll ",
    ]
    .iter()
    .any(|prefix| normalized.starts_with(prefix))
}

fn extract_transcript_candidate_frames(
    ffmpeg: &Path,
    input: &Path,
    crop: Option<Crop>,
    candidates: &[TranscriptCandidate],
    captures_dir: &Path,
    prefix: &str,
) -> Result<()> {
    for (index, candidate) in candidates.iter().enumerate() {
        let output = captures_dir.join(format!("{prefix}-{:04}.png", index + 1));
        let timestamp_seconds = format!(
            "{:.3}",
            candidate.representative_timestamp_ms() as f64 / 1000.0
        );
        let mut command = ProcessCommand::new(ffmpeg);
        command
            .args(["-hide_banner", "-nostdin", "-loglevel", "error", "-ss"])
            .arg(timestamp_seconds)
            .arg("-i")
            .arg(input);
        if let Some(crop) = crop {
            command.args(["-vf", crop.ffmpeg_filter().as_str()]);
        }
        let status = command
            .args(["-frames:v", "1"])
            .arg(&output)
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
    }
    Ok(())
}

fn format_timestamp_ms(timestamp_ms: u64) -> String {
    format!(
        "{:02}:{:02}.{:03}",
        timestamp_ms / 60_000,
        (timestamp_ms / 1_000) % 60,
        timestamp_ms % 1_000
    )
}

fn video_filter(every_seconds: f32, crop: Option<Crop>) -> String {
    let fps = format!("fps=1/{every_seconds}");
    match crop {
        Some(crop) => format!("{},{}", crop.ffmpeg_filter(), fps),
        None => fps,
    }
}

fn default_transition_duration_ms(every_seconds: f32) -> u32 {
    let milliseconds = (every_seconds * 1000.0).round();
    milliseconds.clamp(1.0, u32::MAX as f32) as u32
}

fn sample_timestamp_ms(index: usize, every_seconds: f32) -> u64 {
    ((index as f64) * f64::from(every_seconds) * 1000.0).round() as u64
}

fn source_label(input: &Path) -> String {
    input
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("source video")
        .to_owned()
}

fn media_type_for_path(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mp4") | Some("m4v") => Some("video/mp4"),
        Some("mov") => Some("video/quicktime"),
        Some("webm") => Some("video/webm"),
        Some("mkv") => Some("video/x-matroska"),
        Some("avi") => Some("video/x-msvideo"),
        _ => None,
    }
}

fn transcript_media_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("srt") => Some("application/x-subrip"),
        Some("vtt") => Some("text/vtt"),
        _ => None,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorialDraft {
    format: &'static str,
    schema_version: u32,
    draft: EditorialDraftMetadata,
    assets: Vec<EditorialDraftAsset>,
    scenes: Vec<EditorialDraftScene>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorialDraftMetadata {
    id: String,
    title: String,
    description: Option<String>,
    source: EditorialDraftSource,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorialDraftSource {
    system: &'static str,
    id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorialDraftAsset {
    id: String,
    path: String,
    mime_type: String,
    sha256: String,
    #[serde(skip_serializing)]
    bundle_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorialDraftScene {
    id: String,
    start_ms: u64,
    end_ms: u64,
    transcript: String,
    actions: Vec<String>,
    screenshot_asset_id: String,
    source_segment_ids: Vec<String>,
    source_segments: Vec<EditorialDraftTranscriptSegment>,
    notes: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorialDraftTranscriptSegment {
    source_id: String,
    id: String,
    start_ms: u64,
    end_ms: u64,
    text: String,
}

#[instrument(fields(bundle = %bundle.display(), output = %output.display()))]
fn export_draft(bundle: &Path, output: &Path) -> Result<()> {
    let manifest = read_manifest(bundle)?;
    reject_validation_errors("bundle", &manifest.validate())?;
    let file_report = validate_referenced_files(bundle, &manifest);
    if !file_report.is_valid() {
        for missing_file in &file_report.missing_files {
            eprintln!("error: {missing_file}");
        }
        bail!(
            "bundle has {} missing referenced file(s)",
            file_report.missing_files.len()
        );
    }
    let transcript_sources = manifest
        .sources
        .iter()
        .map(|source| (source.id.as_str(), source))
        .collect::<HashMap<_, _>>();
    let mut scenes = Vec::new();
    let mut assets = Vec::new();
    for scene in &manifest.graph.scenes {
        let Some(provenance) = &scene.provenance else {
            continue;
        };
        if provenance.transcript_segments.is_empty() {
            continue;
        }
        let source_segments = provenance
            .transcript_segments
            .iter()
            .map(|segment_ref| {
                let source = transcript_sources
                    .get(segment_ref.source_id.as_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "scene `{}` references missing transcript source `{}`",
                            scene.id,
                            segment_ref.source_id
                        )
                    })?;
                let segment = source
                    .transcript_segments
                    .iter()
                    .find(|segment| segment.id == segment_ref.segment_id)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "scene `{}` references missing transcript segment `{}`",
                            scene.id,
                            segment_ref.segment_id
                        )
                    })?;
                Ok(editorial_transcript_segment(&source.id, segment))
            })
            .collect::<Result<Vec<_>>>()?;
        let start_ms = source_segments
            .iter()
            .map(|segment| segment.start_ms)
            .min()
            .expect("transcript-backed draft scene has transcript segments");
        let end_ms = source_segments
            .iter()
            .map(|segment| segment.end_ms)
            .max()
            .expect("transcript-backed draft scene has transcript segments");
        let screenshot = scene.assets.screenshot.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "transcript-backed draft scene `{}` has no screenshot asset",
                scene.id
            )
        })?;
        let asset_id = format!("{}-frame", scene.id);
        let asset_path = draft_asset_path(&asset_id, screenshot);
        let source_path = bundle.join(screenshot);
        assets.push(EditorialDraftAsset {
            id: asset_id.clone(),
            path: asset_path,
            mime_type: asset_media_type(&source_path).to_owned(),
            sha256: sha256_file(&source_path)?,
            bundle_path: screenshot.to_owned(),
        });
        scenes.push(EditorialDraftScene {
            id: scene.id.as_str().to_owned(),
            start_ms,
            end_ms,
            transcript: source_segments
                .iter()
                .map(|segment| segment.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            actions: Vec::new(),
            screenshot_asset_id: asset_id,
            source_segment_ids: source_segments
                .iter()
                .map(|segment| format!("{}:{}", segment.source_id, segment.id))
                .collect(),
            source_segments,
            notes: scene.notes.clone(),
        });
    }
    if scenes.is_empty() {
        bail!("bundle contains no transcript-backed draft scenes");
    }

    let draft = EditorialDraft {
        format: "editorial-draft",
        schema_version: 1,
        draft: EditorialDraftMetadata {
            id: format!("scenecast-{}", scenes[0].id),
            title: manifest.title,
            description: None,
            source: EditorialDraftSource {
                system: "scenecast",
                id: manifest.graph.start_scene.as_str().to_owned(),
            },
        },
        assets,
        scenes,
    };
    fs::create_dir_all(output)
        .with_context(|| format!("failed to create output directory `{}`", output.display()))?;
    copy_draft_assets(bundle, output, &draft)?;
    let draft_path = output.join("draft.json");
    let contents = serde_json::to_string_pretty(&draft)?;
    fs::write(&draft_path, format!("{contents}\n"))
        .with_context(|| format!("failed to write `{}`", draft_path.display()))?;
    info!(scene_count = draft.scenes.len(), "exported editorial draft");

    println!("Exported {}", draft_path.display());
    Ok(())
}

fn editorial_transcript_segment(
    source_id: &str,
    segment: &TranscriptSegment,
) -> EditorialDraftTranscriptSegment {
    EditorialDraftTranscriptSegment {
        source_id: source_id.to_owned(),
        id: segment.id.clone(),
        start_ms: segment.start_ms,
        end_ms: segment.end_ms,
        text: segment.text.clone(),
    }
}

fn copy_draft_assets(bundle: &Path, output: &Path, draft: &EditorialDraft) -> Result<()> {
    for asset in &draft.assets {
        let source = bundle.join(&asset.bundle_path);
        let destination = output.join(&asset.path);
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

fn draft_asset_path(asset_id: &str, screenshot: &str) -> String {
    let extension = Path::new(screenshot)
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .unwrap_or("png");
    format!("assets/{asset_id}.{extension}")
}

fn asset_media_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("gif") => "image/gif",
        Some("avif") => "image/avif",
        Some("bmp") => "image/bmp",
        Some("tif") | Some("tiff") => "image/tiff",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read `{}`", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn source_kind_name(kind: SourceArtifactKind) -> &'static str {
    match kind {
        SourceArtifactKind::Video => "video",
        SourceArtifactKind::Transcript => "transcript",
        SourceArtifactKind::Trace => "trace",
        SourceArtifactKind::Manual => "manual",
        SourceArtifactKind::Other => "other",
    }
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
    let player_data = render_player_data(manifest);

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
#capture, #video-capture {{ position: fixed; inset: 0; display: block; width: 100vw; height: 100vh; max-width: none; max-height: none; object-fit: fill; }}
#video-capture {{ display: none; }}
#guide-layer, #debug-layer {{ position: fixed; inset: 0; pointer-events: none; }}
.guide-mark {{ position: absolute; border: 3px solid rgba(0, 255, 247, 0.92); border-radius: 999px; box-shadow: 0 0 0 6px rgba(0, 255, 247, 0.18), 0 0 26px rgba(0, 255, 247, 0.55); opacity: 0.95; }}
.guide-mark[data-style="pulse"] {{ animation: scenecast-pulse 1.35s ease-in-out infinite; }}
.guide-mark[data-style="highlight"] {{ border-radius: 12px; background: rgba(0, 255, 247, 0.14); }}
@keyframes scenecast-pulse {{ 0%, 100% {{ transform: scale(1); opacity: 0.9; }} 50% {{ transform: scale(1.035); opacity: 0.55; }} }}
.hotspot-debug {{ position: absolute; border: 1px dashed rgba(255, 214, 102, 0.9); background: rgba(255, 214, 102, 0.14); color: rgba(255, 255, 255, 0.95); font: 12px/1.2 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; padding: 2px 4px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }}
#toc-toggle {{ position: fixed; top: 16px; left: 16px; z-index: 4; border: 0; border-radius: 999px; background: rgba(18, 18, 18, 0.78); color: #fff; font: 600 14px/1 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; padding: 10px 14px; backdrop-filter: blur(10px); cursor: pointer; }}
#toc {{ position: fixed; top: 58px; left: 16px; z-index: 4; width: min(360px, calc(100vw - 32px)); max-height: calc(100vh - 76px); overflow: auto; border-radius: 16px; background: rgba(18, 18, 18, 0.88); color: #fff; font: 14px/1.35 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; padding: 14px; box-shadow: 0 14px 48px rgba(0, 0, 0, 0.38); backdrop-filter: blur(14px); }}
#toc[hidden] {{ display: none; }}
.toc-title {{ margin: 0 0 10px; font-weight: 700; }}
.toc-section {{ margin: 12px 0 6px; color: rgba(255, 255, 255, 0.72); font-size: 12px; text-transform: uppercase; letter-spacing: 0.08em; }}
.toc-scene {{ display: block; width: 100%; border: 0; border-radius: 10px; background: transparent; color: inherit; text-align: left; padding: 8px 10px; cursor: pointer; }}
.toc-scene:hover, .toc-scene[aria-current="true"] {{ background: rgba(0, 255, 247, 0.16); }}
.sr-only {{ position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }}
</style>
</head>
<body>
<img id="capture" alt="">
<video id="video-capture" playsinline muted autoplay loop></video>
<div id="guide-layer" aria-hidden="true"></div>
<div id="debug-layer" hidden></div>
<button id="toc-toggle" type="button" aria-expanded="false" aria-controls="toc">Scenes</button>
<nav id="toc" hidden aria-label="Scene table of contents"></nav>
<div id="scene-status" class="sr-only" aria-live="polite"></div>
<script>
const playerData = {player_data};
const bundleBase = new URL("./", location.href);
const startScene = playerData.startScene;
const scenes = playerData.scenes;
const sceneOrder = playerData.sceneOrder;
const sections = playerData.sections;
const image = document.getElementById("capture");
const video = document.getElementById("video-capture");
const guideLayer = document.getElementById("guide-layer");
const debugLayer = document.getElementById("debug-layer");
const tocToggle = document.getElementById("toc-toggle");
const toc = document.getElementById("toc");
const sceneStatus = document.getElementById("scene-status");
const cacheKey = new URLSearchParams(location.search).get("v") || String(Date.now());
const debugHotspots = new URLSearchParams(location.search).get("debug") === "1";
let currentSceneId = parseSceneFromHash() || startScene;
let transitionRunId = 0;
let isTransitioning = false;
const imagePreloads = new Map();

function sceneForCurrent() {{
  return scenes[currentSceneId];
}}

function capturePoint(clientX, clientY) {{
  const width = image.naturalWidth || window.innerWidth || 1;
  const height = image.naturalHeight || window.innerHeight || 1;
  const x = clientX * (width / window.innerWidth);
  const y = clientY * (height / window.innerHeight);
  return {{ x, y, width, height }};
}}

function scrollDirectionMatches(hotspot, deltaY) {{
  const direction = hotspot.scrollDirection || "any";
  if (direction === "any") return true;
  if (direction === "down") return deltaY > 0;
  if (direction === "up") return deltaY < 0;
  return true;
}}

function hotspotAt(scene, clientX, clientY, trigger, deltaY = 0) {{
  if (!scene) return null;
  const point = capturePoint(clientX, clientY);
  return scene.hotspots.find((hotspot) => hotspot.trigger === trigger
    && (trigger !== "scroll" || scrollDirectionMatches(hotspot, deltaY))
    && point.x >= hotspot.x
    && point.x <= hotspot.x + hotspot.width
    && point.y >= hotspot.y
    && point.y <= hotspot.y + hotspot.height);
}}

function frameUrl(path) {{
  if (!path) return "";
  const frame = new URL(path, bundleBase);
  frame.searchParams.set("scenecast-cache", cacheKey);
  return frame.href;
}}

function sleep(durationMs) {{
  return new Promise((resolve) => setTimeout(resolve, durationMs));
}}

function preloadImage(url) {{
  if (!url) return Promise.resolve();
  if (imagePreloads.has(url)) return imagePreloads.get(url);

  const preload = new Promise((resolve) => {{
    const loader = new Image();
    loader.onload = () => resolve();
    loader.onerror = () => resolve();
    loader.src = url;
  }});
  imagePreloads.set(url, preload);
  return preload;
}}

function preloadScene(scene) {{
  if (!scene) return;
  if (scene.screenshot) void preloadImage(frameUrl(scene.screenshot));
  for (const hotspot of scene.hotspots) {{
    const target = scenes[hotspot.target];
    if (target?.screenshot) void preloadImage(frameUrl(target.screenshot));
    const frames = hotspot.transition?.frames || [];
    for (const frame of frames) {{
      if (frame.path) void preloadImage(frameUrl(frame.path));
    }}
  }}
}}

async function playTransition(hotspot, runId) {{
  const transition = hotspot.transition;
  if (!transition || !Array.isArray(transition.frames) || transition.frames.length === 0) {{
    return;
  }}

  for (const frame of transition.frames) {{
    if (runId !== transitionRunId) return;
    const url = frameUrl(frame.path);
    await preloadImage(url);
    if (runId !== transitionRunId) return;
    image.src = url;
    const duration = frame.durationMs ?? transition.defaultFrameDurationMs ?? 90;
    await sleep(Math.max(1, duration));
  }}
}}

function routeForScene(sceneId) {{
  for (let sectionIndex = 0; sectionIndex < sections.length; sectionIndex += 1) {{
    const screenIndex = sections[sectionIndex].scenes.indexOf(sceneId);
    if (screenIndex >= 0) return `#/${{sectionIndex}}/${{screenIndex}}`;
  }}
  return `#${{sceneId}}`;
}}

function parseSceneFromHash() {{
  const hash = location.hash.slice(1);
  if (!hash) return null;
  const match = hash.match(/^\/(\d+)\/(\d+)$/);
  if (match) {{
    const section = sections[Number(match[1])];
    return section?.scenes?.[Number(match[2])] || null;
  }}
  return scenes[hash] ? hash : null;
}}

function mediaSize() {{
  if (video.style.display !== "none" && video.videoWidth && video.videoHeight) {{
    return {{ width: video.videoWidth, height: video.videoHeight }};
  }}
  return {{
    width: image.naturalWidth || scenes[currentSceneId]?.width || window.innerWidth || 1,
    height: image.naturalHeight || scenes[currentSceneId]?.height || window.innerHeight || 1
  }};
}}

function renderDebugHotspots() {{
  if (!debugHotspots) return;
  if (isTransitioning) {{
    debugLayer.replaceChildren();
    debugLayer.hidden = true;
    return;
  }}
  debugLayer.hidden = false;
  debugLayer.replaceChildren();
  const scene = sceneForCurrent();
  if (!scene) return;

  const size = mediaSize();
  const widthScale = window.innerWidth / size.width;
  const heightScale = window.innerHeight / size.height;
  for (const hotspot of scene.hotspots) {{
    const element = document.createElement("div");
    element.className = "hotspot-debug";
    const scrollIcon = hotspot.scrollDirection === "up" ? "\u21e7 " : hotspot.scrollDirection === "down" ? "\u21e9 " : "\u21f5 ";
    element.textContent = (hotspot.trigger === "scroll" ? scrollIcon : "\u25cf ") + hotspot.id;
    element.style.left = (hotspot.x * widthScale) + "px";
    element.style.top = (hotspot.y * heightScale) + "px";
    element.style.width = Math.max(1, hotspot.width * widthScale) + "px";
    element.style.height = Math.max(1, hotspot.height * heightScale) + "px";
    debugLayer.appendChild(element);
  }}
}}

function renderGuideMarks() {{
  guideLayer.replaceChildren();
  if (isTransitioning) return;
  const scene = sceneForCurrent();
  if (!scene) return;
  const size = mediaSize();
  const widthScale = window.innerWidth / size.width;
  const heightScale = window.innerHeight / size.height;
  for (const guideMark of scene.guideMarks) {{
    const element = document.createElement("div");
    element.className = "guide-mark";
    element.dataset.style = guideMark.style;
    element.title = guideMark.label;
    element.style.left = (guideMark.x * widthScale) + "px";
    element.style.top = (guideMark.y * heightScale) + "px";
    element.style.width = Math.max(1, guideMark.width * widthScale) + "px";
    element.style.height = Math.max(1, guideMark.height * heightScale) + "px";
    guideLayer.appendChild(element);
  }}
}}

function clearSceneOverlays() {{
  guideLayer.replaceChildren();
  debugLayer.replaceChildren();
  debugLayer.hidden = true;
}}

function renderToc() {{
  toc.replaceChildren();
  const title = document.createElement("p");
  title.className = "toc-title";
  title.textContent = playerData.title;
  toc.appendChild(title);
  for (const section of sections) {{
    const heading = document.createElement("div");
    heading.className = "toc-section";
    heading.textContent = section.title;
    toc.appendChild(heading);
    for (const sceneId of section.scenes) {{
      const scene = scenes[sceneId];
      if (!scene) continue;
      const button = document.createElement("button");
      button.type = "button";
      button.className = "toc-scene";
      button.textContent = scene.title;
      button.setAttribute("aria-current", String(sceneId === currentSceneId));
      button.addEventListener("click", () => {{
        show(sceneId);
        setTocOpen(false);
      }});
      toc.appendChild(button);
    }}
  }}
}}

function setTocOpen(open) {{
  toc.hidden = !open;
  tocToggle.setAttribute("aria-expanded", String(open));
}}

function show(sceneId) {{
  const scene = scenes[sceneId];
  if (!scene) return;
  currentSceneId = sceneId;
  const nextHash = routeForScene(sceneId);
  if (location.hash !== nextHash) location.hash = nextHash;
  video.pause();
  video.removeAttribute("src");
  video.style.display = "none";
  image.style.display = "block";
  if (scene.screenshot) {{
    image.src = frameUrl(scene.screenshot);
  }}
  if (scene.video) {{
    video.src = frameUrl(scene.video);
    video.style.display = "block";
    image.style.display = "none";
    void video.play();
  }}
  image.alt = scene.description || scene.title;
  sceneStatus.textContent = scene.description || scene.title;
  preloadScene(scene);
  renderGuideMarks();
  renderDebugHotspots();
  renderToc();
}}

async function activateHotspot(hotspot) {{
  if (isTransitioning) return;
  isTransitioning = true;
  const runId = ++transitionRunId;
  clearSceneOverlays();
  try {{
    await playTransition(hotspot, runId);
    if (runId !== transitionRunId) return;
    isTransitioning = false;
    show(hotspot.target);
  }} finally {{
    if (runId === transitionRunId) isTransitioning = false;
  }}
}}

document.body.addEventListener("click", (event) => {{
  const scene = sceneForCurrent();
  const hotspot = hotspotAt(scene, event.clientX, event.clientY, "click");
  if (!hotspot) return;
  void activateHotspot(hotspot);
}});

document.body.addEventListener("wheel", (event) => {{
  if (isTransitioning) {{
    event.preventDefault();
    return;
  }}
  const scene = sceneForCurrent();
  const hotspot = hotspotAt(scene, event.clientX, event.clientY, "scroll", event.deltaY);
  if (!hotspot) return;
  event.preventDefault();
  void activateHotspot(hotspot);
}}, {{ passive: false }});

function moveBy(offset) {{
  const index = sceneOrder.indexOf(currentSceneId);
  const target = sceneOrder[index + offset];
  if (target) show(target);
}}

document.addEventListener("keydown", (event) => {{
  if (event.target && ["INPUT", "TEXTAREA", "SELECT", "BUTTON"].includes(event.target.tagName)) return;
  if (event.key === "ArrowRight" || event.key === " " || event.key === "PageDown") {{
    event.preventDefault();
    moveBy(1);
  }} else if (event.key === "ArrowLeft" || event.key === "PageUp") {{
    event.preventDefault();
    moveBy(-1);
  }} else if (event.key.toLowerCase() === "t") {{
    setTocOpen(toc.hidden);
  }} else if (event.key === "Escape") {{
    setTocOpen(false);
  }}
}});

tocToggle.addEventListener("click", () => setTocOpen(toc.hidden));
window.addEventListener("hashchange", () => show(parseSceneFromHash() || startScene));
window.addEventListener("resize", () => {{ renderGuideMarks(); renderDebugHotspots(); }});
image.addEventListener("load", () => {{ renderGuideMarks(); renderDebugHotspots(); }});
video.addEventListener("loadedmetadata", () => {{ renderGuideMarks(); renderDebugHotspots(); }});
renderToc();
show(currentSceneId);
</script>
</body>
</html>
"#,
        title = escape_html(&manifest.title),
        player_data = script_safe_json(&player_data)
    )
}

fn render_player_data(manifest: &BundleManifest) -> serde_json::Value {
    let scene_order = manifest
        .graph
        .scenes
        .iter()
        .map(|scene| scene.id.as_str().to_owned())
        .collect::<Vec<_>>();
    let sections = export_sections(manifest, &scene_order);
    let sources = manifest
        .sources
        .iter()
        .map(|source| {
            json!({
                "id": source.id,
                "kind": source_kind_name(source.kind),
                "label": source.label,
                "mediaType": source.media_type,
                "transcriptSegments": source.transcript_segments.iter().map(|segment| {
                    json!({
                        "id": segment.id,
                        "startMs": segment.start_ms,
                        "endMs": segment.end_ms,
                        "text": segment.text,
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    let scenes = manifest
        .graph
        .scenes
        .iter()
        .map(|scene| {
            let hotspots = scene
                .hotspots
                .iter()
                .map(|hotspot| {
                    let transition = hotspot.transition.as_ref().map(|transition| {
                        let kind = match transition.kind {
                            TransitionKind::FrameSequence => "frame-sequence",
                        };
                        json!({
                            "kind": kind,
                            "defaultFrameDurationMs": transition.default_frame_duration_ms,
                            "frames": transition.frames.iter().map(|frame| {
                                json!({
                                    "path": frame.path,
                                    "durationMs": frame.duration_ms,
                                })
                            }).collect::<Vec<_>>(),
                        })
                    });
                    let trigger = match hotspot.trigger {
                        InteractionTrigger::Click => "click",
                        InteractionTrigger::Scroll => "scroll",
                    };
                    let scroll_direction = match hotspot.scroll_direction {
                        ScrollDirection::Any => "any",
                        ScrollDirection::Down => "down",
                        ScrollDirection::Up => "up",
                    };
                    json!({
                        "id": hotspot.id.as_str(),
                        "label": hotspot.label,
                        "target": hotspot.target.as_str(),
                        "trigger": trigger,
                        "scrollDirection": scroll_direction,
                        "x": hotspot.bounds.x,
                        "y": hotspot.bounds.y,
                        "width": hotspot.bounds.width,
                        "height": hotspot.bounds.height,
                        "transition": transition,
                    })
                })
                .collect::<Vec<_>>();
            let guide_marks = scene
                .guide_marks
                .iter()
                .map(|guide_mark| {
                    let style = match guide_mark.style {
                        GuideMarkStyle::Pulse => "pulse",
                        GuideMarkStyle::Ring => "ring",
                        GuideMarkStyle::Highlight => "highlight",
                    };
                    json!({
                        "id": guide_mark.id.as_str(),
                        "label": guide_mark.label,
                        "style": style,
                        "x": guide_mark.bounds.x,
                        "y": guide_mark.bounds.y,
                        "width": guide_mark.bounds.width,
                        "height": guide_mark.bounds.height,
                    })
                })
                .collect::<Vec<_>>();
            let provenance = scene.provenance.as_ref().map(|provenance| {
                json!({
                    "sourceId": provenance.source_id,
                    "timestampMs": provenance.timestamp_ms,
                    "transcriptSegmentIds": provenance.transcript_segment_ids,
                    "transcriptSegments": provenance.transcript_segments.iter().map(|segment| {
                        json!({
                            "sourceId": segment.source_id,
                            "segmentId": segment.segment_id,
                        })
                    }).collect::<Vec<_>>(),
                    "evidence": provenance.evidence.iter().map(|evidence| {
                        let kind = match evidence.kind {
                            EvidenceKind::Frame => "frame",
                            EvidenceKind::TranscriptSegment => "transcript-segment",
                            EvidenceKind::Event => "event",
                            EvidenceKind::Heuristic => "heuristic",
                            EvidenceKind::Agent => "agent",
                            EvidenceKind::Other => "other",
                        };
                        json!({
                            "kind": kind,
                            "id": evidence.id,
                            "sourceId": evidence.source_id,
                            "label": evidence.label,
                        })
                    }).collect::<Vec<_>>(),
                    "confidence": provenance.confidence,
                })
            });
            (
                scene.id.as_str().to_owned(),
                json!({
                    "title": scene.title,
                    "description": scene.description,
                    "notes": scene.notes,
                    "screenshot": scene.assets.screenshot,
                    "video": scene.assets.video,
                    "hotspots": hotspots,
                    "guideMarks": guide_marks,
                    "provenance": provenance,
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();

    json!({
        "title": manifest.title,
        "startScene": manifest.graph.start_scene.as_str(),
        "sceneOrder": scene_order,
        "sections": sections,
        "sources": sources,
        "scenes": scenes,
    })
}

fn export_sections(manifest: &BundleManifest, scene_order: &[String]) -> Vec<serde_json::Value> {
    let mut sections = if manifest.sections.is_empty() {
        vec![json!({
            "id": "main",
            "title": "Scenes",
            "scenes": scene_order,
        })]
    } else {
        manifest
            .sections
            .iter()
            .map(|section| {
                json!({
                    "id": section.id,
                    "title": section.title,
                    "scenes": section
                        .scenes
                        .iter()
                        .map(|scene_id| scene_id.as_str())
                        .collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>()
    };

    let sectioned_scene_ids = manifest
        .sections
        .iter()
        .flat_map(|section| section.scenes.iter().map(SceneId::as_str))
        .collect::<HashSet<_>>();
    let unsectioned = scene_order
        .iter()
        .filter(|scene_id| !sectioned_scene_ids.contains(scene_id.as_str()))
        .collect::<Vec<_>>();
    if !manifest.sections.is_empty() && !unsectioned.is_empty() {
        sections.push(json!({
            "id": "unsectioned",
            "title": "Unsectioned",
            "scenes": unsectioned,
        }));
    }

    sections
}

fn section_count(manifest: &BundleManifest) -> usize {
    if manifest.sections.is_empty() {
        1
    } else {
        manifest.sections.len()
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn script_safe_json(value: &serde_json::Value) -> String {
    serde_json::to_string(value)
        .expect("player data is serializable")
        .replace("</script", "<\\/script")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_webvtt_cues_with_stable_segment_ids() {
        let temp = tempfile::tempdir().unwrap();
        let transcript = temp.path().join("demo.vtt");
        fs::write(
            &transcript,
            "WEBVTT\n\n00:00.000 --> 00:01.250\nFirst cue.\n\n00:03.000 --> 00:04.000 align:start\nSecond cue.\n",
        )
        .unwrap();

        let cues = parse_transcript(&transcript, "demo").unwrap();

        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].id, "demo-transcript-0001");
        assert_eq!(cues[0].start_ms, 0);
        assert_eq!(cues[0].end_ms, 1_250);
        assert_eq!(cues[1].id, "demo-transcript-0002");
        assert_eq!(cues[1].start_ms, 3_000);
    }

    #[test]
    fn preserves_multiline_transcript_cue_text() {
        let temp = tempfile::tempdir().unwrap();
        let transcript = temp.path().join("demo.srt");
        fs::write(
            &transcript,
            "1\n00:00:00,000 --> 00:00:01,000\nFirst line.\nSecond line.\n",
        )
        .unwrap();

        let cues = parse_transcript(&transcript, "demo").unwrap();

        assert_eq!(cues[0].text, "First line.\nSecond line.");
    }

    #[test]
    fn groups_cues_at_pause_boundaries() {
        let cues = vec![
            TranscriptSegment {
                id: "demo-transcript-0001".to_owned(),
                start_ms: 0,
                end_ms: 1_000,
                text: "First cue.".to_owned(),
            },
            TranscriptSegment {
                id: "demo-transcript-0002".to_owned(),
                start_ms: 1_200,
                end_ms: 2_000,
                text: "Second cue.".to_owned(),
            },
            TranscriptSegment {
                id: "demo-transcript-0003".to_owned(),
                start_ms: 4_000,
                end_ms: 5_000,
                text: "Third cue.".to_owned(),
            },
        ];

        let candidates = group_transcript_cues(cues);

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].cues.len(), 2);
        assert_eq!(candidates[0].representative_timestamp_ms(), 1_000);
        assert_eq!(candidates[1].cues[0].id, "demo-transcript-0003");
    }
}
