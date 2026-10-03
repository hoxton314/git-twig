/** Cross-window sync: after saving shared state, tell the other windows. */
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type SyncKind = "settings" | "repo-settings" | "repo-history";

interface SyncMessage {
  kind: SyncKind;
  from: string;
  payload: unknown;
}

const EVENT = "twig://sync";

function selfLabel(): string {
  try {
    return getCurrentWindow().label;
  } catch {
    return "main";
  }
}

/** Announce a saved value to the other windows. */
export function emitSync(kind: SyncKind, payload: unknown): void {
  emit(EVENT, { kind, from: selfLabel(), payload } satisfies SyncMessage).catch(() => {});
}

/** Receive values the other windows saved. */
export function onSync(handler: (kind: SyncKind, payload: unknown) => void): Promise<UnlistenFn> {
  const me = selfLabel();
  return listen<SyncMessage>(EVENT, (e) => {
    if (e.payload.from !== me) handler(e.payload.kind, e.payload.payload);
  });
}
