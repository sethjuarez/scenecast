use std::collections::HashSet;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

pub const MANIFEST_FILE_NAME: &str = "manifest.json";
pub const SCHEMA_VERSION: &str = "scenecast.bundle.v1";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SceneId(String);

impl SceneId {
    pub fn new(value: impl Into<String>) -> Result<Self, SceneIdError> {
        let value = value.into();
        if let Err(reason) = validate_identifier(&value) {
            return Err(SceneIdError { value, reason });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SceneId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for SceneId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SceneId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid scene id `{value}`: {reason}")]
pub struct SceneIdError {
    value: String,
    reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HotspotId(String);

impl HotspotId {
    pub fn new(value: impl Into<String>) -> Result<Self, HotspotIdError> {
        let value = value.into();
        if let Err(reason) = validate_identifier(&value) {
            return Err(HotspotIdError { value, reason });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HotspotId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GuideMarkId(String);

impl GuideMarkId {
    pub fn new(value: impl Into<String>) -> Result<Self, GuideMarkIdError> {
        let value = value.into();
        if let Err(reason) = validate_identifier(&value) {
            return Err(GuideMarkIdError { value, reason });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GuideMarkId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for GuideMarkId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for GuideMarkId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid guide mark id `{value}`: {reason}")]
pub struct GuideMarkIdError {
    value: String,
    reason: &'static str,
}

impl Serialize for HotspotId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for HotspotId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid hotspot id `{value}`: {reason}")]
pub struct HotspotIdError {
    value: String,
    reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleManifest {
    pub schema_version: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<Section>,
    pub graph: SceneGraph,
    #[serde(default)]
    pub assets: Vec<BundleAsset>,
}

impl BundleManifest {
    pub fn starter(title: impl Into<String>) -> Self {
        let title = title.into();
        let start_scene = SceneId::new("start").expect("static scene id is valid");

        Self {
            schema_version: SCHEMA_VERSION.to_owned(),
            title: title.clone(),
            sections: vec![Section {
                id: "main".to_owned(),
                title: "Main".to_owned(),
                scenes: vec![start_scene.clone()],
            }],
            graph: SceneGraph {
                start_scene: start_scene.clone(),
                scenes: vec![Scene {
                    id: start_scene,
                    title: "Start".to_owned(),
                    kind: SceneKind::Screenshot,
                    assets: SceneAssets::default(),
                    hotspots: Vec::new(),
                    guide_marks: Vec::new(),
                    description: None,
                    notes: Some(format!("Starter scene for {title}")),
                }],
            },
            assets: Vec::new(),
        }
    }

    pub fn add_scene(&mut self, scene: Scene) {
        self.graph.scenes.push(scene);
    }

    pub fn add_hotspot(
        &mut self,
        scene_id: &SceneId,
        hotspot: Hotspot,
    ) -> Result<(), AddHotspotError> {
        let scene =
            self.graph
                .scene_mut(scene_id)
                .ok_or_else(|| AddHotspotError::MissingScene {
                    scene_id: scene_id.clone(),
                })?;

        if scene.hotspot(&hotspot.id).is_some() {
            return Err(AddHotspotError::DuplicateHotspotId {
                scene_id: scene_id.clone(),
                hotspot_id: hotspot.id,
            });
        }

        scene.hotspots.push(hotspot);
        Ok(())
    }

    pub fn referenced_asset_paths(&self) -> Vec<&str> {
        let mut paths = Vec::new();

        paths.extend(self.assets.iter().map(|asset| asset.path.as_str()));
        for scene in &self.graph.scenes {
            if let Some(path) = scene.assets.screenshot.as_deref() {
                paths.push(path);
            }
            if let Some(path) = scene.assets.video.as_deref() {
                paths.push(path);
            }
            for hotspot in &scene.hotspots {
                if let Some(transition) = &hotspot.transition {
                    paths.extend(transition.frames.iter().map(|frame| frame.path.as_str()));
                }
            }
        }

        paths
    }

    pub fn validate(&self) -> ValidationReport {
        let mut report = ValidationReport::default();

        if self.schema_version != SCHEMA_VERSION {
            report
                .errors
                .push(ValidationError::UnsupportedSchemaVersion {
                    expected: SCHEMA_VERSION.to_owned(),
                    actual: self.schema_version.clone(),
                });
        }

        if self.title.trim().is_empty() {
            report.errors.push(ValidationError::EmptyTitle);
        }

        let mut asset_paths = HashSet::new();
        for asset in &self.assets {
            if let Err(reason) = validate_portable_asset_path(&asset.path) {
                report.errors.push(ValidationError::InvalidAssetPath {
                    path: asset.path.clone(),
                    reason,
                });
            }

            if !asset_paths.insert(asset.path.clone()) {
                report.errors.push(ValidationError::DuplicateAssetPath {
                    path: asset.path.clone(),
                });
            }

            if asset.media_type.trim().is_empty() {
                report.errors.push(ValidationError::EmptyAssetMediaType {
                    path: asset.path.clone(),
                });
            }
        }

        report.extend(validate_sections(&self.sections, &self.graph));
        report.extend(self.graph.validate());
        report
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub scenes: Vec<SceneId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AddHotspotError {
    #[error("scene `{scene_id}` does not exist")]
    MissingScene { scene_id: SceneId },
    #[error("scene `{scene_id}` already contains hotspot `{hotspot_id}`")]
    DuplicateHotspotId {
        scene_id: SceneId,
        hotspot_id: HotspotId,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleAsset {
    pub path: String,
    pub media_type: String,
    #[serde(default)]
    pub role: AssetRole,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetRole {
    #[default]
    Capture,
    Thumbnail,
    Supporting,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneGraph {
    pub start_scene: SceneId,
    pub scenes: Vec<Scene>,
}

impl SceneGraph {
    pub fn scene(&self, id: &SceneId) -> Option<&Scene> {
        self.scenes.iter().find(|scene| scene.id == *id)
    }

    pub fn scene_mut(&mut self, id: &SceneId) -> Option<&mut Scene> {
        self.scenes.iter_mut().find(|scene| scene.id == *id)
    }

    pub fn validate(&self) -> ValidationReport {
        let mut report = ValidationReport::default();
        let mut scene_ids = HashSet::new();

        if self.scenes.is_empty() {
            report.errors.push(ValidationError::NoScenes);
            return report;
        }

        for scene in &self.scenes {
            if !scene_ids.insert(scene.id.clone()) {
                report.errors.push(ValidationError::DuplicateSceneId {
                    scene_id: scene.id.clone(),
                });
            }

            if scene.title.trim().is_empty() {
                report.errors.push(ValidationError::EmptySceneTitle {
                    scene_id: scene.id.clone(),
                });
            }

            report.extend(scene.validate());
        }

        if !scene_ids.contains(&self.start_scene) {
            report.errors.push(ValidationError::MissingStartScene {
                scene_id: self.start_scene.clone(),
            });
        }

        for scene in &self.scenes {
            for hotspot in &scene.hotspots {
                if !scene_ids.contains(&hotspot.target) {
                    report.errors.push(ValidationError::MissingHotspotTarget {
                        scene_id: scene.id.clone(),
                        hotspot_id: hotspot.id.clone(),
                        target: hotspot.target.clone(),
                    });
                }
            }
        }

        report
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub id: SceneId,
    pub title: String,
    #[serde(default)]
    pub kind: SceneKind,
    #[serde(default)]
    pub assets: SceneAssets,
    #[serde(default)]
    pub hotspots: Vec<Hotspot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guide_marks: Vec<GuideMark>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl Scene {
    pub fn screenshot(id: SceneId, title: impl Into<String>, screenshot: Option<String>) -> Self {
        Self {
            id,
            title: title.into(),
            kind: SceneKind::Screenshot,
            assets: SceneAssets {
                screenshot,
                video: None,
            },
            hotspots: Vec::new(),
            guide_marks: Vec::new(),
            description: None,
            notes: None,
        }
    }

    pub fn hotspot(&self, id: &HotspotId) -> Option<&Hotspot> {
        self.hotspots.iter().find(|hotspot| hotspot.id == *id)
    }

    fn validate(&self) -> ValidationReport {
        let mut report = ValidationReport::default();
        let mut hotspot_ids = HashSet::new();
        let mut guide_mark_ids = HashSet::new();

        if self.assets.screenshot.is_none() && self.assets.video.is_none() {
            report.warnings.push(ValidationWarning::SceneHasNoCapture {
                scene_id: self.id.clone(),
            });
        }

        for path in [&self.assets.screenshot, &self.assets.video]
            .into_iter()
            .flatten()
        {
            if let Err(reason) = validate_portable_asset_path(path) {
                report.errors.push(ValidationError::InvalidAssetPath {
                    path: path.clone(),
                    reason,
                });
            }
        }

        for hotspot in &self.hotspots {
            if !hotspot_ids.insert(hotspot.id.clone()) {
                report.errors.push(ValidationError::DuplicateHotspotId {
                    scene_id: self.id.clone(),
                    hotspot_id: hotspot.id.clone(),
                });
            }

            if hotspot.label.trim().is_empty() {
                report.errors.push(ValidationError::EmptyHotspotLabel {
                    scene_id: self.id.clone(),
                    hotspot_id: hotspot.id.clone(),
                });
            }

            if !hotspot.bounds.is_valid() {
                report.errors.push(ValidationError::InvalidHotspotBounds {
                    scene_id: self.id.clone(),
                    hotspot_id: hotspot.id.clone(),
                    bounds: hotspot.bounds,
                });
            }

            if let Some(transition) = &hotspot.transition {
                if transition.frames.is_empty() {
                    report.errors.push(ValidationError::EmptyTransitionFrames {
                        scene_id: self.id.clone(),
                        hotspot_id: hotspot.id.clone(),
                    });
                }

                if let Some(duration_ms) = transition.default_frame_duration_ms
                    && duration_ms == 0
                {
                    report
                        .errors
                        .push(ValidationError::InvalidTransitionDefaultDuration {
                            scene_id: self.id.clone(),
                            hotspot_id: hotspot.id.clone(),
                            duration_ms,
                        });
                }

                for frame in &transition.frames {
                    if let Err(reason) = validate_portable_asset_path(&frame.path) {
                        report
                            .errors
                            .push(ValidationError::InvalidTransitionFramePath {
                                scene_id: self.id.clone(),
                                hotspot_id: hotspot.id.clone(),
                                path: frame.path.clone(),
                                reason,
                            });
                    }

                    if let Some(duration_ms) = frame.duration_ms
                        && duration_ms == 0
                    {
                        report
                            .errors
                            .push(ValidationError::InvalidTransitionFrameDuration {
                                scene_id: self.id.clone(),
                                hotspot_id: hotspot.id.clone(),
                                path: frame.path.clone(),
                                duration_ms,
                            });
                    }
                }
            }
        }

        for guide_mark in &self.guide_marks {
            if !guide_mark_ids.insert(guide_mark.id.clone()) {
                report.errors.push(ValidationError::DuplicateGuideMarkId {
                    scene_id: self.id.clone(),
                    guide_mark_id: guide_mark.id.clone(),
                });
            }

            if guide_mark.label.trim().is_empty() {
                report.errors.push(ValidationError::EmptyGuideMarkLabel {
                    scene_id: self.id.clone(),
                    guide_mark_id: guide_mark.id.clone(),
                });
            }

            if !guide_mark.bounds.is_valid() {
                report.errors.push(ValidationError::InvalidGuideMarkBounds {
                    scene_id: self.id.clone(),
                    guide_mark_id: guide_mark.id.clone(),
                    bounds: guide_mark.bounds,
                });
            }
        }

        report
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SceneKind {
    #[default]
    Screenshot,
    VideoFrame,
    Composite,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneAssets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionTrigger {
    #[default]
    Click,
    Scroll,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hotspot {
    pub id: HotspotId,
    pub label: String,
    pub target: SceneId,
    pub bounds: Rect,
    #[serde(default)]
    pub trigger: InteractionTrigger,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<Transition>,
}

impl Hotspot {
    pub fn new(id: HotspotId, label: impl Into<String>, target: SceneId, bounds: Rect) -> Self {
        Self {
            id,
            label: label.into(),
            target,
            bounds,
            trigger: InteractionTrigger::Click,
            transition: None,
        }
    }

    pub fn with_trigger(mut self, trigger: InteractionTrigger) -> Self {
        self.trigger = trigger;
        self
    }

    pub fn with_transition(mut self, transition: Transition) -> Self {
        self.transition = Some(transition);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuideMark {
    pub id: GuideMarkId,
    pub label: String,
    pub bounds: Rect,
    #[serde(default)]
    pub style: GuideMarkStyle,
}

impl GuideMark {
    pub fn new(id: GuideMarkId, label: impl Into<String>, bounds: Rect) -> Self {
        Self {
            id,
            label: label.into(),
            bounds,
            style: GuideMarkStyle::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GuideMarkStyle {
    #[default]
    Pulse,
    Ring,
    Highlight,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    #[serde(default)]
    pub kind: TransitionKind,
    #[serde(default)]
    pub frames: Vec<TransitionFrame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_frame_duration_ms: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransitionKind {
    #[default]
    FrameSequence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionFrame {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn into_result(self) -> Result<(), ValidationReport> {
        if self.is_valid() { Ok(()) } else { Err(self) }
    }

    fn extend(&mut self, other: ValidationReport) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    UnsupportedSchemaVersion {
        expected: String,
        actual: String,
    },
    EmptyTitle,
    InvalidAssetPath {
        path: String,
        reason: &'static str,
    },
    DuplicateAssetPath {
        path: String,
    },
    EmptyAssetMediaType {
        path: String,
    },
    InvalidSectionId {
        section_id: String,
        reason: &'static str,
    },
    DuplicateSectionId {
        section_id: String,
    },
    EmptySectionTitle {
        section_id: String,
    },
    MissingSectionScene {
        section_id: String,
        scene_id: SceneId,
    },
    NoScenes,
    MissingStartScene {
        scene_id: SceneId,
    },
    DuplicateSceneId {
        scene_id: SceneId,
    },
    EmptySceneTitle {
        scene_id: SceneId,
    },
    DuplicateHotspotId {
        scene_id: SceneId,
        hotspot_id: HotspotId,
    },
    EmptyHotspotLabel {
        scene_id: SceneId,
        hotspot_id: HotspotId,
    },
    DuplicateGuideMarkId {
        scene_id: SceneId,
        guide_mark_id: GuideMarkId,
    },
    EmptyGuideMarkLabel {
        scene_id: SceneId,
        guide_mark_id: GuideMarkId,
    },
    InvalidGuideMarkBounds {
        scene_id: SceneId,
        guide_mark_id: GuideMarkId,
        bounds: Rect,
    },
    InvalidHotspotBounds {
        scene_id: SceneId,
        hotspot_id: HotspotId,
        bounds: Rect,
    },
    EmptyTransitionFrames {
        scene_id: SceneId,
        hotspot_id: HotspotId,
    },
    InvalidTransitionFramePath {
        scene_id: SceneId,
        hotspot_id: HotspotId,
        path: String,
        reason: &'static str,
    },
    InvalidTransitionFrameDuration {
        scene_id: SceneId,
        hotspot_id: HotspotId,
        path: String,
        duration_ms: u32,
    },
    InvalidTransitionDefaultDuration {
        scene_id: SceneId,
        hotspot_id: HotspotId,
        duration_ms: u32,
    },
    MissingHotspotTarget {
        scene_id: SceneId,
        hotspot_id: HotspotId,
        target: SceneId,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { expected, actual } => {
                write!(
                    formatter,
                    "unsupported schema version `{actual}`; expected `{expected}`"
                )
            }
            Self::EmptyTitle => formatter.write_str("bundle title must not be empty"),
            Self::InvalidAssetPath { path, reason } => {
                write!(formatter, "asset path `{path}` is not portable: {reason}")
            }
            Self::DuplicateAssetPath { path } => {
                write!(formatter, "bundle asset path `{path}` is duplicated")
            }
            Self::EmptyAssetMediaType { path } => {
                write!(
                    formatter,
                    "bundle asset `{path}` media_type must not be empty"
                )
            }
            Self::InvalidSectionId { section_id, reason } => {
                write!(
                    formatter,
                    "section id `{section_id}` is not valid: {reason}"
                )
            }
            Self::DuplicateSectionId { section_id } => {
                write!(formatter, "section id `{section_id}` is duplicated")
            }
            Self::EmptySectionTitle { section_id } => {
                write!(formatter, "section `{section_id}` title must not be empty")
            }
            Self::MissingSectionScene {
                section_id,
                scene_id,
            } => {
                write!(
                    formatter,
                    "section `{section_id}` references missing scene `{scene_id}`"
                )
            }
            Self::NoScenes => formatter.write_str("scene graph must contain at least one scene"),
            Self::MissingStartScene { scene_id } => {
                write!(formatter, "start scene `{scene_id}` does not exist")
            }
            Self::DuplicateSceneId { scene_id } => {
                write!(formatter, "scene id `{scene_id}` is duplicated")
            }
            Self::EmptySceneTitle { scene_id } => {
                write!(formatter, "scene `{scene_id}` title must not be empty")
            }
            Self::DuplicateHotspotId {
                scene_id,
                hotspot_id,
            } => {
                write!(
                    formatter,
                    "scene `{scene_id}` hotspot id `{hotspot_id}` is duplicated"
                )
            }
            Self::EmptyHotspotLabel {
                scene_id,
                hotspot_id,
            } => {
                write!(
                    formatter,
                    "scene `{scene_id}` hotspot `{hotspot_id}` label must not be empty"
                )
            }
            Self::InvalidHotspotBounds {
                scene_id,
                hotspot_id,
                bounds,
            } => {
                write!(
                    formatter,
                    "scene `{scene_id}` hotspot `{hotspot_id}` has invalid bounds x={} y={} width={} height={}",
                    bounds.x, bounds.y, bounds.width, bounds.height
                )
            }
            Self::DuplicateGuideMarkId {
                scene_id,
                guide_mark_id,
            } => {
                write!(
                    formatter,
                    "scene `{scene_id}` guide mark id `{guide_mark_id}` is duplicated"
                )
            }
            Self::EmptyGuideMarkLabel {
                scene_id,
                guide_mark_id,
            } => {
                write!(
                    formatter,
                    "scene `{scene_id}` guide mark `{guide_mark_id}` label must not be empty"
                )
            }
            Self::InvalidGuideMarkBounds {
                scene_id,
                guide_mark_id,
                bounds,
            } => {
                write!(
                    formatter,
                    "scene `{scene_id}` guide mark `{guide_mark_id}` has invalid bounds x={} y={} width={} height={}",
                    bounds.x, bounds.y, bounds.width, bounds.height
                )
            }
            Self::EmptyTransitionFrames {
                scene_id,
                hotspot_id,
            } => {
                write!(
                    formatter,
                    "scene `{scene_id}` hotspot `{hotspot_id}` transition must contain at least one frame"
                )
            }
            Self::InvalidTransitionFramePath {
                scene_id,
                hotspot_id,
                path,
                reason,
            } => {
                write!(
                    formatter,
                    "scene `{scene_id}` hotspot `{hotspot_id}` transition frame path `{path}` is not portable: {reason}"
                )
            }
            Self::InvalidTransitionFrameDuration {
                scene_id,
                hotspot_id,
                path,
                duration_ms,
            } => {
                write!(
                    formatter,
                    "scene `{scene_id}` hotspot `{hotspot_id}` transition frame `{path}` has invalid duration `{duration_ms}`ms"
                )
            }
            Self::InvalidTransitionDefaultDuration {
                scene_id,
                hotspot_id,
                duration_ms,
            } => {
                write!(
                    formatter,
                    "scene `{scene_id}` hotspot `{hotspot_id}` transition default duration must be greater than 0ms (got `{duration_ms}`ms)"
                )
            }
            Self::MissingHotspotTarget {
                scene_id,
                hotspot_id,
                target,
            } => {
                write!(
                    formatter,
                    "scene `{scene_id}` hotspot `{hotspot_id}` targets missing scene `{target}`"
                )
            }
        }
    }
}

fn validate_sections(sections: &[Section], graph: &SceneGraph) -> ValidationReport {
    let mut report = ValidationReport::default();
    let scene_ids = graph
        .scenes
        .iter()
        .map(|scene| scene.id.clone())
        .collect::<HashSet<_>>();
    let mut section_ids = HashSet::new();

    for section in sections {
        if let Err(reason) = validate_identifier(&section.id) {
            report.errors.push(ValidationError::InvalidSectionId {
                section_id: section.id.clone(),
                reason,
            });
        }

        if !section_ids.insert(section.id.clone()) {
            report.errors.push(ValidationError::DuplicateSectionId {
                section_id: section.id.clone(),
            });
        }

        if section.title.trim().is_empty() {
            report.errors.push(ValidationError::EmptySectionTitle {
                section_id: section.id.clone(),
            });
        }

        for scene_id in &section.scenes {
            if !scene_ids.contains(scene_id) {
                report.errors.push(ValidationError::MissingSectionScene {
                    section_id: section.id.clone(),
                    scene_id: scene_id.clone(),
                });
            }
        }
    }

    report
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationWarning {
    SceneHasNoCapture { scene_id: SceneId },
}

impl fmt::Display for ValidationWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SceneHasNoCapture { scene_id } => {
                write!(
                    formatter,
                    "scene `{scene_id}` has no screenshot or video asset"
                )
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum BundleIoError {
    #[error("failed to read `{path}`")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write `{path}`")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to replace `{path}`")]
    Replace {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse bundle manifest `{path}`")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to serialize bundle manifest")]
    Serialize(#[from] serde_json::Error),
}

pub fn manifest_path(bundle_path: impl AsRef<Path>) -> std::path::PathBuf {
    bundle_path.as_ref().join(MANIFEST_FILE_NAME)
}

pub fn read_manifest(bundle_path: impl AsRef<Path>) -> Result<BundleManifest, BundleIoError> {
    let path = manifest_path(bundle_path);
    let contents = fs::read_to_string(&path).map_err(|source| BundleIoError::Read {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| BundleIoError::Parse {
        path: path.display().to_string(),
        source,
    })
}

pub fn write_manifest(
    bundle_path: impl AsRef<Path>,
    manifest: &BundleManifest,
) -> Result<(), BundleIoError> {
    let path = manifest_path(bundle_path);
    let contents = serde_json::to_string_pretty(manifest)?;
    let temp_path = path.with_file_name(format!("{MANIFEST_FILE_NAME}.tmp-{}", std::process::id()));
    let display_path = path.display().to_string();
    let temp_contents = format!("{contents}\n");

    if let Err(source) = write_temp_file(&temp_path, &temp_contents) {
        let _ = fs::remove_file(&temp_path);
        return Err(BundleIoError::Write {
            path: temp_path.display().to_string(),
            source,
        });
    }

    match fs::rename(&temp_path, &path) {
        Ok(()) => Ok(()),
        Err(_first_error) if path.exists() => {
            fs::remove_file(&path).map_err(|source| BundleIoError::Replace {
                path: display_path.clone(),
                source,
            })?;
            fs::rename(&temp_path, &path).map_err(|source| BundleIoError::Replace {
                path: display_path,
                source,
            })
        }
        Err(source) => Err(BundleIoError::Replace {
            path: display_path,
            source,
        }),
    }
}

fn write_temp_file(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut temp_file = File::create(path)?;
    temp_file.write_all(contents.as_bytes())?;
    temp_file.sync_all()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileValidationReport {
    pub missing_files: Vec<MissingReferencedFile>,
}

impl FileValidationReport {
    pub fn is_valid(&self) -> bool {
        self.missing_files.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingReferencedFile {
    pub path: String,
}

impl fmt::Display for MissingReferencedFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "referenced asset `{}` does not exist", self.path)
    }
}

pub fn validate_referenced_files(
    bundle_path: impl AsRef<Path>,
    manifest: &BundleManifest,
) -> FileValidationReport {
    let bundle_path = bundle_path.as_ref();
    let mut report = FileValidationReport::default();
    let mut seen = HashSet::new();

    for path in manifest.referenced_asset_paths() {
        if validate_portable_asset_path(path).is_err() || !seen.insert(path) {
            continue;
        }

        if !referenced_file_exists_exact_case(bundle_path, path) {
            report.missing_files.push(MissingReferencedFile {
                path: path.to_owned(),
            });
        }
    }

    report
}

fn referenced_file_exists_exact_case(bundle_path: &Path, portable_path: &str) -> bool {
    let mut current = bundle_path.to_path_buf();
    let mut components = portable_path.split('/').peekable();

    while let Some(component) = components.next() {
        let is_last = components.peek().is_none();
        let Ok(entries) = fs::read_dir(&current) else {
            return false;
        };
        let Some(entry) = entries
            .filter_map(Result::ok)
            .find(|entry| entry.file_name() == OsStr::new(component))
        else {
            return false;
        };
        let Ok(file_type) = entry.file_type() else {
            return false;
        };

        if is_last {
            return file_type.is_file();
        }

        if !file_type.is_dir() {
            return false;
        }
        current = entry.path();
    }

    false
}

fn validate_identifier(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("must not be empty");
    }

    if matches!(value, "." | "..") {
        return Err("must not be `.` or `..`");
    }

    if value.len() > 96 {
        return Err("must be 96 characters or fewer");
    }

    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return Err("must contain only ASCII letters, numbers, dots, dashes, or underscores");
    }

    Ok(())
}

fn validate_portable_asset_path(value: &str) -> Result<(), &'static str> {
    if value.trim().is_empty() {
        return Err("must not be empty");
    }

    if value != value.trim() {
        return Err("must not start or end with whitespace");
    }

    if value.contains('\\') {
        return Err("must use forward slashes");
    }

    if value.contains(':') {
        return Err("must not contain a URL scheme or Windows drive prefix");
    }

    if value.starts_with('/') {
        return Err("must be relative to the bundle root");
    }

    if value
        .split('/')
        .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err("must not contain empty, current-directory, or parent-directory segments");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_manifest_is_valid_with_capture_warning() {
        let manifest = BundleManifest::starter("Demo");

        let report = manifest.validate();

        assert!(report.is_valid(), "{:?}", report.errors);
        assert_eq!(report.warnings.len(), 1);
    }

    #[test]
    fn validation_reports_missing_hotspot_targets() {
        let mut manifest = BundleManifest::starter("Demo");
        manifest.graph.scenes[0].hotspots.push(Hotspot {
            id: HotspotId::new("cta").unwrap(),
            label: "Open missing scene".to_owned(),
            target: SceneId::new("missing").unwrap(),
            bounds: Rect {
                x: 10.0,
                y: 10.0,
                width: 50.0,
                height: 24.0,
            },
            trigger: InteractionTrigger::Click,
            transition: None,
        });

        let report = manifest.validate();

        assert!(matches!(
            report.errors.as_slice(),
            [ValidationError::MissingHotspotTarget { .. }]
        ));
    }

    #[test]
    fn validation_rejects_duplicate_scene_ids_and_invalid_bounds() {
        let duplicate_id = SceneId::new("start").unwrap();
        let mut manifest = BundleManifest::starter("Demo");
        manifest.add_scene(Scene {
            id: duplicate_id,
            title: "Duplicate".to_owned(),
            kind: SceneKind::Screenshot,
            assets: SceneAssets::default(),
            hotspots: vec![Hotspot {
                id: HotspotId::new("bad-bounds").unwrap(),
                label: "Bad".to_owned(),
                target: SceneId::new("start").unwrap(),
                bounds: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 10.0,
                },
                trigger: InteractionTrigger::Click,
                transition: None,
            }],
            guide_marks: Vec::new(),
            description: None,
            notes: None,
        });

        let report = manifest.validate();

        assert!(
            report
                .errors
                .iter()
                .any(|error| matches!(error, ValidationError::DuplicateSceneId { .. }))
        );
        assert!(
            report
                .errors
                .iter()
                .any(|error| matches!(error, ValidationError::InvalidHotspotBounds { .. }))
        );
    }

    #[test]
    fn identifiers_are_url_and_path_segment_friendly() {
        assert!(SceneId::new("home.screen-1").is_ok());
        assert!(SceneId::new(".").is_err());
        assert!(SceneId::new("..").is_err());
        assert!(SceneId::new("home screen").is_err());
        assert!(HotspotId::new("").is_err());
    }

    #[test]
    fn deserialization_rejects_invalid_identifiers() {
        let json = r#"{
            "schema_version": "scenecast.bundle.v1",
            "title": "Demo",
            "graph": {
                "start_scene": "start",
                "scenes": [
                    {
                        "id": "bad scene",
                        "title": "Bad",
                        "kind": "screenshot",
                        "assets": {},
                        "hotspots": []
                    }
                ]
            },
            "assets": []
        }"#;

        assert!(serde_json::from_str::<BundleManifest>(json).is_err());
    }

    #[test]
    fn validation_rejects_non_portable_asset_paths() {
        let mut manifest = BundleManifest::starter("Demo");
        manifest.graph.scenes[0].assets.screenshot = Some("../outside.png".to_owned());
        manifest.assets.push(BundleAsset {
            path: "assets\\logo.png".to_owned(),
            media_type: "image/png".to_owned(),
            role: AssetRole::Supporting,
        });

        let report = manifest.validate();

        assert_eq!(
            report
                .errors
                .iter()
                .filter(|error| matches!(error, ValidationError::InvalidAssetPath { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn file_validation_reports_missing_referenced_assets() {
        let mut manifest = BundleManifest::starter("Demo");
        manifest.graph.scenes[0].assets.screenshot = Some("captures/start.png".to_owned());

        let report = validate_referenced_files("missing-bundle", &manifest);

        assert_eq!(
            report.missing_files,
            vec![MissingReferencedFile {
                path: "captures/start.png".to_owned()
            }]
        );
    }

    #[test]
    fn file_validation_requires_exact_case_for_portability() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("Captures")).unwrap();
        fs::write(temp.path().join("Captures").join("Start.png"), []).unwrap();
        let mut manifest = BundleManifest::starter("Demo");
        manifest.graph.scenes[0].assets.screenshot = Some("captures/start.png".to_owned());

        let report = validate_referenced_files(temp.path(), &manifest);

        assert_eq!(
            report.missing_files,
            vec![MissingReferencedFile {
                path: "captures/start.png".to_owned()
            }]
        );
    }

    #[test]
    fn write_manifest_round_trips_through_disk() {
        let temp = tempfile::tempdir().unwrap();
        let manifest = BundleManifest::starter("Disk round trip");

        write_manifest(temp.path(), &manifest).unwrap();

        assert_eq!(read_manifest(temp.path()).unwrap(), manifest);
    }

    #[test]
    fn add_hotspot_links_existing_scenes() {
        let mut manifest = BundleManifest::starter("Demo");
        manifest.add_scene(Scene::screenshot(
            SceneId::new("pricing").unwrap(),
            "Pricing",
            Some("captures/pricing.png".to_owned()),
        ));
        let hotspot = Hotspot::new(
            HotspotId::new("pricing-link").unwrap(),
            "Pricing",
            SceneId::new("pricing").unwrap(),
            Rect::new(10.0, 20.0, 100.0, 40.0),
        );

        manifest
            .add_hotspot(&SceneId::new("start").unwrap(), hotspot)
            .unwrap();

        assert!(manifest.validate().is_valid());
        assert_eq!(manifest.graph.scenes[0].hotspots.len(), 1);
    }

    #[test]
    fn add_hotspot_rejects_missing_source_scene() {
        let mut manifest = BundleManifest::starter("Demo");
        let error = manifest
            .add_hotspot(
                &SceneId::new("missing").unwrap(),
                Hotspot::new(
                    HotspotId::new("cta").unwrap(),
                    "CTA",
                    SceneId::new("start").unwrap(),
                    Rect::new(0.0, 0.0, 10.0, 10.0),
                ),
            )
            .unwrap_err();

        assert!(matches!(error, AddHotspotError::MissingScene { .. }));
    }

    #[test]
    fn transition_validation_rejects_empty_frames_and_bad_durations() {
        let mut manifest = BundleManifest::starter("Demo");
        manifest.add_scene(Scene::screenshot(
            SceneId::new("pricing").unwrap(),
            "Pricing",
            Some("captures/pricing.png".to_owned()),
        ));
        manifest.graph.scenes[0].hotspots.push(
            Hotspot::new(
                HotspotId::new("scroll-pricing").unwrap(),
                "Scroll to pricing",
                SceneId::new("pricing").unwrap(),
                Rect::new(0.0, 0.0, 100.0, 40.0),
            )
            .with_trigger(InteractionTrigger::Scroll)
            .with_transition(Transition {
                kind: TransitionKind::FrameSequence,
                frames: Vec::new(),
                default_frame_duration_ms: Some(0),
            }),
        );

        let report = manifest.validate();
        assert!(
            report
                .errors
                .iter()
                .any(|error| matches!(error, ValidationError::EmptyTransitionFrames { .. }))
        );
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidTransitionDefaultDuration { .. }
        )));
    }

    #[test]
    fn transition_frames_are_referenced_assets() {
        let mut manifest = BundleManifest::starter("Demo");
        manifest.add_scene(Scene::screenshot(
            SceneId::new("pricing").unwrap(),
            "Pricing",
            Some("captures/pricing.png".to_owned()),
        ));
        manifest.graph.scenes[0].hotspots.push(
            Hotspot::new(
                HotspotId::new("scroll-pricing").unwrap(),
                "Scroll to pricing",
                SceneId::new("pricing").unwrap(),
                Rect::new(0.0, 0.0, 100.0, 40.0),
            )
            .with_trigger(InteractionTrigger::Scroll)
            .with_transition(Transition {
                kind: TransitionKind::FrameSequence,
                frames: vec![
                    TransitionFrame {
                        path: "captures/scroll-0001.png".to_owned(),
                        duration_ms: Some(80),
                    },
                    TransitionFrame {
                        path: "captures/scroll-0002.png".to_owned(),
                        duration_ms: None,
                    },
                ],
                default_frame_duration_ms: Some(90),
            }),
        );

        let paths = manifest.referenced_asset_paths();
        assert!(paths.contains(&"captures/scroll-0001.png"));
        assert!(paths.contains(&"captures/scroll-0002.png"));
    }
}
