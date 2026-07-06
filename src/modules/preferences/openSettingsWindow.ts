import { invoke } from "@tauri-apps/api/core";
import { Window } from "@tauri-apps/api/window";

export type SettingsTab = "general" | "playback" | "themes" | "about";

/**
 * Opens the settings OS window (idempotent on the Rust side) and,
 * if a tab was requested, asks it to focus the matching section.
 *
 * The emit previously targeted `getCurrentWindow()` which always
 * returned the *main* window — the call landed on the wrong side
 * of the IPC boundary and the settings window's
 * `sinfonic:settings-tab` listener never fired. Targeting by the
 * `settings` label instead makes the routing actually work, so
 * `openSettingsWindow("playback")` from the QueuePanel Crossfade
 * button finally lands on the Playback tab.
 */
export async function openSettingsWindow(tab?: SettingsTab): Promise<void> {
  await invoke("open_settings_window");
  if (!tab) {
    return;
  }
  const settings = await Window.getByLabel("settings");
  if (!settings) {
    // The window was just created; wait one tick and retry once
    // before giving up. Opening happens through the runtime, not
    // the JS-side object, so there's a small race where `getByLabel`
    // may transiently return undefined.
    await new Promise((r) => setTimeout(r, 50));
    const retry = await Window.getByLabel("settings");
    if (!retry) {
      return;
    }
    await retry.emit("sinfonic:settings-tab", tab);
    return;
  }
  await settings.emit("sinfonic:settings-tab", tab);
}
