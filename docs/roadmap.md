# Roadmap

This document captures the near-term direction after the initial Rust foundation. It is not a commitment to a specific release order, but it describes the intended layers so future work stays coherent.

## Current foundation

- Rust workspace with `scenecast-core` and `scenecast-cli`.
- Portable `.scenecast` directory bundle with `manifest.json`.
- Scene graph, scenes, hotspots, captures, bundle assets, and validation.
- CLI authoring for create, inspect, validate, add-scene, add-hotspot, ffmpeg-backed import-video, and static HTML export.
- Cross-platform CI for formatting, tests, and clippy.

## Remaining foundation work

- Add an `add-asset` command for bundle-level supporting assets and non-capture media references.
- Add package/archive support so a `.scenecast` directory can become a single portable file when needed.
- Add JSON schema export or generated bindings for web/native consumers.
- Add richer validation against capture dimensions (for both hotspots and transition frame compatibility).
- Add sample bundles that become end-to-end fixtures for click + scroll + transition playback.
- Add transition authoring ergonomics beyond manual frame lists (for example, sequence import helpers).

## Product surfaces

- Web authoring and production web player for interactive click-through demos.
- Manual screenshot import workflow.
- Video ingest that can extract frames into scenes, currently backed by an OS-provided ffmpeg executable.
- Native/Tauri capture workflows.
- Rust CLI workflows for automation and CI.
- Later cloud publishing, sharing, and analytics.

## Design principles

- Keep bundle semantics portable and explicit.
- Keep core validation independent of any single UI.
- Prefer incremental authoring: drafts may warn while still being inspectable and repairable.
- Make authored bundles safe to move between Windows, macOS, Linux, local development, CI, and cloud publishing.
