// Dialog helpers — thin wrappers over `@tauri-apps/plugin-dialog`
// so we have one place to change defaults (e.g. title text) when
// the dialog UX is revamped.

import { open as openDialog } from "@tauri-apps/plugin-dialog";

/**
 * Open the native folder picker and return the absolute path the
 * user chose, or `undefined` if they cancelled.
 *
 * Centralised because the option bag (directory, multiple, title)
 * was duplicated verbatim across `SetupView`, `LoginDialog`, and
 * `ServerManager`.
 */
export async function pickLocalFolder(): Promise<string | undefined> {
  const picked = await openDialog({
    directory: true,
    multiple: false,
    title: "Select your music folder",
  });
  return picked ?? undefined;
}
