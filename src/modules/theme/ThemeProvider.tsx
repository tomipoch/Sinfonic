import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { DEFAULT_THEME_ID, listBuiltinThemes, getBuiltinTheme, getDefaultTheme } from "@/modules/theme/themes";
import { applyTheme, clearTheme } from "@/modules/theme/applyTheme";
import { setTheme as persistTheme, setThemeId as persistThemeId, onPreferencesChange } from "@/modules/settings/store";
import type { Theme } from "@/modules/theme/types";

export type { Theme };
export type ThemeModePref = "system" | "light" | "dark";

type ThemeProviderProps = {
  children: ReactNode;
  defaultMode?: ThemeModePref;
};

type ThemeProviderState = {
  mode: ThemeModePref;
  resolvedMode: "dark" | "light";
  themeId: string;
  customThemes: Theme[];
  setMode: (mode: ThemeModePref) => void;
  setThemeId: (id: string) => void;
  previewThemeId: (id: string | null) => void;
  listBuiltinThemes: () => Theme[];
};

const ThemeProviderContext = createContext<ThemeProviderState | null>(null);

const FAST_PATH_KEY = "sinfonic-ui-theme-shadow";
const FAST_PATH_THEME_ID = "sinfonic-ui-theme-id-shadow";

function readFastMode(fallback: ThemeModePref): ThemeModePref {
  if (typeof window === "undefined") return fallback;
  const v = window.localStorage.getItem(FAST_PATH_KEY);
  return v === "dark" || v === "light" || v === "system" ? v : fallback;
}

function writeFastMode(t: ThemeModePref): void {
  try { window.localStorage.setItem(FAST_PATH_KEY, t); } catch { /* ignore */ }
}

function readFastThemeId(): string {
  if (typeof window === "undefined") return DEFAULT_THEME_ID;
  return window.localStorage.getItem(FAST_PATH_THEME_ID) ?? DEFAULT_THEME_ID;
}

function writeFastThemeId(id: string): void {
  try { window.localStorage.setItem(FAST_PATH_THEME_ID, id); } catch { /* ignore */ }
}

function resolveTheme(id: string, custom: Theme[]): Theme {
  return custom.find((t) => t.id === id) ?? getBuiltinTheme(id) ?? getDefaultTheme();
}

export function ThemeProvider({ children, defaultMode = "system" }: ThemeProviderProps) {
  const [mode, setModeState] = useState<ThemeModePref>(() => readFastMode(defaultMode));
  const [themeId, setThemeIdState] = useState<string>(() => readFastThemeId());
  const [previewId, setPreviewId] = useState<string | null>(null);
  const [customThemes] = useState<Theme[]>([]);
  const [systemDark, setSystemDark] = useState<boolean>(() =>
    typeof window === "undefined"
      ? true
      : window.matchMedia("(prefers-color-scheme: dark)").matches,
  );

  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  const resolvedMode: "dark" | "light" =
    mode === "system" ? (systemDark ? "dark" : "light") : mode;

  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("light", "dark");
    root.classList.add(resolvedMode);
  }, [resolvedMode]);

  const effectiveId = previewId ?? themeId;
  useEffect(() => {
    if (effectiveId === DEFAULT_THEME_ID) {
      clearTheme();
      return;
    }
    applyTheme(resolveTheme(effectiveId, customThemes), resolvedMode);
  }, [effectiveId, resolvedMode, customThemes]);

  const setMode = useCallback((next: ThemeModePref) => {
    setModeState(next);
    writeFastMode(next);
    void persistTheme(next);
  }, []);

  const setThemeId = useCallback((id: string) => {
    setPreviewId(null);
    setThemeIdState(id);
    writeFastThemeId(id);
    void persistThemeId(id);
  }, []);

  useEffect(() => {
    let active = true;
    const setup = async () => {
      const un1 = await onPreferencesChange("theme", (_k, v) => {
        if (active) setModeState(v as ThemeModePref);
      });
      const un2 = await onPreferencesChange("themeId", (_k, v) => {
        if (active) setThemeIdState(v as string);
      });
      return () => { un1(); un2(); };
    };
    const cleanup = setup();
    return () => { active = false; void cleanup.then((fn) => fn()); };
  }, []);

  const previewThemeId = useCallback((id: string | null) => {
    setPreviewId(id);
  }, []);

  const value = useMemo<ThemeProviderState>(
    () => ({
      mode,
      resolvedMode,
      themeId,
      customThemes,
      setMode,
      setThemeId,
      previewThemeId,
      listBuiltinThemes,
    }),
    [mode, resolvedMode, themeId, customThemes, setMode, setThemeId, previewThemeId],
  );

  return (
    <ThemeProviderContext.Provider value={value}>
      {children}
    </ThemeProviderContext.Provider>
  );
}

export function useTheme(): ThemeProviderState {
  const ctx = useContext(ThemeProviderContext);
  if (!ctx) throw new Error("useTheme must be used within a <ThemeProvider>");
  return ctx;
}
