/**
 * Minimal, safe Markdown → HTML renderer for PR/MR descriptions.
 *
 * Safety model: raw HTML is never passed through. All text is HTML-escaped
 * first and markup is only generated from recognised Markdown constructs.
 * Links are limited to http(s)/mailto URLs and carry no inline handlers;
 * the host component intercepts clicks and opens them in the system
 * browser. Images are rendered as links (no remote loads / tracking pixels).
 *
 * Supported: headings, paragraphs, line breaks, fenced + indented code,
 * blockquotes, (task) lists, horizontal rules, GFM tables, inline code,
 * bold, italic, strikethrough, links, images-as-links and autolinks.
 */

export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/** Only allow absolute http(s) and mailto links. Input is already escaped. */
function safeUrl(escapedUrl: string): string | null {
  const url = escapedUrl.trim();
  // Never let a stashed fragment (placeholder) end up inside an attribute.
  if (url.includes(PH)) return null;
  return /^(https?:\/\/|mailto:)/i.test(url) ? url : null;
}

const PH = "\u0000";

function renderInline(src: string): string {
  const slots: string[] = [];
  const stash = (html: string) => `${PH}${slots.push(html) - 1}${PH}`;

  // Code spans first so their contents are not formatted.
  let text = src.replace(/(`+)([\s\S]*?[^`])\1(?!`)/g, (_, _ticks, code: string) =>
    stash(`<code>${escapeHtml(code.trim())}</code>`),
  );
  text = escapeHtml(text);

  // Images → link to the image. ![alt](url)
  text = text.replace(/!\[([^\]]*)\]\(([^)\s\u0000]+)(?:\s+&quot;[^&]*&quot;)?\)/g, (m, alt: string, url: string) => {
    const href = safeUrl(url);
    return href ? stash(`<a href="${href}" class="md-link">[image: ${alt || "image"}]</a>`) : m;
  });
  // Links [text](url "title")
  text = text.replace(/\[([^\]]+)\]\(([^)\s\u0000]+)(?:\s+&quot;[^&]*&quot;)?\)/g, (m, label: string, url: string) => {
    const href = safeUrl(url);
    return href ? stash(`<a href="${href}" class="md-link">${label}</a>`) : m;
  });
  // Autolinks <https://...> (escaped as &lt;...&gt;) and bare URLs.
  text = text.replace(/&lt;((?:https?:\/\/|mailto:)[^\s&\u0000]+)&gt;/g, (_, url: string) =>
    stash(`<a href="${url}" class="md-link">${url}</a>`),
  );
  text = text.replace(/(^|[\s(])(https?:\/\/[^\s<\u0000]+[^\s<\u0000.,;:!?)\]'"])/g, (_, pre: string, url: string) =>
    `${pre}${stash(`<a href="${url}" class="md-link">${url}</a>`)}`,
  );

  text = text
    .replace(/\*\*(?=\S)([\s\S]*?\S)\*\*/g, "<strong>$1</strong>")
    .replace(/__(?=\S)([\s\S]*?\S)__/g, "<strong>$1</strong>")
    .replace(/~~(?=\S)([\s\S]*?\S)~~/g, "<del>$1</del>")
    .replace(/(^|[^*\w])\*(?=\S)([^*]*?\S)\*(?!\w)/g, "$1<em>$2</em>")
    .replace(/(^|[^_\w])_(?=\S)([^_]*?\S)_(?!\w)/g, "$1<em>$2</em>");

  // Restore stashed fragments (they may nest, e.g. code inside a link label).
  const restore = (s: string): string =>
    s.replace(new RegExp(`${PH}(\\d+)${PH}`, "g"), (_, i: string) => restore(slots[Number(i)] ?? ""));
  return restore(text);
}

function isTableSeparator(line: string): boolean {
  return /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/.test(line) && line.includes("-");
}

function splitRow(line: string): string[] {
  let l = line.trim();
  if (l.startsWith("|")) l = l.slice(1);
  if (l.endsWith("|") && !l.endsWith("\\|")) l = l.slice(0, -1);
  return l.split(/(?<!\\)\|/).map((c) => c.trim().replace(/\\\|/g, "|"));
}

export function renderMarkdown(source: string): string {
  // HTML comments (PR templates are full of them) are dropped entirely.
  const src = source.replace(/\r\n?/g, "\n").replace(/<!--[\s\S]*?-->/g, "");
  const lines = src.split("\n");
  const out: string[] = [];
  let i = 0;

  const isBlank = (l: string) => l.trim() === "";
  const startsBlock = (l: string) =>
    /^\s{0,3}(#{1,6}\s|>|```|~~~|[-*+]\s|\d+[.)]\s|(?:[-*_]\s*){3,}$)/.test(l);

  while (i < lines.length) {
    const line = lines[i];

    if (isBlank(line)) {
      i++;
      continue;
    }

    // Fenced code
    const fence = line.match(/^\s{0,3}(`{3,}|~{3,})\s*([\w+-]*)/);
    if (fence) {
      const marker = fence[1];
      const body: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trim().startsWith(marker)) {
        body.push(lines[i]);
        i++;
      }
      i++; // closing fence
      out.push(`<pre><code>${escapeHtml(body.join("\n"))}</code></pre>`);
      continue;
    }

    // Indented code (4 spaces / tab), only when not continuing a paragraph.
    if (/^( {4}|\t)/.test(line)) {
      const body: string[] = [];
      while (i < lines.length && (/^( {4}|\t)/.test(lines[i]) || isBlank(lines[i]))) {
        body.push(lines[i].replace(/^( {4}|\t)/, ""));
        i++;
      }
      while (body.length && isBlank(body[body.length - 1])) body.pop();
      out.push(`<pre><code>${escapeHtml(body.join("\n"))}</code></pre>`);
      continue;
    }

    // Heading
    const h = line.match(/^\s{0,3}(#{1,6})\s+(.*?)\s*#*\s*$/);
    if (h) {
      const level = h[1].length;
      out.push(`<h${level}>${renderInline(h[2])}</h${level}>`);
      i++;
      continue;
    }

    // Horizontal rule
    if (/^\s{0,3}((?:-\s*){3,}|(?:\*\s*){3,}|(?:_\s*){3,})$/.test(line)) {
      out.push("<hr>");
      i++;
      continue;
    }

    // Blockquote (rendered recursively)
    if (/^\s{0,3}>/.test(line)) {
      const body: string[] = [];
      while (i < lines.length && /^\s{0,3}>/.test(lines[i])) {
        body.push(lines[i].replace(/^\s{0,3}>\s?/, ""));
        i++;
      }
      out.push(`<blockquote>${renderMarkdown(body.join("\n"))}</blockquote>`);
      continue;
    }

    // Lists (nesting is flattened, continuation lines are joined)
    const li = line.match(/^\s*([-*+]|\d+[.)])\s+/);
    if (li) {
      const ordered = /\d/.test(li[1]);
      const items: string[] = [];
      while (i < lines.length) {
        const m = lines[i].match(/^\s*([-*+]|\d+[.)])\s+(.*)$/);
        if (m && /\d/.test(m[1]) === ordered) {
          items.push(m[2]);
          i++;
        } else if (m) {
          break; // switched list type
        } else if (!isBlank(lines[i]) && items.length && /^\s+\S/.test(lines[i])) {
          items[items.length - 1] += " " + lines[i].trim();
          i++;
        } else {
          break;
        }
      }
      const tag = ordered ? "ol" : "ul";
      const rendered = items.map((it) => {
        const task = it.match(/^\[([ xX])\]\s+(.*)$/);
        if (task) {
          const checked = task[1] !== " ";
          return `<li class="md-task"><span class="md-check${checked ? " done" : ""}">${checked ? "☑" : "☐"}</span> ${renderInline(task[2])}</li>`;
        }
        return `<li>${renderInline(it)}</li>`;
      });
      out.push(`<${tag}>${rendered.join("")}</${tag}>`);
      continue;
    }

    // GFM table
    if (line.includes("|") && i + 1 < lines.length && isTableSeparator(lines[i + 1])) {
      const header = splitRow(line);
      i += 2;
      const rows: string[][] = [];
      while (i < lines.length && lines[i].includes("|") && !isBlank(lines[i])) {
        rows.push(splitRow(lines[i]));
        i++;
      }
      const th = header.map((c) => `<th>${renderInline(c)}</th>`).join("");
      const trs = rows
        .map((r) => `<tr>${header.map((_, ci) => `<td>${renderInline(r[ci] ?? "")}</td>`).join("")}</tr>`)
        .join("");
      out.push(`<table><thead><tr>${th}</tr></thead><tbody>${trs}</tbody></table>`);
      continue;
    }

    // Paragraph: until blank line or another block starts.
    const para: string[] = [line.trim()];
    i++;
    while (i < lines.length && !isBlank(lines[i]) && !startsBlock(lines[i])) {
      para.push(lines[i].trim());
      i++;
    }
    // Hard breaks: GitHub renders single newlines in PR bodies as <br>.
    out.push(`<p>${para.map(renderInline).join("<br>")}</p>`);
  }

  return out.join("\n");
}
