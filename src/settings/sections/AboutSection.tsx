import { SettingsCard, SettingsSection, SettingsTitle } from "@/components/settings/primitives";

export function AboutSection() {
  return (
    <div className="flex flex-col gap-8">
      <SettingsTitle
        title="About"
        subtitle="Cross-platform desktop music client."
      />

      <SettingsSection label="Sinfonic">
        <SettingsCard>
          <div className="flex flex-col gap-1 px-4 py-4">
            <div className="text-base font-medium text-foreground">Sinfonic</div>
            <div className="text-xs text-muted-foreground">Version 0.1.0</div>
            <div className="mt-2 text-xs text-muted-foreground">
              Built with Tauri v2, React 19, TypeScript, and Tailwind CSS.
            </div>
          </div>
        </SettingsCard>
      </SettingsSection>
    </div>
  );
}
