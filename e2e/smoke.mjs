// End-to-end smoke test against the real app via tauri-driver.
//
//   npm run tauri build -- --debug --no-bundle   # builds src-tauri/target/debug/twig
//   xvfb-run -a -s "-screen 0 1440x900x24" npm run test:e2e   # needs tauri-driver + WebKitWebDriver
//
// Uses throwaway XDG dirs, git config and repo, so a developer's real
// settings, session and repos are never touched.
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, existsSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Session } from "./webdriver.mjs";

const root = resolve(import.meta.dirname, "..");
const app = process.env.TWIG_E2E_APP ?? join(root, "src-tauri/target/debug/twig");
const driverBin = process.env.TAURI_DRIVER ?? "tauri-driver";
const port = Number(process.env.TWIG_E2E_PORT ?? 4444);

if (!existsSync(app)) {
  console.error(`App binary not found: ${app}\nBuild it with: npm run tauri build -- --debug --no-bundle`);
  process.exit(2);
}

// ── Isolated environment ────────────────────────────────────────────
const tmp = mkdtempSync(join(tmpdir(), "twig-e2e-"));
const xdg = {
  data: join(tmp, "data"),
  config: join(tmp, "config"),
  cache: join(tmp, "cache"),
  runtime: join(tmp, "run"),
};
mkdirSync(xdg.runtime, { recursive: true, mode: 0o700 });
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
// A 20k-line file whose every line changes: a 40k-row diff (windowing test).
const BIG_LINES = 20_000;
writeFileSync(join(repo, "big.txt"), Array.from({ length: BIG_LINES }, (_, i) => `line ${i}\n`).join(""));
// Every 10th line changes: ~2000 small hunks, too many rows to render unwindowed.
writeFileSync(join(repo, "scattered.txt"), Array.from({ length: BIG_LINES }, (_, i) => `row ${i}\n`).join(""));
git("add", "README.md", "big.txt", "scattered.txt");
git("commit", "-q", "-m", "initial commit");
// Installed after the initial commit: its output must show up in the app.
writeFileSync(join(repo, ".git", "hooks", "pre-commit"), "#!/bin/sh\necho 'e2e pre-commit hook ran'\n", { mode: 0o755 });
writeFileSync(join(repo, "notes.txt"), "from the e2e test\n");
writeFileSync(
  join(repo, "scattered.txt"),
  Array.from({ length: BIG_LINES }, (_, i) => (i % 10 === 0 ? `ROW ${i}\n` : `row ${i}\n`)).join(""),
);
writeFileSync(
  join(repo, "big.txt"),
  Array.from({ length: BIG_LINES }, (_, i) => `LINE ${i}\n`).join("") + "needle-at-the-end\n",
);

// Another repository in the default folder, found by the "+" menu search.
const extra = join(tmp, "extra-proj");
mkdirSync(extra);
execFileSync("git", ["init", "-q"], { cwd: extra, env });
execFileSync("git", ["commit", "-q", "--allow-empty", "-m", "extra"], { cwd: extra, env });

// Open the repo through a saved session (no native file dialog needed).
writeFileSync(join(appData, "session.json"), JSON.stringify({ paths: [repo], active: repo }));
writeFileSync(
  join(appData, "settings.json"),
  JSON.stringify({
    check_updates_on_startup: false,
    auto_fetch_interval: 0,
    confirm_destructive_ops: false,
    default_repo_dir: tmp,
  }),
);

