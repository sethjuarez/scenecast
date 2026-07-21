# Roadmap

This document captures the near-term direction after the initial Rust foundation. It is not a commitment to a specific release order, but it describes the intended layers so future work stays coherent.

## Current foundation

- Rust workspace with `scenecast-core` and `scenecast-cli`.
- Portable `.scenecast` directory bundle with `manifest.json`.
- Scene graph, sections, scenes, hotspots, guide marks, captures, bundle assets, and validation.
- CLI authoring for create, inspect, validate, add-scene, add-hotspot, add-guide-mark, add-section, ffmpeg-backed import-video, and static HTML export.
- Cross-platform CI for formatting, tests, and clippy.

## Remaining foundation work

- Keep ingestion acceptance fixtures focused on real source artifact -> `.scenecast` bundle -> validation -> static HTML export before freezing generated contracts.
- Add an `add-asset` command for bundle-level supporting assets and non-capture media references.
- Add package/archive support so a `.scenecast` directory can become a single portable file when needed.
- Add JSON schema export or generated bindings for web/native consumers.
- Add richer validation against capture dimensions (for both hotspots and transition frame compatibility).
- Expand sample bundles that become end-to-end fixtures for click + scroll + transition playback.
- Add transition authoring ergonomics beyond manual frame lists (for example, sequence import helpers).
- Add GitHub Pages examples that publish exported players from `docs/` and `gh-pages`.

## Product surfaces

- Web authoring and production web player for interactive click-through demos.
- Manual screenshot import workflow.
- Video ingest that can extract frames into scenes, currently backed by an OS-provided ffmpeg executable.
- Native/Tauri capture workflows.
- Rust CLI workflows for automation and CI.
- GitHub Pages publishing for static player output.
- Later sharing and analytics.

## Design principles

- Keep bundle semantics portable and explicit.
- Keep core validation independent of any single UI.
- Prefer incremental authoring: drafts may warn while still being inspectable and repairable.
- Make authored bundles safe to move between Windows, macOS, Linux, local development, CI, and cloud publishing.
