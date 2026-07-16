# Playwright scroll capture example

This starter shows how to turn a scrollable page into a static Scenecast bundle without recording video. Playwright captures a viewport-sized screenshot at the top of the page, several intermediate scroll frames, and the bottom of the page, then writes a `manifest.json` that connects those images with wheel-triggered hotspots for scrolling down and back up.

```powershell
cd examples\playwright-scroll-capture
npm install
npm run install-browsers
npm run capture
npm run validate
npm run export
```

Open `generated\player\index.html?debug=1` to see the exported click-through with hotspot outlines.

The generated bundle is written to `generated\playwright-scroll.scenecast` and is intentionally ignored by git.
