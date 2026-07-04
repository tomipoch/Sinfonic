import { useLayoutEffect, useRef, useState } from "react";

import { cn } from "@/lib/cn";

export interface MarqueeTextProps {
  /** Text content. May include inline elements (links, icons). */
  children: React.ReactNode;

  /** Tailwind classes applied to the outer (clipped) wrapper. */
  className?: string;

  /** Tailwind classes applied to the inner scrolling span. */
  innerClassName?: string;

  /**
   * Duration of one scroll cycle in seconds. Defaults to 12s which
   * reads as a comfortable read speed for medium-length strings.
   */
  durationSeconds?: number;

  /**
   * Pause between scrolls. Defaults to 1s. The track will sit at
   * its final position for this long after the animation completes.
   * Implemented by extending the keyframes with a hold phase — done
   * here by leaving the animation paused at 100% via fill-mode.
   */
  pauseSeconds?: number;
}

/**
 * MarqueeText — horizontally scrolls long text on hover.
 *
 * The outer `.marquee` div clips overflow and the inner span uses
 * CSS keyframes (`marquee-scroll` defined in `index.css`) to
 * translate from `0` to `-shift` when hovered. `shift` is computed
 * once on mount/resize as `scrollWidth - clientWidth`, so it works
 * for arbitrary text lengths without hardcoding widths.
 *
 * If the content fits (no overflow), the component renders plain
 * text — no marquee animation, no JS overhead.
 */
export function MarqueeText({
  children,
  className,
  innerClassName,
  durationSeconds = 12,
  pauseSeconds = 1,
}: MarqueeTextProps) {
  const containerRef = useRef<HTMLSpanElement>(null);
  const trackRef = useRef<HTMLSpanElement>(null);
  const [shift, setShift] = useState(0);

  useLayoutEffect(() => {
    const container = containerRef.current;
    const track = trackRef.current;
    if (!container || !track) return;

    const measure = () => {
      const overflow = track.scrollWidth - container.clientWidth;
      setShift(overflow > 0 ? overflow : 0);
    };

    measure();

    // Re-measure on resize so the animation distance stays correct
    // when the layout changes (e.g., window resize, sidebar toggle).
    const ro = new ResizeObserver(measure);
    ro.observe(container);
    ro.observe(track);

    return () => ro.disconnect();
  }, [children]);

  const animationStyle: React.CSSProperties =
    shift > 0
      ? ({
          "--marquee-shift": `${shift}px`,
          "--marquee-duration": `${durationSeconds + pauseSeconds}s`,
          animationDelay: "0s",
        } as React.CSSProperties)
      : {};

  const innerStyle: React.CSSProperties =
    shift > 0
      ? ({
          paddingRight: "32px",
        } as React.CSSProperties)
      : {};

  return (
    <span
      ref={containerRef}
      className={cn("marquee", className)}
      tabIndex={shift > 0 ? 0 : undefined}
    >
      <span
        ref={trackRef}
        className={cn("marquee__track", innerClassName)}
        style={{ ...innerStyle, ...animationStyle }}
      >
        {children}
      </span>
    </span>
  );
}
