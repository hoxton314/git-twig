import { describe, expect, it } from "vitest";
import { escapeHtml, renderMarkdown } from "./markdown";

describe("renderMarkdown safety", () => {
  it("never passes raw HTML through", () => {
    const html = renderMarkdown('<img src=x onerror="alert(1)"> <script>alert(2)</script>');
    expect(html).not.toMatch(/<img|<script/i);
    expect(html).toContain("&lt;script&gt;");
  });

  it("drops non-http(s) link targets", () => {
    for (const href of ["javascript:alert(1)", "data:text/html,x", "vbscript:x", "file:///etc/passwd"]) {
      const html = renderMarkdown(`[click](${href})`);
      expect(html).not.toMatch(/href="(?!https?:|mailto:)/i);
      expect(html.toLowerCase()).not.toContain(`href="${href.toLowerCase()}`);
    }
  });

  it("keeps http(s) links and cannot break out of the attribute", () => {
    const html = renderMarkdown('[ok](https://example.com/a?b="c"&d=<e>)');
    expect(html).toContain('href="https://example.com/a?b=&quot;c&quot;&amp;d=&lt;e&gt;"');
    expect(html).not.toMatch(/on\w+=/i);
  });

  it("renders images as links, never as <img>", () => {
    const html = renderMarkdown("![pixel](https://tracker.example/p.gif)");
    expect(html).not.toMatch(/<img/i);
    expect(html).toContain("https://tracker.example/p.gif");
  });

  it("escapes code blocks and inline code", () => {
    expect(renderMarkdown("```\n<b>x</b>\n```")).toContain("&lt;b&gt;x&lt;/b&gt;");
    expect(renderMarkdown("`<i>`")).toContain("&lt;i&gt;");
  });
});

describe("escapeHtml", () => {
  it("escapes all five special characters", () => {
    expect(escapeHtml(`<a href="x" title='y'>&</a>`)).toBe(
      "&lt;a href=&quot;x&quot; title=&#39;y&#39;&gt;&amp;&lt;/a&gt;",
    );
  });
});
