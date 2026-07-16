# Architecture

Scenecast starts with a Rust-first foundation for authoring and validating portable interactive demos. The current implementation intentionally focuses on the core bundle model and CLI workflows before adding capture, web authoring, playback, publishing, or analytics.

## Crates

```text
crates\
  scenecast-core\
  scenecast-cli\
```

### `scenecast-core`

`scenecast-core` owns the data model and validation rules that every future surface should share:

- typed `SceneId` and `HotspotId` values with validated serialization and deserialization;
- `BundleManifest`, `Section`, `SceneGraph`, `Scene`, `Hotspot`, `GuideMark`, `Rect`, and asset primitives;
- structural validation for scene graph integrity, hotspot targets, bounds, schema version, and portable asset paths;
- filesystem validation for exact-case referenced files inside a bundle directory;
- manifest read/write helpers.

The core crate does not depend on CLI concerns. Future web, Tauri, native capture, or cloud services should use the same core primitives or generated equivalents so bundle behavior stays consistent.

### `scenecast-cli`

`scenecast-cli` provides the first authoring surface:

- create bundles with `new` or `init`;
- inspect bundle summaries;
- validate structural and filesystem correctness;
- add screenshot-backed scenes;
- add click-through hotspots between scenes;
- add guide marks that visually focus attention without navigating;
- add table-of-contents sections for static players.

The CLI validates edits before writing them. It rejects errors introduced by the requested edit while still allowing unrelated pre-existing manifest errors, which keeps the CLI useful for repairing hand-authored bundles.

## Bundle boundary

A `.scenecast` is currently a directory with `manifest.json` plus referenced captures/assets. The manifest stores portable paths with `/` separators and no absolute, parent-directory, URL-scheme, or Windows-drive paths. CLI validation checks referenced files with exact case so a bundle authored on Windows or macOS does not accidentally fail later on Linux.

## Static player boundary

`export-html` emits a static player folder that is designed for GitHub Pages and other static hosts. It keeps all bundle references relative, uses hash routes for deep links, and requires no server-side rewrites. The player supports screenshot scenes, video scenes, hotspots, guide marks, section-based table of contents, keyboard navigation, authored descriptions, and presenter notes embedded in the exported data model.

## Write safety

Manifest writes go through a temporary file in the bundle directory before replacing `manifest.json`. This keeps normal CLI edits from directly truncating the existing manifest during serialization or write failures.

## Tracing

The CLI initializes `tracing` and instruments the mutation and validation paths that matter for authoring operations. Default output remains quiet; set `RUST_LOG` when investigating CLI behavior.
