<script lang="ts">
  import type { GraphEntry, RefLabel, GraphDateFormat } from "../../lib/types/git";
  import { ArrowUp, ShieldCheck, ShieldAlert, ShieldX } from "lucide-svelte";
  import { formatCommitDate } from "./graphLayout";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { signatureFor } from "../../lib/stores/signatures";
  import { t } from "../../lib/i18n";

  interface Props {
    entry: GraphEntry;
    isSelected: boolean;
    isUnpushed: boolean;
    refs: RefLabel[];
    onSelect: (e: MouseEvent) => void;
    /** Column visibility (widths come from CSS vars set by CommitGraph). */
    showAuthor?: boolean;
    showSha?: boolean;
    showDate?: boolean;
    dateFormat?: GraphDateFormat;
    /** Shared clock (ms) so relative dates refresh. */
    now?: number;
    avatarSize?: number;
    /** Search highlight state. */
    searchMatch?: "none" | "match" | "current";
    oncontextmenu?: (e: MouseEvent) => void;
    /** Bisect mark ("first-bad" once found, "testing" for the commit under test). */
    bisect?: "good" | "bad" | "skip" | "first-bad" | "testing" | null;
    /** Display names for bisect's good / bad terms. */
    bisectTerms?: { good: string; bad: string };
  }

  let {
    entry,
    isSelected,
    isUnpushed,
    refs,
    onSelect,
    showAuthor = true,
    showSha = true,
    showDate = true,
    dateFormat = "relative",
    now = Date.now(),
    avatarSize = 20,
    searchMatch = "none",
    oncontextmenu,
    bisect = null,
    bisectTerms = { good: "good", bad: "bad" },
  }: Props = $props();

  const bisectLabel = $derived(
    bisect === "good" ? bisectTerms.good
    : bisect === "bad" ? bisectTerms.bad
    : bisect === "first-bad" ? $t("graph.bisectFirst", { term: bisectTerms.bad })
    : bisect === "testing" ? $t("graph.bisectTesting")
    : bisect === "skip" ? $t("graph.bisectSkipped")
    : "",
  );

  const commit = $derived(entry.commit);
  const gravatarUrl = $derived(
    `https://www.gravatar.com/avatar/${commit.author_gravatar}?s=${avatarSize * 2}&d=identicon`
  );

  // Sort refs: local first, then remote, then tags
  const sortedRefs = $derived(
    [...refs].sort((a, b) => {
      const order = { local: 0, tag: 1, remote: 2 };
      return (order[a.ref_type] ?? 3) - (order[b.ref_type] ?? 3);
    })
  );

  // Reading $t re-formats relative dates when the language changes.
  const dateLabel = $derived.by(() => {
    void $t;
    return formatCommitDate(commit.timestamp, dateFormat, now);
  });

  // Signed commits get a badge; unsigned ones show nothing.
  const signature = $derived(signatureFor($activeRepoPath, commit.oid));
  const sigTone = $derived(
    $signature?.status === "good"
      ? "good"
      : $signature?.status === "bad" || $signature?.status === "revoked"
        ? "bad"
        : "warn",
  );
  const SIG_KEYS = {
    good: "graph.sigGood",
    untrusted: "graph.sigUntrusted",
    expired: "graph.sigExpired",
    expired_key: "graph.sigExpiredKey",
    revoked: "graph.sigRevoked",
    unknown_key: "graph.sigUnknownKey",
    bad: "graph.sigBad",
  } as const;
  const sigTitle = $derived(
    $signature
      ? [
          $t(SIG_KEYS[$signature.status as keyof typeof SIG_KEYS] ?? "graph.sigSigned"),
          $signature.signer,
          $signature.key && $t("graph.sigKey", { key: $signature.key }),
        ]
          .filter(Boolean)
          .join(" — ")
      : "",
  );

  // Shift-click extends the selection; don't let it select text too.
  function preventShiftSelect(e: MouseEvent) {
    if (e.shiftKey) e.preventDefault();
  }
</script>

<button
  class="commit-row"
  class:selected={isSelected}
  class:search-match={searchMatch !== "none"}
  class:search-current={searchMatch === "current"}
  aria-pressed={isSelected}
  tabindex="-1"
  onclick={onSelect}
  onmousedown={preventShiftSelect}
  {oncontextmenu}
