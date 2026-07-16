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

The summary includes title, schema version, start scene, scene count, asset count, warning count, and error count.

## Add a scene

```powershell
cargo run -p scenecast-cli -- add-scene demos\hello.scenecast pricing "Pricing" --screenshot captures\pricing.png
```

The screenshot path is stored relative to the bundle root. Paths must be portable and use `/` in the manifest; PowerShell accepts the command argument shown above, and the manifest examples use `/`.

## Add a hotspot

```powershell
cargo run -p scenecast-cli -- add-hotspot demos\hello.scenecast start pricing-link "View pricing" pricing --x 420 --y 240 --width 160 --height 48
```

Hotspots belong to a source scene and target another scene by ID. Coordinates are capture-pixel values. Width and height must be positive.

## Import video frames

```powershell
cargo run -p scenecast-cli -- import-video demos\hello.scenecast demo.mp4 --scene-prefix demo --every-seconds 5 --crop 0,120,1920,960
```

`import-video` shells out to an existing `ffmpeg` executable on the OS. By default it runs `ffmpeg` from `PATH`; use `--ffmpeg <path>` or the `SCENECAST_FFMPEG` environment variable when the binary lives somewhere else.

The command extracts PNG frames into `captures\` and adds each generated frame as a screenshot-backed scene. For example, `--scene-prefix demo` creates scene IDs such as `demo-0001` with screenshots such as `captures/demo-0001.png`.

Imported frames are linked as a click-through sequence. A fresh starter bundle uses the first imported frame as the start scene, and each imported frame gets a full-frame `next` hotspot to the following frame.

Use `--crop x,y,width,height` to remove browser chrome or other recording matte before frames are extracted. The crop is applied by ffmpeg before sampling frames.

## Export a local HTML player

```powershell
cargo run -p scenecast-cli -- export-html demos\hello.scenecast demos\hello-player
```

The command writes a minimal `index.html` and copies referenced captures/assets into the output directory. Open the generated `index.html` in a browser to test the click-through locally.

Exported HTML is intentionally chrome-free: the body contains the current scene image stretched to the viewport, with a document-level click handler for moving through the generated sequence.

## Validate a bundle

```powershell
cargo run -p scenecast-cli -- validate demos\hello.scenecast
```

Validation checks both manifest structure and referenced files:

- schema version and title;
- start scene existence;
- duplicate scene and hotspot IDs;
- hotspot targets;
- hotspot bounds;
- portable asset paths;
- exact-case file existence for screenshots, videos, and bundle assets.

Warnings do not fail validation. For example, a scene with no capture asset is valid during incremental authoring but emits a warning.
