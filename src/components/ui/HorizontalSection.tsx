// HorizontalSection — horizontally scrolleable section with navigation arrows.
//
// Layout:
//   [←] Title                [→]
//   [card][card][card][card] →

import { ArrowLeft01Icon, ArrowRight01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { type ReactNode, useRef } from "react";
import { cn } from "@/lib/cn";

type Props = {
  title: string;
  children: ReactNode;
  className?: string;
};

export function HorizontalSection({ title, children, className }: Props) {
  const scrollRef = useRef<HTMLDivElement>(null);

  const scroll = (direction: "left" | "right") => {
    const el = scrollRef.current;
    if (!el) return;
    const amount = el.offsetWidth * 0.75;
    el.scrollBy({ left: direction === "left" ? -amount : amount, behavior: "smooth" });
  };

  return (
    <section className={cn("flex flex-col gap-3", className)}>
      <div className="flex items-center justify-between pr-6">
        <h2 className="text-base font-semibold text-foreground">{title}</h2>
        <div className="flex gap-1">
          <button
            type="button"
            onClick={() => scroll("left")}
            aria-label={`Scroll ${title} left`}
            className="size-7 rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          >
            <HugeiconsIcon icon={ArrowLeft01Icon} size={15} strokeWidth={1.75} />
          </button>
          <button
            type="button"
            onClick={() => scroll("right")}
            aria-label={`Scroll ${title} right`}
            className="size-7 rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          >
            <HugeiconsIcon icon={ArrowRight01Icon} size={15} strokeWidth={1.75} />
          </button>
        </div>
      </div>
      <div
        ref={scrollRef}
        className="flex gap-3 overflow-x-auto pb-1 [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
        style={{ scrollSnapType: "x mandatory" }}
      >
        {children}
      </div>
    </section>
  );
}
