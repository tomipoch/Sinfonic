import { create } from "zustand";
import {
  DEFAULTS,
  loadPreferences,
  onPreferencesChange,
  type Preferences,
} from "./store";

type State = Preferences & {
  hydrated: boolean;
  init: () => Promise<void>;
};

let initialized = false;

export const usePreferencesStore = create<State>((set) => ({
  ...DEFAULTS,
  hydrated: false,
  init: async () => {
    if (initialized) return;
    initialized = true;
    const prefs = await loadPreferences();
    set({ ...prefs, hydrated: true });
    void onPreferencesChange("theme", (key, value) => {
      set({ [key]: value } as Partial<State>);
    });
    void onPreferencesChange("themeId", (key, value) => {
      set({ [key]: value } as Partial<State>);
    });
  },
}));