>
  <img
    class="avatar"
    src={gravatarUrl}
    alt=""
    title={`${commit.author_name} <${commit.author_email}>`}
    width={avatarSize}
    height={avatarSize}
    loading="lazy"
  />
  {#if sortedRefs.length > 0}
    <span class="ref-labels">
      {#each sortedRefs as ref (ref.ref_type + ":" + ref.name)}
        <span class="ref-pill ref-{ref.ref_type}" title={ref.name}>{ref.name}</span>
      {/each}
    </span>
  {/if}
  {#if bisect}
    <span class="bisect-pill bisect-{bisect}" title={$t("graph.bisectTitle", { label: bisectLabel })}>{bisectLabel}</span>
  {/if}
  <span class="summary" title={commit.summary}>{commit.summary}</span>
  <span class="spacer"></span>
  {#if $signature}
    <span class="signature sig-{sigTone}" title={sigTitle} aria-label={sigTitle}>
      {#if sigTone === "good"}<ShieldCheck size={12} />{:else if sigTone === "bad"}<ShieldX size={12} />{:else}<ShieldAlert size={12} />{/if}
    </span>
  {/if}
  {#if isUnpushed}
    <span class="unpushed" title={$t("graph.notPushed")}>
      <ArrowUp size={11} />
    </span>
  {/if}
  {#if showAuthor}
    <span class="author" title={`${commit.author_name} <${commit.author_email}>`}>{commit.author_name}</span>
  {/if}
  {#if showSha}
    <span class="oid">{commit.short_oid}</span>
  {/if}
  {#if showDate}
    <span class="time" title={new Date(commit.timestamp * 1000).toLocaleString()}>{dateLabel}</span>
  {/if}
</button>

<style>
  .commit-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    padding: 0 12px 0 4px;
    height: 100%;
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
    text-align: left;
    overflow: hidden;
    transition: background 0.1s;
  }

  .commit-row:hover {
    background: var(--color-surface);
  }

  .commit-row.selected {
    background: var(--color-surface-elevated);
    box-shadow: inset 2px 0 0 var(--color-accent); /* no layout shift on select */
  }

  .commit-row.search-match {
    background: color-mix(in srgb, var(--color-accent) 10%, transparent);
  }

  .commit-row.search-match:hover {
    background: color-mix(in srgb, var(--color-accent) 16%, transparent);
  }

  .commit-row.search-current {
    background: color-mix(in srgb, var(--color-accent) 22%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-accent) 60%, transparent);
  }

  /* Keep the selection stripe when the selected row is also the current match. */
  .commit-row.selected.search-current {
    box-shadow:
      inset 2px 0 0 var(--color-accent),
      inset 0 0 0 1px color-mix(in srgb, var(--color-accent) 60%, transparent);
  }

  .avatar {
    border-radius: 50%;
    flex-shrink: 0;
  }

  .ref-labels {
    display: flex;
    gap: 4px;
    /* Shrinks (clipping pills) before the fixed columns get pushed out. */
    flex-shrink: 1;
    min-width: 0;
    max-width: 300px;
    overflow: hidden;
  }

  .ref-pill {
    display: inline-flex;
    align-items: center;
    padding: 1px 6px;
    border-radius: 3px;
    font-size: 10px;
    font-weight: 600;
    white-space: nowrap;
    line-height: 16px;
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ref-local {
    background: color-mix(in srgb, var(--color-lane-0) 20%, transparent);
    color: var(--color-lane-0);
  }

  .ref-remote {
    background: color-mix(in srgb, var(--color-text-muted) 20%, transparent);
    color: var(--color-text-muted);
  }

  .ref-tag {
    background: color-mix(in srgb, var(--color-lane-2) 20%, transparent);
    color: var(--color-lane-2);
  }

  .signature {
    display: inline-flex;
    flex-shrink: 0;
    margin-left: 4px;
  }
  .bisect-pill {
    flex-shrink: 0;
    margin-right: 6px;
    padding: 0 5px;
    border-radius: 3px;
    border: 1px solid currentColor;
    font-size: 10px;
    line-height: 15px;
    text-transform: lowercase;
  }
  .bisect-good { color: var(--color-diff-add-text); }
  .bisect-bad, .bisect-first-bad { color: var(--color-diff-del-text); }
  .bisect-first-bad { background: var(--color-diff-del-bg); font-weight: 600; }
  .bisect-skip { color: var(--color-text-muted); }
  .bisect-testing { color: var(--color-accent); }
  .sig-good { color: var(--color-diff-add-text); }
  .sig-warn { color: var(--color-lane-2); }
  .sig-bad { color: var(--color-diff-del-text); }

  .summary {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  /* The summary takes all free space; the fixed columns align right. */
  .spacer {
    flex: 0 0 0;
  }

  .unpushed {
    display: flex;
    align-items: center;
    color: var(--color-lane-2);
    flex-shrink: 0;
    opacity: 0.8;
  }

  /* Fixed-width columns; widths are CSS vars set by CommitGraph so the
     header and every row line up and resize together. */
  .author,
  .oid,
  .time {
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 0;
  }

  .author {
    color: var(--color-text-muted);
    width: var(--graph-col-author, 120px);
  }

  .oid {
    font-family: var(--font-mono);
    color: var(--color-accent);
    width: var(--graph-col-sha, 64px);
  }

  .time {
    color: var(--color-text-muted);
    text-align: right;
    width: var(--graph-col-date, 90px);
  }
</style>
