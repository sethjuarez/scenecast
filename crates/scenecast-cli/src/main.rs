use std::fs;
use std::path::{Path, PathBuf};

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
