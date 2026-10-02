import { readable } from "svelte/store";

/**
 * Shared wall clock (ms since epoch), ticking once a minute on the minute.
 * Subscribe from anything that renders relative times ("5m ago") so they
 * stay current without each row running its own timer. The timer only runs
 * while there are subscribers.
 */
export const now = readable(Date.now(), (set) => {
  let interval: ReturnType<typeof setInterval> | null = null;
  // Align ticks to minute boundaries so all relative labels flip together.
  const timeout = setTimeout(() => {
    set(Date.now());
    interval = setInterval(() => set(Date.now()), 60_000);
  }, 60_000 - (Date.now() % 60_000));
  set(Date.now());
  return () => {
    clearTimeout(timeout);
    if (interval) clearInterval(interval);
  };
});
