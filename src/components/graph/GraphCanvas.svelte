<script lang="ts">
  import type { GraphEntry } from "../../lib/types/git";
  import { laneColor } from "../../lib/types/git";

  interface Props {
    entry: GraphEntry;
    totalLanes: number;
    height: number;
  }

  let { entry, totalLanes, height }: Props = $props();

  const LANE_WIDTH = 16;
  const NODE_RADIUS = 4;

  const svgWidth = $derived(Math.max((totalLanes + 1) * LANE_WIDTH, 40));
  const cx = $derived(entry.lane * LANE_WIDTH + LANE_WIDTH / 2);
  const cy = $derived(height / 2);
</script>

<svg
  width={svgWidth}
  height={height}
  class="graph-canvas"
  style="min-width: {svgWidth}px; flex-shrink: 0;"
>
  <!-- Edges to parents -->
  {#each entry.edges as [fromLane, toLane, _parentOid]}
    {@const x1 = fromLane * LANE_WIDTH + LANE_WIDTH / 2}
    {@const x2 = toLane * LANE_WIDTH + LANE_WIDTH / 2}
    <path
      d="M {x1} {cy} C {x1} {height}, {x2} {cy}, {x2} {height}"
      fill="none"
      stroke={laneColor(toLane)}
      stroke-width="2"
      opacity="0.7"
    />
  {/each}

  <!-- Continuation lines for active lanes that pass through this row -->
  <!-- (handled implicitly by parent/child edges spanning rows) -->

  <!-- Commit node -->
  <circle
    cx={cx}
    cy={cy}
    r={NODE_RADIUS}
    fill={laneColor(entry.lane)}
    stroke={laneColor(entry.lane)}
    stroke-width="1.5"
  />
</svg>

<style>
  .graph-canvas {
    display: block;
  }
</style>
