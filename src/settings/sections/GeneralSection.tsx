import { SettingsTitle } from "@/components/primitives/primitives";
import { ServerManager } from "@/components/primitives/ServerManager";

export function GeneralSection() {
  return (
    <div className="flex flex-col gap-8">
      <SettingsTitle title="General" subtitle="Music sources and integrations." />
      <ServerManager />
    </div>
  );
}
