<!--
  Small CI status badge (✓ / ✗ / ● / –) with a tooltip listing the checks.

  Drop-in usage:
    <CiBadge sha={branch.oid} />                      // active repo, preferred remote
    <CiBadge sha={sha} repoPath={path} remote="upstream" />
    <CiBadge state="success" />                       // pre-known state, no fetch

  Renders nothing while loading for the first time, when the repo has no
  hosted remote / no token, or when the commit has no CI at all.
-->
<script lang="ts">
  import { ciStatusFor, type CiEntry } from "../../lib/stores/ci";
  import { activeRepoPath } from "../../lib/stores/repos";
  import type { CiState, CiStatus } from "../../lib/types/hosting";
  import { readable, type Readable } from "svelte/store";

  interface Props {
    sha?: string;
    repoPath?: string | null;
    remote?: string | null;
    /** Known state (e.g. from a PR list); skips fetching when no `sha`. */
    state?: CiState | null;
    size?: number;
    /** Show "none" as a muted dash instead of hiding the badge. */
    showNone?: boolean;
  }

  let { sha = "", repoPath = undefined, remote = null, state = null, size = 12, showNone = false }: Props = $props();

  const EMPTY: Readable<CiEntry> = readable({ status: null, error: null, loading: false });
  const entry = $derived(sha ? ciStatusFor(sha, repoPath === undefined ? $activeRepoPath : repoPath, remote) : EMPTY);

  const status = $derived<CiStatus | null>($entry.status);
  const effective = $derived<CiState | null>(status?.state ?? state);

  const ICON: Record<CiState, string> = {
    success: "✓",
    failure: "✗",
    pending: "●",
    neutral: "–",
    none: "–",
  };
  const LABEL: Record<CiState, string> = {
    success: "Checks passed",
    failure: "Checks failed",
    pending: "Checks running",
    neutral: "Checks neutral / skipped",
    none: "No CI checks",
  };

  const tooltip = $derived.by(() => {
    if (!effective) return "";
    const head = LABEL[effective];
    if (!status || status.checks.length === 0) return head;
    const lines = status.checks
      .slice(0, 20)
      .map((c) => `${ICON[c.state]} ${c.name}${c.description ? ` — ${c.description}` : ""}`);
    if (status.checks.length > 20) lines.push(`… and ${status.checks.length - 20} more`);
    return `${head}\n${lines.join("\n")}`;
  });

  const visible = $derived(effective !== null && (effective !== "none" || showNone));
</script>

{#if visible && effective}
  <span
    class="ci-badge {effective}"
    style="font-size: {size}px"
    title={tooltip}
    role="img"
    aria-label={LABEL[effective]}
  >{ICON[effective]}</span>
{/if}

<style>
  .ci-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    line-height: 1;
    font-weight: 700;
    flex-shrink: 0;
    cursor: default;
    user-select: none;
  }

  .success {
    color: var(--color-diff-add-text);
  }

  .failure {
    color: var(--color-diff-del-text);
  }

  .pending {
    color: var(--color-lane-2);
  }

  .neutral,
  .none {
    color: var(--color-text-muted);
  }
</style>
