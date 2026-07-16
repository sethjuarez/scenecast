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
