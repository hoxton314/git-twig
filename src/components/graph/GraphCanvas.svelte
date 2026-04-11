<script lang="ts">
  import type { GraphEntry } from "../../lib/types/git";
  import { laneColor } from "../../lib/types/git";

  interface Props {
    entry: GraphEntry;
    totalLanes: number;
    height: number;
  }

  let { entry, totalLanes, height }: Props = $props();

  const LANE_WIDTH = 20;
  const NODE_RADIUS = 5;
  const STROKE_WIDTH = 2;
  const OPACITY = 0.7;

  const svgWidth = $derived(Math.max((totalLanes + 1) * LANE_WIDTH, 48));
  const cy = $derived(height / 2);

  /** X center for a given lane index. */
  function lx(lane: number): number {
    return lane * LANE_WIDTH + LANE_WIDTH / 2;
  }
</script>

<svg
  width={svgWidth}
  height={height}
  class="graph-canvas"
  style="min-width: {svgWidth}px; flex-shrink: 0;"
>
  <!-- Pass-through rails: straight vertical lines for other active branches -->
  {#each entry.rails as rail}
    <line
      x1={lx(rail)} y1={0}
      x2={lx(rail)} y2={height}
      stroke={laneColor(rail)}
      stroke-width={STROKE_WIDTH}
      opacity={OPACITY}
    />
  {/each}

  <!-- Incoming line: from top of row down to the commit node -->
  {#if entry.has_incoming}
    <line
      x1={lx(entry.lane)} y1={0}
      x2={lx(entry.lane)} y2={cy}
      stroke={laneColor(entry.lane)}
      stroke-width={STROKE_WIDTH}
      opacity={OPACITY}
    />
  {/if}

  <!-- Outgoing lines: from the commit node down to each parent's lane -->
  {#each entry.parent_lanes as parentLane}
    {#if parentLane === entry.lane}
      <!-- First parent on same lane: straight line down -->
      <line
        x1={lx(entry.lane)} y1={cy}
        x2={lx(entry.lane)} y2={height}
        stroke={laneColor(entry.lane)}
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
        stroke={laneColor(parentLane)}
        stroke-width={STROKE_WIDTH}
        opacity={OPACITY}
      />
    {/if}
  {/each}

  <!-- Commit node -->
  <circle
    cx={lx(entry.lane)}
    cy={cy}
    r={NODE_RADIUS}
    fill={laneColor(entry.lane)}
  />
</svg>

<style>
  .graph-canvas {
    display: block;
  }
</style>
