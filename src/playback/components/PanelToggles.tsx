// Panel toggle buttons (queue, lyrics, EQ) plus the EQ popover.
//
// The EQ popover is rendered as an absolutely-positioned child of the
// player bar so it sits above the right-hand cluster. Esc closes it.

import { LeftToRightListBulletIcon, SlidersHorizontalIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { type ReactNode, useEffect, useState } from "react";
import { MaterialSymbol } from "@/components/ui/MaterialSymbol";
import { EqPanel } from "@/components/views/EqPanel";
import { cn } from "@/lib/cn";

interface PanelTogglesProps {
  queueOpen: boolean;
  onToggleQueue: () => void;
  lyricsOpen: boolean;
  onToggleLyrics: () => void;
}

interface IconButtonProps {
  ariaLabel: string;
  children: ReactNode;
  onClick?: () => void;
  active?: boolean;
}

function IconButton({ ariaLabel, children, onClick, active }: IconButtonProps) {
  return (
    <button
      type="button"
      aria-label={ariaLabel}
      title={ariaLabel}
      onClick={onClick}
      className={cn(
        "flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-all",
        "hover:bg-muted hover:text-foreground",
        "focus:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        active && "bg-muted text-primary hover:bg-muted hover:text-primary",
      )}
    >
      {children}
    </button>
  );
}

export function PanelToggles({
  queueOpen,
  onToggleQueue,
  lyricsOpen,
  onToggleLyrics,
}: PanelTogglesProps) {
  const [eqOpen, setEqOpen] = useState(false);

  // Esc closes the EQ popover.
  useEffect(() => {
    if (!eqOpen) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setEqOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [eqOpen]);

  return (
    <>
      {eqOpen && (
        <div className="absolute bottom-full right-5 mb-2 w-[min(36rem,calc(100vw-2rem))] z-20">
          <EqPanel />
        </div>
      )}
      <IconButton
        ariaLabel="Toggle queue"
        onClick={onToggleQueue}
        aria-expanded={queueOpen}
        aria-pressed={queueOpen}
        active={queueOpen}
      >
        <HugeiconsIcon icon={LeftToRightListBulletIcon} size={16} strokeWidth={1.75} />
      </IconButton>
      <IconButton
        ariaLabel="Toggle lyrics"
        onClick={onToggleLyrics}
        aria-expanded={lyricsOpen}
        aria-pressed={lyricsOpen}
        active={lyricsOpen}
      >
        <MaterialSymbol name="lyrics" size={18} />
      </IconButton>
      <IconButton
        ariaLabel="Toggle equalizer"
        onClick={() => setEqOpen((open) => !open)}
        aria-expanded={eqOpen}
        aria-pressed={eqOpen}
        active={eqOpen}
      >
        <HugeiconsIcon icon={SlidersHorizontalIcon} size={16} strokeWidth={1.75} />
      </IconButton>
    </>
  );
}
