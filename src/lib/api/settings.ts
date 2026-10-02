/** App settings and global git config. */
import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  CommandResult,
  GitConfig,
  SigningKey,
} from "../types/git";

// ── Settings ─────────────────────────────────────────────────────────

export function loadSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("load_settings");
}

export function saveSettings(settings: AppSettings): Promise<void> {
  return invoke<void>("save_settings", { settings });
}

// ── Git Config ───────────────────────────────────────────────────────

export function getGitConfig(): Promise<GitConfig> {
  return invoke<GitConfig>("get_git_config");
}

export function setGitConfig(config: GitConfig): Promise<void> {
  return invoke<void>("set_git_config", { config });
}

/** GPG secret keys or SSH public keys usable for signing. */
export function listSigningKeys(format: "openpgp" | "ssh"): Promise<SigningKey[]> {
  return invoke<SigningKey[]>("list_signing_keys", { format });
}

/** Sign a throwaway commit with the saved signing settings. */
export function testSigning(): Promise<CommandResult> {
  return invoke<CommandResult>("test_signing");
}
