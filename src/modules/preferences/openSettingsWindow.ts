import { invoke } from "@tauri-apps/api/core";

export type SettingsTab = "general" | "playback" | "themes" | "about";

export async function openSettingsWindow(tab?: SettingsTab): Promise<void> {
  await invoke("open_settings_window");
  if (tab) {
    const win = await import("@tauri-apps/api/window").then((m) => m.getCurrentWindow());
    await win.emit("sinfonic:settings-tab", tab);
  }
}
