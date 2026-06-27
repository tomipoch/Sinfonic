import { ServerManager } from "@/components/settings/ServerManager";
import { SettingsTitle } from "@/components/settings/primitives";

export function GeneralSection() {
  return (
    <div className="flex flex-col gap-8">
      <SettingsTitle
        title="General"
        subtitle="Music sources and integrations."
      />
      <ServerManager />
    </div>
  );
}
