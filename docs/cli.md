# CLI

The Scenecast CLI is the first authoring interface for `.scenecast` bundles.

## Create a bundle

```powershell
cargo run -p scenecast-cli -- new demos\hello.scenecast --title "Hello Scenecast"
```

`init` is an alias for `new`:

```powershell
cargo run -p scenecast-cli -- init demos\hello.scenecast --title "Hello Scenecast"
```

The command creates:

```text
hello.scenecast\
  manifest.json
  assets\
  captures\
```

## Inspect a bundle

```powershell
cargo run -p scenecast-cli -- inspect demos\hello.scenecast
```

The summary includes title, schema version, start scene, section count, scene count, asset count, warning count, and error count.

## Add a scene

```powershell
cargo run -p scenecast-cli -- add-scene demos\hello.scenecast pricing "Pricing" --screenshot captures\pricing.png --description "Pricing page overview" --notes "Mention annual plan"
```

The screenshot path is stored relative to the bundle root. Paths must be portable and use `/` in the manifest; PowerShell accepts the command argument shown above, and the manifest examples use `/`. `--description` stores authored text for search, accessibility, narration, and review. `--notes` stores presenter or authoring notes.

## Add a hotspot

```powershell
cargo run -p scenecast-cli -- add-hotspot demos\hello.scenecast start pricing-link "View pricing" pricing --x 420 --y 240 --width 160 --height 48 --trigger scroll
```

Hotspots belong to a source scene and target another scene by ID. Coordinates are capture-pixel values. Width and height must be positive. Trigger defaults to `click`; use `scroll` for wheel-style interactions.

## Add a guide mark

```powershell
cargo run -p scenecast-cli -- add-guide-mark demos\hello.scenecast pricing price-callout "Plan selector" --x 320 --y 220 --width 240 --height 72 --style highlight
```

Guide marks are visible overlays that draw attention without changing navigation. Coordinates are capture-pixel values. Styles are `pulse`, `ring`, and `highlight`.

## Add a table-of-contents section

```powershell
cargo run -p scenecast-cli -- add-section demos\hello.scenecast overview "Overview" --scenes start,pricing
```

Sections group scenes for the exported player's table of contents and hash deep links. Scene IDs are comma-separated and stored in playback order.

## Add a transition frame sequence

```powershell
cargo run -p scenecast-cli -- add-transition demos\hello.scenecast start pricing-link --frames captures\scroll-0001.png,captures\scroll-0002.png --frame-duration-ms 90
```

`add-transition` attaches a frame-sequence transition to an existing hotspot. Frames are stored as portable bundle-relative paths and replay before the hotspot lands on the target scene.

## Import video frames

```powershell
cargo run -p scenecast-cli -- import-video demos\hello.scenecast demo.mp4 --scene-prefix demo --every-seconds 5 --crop 0,120,1920,960
```

`import-video` shells out to an existing `ffmpeg` executable on the OS. By default it runs `ffmpeg` from `PATH`; use `--ffmpeg <path>` or the `SCENECAST_FFMPEG` environment variable when the binary lives somewhere else.

The command extracts PNG frames into `captures\` and adds each generated frame as a screenshot-backed scene. For example, `--scene-prefix demo` creates scene IDs such as `demo-0001` with screenshots such as `captures/demo-0001.png`.

Imported frames are linked as a wheel-driven sequence. A fresh starter bundle uses the first imported frame as the start scene, and each imported frame gets a full-frame `next` hotspot to the following frame with a short frame-sequence transition.

Use `--crop x,y,width,height` to remove browser chrome or other recording matte before frames are extracted. The crop is applied by ffmpeg before sampling frames.

## Export a local HTML player

```powershell
cargo run -p scenecast-cli -- export-html demos\hello.scenecast demos\hello-player
```

The command writes `index.html` and copies referenced captures/assets into the output directory. Open the generated `index.html` in a browser to test the click-through locally.

Exported HTML is intentionally static-host friendly: the output folder contains `index.html` plus copied relative assets and can be published from `docs/`, a `gh-pages` branch, or any static file host. The player stretches screenshot or video scenes to the viewport, renders guide marks, provides a table of contents, supports keyboard navigation, and replays transition frames before landing on the destination scene. Hash routes such as `#/0/0` deep-link to section and scene positions without server rewrites. Add `?debug=1` to the URL for a minimal hotspot overlay while tuning bounds.

## Validate a bundle

```powershell
cargo run -p scenecast-cli -- validate demos\hello.scenecast
```

Validation checks both manifest structure and referenced files:

- schema version and title;
- start scene existence;
- duplicate scene and hotspot IDs;
- duplicate guide mark IDs;
- section scene references;
- hotspot targets;
- hotspot and guide mark bounds;
- hotspot trigger and transition shape;
- portable asset paths;
- exact-case file existence for screenshots, videos, transition frames, and bundle assets.

Warnings do not fail validation. For example, a scene with no capture asset is valid during incremental authoring but emits a warning.
