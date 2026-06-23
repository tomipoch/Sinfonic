import { platform } from "@tauri-apps/plugin-os";

const PLATFORM = (() => {
  try {
    return platform();
  } catch {
    return "";
  }
})();

export const IS_MAC = PLATFORM === "macos";
export const IS_LINUX = PLATFORM === "linux";
export const IS_WINDOWS = PLATFORM === "windows";

export const USE_CUSTOM_WINDOW_CONTROLS = PLATFORM !== "";

export const MOD_KEY = IS_MAC ? "⌘" : "Ctrl";
export const MOD_PROP: "meta" | "ctrl" = IS_MAC ? "meta" : "ctrl";
export const CTRL_KEY = IS_MAC ? "⌃" : "Ctrl";
export const ALT_KEY = IS_MAC ? "⌥" : "Alt";
export const SHIFT_KEY = IS_MAC ? "⇧" : "Shift";
export const TAB_KEY = IS_MAC ? "⇥" : "Tab";
export const ENTER_KEY = IS_MAC ? "↵" : "Enter";

export const KEY_SEP = IS_MAC ? "" : "+";

export function fmtShortcut(...parts: string[]): string {
  return parts.join(KEY_SEP);
}
