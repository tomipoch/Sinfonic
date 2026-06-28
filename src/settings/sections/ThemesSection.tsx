import {
  CheckmarkCircle02Icon,
  ComputerIcon,
  Moon01Icon,
  Sun01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  ChoiceCard,
  SettingsCard,
  SettingsSection,
  SettingsTitle,
} from "@/components/primitives/primitives";
import { cn } from "@/lib/cn";
import { useTheme } from "@/modules/theme/ThemeProvider";
import { listBuiltinThemes } from "@/modules/theme/themes";

const MODES = [
  { id: "system", label: "System", icon: ComputerIcon },
  { id: "light", label: "Light", icon: Sun01Icon },
  { id: "dark", label: "Dark", icon: Moon01Icon },
] as const;

export function ThemesSection() {
  const { mode, themeId, setMode, setThemeId } = useTheme();
  const themes = listBuiltinThemes();

  return (
    <div className="flex flex-col gap-8">
      <SettingsTitle title="Themes" subtitle="Choose how Sinfonic looks." />

      <SettingsSection label="Appearance">
        <div className="grid grid-cols-3 gap-2">
          {MODES.map((m) => (
            <ChoiceCard
              key={m.id}
              selected={mode === m.id}
              onClick={() => setMode(m.id)}
              icon={<HugeiconsIcon icon={m.icon} size={20} strokeWidth={1.5} />}
              label={m.label}
            />
          ))}
        </div>
        <div className="text-xs text-muted-foreground">
          For background and per-theme customization, see the{" "}
          <span className="font-medium text-foreground">Themes</span> tab.
        </div>
      </SettingsSection>

      <SettingsSection label="Theme">
        <SettingsCard>
          <div className="grid grid-cols-3 gap-3 p-4 sm:grid-cols-4">
            {themes.map((theme) => {
              const isActive = themeId === theme.id;
              const variant =
                mode === "dark"
                  ? (theme.variants.dark ?? theme.variants.light)
                  : (theme.variants.light ?? theme.variants.dark);
              const colors = variant?.colors;

              return (
                <button
                  key={theme.id}
                  type="button"
                  onClick={() => setThemeId(theme.id)}
                  className={cn(
                    "relative flex flex-col items-center gap-2 rounded-lg border p-3 transition-all",
                    isActive
                      ? "border-foreground/40 bg-card"
                      : "border-border bg-card/30 hover:border-muted-foreground/40 hover:bg-card/60",
                  )}
                >
                  <div className="flex h-8 w-full gap-0.5 overflow-hidden rounded-md">
                    <div
                      className="flex-1 rounded-l-md"
                      style={{ backgroundColor: colors?.background ?? "#0b0b0e" }}
                    />
                    <div
                      className="flex-1"
                      style={{ backgroundColor: colors?.primary ?? "#10b981" }}
                    />
                    <div
                      className="flex-1"
                      style={{ backgroundColor: colors?.accent ?? "#1f1f25" }}
                    />
                    <div
                      className="flex-1 rounded-r-md"
                      style={{ backgroundColor: colors?.card ?? "#1f1f25" }}
                    />
                  </div>
                  <span className="text-center text-xs font-medium leading-tight text-foreground">
                    {theme.name}
                  </span>
                  {isActive ? (
                    <div className="absolute right-1.5 top-1.5 rounded-full bg-primary p-0.5">
                      <HugeiconsIcon
                        icon={CheckmarkCircle02Icon}
                        size={9}
                        className="text-primary-foreground"
                      />
                    </div>
                  ) : null}
                </button>
              );
            })}
          </div>
        </SettingsCard>
      </SettingsSection>
    </div>
  );
}
