// PlaybackSection — crossfade configuration.
//
// The toggle + slider drive `setCrossfade` IPC, which persists the
// values via `library.set_preference`. On mount the section
// hydrates from `get_crossfade_config` (or falls back to defaults
// if the backend is unreachable in tests). The
// `playback-config-changed` listener keeps the slider in sync if
// the value is updated from another window.

import { useEffect, useState } from "react";
import { toast } from "sonner";
import {
  SettingsCard,
  SettingsSection,
  SettingsTitle,
  SliderCard,
  ToggleCard,
} from "@/components/primitives/primitives";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { extractError } from "@/lib/errors";
import { getCrossfadeConfig, setCrossfade } from "@/lib/tauri";
import {
  CROSSFADE_SECONDS_DEFAULT,
  CROSSFADE_SECONDS_MAX,
  CROSSFADE_SECONDS_MIN,
  usePlaybackConfigStore,
} from "@/stores/playbackConfigStore";

function formatSeconds(seconds: number): string {
  if (seconds === 0) return "Off";
  return `${seconds.toFixed(seconds % 1 === 0 ? 0 : 1)} s`;
}

export function PlaybackSection() {
  const crossfadeEnabled = usePlaybackConfigStore((s) => s.crossfadeEnabled);
  const crossfadeSeconds = usePlaybackConfigStore((s) => s.crossfadeSeconds);
  const hydrate = usePlaybackConfigStore((s) => s.hydrate);
  const [busy, setBusy] = useState(false);

  // Hydrate from the backend on mount so the slider reflects the
  // persisted value (which was restored on launch). Best-effort:
  // any failure keeps the in-store defaults.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const cfg = await getCrossfadeConfig();
        if (!cancelled) hydrate(cfg);
      } catch {
        // Backend may be unreachable in tests; the defaults (off,
        // 6 s) are good enough.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [hydrate]);

  // Keep the slider in sync if the value changes from another
  // window (or a future keyboard shortcut, etc).
  useTauriEvent<{ crossfadeEnabled: boolean; crossfadeSeconds: number }>(
    "playback-config-changed",
    (payload) => {
      hydrate(payload);
    },
  );

  const applyConfig = async (enabled: boolean, seconds: number) => {
    if (busy) return;
    setBusy(true);
    try {
      await setCrossfade(enabled, seconds);
    } catch (err) {
      toast.error(`Crossfade: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  const onToggleChange = (next: boolean) => {
    void applyConfig(next, crossfadeSeconds);
  };

  const onSliderChange = (next: number) => {
    const clamped = Math.max(CROSSFADE_SECONDS_MIN, Math.min(CROSSFADE_SECONDS_MAX, next));
    void applyConfig(crossfadeEnabled || clamped > 0, clamped);
  };

  return (
    <div className="flex flex-col gap-8">
      <SettingsTitle title="Playback" subtitle="Configure how tracks transition into each other." />

      <SettingsSection label="Crossfade">
        <ToggleCard
          title="Crossfade between tracks"
          description="Overlap the outgoing and incoming track for a smoother transition when a song ends or you skip forward."
          checked={crossfadeEnabled}
          onChange={onToggleChange}
          disabled={busy}
        />
        <SliderCard
          label="Crossfade duration"
          value={crossfadeSeconds}
          displayValue={formatSeconds(crossfadeSeconds)}
          min={CROSSFADE_SECONDS_MIN}
          max={CROSSFADE_SECONDS_MAX}
          step={0.5}
          onChange={onSliderChange}
          disabled={busy || !crossfadeEnabled}
        />
        <SettingsCard>
          <div className="px-4 py-3 text-xs text-muted-foreground">
            When the toggle is off, tracks cut instantly. Default duration:{" "}
            {CROSSFADE_SECONDS_DEFAULT} s. Maximum: {CROSSFADE_SECONDS_MAX} s.
          </div>
        </SettingsCard>
      </SettingsSection>
    </div>
  );
}
