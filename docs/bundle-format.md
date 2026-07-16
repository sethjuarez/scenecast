# `.scenecast` bundle format

A `.scenecast` bundle is currently a portable directory. It is designed so the same structure can later be zipped or published without changing the manifest model.

```text
demo.scenecast\
  manifest.json
  captures\
    start.png
  assets\
    logo.svg
```

## `manifest.json`

The manifest is UTF-8 JSON with schema version `scenecast.bundle.v1`.

```json
{
  "schema_version": "scenecast.bundle.v1",
  "title": "Product demo",
  "graph": {
    "start_scene": "start",
    "scenes": [
      {
        "id": "start",
        "title": "Start",
        "kind": "screenshot",
        "assets": {
          "screenshot": "captures/start.png"
        },
        "hotspots": [
          {
            "id": "pricing-link",
            "label": "View pricing",
            "target": "pricing",
            "bounds": { "x": 420, "y": 240, "width": 160, "height": 48 }
          }
        ]
      }
    ]
  },
  "assets": [
    {
      "path": "assets/logo.svg",
      "media_type": "image/svg+xml",
      "role": "supporting"
    }
  ]
}
```

## Validation rules

- `schema_version` must be `scenecast.bundle.v1`.
- `title` must not be empty.
- The graph must contain at least one scene.
- `start_scene` must reference an existing scene.
- Scene and hotspot identifiers must be unique within their scope.
- Hotspot targets must reference existing scenes.
- Hotspot bounds must be finite and have positive width and height.
- Asset paths must be portable bundle-relative paths using `/`, with no absolute paths, drive prefixes, URL schemes, empty segments, `.`, or `..`.
- CLI validation also verifies that referenced screenshot, video, and bundle asset files exist in the bundle directory.
- Scenes without a screenshot or video asset are valid but produce a warning, which supports incremental manual authoring.
- `kind` is advisory in v1. Players should prefer the populated asset fields when deciding how to render a scene.
- Hotspot `x` and `y` are finite capture-pixel coordinates and may be negative for off-canvas or transitional authoring states; `width` and `height` must be positive.

## Authoring flow

The CLI creates and edits the manifest while preserving validation rules:

```powershell
cargo run -p scenecast-cli -- new demos\hello.scenecast --title "Hello Scenecast"
cargo run -p scenecast-cli -- add-scene demos\hello.scenecast pricing "Pricing" --screenshot captures\pricing.png
cargo run -p scenecast-cli -- add-hotspot demos\hello.scenecast start pricing-link "View pricing" pricing --x 420 --y 240 --width 160 --height 48
cargo run -p scenecast-cli -- validate demos\hello.scenecast
```

Coordinates are currently capture-pixel coordinates. Future authoring surfaces can layer normalized coordinates or responsive layout metadata on top of this manifest without changing the initial click-through graph model.

CLI edit commands reject validation errors introduced by the requested edit, but they do not block edits solely because a hand-authored manifest already contains unrelated errors. This keeps the CLI usable for incremental repair workflows.