// ── Driver ──────────────────────────────────────────────────────────
const base = `http://127.0.0.1:${port}`;
const listening = () => fetch(`${base}/status`).then(() => true, () => false);
if (await listening()) {
  console.error(`Port ${port} is already in use; set TWIG_E2E_PORT to a free port.`);
  process.exit(2);
}
// A private D-Bus session: Twig's single-instance lock is a D-Bus name, so
// on the shared session bus the test app would hand off to a real running
// Twig (and focus it) instead of starting.
const hasDbusRunSession = (() => {
  try {
    execFileSync("dbus-run-session", ["--version"], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
})();
if (!hasDbusRunSession) console.warn("dbus-run-session not found; using the shared session bus");
// With a private bus also use a private runtime dir: otherwise a keyring
// daemon activated on that bus finds and uses the developer's real one.
// (Without one, keep the real runtime dir: zbus may locate the shared bus there.)
if (hasDbusRunSession) env.XDG_RUNTIME_DIR = xdg.runtime;
const [cmd, ...cmdArgs] = hasDbusRunSession
  ? ["dbus-run-session", "--", driverBin, "--port", String(port)]
  : [driverBin, "--port", String(port)];
// Own process group, so cleanup also reaches tauri-driver under dbus-run-session.
const driver = spawn(cmd, cmdArgs, { env, stdio: ["ignore", "inherit", "inherit"], detached: true });
const killDriver = () => {
  try {
    process.kill(-driver.pid, "SIGTERM");
  } catch {
    driver.kill();
  }
};
let driverExit = null;
driver.on("error", (err) => {
  console.error(`Could not start ${driverBin}: ${err.message}\nInstall it with: cargo install tauri-driver --locked`);
  process.exit(2);
});
driver.on("exit", (code, signal) => {
  driverExit = `tauri-driver exited (${signal ?? code})`;
});

/** Poll a condition outside the app (e.g. the repo's real git state). */
async function until(cond, what, timeout = 15_000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    if (cond()) return;
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(`timed out waiting for ${what}`);
}

let session;
let failed = false;
const step = async (name, fn) => {
  process.stdout.write(`• ${name} … `);
  await fn();
  console.log("ok");
};

try {
  for (let i = 0; !(await listening()); i++) {
    if (driverExit) throw new Error(`${driverExit} before accepting connections (is WebKitWebDriver installed?)`);
    if (i > 100) throw new Error("tauri-driver did not start within 10s");
    await new Promise((r) => setTimeout(r, 100));
  }
  session = await Session.create(base, { "tauri:options": { application: app } });

  await step("app starts and restores the repo tab", async () => {
    await session.waitFor(".commit-row .summary", { pred: (t) => t.includes("initial commit") });
  });

  await step("untracked file is listed and can be staged", async () => {
    // Row actions show on hover/selection: select the row like a user would.
    const rows = await session.waitFor(".file-item", { pred: (t) => t.some((x) => x.includes("notes.txt")) });
    const texts = await session.texts(".file-item");
    await session.click(rows[texts.findIndex((t) => t.includes("notes.txt"))]);
    const [stage] = await session.waitFor('.file-item.selected button[aria-label="Stage file"]');
    await session.click(stage);
    await until(() => git("diff", "--cached", "--name-only").includes("notes.txt"), "notes.txt staged");
  });

  await step("commit creates a new commit", async () => {
    const box = await session.find('textarea[aria-label="Commit message"]');
    await session.type(box, "add notes from e2e");
    await session.click(await session.find(".commit-btn"));
    await session.waitFor(".commit-row .summary", { pred: (t) => t.includes("add notes from e2e") });
    await session.waitFor(".hook-output", { pred: (t) => t.some((x) => x.includes("e2e pre-commit hook ran") && x.includes("pre-commit")) });
    await session.click(await session.find('button[aria-label="Dismiss hook output"]'));
    const log = git("log", "--format=%s", "-n", "2").trim().split("\n");
    if (log[0] !== "add notes from e2e" || log[1] !== "initial commit") {
      throw new Error(`unexpected history: ${JSON.stringify(log)}`);
    }
    // Only the pre-modified big.txt (used by the next step) is left.
    const status = git("--no-optional-locks", "status", "--porcelain").replace(/\n$/, "");
    if (status !== " M big.txt\n M scattered.txt") throw new Error(`unexpected status: ${JSON.stringify(status)}`);
  });

  await step("Ctrl-clicking a second commit compares the two", async () => {
    await session.click(await session.findByText(".commit-row", "add notes from e2e"));
    await session.waitFor(".diff-title .oid");
    // WebDriver's element click carries no modifiers; dispatch a Ctrl-click.
    const older = await session.findByText(".commit-row", "initial commit");
    await session.execute(
      "arguments[0].dispatchEvent(new MouseEvent('click', { bubbles: true, ctrlKey: true }));",
      [{ "element-6066-11e4-a52e-4f735466cecf": older }],
    );
    await session.waitFor(".diff-title", { pred: (t) => t.some((x) => x.includes("Comparing")) });
    const selected = await session.execute("return document.querySelectorAll('.commit-row.selected').length;");
    if (selected !== 2) throw new Error(`expected 2 selected rows, found ${selected}`);
    await session.waitFor(".diff-files", { pred: (t) => t.some((x) => x.includes("notes.txt")) });
    // Escape clears the whole selection.
    await session.execute(
      "document.querySelector('.commit-graph')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));",
    );
    const end = Date.now() + 5_000;
    let left = -1;
    while (Date.now() < end && left !== 0) {
      left = await session.execute("return document.querySelectorAll('.commit-row.selected').length;");
      if (left !== 0) await new Promise((r) => setTimeout(r, 100));
    }
    if (left !== 0) throw new Error(`Escape left ${left} rows selected`);
  });

  await step("squashing two selected commits rewrites them into one", async () => {
    const el = (id) => ({ "element-6066-11e4-a52e-4f735466cecf": id });
    await session.click(await session.findByText(".commit-row", "add notes from e2e"));
    const older = await session.findByText(".commit-row", "initial commit");
    await session.execute(
      "arguments[0].dispatchEvent(new MouseEvent('click', { bubbles: true, ctrlKey: true }));",
      [el(older)],
    );
    await session.execute(
      "const r = arguments[0].getBoundingClientRect(); arguments[0].dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.left + 40, clientY: r.top + 5 }));",
      [el(older)],
    );
    await session.click(await session.findByText(".context-menu .item", "Squash 2 commits"));
    const [box] = await session.waitFor('textarea[aria-label="Squashed commit message"]');
    // A bound textarea has no text content; read its value.
    const end = Date.now() + 10_000;
    let value = "";
    while (Date.now() < end && !value.includes("initial commit")) {
      value = await session.execute("return arguments[0].value;", [el(box)]);
      if (!value.includes("initial commit")) await new Promise((r) => setTimeout(r, 100));
    }
    if (!value.includes("initial commit") || !value.includes("add notes from e2e")) {
      throw new Error(`unexpected default message ${JSON.stringify(value)}`);
    }
    await session.execute(
      "arguments[0].value = 'add notes from e2e'; arguments[0].dispatchEvent(new Event('input', { bubbles: true }));",
      [el(box)],
    );
    await session.click(await session.findByText("button[type=submit]", "Squash 2 Commits"));
    // Wait for the app to finish (the dialog closes) before touching the repo:
    // a plain `git status` mid-rebase takes index.lock and can make git fail
    // to re-apply the autostash (it then parks it in the stash list).
    const closeBy = Date.now() + 60_000;
    while (Date.now() < closeBy && (await session.execute("return !!document.querySelector('[aria-modal=\"true\"]');"))) {
      await new Promise((r) => setTimeout(r, 100));
    }
    if (await session.execute("return !!document.querySelector('[aria-modal=\"true\"]');")) {
      throw new Error("squash dialog did not close");
    }
    const log = git("log", "--format=%s").trim();
    if (log !== "add notes from e2e") throw new Error(`unexpected history after squash: ${JSON.stringify(log)}`);
    if (!existsSync(join(repo, "notes.txt"))) throw new Error("notes.txt lost in the squash");
    // Autostash put the working changes back.
    const after = git("--no-optional-locks", "status", "--porcelain").replace(/\n$/, "");
    if (after !== " M big.txt\n M scattered.txt") throw new Error(`unexpected status after squash: ${JSON.stringify(after)}`);
    const stashes = git("stash", "list").trim();
    if (stashes) throw new Error(`squash left a stash entry: ${stashes}`);
  });

  await step("code search finds text and opens blame at the line", async () => {
    await session.execute(
      "window.dispatchEvent(new KeyboardEvent('keydown', { key: 'G', code: 'KeyG', ctrlKey: true, shiftKey: true, bubbles: true }));",
    );
    const [box] = await session.waitFor('input[aria-label="Search code"]');
    await session.type(box, "from the e2e");
    await session.waitFor(".search .file-path", { pred: (t) => t.includes("notes.txt") });
    await session.waitFor(".search mark", { pred: (t) => t.some((x) => x.toLowerCase() === "from the e2e") });
    const match = await session.findByText(".search .match", "from the e2e test");
    await session.click(match);
    await session.waitFor(".line.target", { pred: (t) => t.some((x) => x.includes("from the e2e test")) });
    // Close the blame view again (Escape on the dialog) before the next step.
    await session.execute(
      "(document.querySelector('[role=dialog]') ?? document.body).dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));",
    );
    const end = Date.now() + 5_000;
    while (Date.now() < end && (await session.execute("return document.querySelectorAll('.line.target').length;")) > 0) {
      await new Promise((r) => setTimeout(r, 100));
    }
    if ((await session.execute("return document.querySelectorAll('.line.target').length;")) > 0) {
      throw new Error("blame view did not close");
    }
  });

  await step("graph code search finds the commit that added a string", async () => {
    await session.click(await session.findByText(".graph-toolbar .tool-btn", "Search"));
    const [box] = await session.waitFor('input[aria-label="Search commits"]');
    await session.click(await session.findByText('.graph-toolbar [role="radio"]', "Code"));
    await session.type(box, "from the e2e test");
    await session.type(box, "\uE007"); // Enter
    await session.waitFor(".graph-toolbar .count", { pred: (t) => t.some((x) => /^1 (of 1|commit)$/.test(x)) });
    const hits = await session.execute("return document.querySelectorAll('.commit-row.search-match').length;");
    if (hits !== 1) throw new Error(`expected 1 highlighted commit, found ${hits}`);
    await session.type(box, "\uE00C"); // Escape closes the search
    await session.waitFor(".graph-toolbar .tool-btn");
  });

  await step("a 40k-row diff renders windowed and search reaches its last line", async () => {
    const rows = await session.waitFor(".file-item", { pred: (t) => t.some((x) => x.includes("big.txt")) });
    const texts = await session.texts(".file-item");
    await session.click(rows[texts.findIndex((t) => t.includes("big.txt"))]);
    await session.waitFor(".diff-files tr.line", { timeout: 60_000 });
    const domRows = await session.execute("return document.querySelectorAll('.diff-files tr.line').length;");
    if (!(domRows > 0 && domRows < 1500)) throw new Error(`expected a windowed diff, found ${domRows} rendered rows`);

    await session.click(await session.find('button[aria-label="Find in diff"]'));
    const [box] = await session.waitFor('input[aria-label="Find in diff"]');
    await session.type(box, "needle-at-the-end");
    await session.waitFor(".match-count", { pred: (t) => t.some((x) => x.includes("1 of 1")), timeout: 20_000 });
    // The match is ~40k rows down: its row must have been brought into the window.
    await session.waitFor(".search-hit.search-active", { pred: (t) => t.some((x) => x.includes("needle")), timeout: 20_000 });
    const after = await session.execute("return document.querySelectorAll('.diff-files tr.line').length;");
    if (!(after < 1500)) throw new Error(`window grew to ${after} rows after jumping to the match`);
  });

  await step("a file with thousands of small hunks is collapsed behind Show anyway", async () => {
    const rows = await session.waitFor(".file-item", { pred: (t) => t.some((x) => x.includes("scattered.txt")) });
    const texts = await session.texts(".file-item");
    await session.click(rows[texts.findIndex((t) => t.includes("scattered.txt"))]);
    await session.waitFor(".binary-notice", { pred: (t) => t.some((x) => x.includes("Large diff")), timeout: 30_000 });
    const domRows = await session.execute("return document.querySelectorAll('.diff-files tr.line').length;");
    if (domRows !== 0) throw new Error(`collapsed diff still rendered ${domRows} rows`);
  });

  await step("clone from URL opens the clone in a new tab", async () => {
    await session.click(await session.find('button[aria-label="Home"]'));
    await session.click(await session.findByText("button", "Clone from URL"));
    const [urlBox] = await session.waitFor('input[placeholder^="https://"]');
    await session.type(urlBox, `file://${repo}`);
    // Pre-filled from default_repo_dir; set it explicitly.
    const parent = await session.find('input[aria-label="Parent folder"]');
    await session.execute("arguments[0].value = ''; arguments[0].dispatchEvent(new Event('input', { bubbles: true }));", [{ "element-6066-11e4-a52e-4f735466cecf": parent }]);
    await session.type(parent, tmp);
    const name = await session.find('input[aria-label="Folder name"]');
    await session.execute("arguments[0].value = ''; arguments[0].dispatchEvent(new Event('input', { bubbles: true }));", [{ "element-6066-11e4-a52e-4f735466cecf": name }]);
    await session.type(name, "cloned");
    await session.click(await session.findByText("button[type=submit]", "Clone"));
    await session.waitFor(".tab .tab-name", { pred: (t) => t.includes("cloned"), timeout: 60_000 });
    await until(() => existsSync(join(tmp, "cloned", ".git")), "clone on disk");
    const head = execFileSync("git", ["log", "--format=%s", "-n", "1"], { cwd: join(tmp, "cloned"), env, encoding: "utf8" }).trim();
    if (head !== "add notes from e2e") throw new Error(`clone HEAD is ${JSON.stringify(head)}`);
  });

  await step("the + menu search finds a repository and Enter opens it", async () => {
    await session.click(await session.find('button[aria-label="Add repository"]'));
    const [box] = await session.waitFor('input[aria-label="Search repositories"]');
    const focused = await session.execute("return document.activeElement?.getAttribute('aria-label');");
    if (focused !== "Search repositories") throw new Error(`search box not focused (${focused})`);
    await session.type(box, "extra");
    await session.waitFor(".tab-menu-item-selected", { pred: (t) => t.some((x) => x.includes("extra-proj")) });
    await session.type(box, "\uE007"); // Enter
    await session.waitFor(".tab .tab-name", { pred: (t) => t.includes("extra-proj"), timeout: 30_000 });
  });

  await step("the dashboard shows every open repository and fetches them all", async () => {
    await session.click(await session.find('button[aria-label="Home"]'));
    await session.click(await session.findByText(".groups .link-btn", "Dashboard"));
    await session.waitFor(".dash tbody tr", { pred: (t) => ["repo", "cloned", "extra-proj"].every((n) => t.some((x) => x.includes(n))) });
    // The clone tracks origin; the others have no upstream.
    const rows = await session.texts(".dash tbody tr");
    const cloned = rows.find((x) => x.includes("cloned")) ?? "";
    if (!cloned.includes("main") || !cloned.includes("✓")) throw new Error(`unexpected clone row: ${JSON.stringify(cloned)}`);
    await session.click(await session.findByText(".dash .btn", "Fetch all"));
    await session.waitFor(".dash td.status", { pred: (t) => t.filter((x) => x === "Fetched").length >= 3, timeout: 60_000 });
    await session.execute(
      "document.querySelector('[role=dialog]')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));",
    );
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
  killDriver();
  // Keep the temp dir only when it holds failure evidence.
  if (!failed) {
    // A document portal started on the private bus mounts a FUSE fs at
    // <runtime>/doc until the bus exits; retry, and never fail a passed run
    // over a leftover temp dir.
    try {
      rmSync(tmp, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
    } catch (err) {
      console.warn(`Could not remove ${tmp}: ${err.message}`);
    }
  }
}
process.exit(failed ? 1 : 0);
