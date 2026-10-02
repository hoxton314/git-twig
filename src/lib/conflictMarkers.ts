/**
 * Parse and rebuild files containing git conflict markers
 * (`<<<<<<<`, optional diff3 `|||||||`, `=======`, `>>>>>>>`).
 */

export interface TextSegment {
  kind: "text";
  lines: string[];
}

export interface ConflictSegment {
  kind: "conflict";
  ours: string[];
  base: string[] | null;
  theirs: string[];
  /** Raw marker lines, to rebuild an unresolved block exactly. */
  markers: { start: string; base: string | null; sep: string; end: string };
  /** Chosen lines once resolved, `null` while unresolved. */
  resolution: string[] | null;
  /** How it was resolved, for display. */
  choice: "ours" | "theirs" | "both" | "both-reversed" | "base" | null;
}

export type Segment = TextSegment | ConflictSegment;

function isMarker(line: string, ch: string, exact = false): boolean {
  const l = line.endsWith("\r") ? line.slice(0, -1) : line;
  const marker = ch.repeat(7);
  if (exact) return l === marker;
  return l === marker || (l.startsWith(marker) && l[7] === " ");
}

/** Split file text into plain text and conflict blocks. */
export function parseConflicts(text: string): Segment[] {
  const lines = text.split("\n");
  const segments: Segment[] = [];
  let textBuf: string[] = [];
  let cur: ConflictSegment | null = null;
  let raw: string[] = [];
  let phase: "ours" | "base" | "theirs" = "ours";

  const flushText = () => {
    if (textBuf.length > 0) segments.push({ kind: "text", lines: textBuf });
    textBuf = [];
  };

  for (const line of lines) {
    if (!cur) {
      if (isMarker(line, "<")) {
        flushText();
        cur = {
          kind: "conflict",
          ours: [],
          base: null,
          theirs: [],
          markers: { start: line, base: null, sep: "", end: "" },
          resolution: null,
          choice: null,
        };
        raw = [line];
        phase = "ours";
      } else {
        textBuf.push(line);
      }
      continue;
    }

    raw.push(line);
    if (phase === "ours" && isMarker(line, "|")) {
      phase = "base";
      cur.base = [];
      cur.markers.base = line;
    } else if (phase !== "theirs" && isMarker(line, "=", true)) {
      phase = "theirs";
      cur.markers.sep = line;
    } else if (phase === "theirs" && isMarker(line, ">")) {
      cur.markers.end = line;
      segments.push(cur);
      cur = null;
      raw = [];
    } else if (phase === "ours") {
      cur.ours.push(line);
    } else if (phase === "base") {
      cur.base?.push(line);
    } else {
      cur.theirs.push(line);
    }
  }

  // Unterminated block: not a real conflict, keep the text as-is.
  if (cur) textBuf.push(...raw);
  flushText();
  return segments;
}

function segmentLines(seg: Segment): string[] {
  if (seg.kind === "text") return seg.lines;
  if (seg.resolution) return seg.resolution;
  const out = [seg.markers.start, ...seg.ours];
  if (seg.base && seg.markers.base !== null) out.push(seg.markers.base, ...seg.base);
  out.push(seg.markers.sep, ...seg.theirs, seg.markers.end);
  return out;
}

/** Rebuild the file text (unresolved blocks keep their markers). */
export function serializeSegments(segments: Segment[]): string {
  return segments.flatMap(segmentLines).join("\n");
}

export function resolveSegment(
  seg: ConflictSegment,
  choice: NonNullable<ConflictSegment["choice"]>,
): ConflictSegment {
  let resolution: string[];
  switch (choice) {
    case "ours":
      resolution = [...seg.ours];
      break;
    case "theirs":
      resolution = [...seg.theirs];
      break;
    case "both":
      resolution = [...seg.ours, ...seg.theirs];
      break;
    case "both-reversed":
      resolution = [...seg.theirs, ...seg.ours];
      break;
    case "base":
      resolution = [...(seg.base ?? [])];
      break;
  }
  return { ...seg, resolution, choice };
}

/** Marker label after `<<<<<<< ` / `>>>>>>> `. */
export function markerLabel(marker: string): string {
  return marker.slice(8).replace(/\r$/, "").trim();
}
