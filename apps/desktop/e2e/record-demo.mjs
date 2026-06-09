// Records the README demo: boots Vite, drives the photogenic half of the
// guided tour (sample → live detection → alias panel → transmit → reply →
// "Model saw") with deliberate pacing, captures video, and converts it to an
// optimized GIF at ../../docs/assets/demo.gif.
//
//   node e2e/record-demo.mjs        (requires ffmpeg on PATH)

import { chromium } from "playwright";
import { spawn, execFileSync } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";
import { mkdirSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

const PORT = 1421; // not 1420, so a running dev server doesn't collide
const BASE = `http://localhost:${PORT}`;
const VIDEO_DIR = resolve("e2e/.videos");
const OUT_GIF = resolve("../../docs/assets/demo.gif");

const vite = spawn("pnpm", ["exec", "vite", "--port", String(PORT), "--strictPort"], { stdio: "ignore" });

async function waitForServer(url, tries = 60) {
  for (let i = 0; i < tries; i++) {
    try { if ((await fetch(url)).ok) return true; } catch {}
    await sleep(500);
  }
  return false;
}

async function main() {
  if (!(await waitForServer(BASE))) throw new Error("vite did not come up");
  mkdirSync(VIDEO_DIR, { recursive: true });
  mkdirSync(resolve("../../docs/assets"), { recursive: true });

  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({
    viewport: { width: 1280, height: 800 },
    recordVideo: { dir: VIDEO_DIR, size: { width: 1280, height: 800 } },
  });
  const page = await ctx.newPage();
  await page.goto(BASE, { waitUntil: "networkidle" });

  // Let the boot sequence play — it's part of the show.
  await sleep(3800);

  // Start the tour and pace through the visual beats.
  const intro = page.getByText("Welcome to the perimeter");
  if (!(await intro.isVisible().catch(() => false))) {
    await page.keyboard.press("Meta+k");
    await page.getByText("Take the guided tour").click();
  }
  await sleep(900);
  await page.getByRole("button", { name: "Start the tour" }).click();
  await sleep(700);
  await page.getByRole("button", { name: "Insert sample prompt" }).click();
  await sleep(2200); // linger on the live highlights
  await page.getByRole("button", { name: "Next" }).click();
  await sleep(600);
  await page.locator('[data-tour="vendetta-toggle"]').click();
  await sleep(2200); // linger on the raw → alias mapping
  await page.getByRole("button", { name: "Next" }).click();
  await sleep(500);
  await page.locator('[data-tour="transmit"]').click();
  // X-ray pass + streamed reply.
  await page.getByText(/What did .* actually receive/).waitFor({ timeout: 15000 });
  await sleep(800);
  await page.locator('[data-tour="modelsaw"]').last().click();
  await sleep(2600); // hold on the aliased wire payload — the money shot

  await ctx.close(); // flushes the video
  await browser.close();

  // Newest webm in the dir is ours.
  const vids = readdirSync(VIDEO_DIR).filter(f => f.endsWith(".webm"))
    .map(f => join(VIDEO_DIR, f))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  if (!vids.length) throw new Error("no video captured");

  // webm → optimized GIF (palette pass for quality, 12fps, 720px wide).
  const palette = join(VIDEO_DIR, "palette.png");
  execFileSync("ffmpeg", ["-y", "-i", vids[0], "-vf", "fps=12,scale=720:-1:flags=lanczos,palettegen", palette], { stdio: "ignore" });
  execFileSync("ffmpeg", ["-y", "-i", vids[0], "-i", palette,
    "-filter_complex", "fps=12,scale=720:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=4", OUT_GIF], { stdio: "ignore" });

  const kb = Math.round(statSync(OUT_GIF).size / 1024);
  console.log(`demo GIF written: ${OUT_GIF} (${kb} KB)`);
}

main()
  .then(() => { vite.kill("SIGTERM"); process.exit(0); })
  .catch((e) => { console.error(e); vite.kill("SIGTERM"); process.exit(1); });
