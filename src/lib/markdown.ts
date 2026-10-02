/**
 * Minimal, safe Markdown -> HTML for release notes.
 *
 * All input is HTML-escaped first; only a small whitelist of constructs is
 * turned into markup: headings, bullet/numbered lists, fenced and inline
 * code, bold/italic, http(s) links (explicit and bare). Raw HTML in the
 * source is shown as text. Links carry `data-external` so the caller can
 * open them with the system browser instead of navigating the webview.
 */

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

const SAFE_URL = /^https?:\/\/[^\s]+$/i;

function link(href: string, text: string): string {
  // `href` is already HTML-escaped (quotes included), so it cannot break out.
  return `<a href="${href}" data-external target="_blank" rel="noopener noreferrer">${text}</a>`;
}

/** Inline formatting on one line of raw (unescaped) text. */
function inline(raw: string): string {
  // Split out `code` spans so nothing inside them is formatted.
  const parts = raw.split(/(`[^`]+`)/g);
  return parts
    .map((part) => {
      if (part.length > 1 && part.startsWith("`") && part.endsWith("`")) {
        return `<code>${escapeHtml(part.slice(1, -1))}</code>`;
      }
      let s = escapeHtml(part);
      const links: string[] = [];
      const stash = (html: string) => {
        links.push(html);
        return `\u0000${links.length - 1}\u0000`;
      };
      // [text](https://...)
      s = s.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (m, text: string, url: string) =>
        SAFE_URL.test(url.replace(/&amp;/g, "&")) ? stash(link(url, text)) : m,
      );
      // Bare URLs
      s = s.replace(/https?:\/\/[^\s<]+[^\s<.,;:!?)]/gi, (url) => stash(link(url, url)));
      s = s.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
      s = s.replace(/__([^_]+)__/g, "<strong>$1</strong>");
      s = s.replace(/(^|[\s(])\*([^*\s][^*]*)\*/g, "$1<em>$2</em>");
      // eslint-disable-next-line no-control-regex
      return s.replace(/\u0000(\d+)\u0000/g, (_, i: string) => links[Number(i)] ?? "");
    })
    .join("");
}

export function renderMarkdown(source: string): string {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const out: string[] = [];
  let list: "ul" | "ol" | null = null;
  let para: string[] = [];
  let inFence = false;
  let fence: string[] = [];

  const flushPara = () => {
    if (para.length) out.push(`<p>${para.map(inline).join("<br>")}</p>`);
    para = [];
  };
  const closeList = () => {
    if (list) out.push(`</${list}>`);
    list = null;
  };

  for (const line of lines) {
    if (/^\s*```/.test(line)) {
      if (inFence) {
        out.push(`<pre><code>${escapeHtml(fence.join("\n"))}</code></pre>`);
        fence = [];
        inFence = false;
      } else {
        flushPara();
        closeList();
        inFence = true;
      }
      continue;
    }
    if (inFence) {
      fence.push(line);
      continue;
    }

    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    const bullet = /^\s*[-*+]\s+(.*)$/.exec(line);
    const numbered = /^\s*\d+[.)]\s+(.*)$/.exec(line);

    if (heading) {
      flushPara();
      closeList();
      // Release notes live inside a small banner: map to h4..h6.
      const level = Math.min(6, heading[1].length + 3);
      out.push(`<h${level}>${inline(heading[2])}</h${level}>`);
    } else if (bullet || numbered) {
      flushPara();
      const kind = bullet ? "ul" : "ol";
      if (list !== kind) {
        closeList();
        out.push(`<${kind}>`);
        list = kind;
      }
      out.push(`<li>${inline((bullet ?? numbered)![1])}</li>`);
    } else if (line.trim() === "") {
      flushPara();
      closeList();
    } else {
      closeList();
      para.push(line.trim());
    }
  }
  if (inFence) out.push(`<pre><code>${escapeHtml(fence.join("\n"))}</code></pre>`);
  flushPara();
  closeList();
  return out.join("");
}
