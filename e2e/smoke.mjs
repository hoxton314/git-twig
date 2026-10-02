// End-to-end smoke test against the real app via tauri-driver.
//
//   npm run tauri build -- --debug --no-bundle   # builds src-tauri/target/debug/twig
//   xvfb-run -a npm run test:e2e                 # needs tauri-driver + WebKitWebDriver
//
// Uses throwaway XDG dirs, git config and repo, so a developer's real
// settings, session and repos are never touched.
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Session } from "./webdriver.mjs";

const root = resolve(import.meta.dirname, "..");
const app = process.env.TWIG_E2E_APP ?? join(root, "src-tauri/target/debug/twig");
const driverBin = process.env.TAURI_DRIVER ?? "tauri-driver";
const port = 4444;

if (!existsSync(app)) {
  console.error(`App binary not found: ${app}\nBuild it with: npm run tauri build -- --debug --no-bundle`);
  process.exit(2);
}

// ── Isolated environment ────────────────────────────────────────────
const tmp = mkdtempSync(join(tmpdir(), "twig-e2e-"));
const xdg = { data: join(tmp, "data"), config: join(tmp, "config"), cache: join(tmp, "cache") };
const appData = join(xdg.data, "dev.twig.app");
mkdirSync(appData, { recursive: true });
const gitconfig = join(tmp, "gitconfig");
writeFileSync(gitconfig, "[user]\n\tname = Twig E2E\n\temail = e2e@twig.invalid\n[commit]\n\tgpgsign = false\n[init]\n\tdefaultBranch = main\n");
const env = {
  ...process.env,
  XDG_DATA_HOME: xdg.data,
  XDG_CONFIG_HOME: xdg.config,
  XDG_CACHE_HOME: xdg.cache,
  GIT_CONFIG_GLOBAL: gitconfig,
  GIT_CONFIG_NOSYSTEM: "1",
  // Software rendering under Xvfb: otherwise screenshots can show a stale frame.
  WEBKIT_DISABLE_COMPOSITING_MODE: "1",
  WEBKIT_DISABLE_DMABUF_RENDERER: "1",
};

const repo = join(tmp, "repo");
mkdirSync(repo);
const git = (...args) => execFileSync("git", args, { cwd: repo, env, encoding: "utf8" });
git("init", "-q");
writeFileSync(join(repo, "README.md"), "hello\n");
git("add", "README.md");
git("commit", "-q", "-m", "initial commit");
writeFileSync(join(repo, "notes.txt"), "from the e2e test\n");

// Open the repo through a saved session (no native file dialog needed).
writeFileSync(join(appData, "session.json"), JSON.stringify({ paths: [repo], active: repo }));
writeFileSync(
  join(appData, "settings.json"),
  JSON.stringify({ check_updates_on_startup: false, auto_fetch_interval: 0, confirm_destructive_ops: false }),
);

// ── Driver ──────────────────────────────────────────────────────────
const driver = spawn(driverBin, ["--port", String(port)], { env, stdio: ["ignore", "inherit", "inherit"] });
driver.on("error", (err) => {
  console.error(`Could not start ${driverBin}: ${err.message}\nInstall it with: cargo install tauri-driver --locked`);
  process.exit(2);
});
const base = `http://127.0.0.1:${port}`;
for (let i = 0; ; i++) {
  try {
    await fetch(`${base}/status`);
    break;
  } catch {
    if (i > 100) throw new Error("tauri-driver did not start");
    await new Promise((r) => setTimeout(r, 100));
  }
}

let session;
let failed = false;
const step = async (name, fn) => {
  process.stdout.write(`• ${name} … `);
  await fn();
  console.log("ok");
};

try {
  session = await Session.create(base, { "tauri:options": { application: app } });

  await step("app starts and restores the repo tab", async () => {
    await session.waitFor(".commit-row .summary", { pred: (t) => t.includes("initial commit") });
  });

  await step("untracked file is listed and can be staged", async () => {
    // Row actions show on hover/selection: select the row like a user would.
    const [row] = await session.waitFor(".file-item", { pred: (t) => t.some((x) => x.includes("notes.txt")) });
    await session.click(row);
    const [stage] = await session.waitFor('.file-item.selected button[aria-label="Stage file"]');
    await session.click(stage);
    await session.waitFor('button[aria-label="Unstage file"]', { timeout: 15_000 }).catch(async () => {
      // Staged rows show their actions on selection too; the staged list is enough.
      await session.waitFor(".file-item", { pred: () => git("diff", "--cached", "--name-only").includes("notes.txt") });
    });
    if (!git("diff", "--cached", "--name-only").includes("notes.txt")) throw new Error("notes.txt not staged");
  });

  await step("commit creates a new commit", async () => {
    const box = await session.find('textarea[aria-label="Commit message"]');
    await session.type(box, "add notes from e2e");
    await session.click(await session.find(".commit-btn"));
    await session.waitFor(".commit-row .summary", { pred: (t) => t.includes("add notes from e2e") });
    const log = git("log", "--format=%s", "-n", "2").trim().split("\n");
    if (log[0] !== "add notes from e2e" || log[1] !== "initial commit") {
      throw new Error(`unexpected history: ${JSON.stringify(log)}`);
    }
    if (git("status", "--porcelain").trim() !== "") throw new Error("working tree not clean");
  });
} catch (err) {
  failed = true;
  console.log("FAILED");
  console.error(err);
  // Leave evidence for CI artifacts / local debugging.
  const out = process.env.TWIG_E2E_ARTIFACTS ?? join(tmp, "artifacts");
  try {
    mkdirSync(out, { recursive: true });
    const png = await session?.screenshot();
    if (png) writeFileSync(join(out, "failure.png"), Buffer.from(png, "base64"));
    const html = await session?.execute("return document.documentElement.outerHTML;");
    if (html) writeFileSync(join(out, "failure.html"), html);
    console.error(`artifacts written to ${out}`);
  } catch (e) {
    console.error("could not capture artifacts:", e.message);
  }
} finally {
  await session?.close().catch(() => {});
  driver.kill();
}
process.exit(failed ? 1 : 0);
