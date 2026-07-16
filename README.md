# Scenecast

Scenecast is a high-fidelity interactive click-through demo platform. This repository currently contains the Rust foundation for authoring and validating portable `.scenecast` bundles.

## Workspace

- `crates/scenecast-core`: scene graph model, validation, bundle manifest primitives, and manifest IO.
- `crates/scenecast-cli`: command-line authoring tools for creating, inspecting, validating, and extending bundles.
- `docs/architecture.md`: the current Rust-first foundation and design boundaries.
- `docs/bundle-format.md`: the initial `.scenecast` bundle structure.
- `docs/cli.md`: command reference and authoring workflow.
- `docs/roadmap.md`: near-term product and technical direction.

## CLI quick start

```powershell
cargo run -p scenecast-cli -- new demos\hello.scenecast --title "Hello Scenecast"
cargo run -p scenecast-cli -- inspect demos\hello.scenecast
cargo run -p scenecast-cli -- add-scene demos\hello.scenecast pricing "Pricing" --screenshot captures\pricing.png
cargo run -p scenecast-cli -- add-hotspot demos\hello.scenecast start pricing-link "View pricing" pricing --x 420 --y 240 --width 160 --height 48
cargo run -p scenecast-cli -- import-video demos\hello.scenecast demo.mp4 --scene-prefix demo --every-seconds 5 --crop 0,120,1920,960
cargo run -p scenecast-cli -- export-html demos\hello.scenecast demos\hello-player
cargo run -p scenecast-cli -- validate demos\hello.scenecast
```

## Development

```powershell
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Pull requests run the same checks through `.github\workflows\rust.yml`.
