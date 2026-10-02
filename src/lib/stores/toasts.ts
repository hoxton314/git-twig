import { writable } from "svelte/store";

export type ToastKind = "info" | "success" | "error" | "warning";

export interface Toast {
  id: number;
  kind: ToastKind;
  title?: string;
  message: string;
  /** Optional inline action button, e.g. "Undo". */
  action?: { label: string; run: () => void | Promise<void> };
}

export interface ToastOptions {
  title?: string;
  /** Milliseconds before auto-dismiss; 0 keeps it until dismissed. */
  duration?: number;
  action?: Toast["action"];
}

export const toasts = writable<Toast[]>([]);

let nextId = 1;

/** Non-blocking notification. Errors stay longer than info by default. */
export function toast(kind: ToastKind, message: string, opts: ToastOptions = {}): number {
  const id = nextId++;
  toasts.update((list) => [...list, { id, kind, message, title: opts.title, action: opts.action }]);
  const duration = opts.duration ?? (kind === "error" ? 8000 : 4000);
  if (duration > 0) setTimeout(() => dismissToast(id), duration);
  return id;
}

export function dismissToast(id: number) {
  toasts.update((list) => list.filter((t) => t.id !== id));
}

/** Show an error toast for a failed operation (accepts thrown values). */
export function toastError(title: string, err: unknown) {
  return toast("error", err instanceof Error ? err.message : String(err), { title });
}
