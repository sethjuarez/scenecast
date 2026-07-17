import { chromium } from "playwright";
import { mkdir, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";

const root = path.dirname(fileURLToPath(import.meta.url));
const pagePath = path.join(root, "sample-page.html");
const bundleRoot = path.join(root, "generated", "playwright-scroll.scenecast");
const capturesRoot = path.join(bundleRoot, "captures");

const viewport = { width: 1440, height: 900 };
const transitionFrameCount = 8;
let transitionFrames = [];

await mkdir(capturesRoot, { recursive: true });

const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport });
  await page.goto(pathToFileURL(pagePath).href);

  const maxScrollY = await page.evaluate(() => document.documentElement.scrollHeight - window.innerHeight);
  transitionFrames = Array.from({ length: transitionFrameCount }, (_, index) => {
    const frameNumber = index + 1;
    const ratio = frameNumber / (transitionFrameCount + 1);
    return {
      y: Math.round(maxScrollY * ratio),
      file: `scroll-${String(frameNumber).padStart(4, "0")}.png`
    };
  });
  const scrollPositions = [
    { y: 0, file: "home-top.png" },
    ...transitionFrames,
    { y: maxScrollY, file: "home-bottom.png" }
  ];

  for (const position of scrollPositions) {
    await page.evaluate((y) => window.scrollTo(0, y), position.y);
    await page.waitForTimeout(100);
    await page.screenshot({
      path: path.join(capturesRoot, position.file),
      fullPage: false
    });
  }
} finally {
  await browser.close();
}

const manifest = {
  schema_version: "scenecast.bundle.v1",
  title: "Playwright scroll capture",
  sections: [
    {
      id: "main",
      title: "Scroll capture",
      scenes: ["home-top", "home-bottom"]
    }
  ],
  graph: {
    start_scene: "home-top",
    scenes: [
      {
        id: "home-top",
        title: "Home top",
        kind: "screenshot",
        description: "Top viewport captured from a scrollable HTML page.",
        assets: {
          screenshot: "captures/home-top.png"
        },
        hotspots: [
          {
            id: "scroll-down",
            label: "Scroll down",
            target: "home-bottom",
            bounds: {
              x: 0,
              y: 0,
              width: viewport.width,
              height: viewport.height
            },
            trigger: "scroll",
            scroll_direction: "down",
            transition: {
              kind: "frame-sequence",
              default_frame_duration_ms: 45,
              frames: transitionFrames.map((frame) => ({ path: `captures/${frame.file}` }))
            }
          }
        ],
        guide_marks: [
          {
            id: "scroll-guide",
            label: "Use the mouse wheel",
            bounds: {
              x: 1050,
              y: 690,
              width: 260,
              height: 104
            },
            style: "pulse"
          }
        ]
      },
      {
        id: "home-bottom",
        title: "Home bottom",
        kind: "screenshot",
        description: "Bottom viewport reached by the scroll-triggered hotspot.",
        assets: {
          screenshot: "captures/home-bottom.png"
        },
        hotspots: [
          {
            id: "scroll-up",
            label: "Scroll up",
            target: "home-top",
            bounds: {
              x: 0,
              y: 0,
              width: viewport.width,
              height: viewport.height
            },
            trigger: "scroll",
            scroll_direction: "up",
            transition: {
              kind: "frame-sequence",
              default_frame_duration_ms: 45,
              frames: [...transitionFrames]
                .reverse()
                .map((frame) => ({ path: `captures/${frame.file}` }))
            }
          },
          {
            id: "back-to-top",
            label: "Back to top",
            target: "home-top",
            bounds: {
              x: 64,
              y: 64,
              width: 170,
              height: 56
            }
          }
        ]
      }
    ]
  },
  assets: []
};

await writeFile(
  path.join(bundleRoot, "manifest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
  "utf8"
);

console.log(`Wrote ${bundleRoot}`);
