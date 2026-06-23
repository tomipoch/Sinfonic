// SettingsApp — top-level shell for the standalone Settings window.
//
// Layout:
//   ┌─ Header (native traffic lights + centered tabs) ────────┐
//   ├─ Content ──────────────────────────────────────────────┤
//   │  SettingsTitle                                          │
//   │  SettingsSection(s) → Cards                             │
//   └────────────────────────────────────────────────────────┘

import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { SettingsTab } from "@/modules/settings/openSettingsWindow";
import { usePreferencesStore } from "@/modules/settings/preferences";
import { InformationCircleIcon, PaintBoardIcon, Settings01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { type JSX, useEffect, useState } from "react";
import { AboutSection } from "./sections/AboutSection";
import { GeneralSection } from "./sections/GeneralSection";
import { ThemesSection } from "./sections/ThemesSection";

const TABS: { id: SettingsTab; label: string; icon: typeof Settings01Icon; component: () => JSX.Element }[] = [
  { id: "general", label: "General", icon: Settings01Icon, component: GeneralSection },
  { id: "themes", label: "Themes", icon: PaintBoardIcon, component: ThemesSection },
  { id: "about", label: "About", icon: InformationCircleIcon, component: AboutSection },
];

const VALID_TABS: SettingsTab[] = ["general", "themes", "about"];

function readInitialTab(): SettingsTab {
  if (typeof window === "undefined") return "general";
  const url = new URL(window.location.href);
  const t = url.searchParams.get("tab");
  if (t && (VALID_TABS as string[]).includes(t)) return t as SettingsTab;
  return "general";
}

export function SettingsApp() {
  const [active, setActive] = useState<SettingsTab>(readInitialTab);
  const init = usePreferencesStore((s) => s.init);
  const ActiveSection = TABS.find((t) => t.id === active)?.component;

  useEffect(() => {
    void init();
  }, [init]);

  useEffect(() => {
    const apply = (detail: string) => {
      if ((VALID_TABS as string[]).includes(detail)) {
        setActive(detail as SettingsTab);
      }
    };
    const unlistenPromise = getCurrentWindow().listen<string>(
      "sinfonic:settings-tab",
      (e) => apply(e.payload),
    );
    return () => {
      void unlistenPromise.then((un) => un());
    };
  }, []);

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-background text-foreground select-none">
      <header
        data-tauri-drag-region
        className="flex h-11 shrink-0 items-center justify-center border-b border-border bg-background px-4"
      >
        <Tabs
          value={active}
          onValueChange={(v: string) => setActive(v as SettingsTab)}
          orientation="horizontal"
        >
          <TabsList
            className="h-7 gap-0.5 bg-muted/40 px-1"
          >
            {TABS.map((t) => (
              <TabsTrigger
                key={t.id}
                value={t.id}
                className="h-6 gap-1.5 px-2.5 text-[11.5px] data-[state=active]:bg-card data-[state=active]:text-foreground"
              >
                <HugeiconsIcon icon={t.icon} size={12} strokeWidth={1.75} />
                <span>{t.label}</span>
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
      </header>

      <main
        data-tauri-drag-region
        className="min-h-0 flex-1 overflow-y-auto px-10 pt-8 pb-10 [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
      >
        <div className="mx-auto w-full max-w-2xl">
          {ActiveSection && <ActiveSection />}
        </div>
      </main>
    </div>
  );
}
