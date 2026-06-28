// Pagination — numeric pager with prev/next arrows + ellipsis.
//
// Usage:
//   <Pagination page={page} totalPages={totalPages} onChange={setPage} />
//
// `page` is 0-indexed; the visible labels are 1-indexed. Renders
// nothing if there is at most one page. The visible sequence is
// `[1, …, N-1, N, N+1, …, last]` collapsed around the current page
// (always shows first / last / current ±1). Style matches the rest
// of the app: `bg-muted text-foreground` for the active pill and the
// HorizontalSection arrow-button sizing.

import { ArrowLeft01Icon, ArrowRight01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { cn } from "@/lib/cn";

interface Props {
  page: number;
  totalPages: number;
  onChange: (page: number) => void;
  className?: string;
}

function pageSequence(page: number, total: number): (number | "…")[] {
  if (total <= 7) {
    return Array.from({ length: total }, (_, i) => i);
  }
  const candidates = new Set<number>([0, total - 1, page - 1, page, page + 1]);
  const sorted = [...candidates].filter((p) => p >= 0 && p < total).sort((a, b) => a - b);
  const out: (number | "…")[] = [];
  for (let i = 0; i < sorted.length; i += 1) {
    const cur = sorted[i];
    if (cur === undefined) continue;
    const prev = i > 0 ? sorted[i - 1] : undefined;
    if (prev !== undefined && cur - prev > 1) out.push("…");
    out.push(cur);
  }
  return out;
}

export function Pagination({ page, totalPages, onChange, className }: Props) {
  if (totalPages <= 1) return null;

  const go = (next: number) => {
    if (next < 0 || next >= totalPages) return;
    onChange(next);
  };

  const seq = pageSequence(page, totalPages);

  const arrowBtn =
    "inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors " +
    "hover:bg-muted hover:text-foreground " +
    "disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-muted-foreground";

  const pageBtn = (isActive: boolean) =>
    cn(
      "inline-flex h-7 min-w-7 items-center justify-center rounded-md px-2 text-xs font-medium transition-colors",
      isActive
        ? "bg-muted text-foreground"
        : "text-muted-foreground hover:bg-muted hover:text-foreground",
    );

  return (
    <nav
      aria-label="Pagination"
      className={cn("flex items-center justify-center gap-1", className)}
    >
      <button
        type="button"
        aria-label="Previous page"
        onClick={() => go(page - 1)}
        disabled={page === 0}
        className={arrowBtn}
      >
        <HugeiconsIcon icon={ArrowLeft01Icon} size={15} strokeWidth={1.75} />
      </button>

      {seq.map((p, i) =>
        p === "…" ? (
          <span key={`e-${i}`} className="px-1 text-xs text-muted-foreground" aria-hidden>
            …
          </span>
        ) : (
          <button
            key={p}
            type="button"
            aria-current={p === page ? "page" : undefined}
            aria-label={`Go to page ${p + 1}`}
            onClick={() => go(p)}
            className={pageBtn(p === page)}
          >
            {p + 1}
          </button>
        ),
      )}

      <button
        type="button"
        aria-label="Next page"
        onClick={() => go(page + 1)}
        disabled={page >= totalPages - 1}
        className={arrowBtn}
      >
        <HugeiconsIcon icon={ArrowRight01Icon} size={15} strokeWidth={1.75} />
      </button>
    </nav>
  );
}
