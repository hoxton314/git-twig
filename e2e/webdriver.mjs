// Minimal W3C WebDriver client (just what the smoke tests use), on Node's
// built-in fetch — no WebdriverIO dependency.
const ELEMENT = "element-6066-11e4-a52e-4f735466cecf";

export class Session {
  constructor(base, id) {
    this.base = base;
    this.id = id;
  }

  static async create(base, capabilities) {
    const res = await call(base, "POST", "/session", { capabilities: { alwaysMatch: capabilities } });
    return new Session(base, res.sessionId);
  }

  cmd(method, path, body) {
    return call(this.base, method, `/session/${this.id}${path}`, body);
  }

  async find(css) {
    const v = await this.cmd("POST", "/element", { using: "css selector", value: css });
    return v[ELEMENT];
  }

  async findAll(css) {
    const v = await this.cmd("POST", "/elements", { using: "css selector", value: css });
    return v.map((e) => e[ELEMENT]);
  }

  /** Poll until `css` matches (and `pred(texts)` holds, if given). */
  async waitFor(css, { timeout = 30_000, pred } = {}) {
    const end = Date.now() + timeout;
    let last = "";
    while (Date.now() < end) {
      try {
        const els = await this.findAll(css);
        if (els.length > 0) {
          if (!pred) return els;
          const texts = await Promise.all(els.map((e) => this.text(e)));
          last = JSON.stringify(texts);
          if (pred(texts)) return els;
        }
      } catch {
        // page still loading
      }
      await new Promise((r) => setTimeout(r, 250));
    }
    throw new Error(`timed out waiting for ${css} ${last}`);
  }

  click(el) {
    return this.cmd("POST", `/element/${el}/click`, {});
  }

  type(el, text) {
    return this.cmd("POST", `/element/${el}/value`, { text });
  }

  text(el) {
    return this.cmd("GET", `/element/${el}/text`);
  }

  execute(script, args = []) {
    return this.cmd("POST", "/execute/sync", { script, args });
  }

  close() {
    return this.cmd("DELETE", "");
  }
}

async function call(base, method, path, body) {
  const res = await fetch(base + path, {
    method,
    headers: { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = await res.json().catch(() => ({}));
  if (!res.ok || json.value?.error) {
    throw new Error(`${method} ${path}: ${res.status} ${JSON.stringify(json.value ?? json)}`);
  }
  return json.value;
}
