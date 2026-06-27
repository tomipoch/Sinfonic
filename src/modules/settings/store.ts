import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { LazyStore } from "@tauri-apps/plugin-store";

export type ThemePref = "system" | "light" | "dark";

export type BackgroundKind = "none" | "image";

const STORE_PATH = "sinfonic-settings.json";

export type Preferences = {
  theme: ThemePref;
  themeId: string;
  backgroundKind: BackgroundKind;
  backgroundImageId: string | null;
  backgroundOpacity: number;
  backgroundBlur: number;
};

export const DEFAULTS: Preferences = {
  theme: "dark",
  themeId: "sinfonic-default",
  backgroundKind: "none",
  backgroundImageId: null,
  backgroundOpacity: 0.5,
  backgroundBlur: 0,
};

const store = new LazyStore(STORE_PATH);

let cached: Preferences | null = null;

export async function loadPreferences(): Promise<Preferences> {
  if (cached) return { ...cached };
  const result: Preferences = { ...DEFAULTS };
  for (const key of Object.keys(DEFAULTS) as (keyof Preferences)[]) {
    try {
      const val = await store.get<Preferences[typeof key]>(key);
      if (val !== undefined && val !== null) {
        (result as Record<string, unknown>)[key] = val;
      }
    } catch { /* ignore */ }
  }
  cached = result;
  return result;
}

export function onPreferencesChange(
  key: keyof Preferences,
  callback: (key: keyof Preferences, value: Preferences[keyof Preferences]) => void,
): Promise<UnlistenFn> {
  return listen<[string, unknown]>("sinfonic://prefs-changed", (e) => {
    const [k, v] = e.payload;
    if (k === key) callback(k as keyof Preferences, v as Preferences[keyof Preferences]);
  });
}

async function persist(key: keyof Preferences, value: Preferences[keyof Preferences]): Promise<void> {
  if (!cached) await loadPreferences();
  (cached as Record<string, unknown>)[key] = value;
  await store.set(key, value);
  await store.save();
  await emit("sinfonic://prefs-changed", [key, value]);
}

export const setTheme = (value: ThemePref) => persist("theme", value);
export const setThemeId = (value: string) => persist("themeId", value);
export const setBackgroundKind = (value: BackgroundKind) => persist("backgroundKind", value);
export const setBackgroundImageId = (value: string | null) => persist("backgroundImageId", value);
export const setBackgroundOpacity = (value: number) => persist("backgroundOpacity", value);
export const setBackgroundBlur = (value: number) => persist("backgroundBlur", value);
