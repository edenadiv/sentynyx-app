// Boots the Vite dev server, waits for it, runs the guided-tour E2E spec,
// then tears the server down. Single entry point so CI / local both work:
//   node e2e/run-e2e.mjs
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const PORT = 1420;
const BASE = `http://localhost:${PORT}`;

const vite = spawn("pnpm", ["exec", "vite", "--port", String(PORT), "--strictPort"], {
  stdio: ["ignore", "pipe", "pipe"],
  env: process.env,
});
vite.stdout.on("data", (d) => process.stdout.write(`[vite] ${d}`));
vite.stderr.on("data", (d) => process.stderr.write(`[vite] ${d}`));

async function waitForServer(url, tries = 60) {
  for (let i = 0; i < tries; i++) {
    try {
      const r = await fetch(url);
      if (r.ok) return true;
    } catch { /* not up yet */ }
    await sleep(500);
  }
  return false;
}

let code = 1;
try {
  if (!(await waitForServer(BASE))) {
    console.error("vite did not come up");
  } else {
    const proc = spawn("node", ["e2e/guided-tour.spec.mjs"], {
      stdio: "inherit",
      env: { ...process.env, E2E_BASE_URL: BASE },
    });
    code = await new Promise((res) => proc.on("exit", res));
  }
} finally {
  vite.kill("SIGTERM");
  await sleep(300);
  process.exit(code ?? 1);
}
