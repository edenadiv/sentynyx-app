// End-to-end walkthrough of the guided tour against the browser preview
// (`pnpm dev`). The preview path needs no API keys — sends use the simulated
// stream — so the whole 11-step machine is exercisable headless. Run via
// `node e2e/run-e2e.mjs`, which boots Vite, runs this, and tears down.
//
// This is the automated form of the manual tour walkthrough in the launch
// checklist: it proves every step advances on its real event, including the
// no-key transmit, the "Model saw" toggle, the inspector, and the SSN block.

import { chromium } from "playwright";

const BASE = process.env.E2E_BASE_URL || "http://localhost:1420";
const HEADLESS = process.env.E2E_HEADED !== "1";

function log(ok, msg) {
  console.log(`${ok ? "✓" : "✗"} ${msg}`);
  if (!ok) process.exitCode = 1;
}

async function main() {
  const browser = await chromium.launch({ headless: HEADLESS });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));

  await page.goto(BASE, { waitUntil: "networkidle" });

  // Boot sequence auto-dismisses (~2.5s of animation). Wait it out.
  await page.waitForTimeout(3500);

  // The tour auto-offers in the browser preview (no persisted tutorial_done).
  // Fall back to the command palette if it isn't already up.
  let intro = page.getByText("Welcome to the perimeter");
  if (!(await intro.isVisible().catch(() => false))) {
    await page.keyboard.press("Meta+k");
    await page.getByText("Take the guided tour").click();
  }
  await intro.waitFor({ state: "visible", timeout: 5000 });
  log(true, "step 0 — intro card visible");

  await page.getByRole("button", { name: "Start the tour" }).click();

  // Step 1 — insert sample, expect highlights → auto-advance to step 2.
  await page.getByRole("button", { name: "Insert sample prompt" }).click();
  await page.getByText("Live detection").waitFor({ state: "visible", timeout: 5000 });
  log(true, "step 1→2 — sample inserted, live detection advanced");

  // Confirm spans actually rendered in the composer mirror.
  const hitCount = await page.locator(".vend-hit").count();
  log(hitCount >= 3, `live highlights present (${hitCount} spans)`);

  await page.getByRole("button", { name: "Next" }).click(); // step 2 → 3

  // Step 3 — open the Vendetta panel.
  await page.locator('[data-tour="vendetta-toggle"]').click();
  await page.getByText("Raw → alias").waitFor({ state: "visible", timeout: 5000 });
  log(true, "step 3→4 — panel opened, alias mapping shown");

  await page.getByRole("button", { name: "Next" }).click(); // step 4 → 5

  // Step 5 — transmit. Browser preview streams a fake reply (no key needed).
  await page.locator('[data-tour="transmit"]').click();
  // Step 6 is a quiet chip; step 7 ("Model saw") appears once the reply ends.
  await page.getByText(/What did .* actually receive/).waitFor({ state: "visible", timeout: 15000 });
  log(true, "step 5→7 — transmit streamed a reply, reached Model-saw step");

  // Step 7 — click the "Model saw" toggle on the assistant message.
  await page.locator('[data-tour="modelsaw"]').last().click();
  await page.getByText(/The proof — Dev Inspector/).waitFor({ state: "visible", timeout: 5000 });
  log(true, "step 7→8 — Model-saw clicked, inspector step shown");

  // Step 8 — opening the Dev Inspector advances the tour by itself; the
  // finale step then auto-closes the inspector so the composer is reachable.
  await page.keyboard.press("Meta+Shift+d");
  await page.getByText(/Some things never leave/).waitFor({ state: "visible", timeout: 5000 });
  log(true, "step 8→9 — inspector opened, finale step shown (inspector auto-closed)");

  // Step 9 — add an SSN, transmit, expect the policy-violation block.
  await page.getByRole("button", { name: "Add a fake SSN for me" }).click();
  await page.locator('[data-tour="transmit"]').click();
  await page.getByText("TRANSMISSION HALTED").waitFor({ state: "visible", timeout: 8000 });
  log(true, "step 9 — SSN transmit was BLOCKED by the perimeter");

  // Acknowledge the block, expect the done card.
  await page.getByRole("button", { name: "ACKNOWLEDGE" }).click();
  await page.getByText("That's the perimeter").waitFor({ state: "visible", timeout: 5000 });
  log(true, "step 10 — done card reached");

  await page.getByRole("button", { name: "Finish" }).click();
  await page.getByText("That's the perimeter").waitFor({ state: "hidden", timeout: 5000 });
  log(true, "tour dismissed cleanly");

  log(errors.length === 0, `no uncaught page errors${errors.length ? ": " + errors.join("; ") : ""}`);

  await browser.close();
  console.log(process.exitCode ? "\nE2E FAILED" : "\nE2E PASSED — all 11 tour steps advanced on real events");
}

main().catch((e) => { console.error("E2E crashed:", e); process.exit(1); });
