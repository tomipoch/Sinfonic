// 10-band graphic EQ panel.
//
// One vertical slider per band (60 Hz … 16 kHz). The slider's
// `value` is the dB gain in `[-12.0, +12.0]`; we commit the change
// to the backend on `pointerUp` / `keyUp` so dragging a slider
// doesn't spam `set_eq_band` 60 times a second.
//
// Subscribes to `eq-changed` and `eq-reset` so external mutations
// (e.g. a future preset system) stay in sync.

import { useEffect, useState } from "react";
import { toast } from "sonner";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { extractError } from "@/lib/errors";
import { type EqBandPayload, getEqBands, resetEq, setEqBand } from "@/lib/tauri";
import { useEqStore } from "@/stores/eqStore";

const MIN_DB = -12;
const MAX_DB = 12;
const STEP_DB = 0.5;

function clamp(value: number): number {
  if (value < MIN_DB) return MIN_DB;
  if (value > MAX_DB) return MAX_DB;
  return value;
}

interface EqBandSliderProps {
  band: EqBandPayload;
  onCommit: (hz: number, gainDb: number) => void;
}

function EqBandSlider({ band, onCommit }: EqBandSliderProps) {
  const [draft, setDraft] = useState<number | null>(null);
  const displayed = draft ?? band.gainDb;
  const commit = () => {
    if (draft === null) return;
    const clamped = clamp(draft);
    setDraft(null);
    onCommit(band.hz, clamped);
  };
  return (
    <label className="flex flex-col items-center gap-2 text-xs text-muted-foreground">
      <span className="font-mono text-[10px]">{displayed.toFixed(1)} dB</span>
      <input
        type="range"
        min={MIN_DB}
        max={MAX_DB}
        step={STEP_DB}
        value={displayed}
        aria-label={`${band.hz} Hz band`}
        onChange={(e) => setDraft(Number(e.currentTarget.value))}
        onPointerUp={commit}
        onKeyUp={(e) => {
          if (e.key !== "Tab") commit();
        }}
        onBlur={commit}
        className="eq-slider h-32 w-6"
        style={{ writingMode: "vertical-rl", direction: "rtl" }}
      />
      <span className="font-mono text-[10px] text-muted">
        {band.hz >= 1000 ? `${(band.hz / 1000).toFixed(band.hz % 1000 ? 1 : 0)}k` : band.hz}
      </span>
    </label>
  );
}

export function EqPanel() {
  const bands = useEqStore((s) => s.bands);
  const setBands = useEqStore((s) => s.setBands);
  const resetBands = useEqStore((s) => s.reset);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const snapshot = await getEqBands();
        if (!cancelled) setBands(snapshot);
      } catch (err) {
        console.warn("get_eq_bands failed", err);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [setBands]);

  useTauriEvent<EqBandPayload>("eq-changed", (payload) => {
    const next = useEqStore
      .getState()
      .bands.map((b) => (b.hz === payload.hz ? { hz: payload.hz, gainDb: payload.gainDb } : b));
    setBands(next);
  });

  useTauriEvent<undefined>("eq-reset", () => {
    resetBands();
  });

  const run = async (fn: () => Promise<unknown>, label: string) => {
    if (busy) return;
    setBusy(true);
    try {
      await fn();
    } catch (err) {
      toast.error(`${label}: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  const onBandCommit = (hz: number, gainDb: number) =>
    void run(() => setEqBand(hz, gainDb), "EQ band");

  const onReset = () => void run(() => resetEq(), "EQ reset");

  if (bands.length === 0) {
    return (
      <div className="flex h-32 items-center justify-center text-xs text-muted-foreground">
        Loading EQ…
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2 rounded-md border border-border bg-muted p-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium">Equalizer</h3>
        <button
          type="button"
          onClick={onReset}
          disabled={busy}
          className="text-xs text-muted-foreground hover:text-foreground disabled:opacity-40"
        >
          Flat
        </button>
      </div>
      <div className="flex items-end gap-3 overflow-x-auto py-1">
        {bands.map((band) => (
          <EqBandSlider key={band.hz} band={band} onCommit={onBandCommit} />
        ))}
      </div>
    </div>
  );
}
