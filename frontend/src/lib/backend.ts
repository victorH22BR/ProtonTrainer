import { invoke } from "@tauri-apps/api/core";

/**
 * Typed wrappers around Tauri commands. Every backend call goes through this
 * module so the command names and payload shapes live in one place.
 */

export interface AppInfo {
  name: string;
  version: string;
}

/** True when running inside the Tauri webview (false in a plain browser). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}
