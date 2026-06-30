// PlayerBar — bottom transport bar.
//
// Three sections (Spotify / Apple Music-style):
//   left:   NowPlaying       — cover + title + artist
//   center: TransportControls + SeekBar
//   right:  VolumeControl + PanelToggles (queue/lyrics/EQ)
//
// Every section reads from `usePlaybackContext()` and renders on its
// own; the parent only composes them. The component itself owns the
// footer container, the drop-target wiring, and the
// `TransportBusyProvider` that lets TransportControls publish its
// in-flight flag to SeekBar without prop-drilling.

import { useEffect, useRef } from "react";
import { toast } from "sonner";

import { useDropTarget } from "@/hooks/useDropTarget";
import { cn } from "@/lib/cn";
import { extractError } from "@/lib/errors";
import { queueAddMany } from "@/lib/tauri";
import { useQueueStore } from "@/stores/queueStore";

import { NowPlaying } from "./NowPlaying";
import { PanelToggles } from "./PanelToggles";
import { SeekBar } from "./SeekBar";
import { TransportBusyProvider } from "./TransportBusyContext";
import { TransportControls } from "./TransportControls";
import { VolumeControl } from "./VolumeControl";

interface Props {
  queueOpen: boolean;
  onToggleQueue: () => void;
  lyricsOpen: boolean;
  onToggleLyrics: () => void;
}

export function PlayerBar({ queueOpen, onToggleQueue, lyricsOpen, onToggleLyrics }: Props) {
  const queueLength = useQueueStore((s) => s.entries.length);
  const canStep = queueLength > 0;

  // Drop-target: drag tracks into the bar to add to queue.
  const { dragOver, droppableProps } = useDropTarget({
    onDrop: async (dropped) => {
      if (dropped.length === 0) return;
      try {
        await queueAddMany(dropped);
        toast.success(`Added ${dropped.length} track${dropped.length !== 1 ? "s" : ""} to queue`);
      } catch (err) {
        toast.error(`Couldn't add to queue: ${extractError(err, "unknown error")}`);
      }
    },
  });

  // Layout probe — runs once on mount and on every window resize
  // so a developer can resize the window and read the actual
  // pixel widths + any overflows from the browser console.
  // Tag the grid children with data-pb-col="nowplaying" /
  // "transport" / "right" to make them easy to address.
  const footerRef = useRef<HTMLElement | null>(null);
  const { ref: dropRef, ...restDroppable } = droppableProps as unknown as {
    ref?: (node: HTMLElement | null) => void;
  };
  const mergeRefs = (el: HTMLElement | null) => {
    footerRef.current = el;
    dropRef?.(el);
  };
  useEffect(() => {
    const footer = footerRef.current;
    if (!footer) return;

    const probe = () => {
      const cols = Array.from(footer.querySelectorAll<HTMLElement>("[data-pb-col]"));
      const summary = cols.map((el) => {
        const overflow = el.scrollWidth - el.clientWidth;
        return {
          col: el.dataset.pbCol ?? "?",
          width: Math.round(el.getBoundingClientRect().width),
          content: el.scrollWidth,
          clientWidth: el.clientWidth,
          overflow,
        };
      });
      console.log("[PlayerBar probe]", {
        viewport: `${window.innerWidth}×${window.innerHeight}`,
        footer: Math.round(footer.getBoundingClientRect().width),
        columns: summary,
      });
    };

    probe();
    window.addEventListener("resize", probe);
    return () => window.removeEventListener("resize", probe);
  }, []);

  return (
    <footer
      ref={mergeRefs}
      {...restDroppable}
      className={cn(
        "relative grid h-14 shrink-0 grid-cols-[1fr_minmax(16rem,24rem)_1fr] items-center gap-1 border-t border-border bg-card px-2 transition-colors sm:h-[5rem] sm:gap-2 sm:px-3 md:h-[5.5rem] md:px-4",
        dragOver && "bg-primary/10 ring-1 ring-inset ring-primary/40",
      )}
      role="contentinfo"
      aria-label="Player controls"
    >
      <div className="min-w-0" data-pb-col="nowplaying">
        <NowPlaying />
      </div>

      <TransportBusyProvider>
        <div className="flex w-full min-w-0 flex-col items-center gap-1.5" data-pb-col="transport">
          <TransportControls canStep={canStep} />
          <SeekBar enabled={true} />
        </div>
      </TransportBusyProvider>

      <div className="flex min-w-0 items-center justify-end gap-1" data-pb-col="right">
        <VolumeControl />
        <div className="mx-1 h-5 w-px shrink-0 bg-border" aria-hidden />
        <PanelToggles
          queueOpen={queueOpen}
          onToggleQueue={onToggleQueue}
          lyricsOpen={lyricsOpen}
          onToggleLyrics={onToggleLyrics}
        />
      </div>
    </footer>
  );
}
