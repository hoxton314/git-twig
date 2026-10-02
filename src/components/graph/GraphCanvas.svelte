<script lang="ts">
  import type { GraphEntry } from "../../lib/types/git";
  import { LANE_COLORS } from "../../lib/types/git";
  import { GRAPH_LANE_WIDTH, GRAPH_LEFT_PAD, graphColumnWidth } from "./graphLayout";

  interface Props {
    entry: GraphEntry;
    totalLanes: number;
    height: number;
    isUnpushed?: boolean;
  }

  let { entry, totalLanes, height, isUnpushed = false }: Props = $props();

  const NODE_RADIUS = 5;
  const STROKE_WIDTH = 2;
  const OPACITY = 0.7;

  const svgWidth = $derived(graphColumnWidth(totalLanes));
  const cy = $derived(height / 2);

  /**
   * Lane color as a CSS variable (themed in app.css, so light mode gets
   * darker lane colors). Applied via `style:` since SVG presentation
   * attributes don't resolve var().
   */
  function laneColor(lane: number): string {
    return LANE_VARS[lane % LANE_VARS.length];
  }

  // Spelled out literally (not built from the index) so Tailwind v4 sees
  // every lane variable as used and keeps it in the CSS; otherwise the dark
  // theme's unused-looking --color-lane-1/3/4/5 are tree-shaken and those
  // lanes render black.
  const LANE_VARS = [
    "var(--color-lane-0)",
    "var(--color-lane-1)",
    "var(--color-lane-2)",
    "var(--color-lane-3)",
    "var(--color-lane-4)",
    "var(--color-lane-5)",
  ] as const satisfies readonly string[] & { length: typeof LANE_COLORS.length };

  /** X center for a given lane index. */
  function lx(lane: number): number {
    return GRAPH_LEFT_PAD + lane * GRAPH_LANE_WIDTH + GRAPH_LANE_WIDTH / 2;
  }
</script>

<svg
  width={svgWidth}
  height={height}
  class="graph-canvas"
  aria-hidden="true"
  style="min-width: {svgWidth}px; flex-shrink: 0;"
>
  <!-- Pass-through rails: straight vertical lines for other active branches -->
  {#each entry.rails as rail}
    <line
      x1={lx(rail)} y1={0}
      x2={lx(rail)} y2={height}
      style:stroke={laneColor(rail)}
      stroke-width={STROKE_WIDTH}
      opacity={OPACITY}
    />
  {/each}

  <!-- Incoming line: from top of row down to the commit node -->
  {#if entry.has_incoming}
    <line
      x1={lx(entry.lane)} y1={0}
      x2={lx(entry.lane)} y2={cy}
      style:stroke={laneColor(entry.lane)}
      stroke-width={STROKE_WIDTH}
      opacity={OPACITY}
    />
  {/if}

  <!-- Merge-ins: other lanes waiting for this commit converge into its node -->
  {#each entry.merge_ins ?? [] as mergeLane}
    <path
      d="M {lx(mergeLane)} 0
         C {lx(mergeLane)} {cy},
           {lx(entry.lane)} 0,
           {lx(entry.lane)} {cy}"
      fill="none"
      style:stroke={laneColor(mergeLane)}
      stroke-width={STROKE_WIDTH}
      opacity={OPACITY}
    />
  {/each}

  <!-- Outgoing lines: from the commit node down to each parent's lane -->
  {#each entry.parent_lanes as parentLane}
    {#if parentLane === entry.lane}
      <!-- First parent on same lane: straight line down -->
      <line
        x1={lx(entry.lane)} y1={cy}
        x2={lx(entry.lane)} y2={height}
        style:stroke={laneColor(entry.lane)}
        stroke-width={STROKE_WIDTH}
        opacity={OPACITY}
      />
    {:else}
      <!-- Merge parent on different lane: smooth S-curve -->
      <path
        d="M {lx(entry.lane)} {cy}
           C {lx(entry.lane)} {height},
             {lx(parentLane)} {cy},
             {lx(parentLane)} {height}"
        fill="none"
        style:stroke={laneColor(parentLane)}
        stroke-width={STROKE_WIDTH}
        opacity={OPACITY}
      />
    {/if}
  {/each}

  <!-- Commit node -->
  {#if isUnpushed}
    <circle
      cx={lx(entry.lane)}
      cy={cy}
      r={NODE_RADIUS}
      style:fill="var(--color-bg)"
      style:stroke={laneColor(entry.lane)}
      stroke-width={STROKE_WIDTH}
    />
  {:else}
    <circle
      cx={lx(entry.lane)}
      cy={cy}
      r={NODE_RADIUS}
      style:fill={laneColor(entry.lane)}
    />
  {/if}
</svg>

<style>
  .graph-canvas {
    display: block;
  }
</style>
