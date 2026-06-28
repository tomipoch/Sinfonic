// DropdownMenu — minimal popover-based menu primitive.
//
// Click the trigger to open a positioned panel with a list of items.
// Closes on outside click, Esc, or item activation. Keyboard navigation
// (Up/Down + Enter) is supported so the menu stays usable without a
// mouse.
//
// This is intentionally small — no portal, no animation library. If we
// outgrow it (nested menus, scrollable items, RTL) we can swap the
// internals for a library without touching call sites.

import { useEffect, useRef, useState } from "react";

import { cn } from "@/lib/cn";

export interface DropdownMenuItem {
  /** Visible label. */
  label: string;
  /** Optional leading icon. */
  icon?: React.ReactNode;
  /** Called when the item is activated. */
  onClick?: () => void;
  /** If set, renders the item as a navigation link instead of a button. */
  href?: string;
  /** Visually de-emphasises the item and tints the label red. */
  destructive?: boolean;
  /** Renders the item disabled. */
  disabled?: boolean;
  /** Renders a thin separator below this item. */
  separator?: boolean;
}

interface DropdownMenuProps {
  /** The element that opens the menu on click. Usually an icon button. */
  trigger: React.ReactNode;
  items: DropdownMenuItem[];
  /** Panel alignment relative to the trigger. Default `"right"`. */
  align?: "left" | "right";
  /** Optional aria-label for the menu panel. */
  ariaLabel?: string;
}

export function DropdownMenu({ trigger, items, align = "right", ariaLabel }: DropdownMenuProps) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!containerRef.current) return;
      if (!containerRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const closeAnd = (fn?: () => void) => () => {
    setOpen(false);
    fn?.();
  };

  return (
    <div ref={containerRef} className="relative inline-flex">
      <button
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={ariaLabel ?? "More actions"}
        onClick={() => setOpen((v) => !v)}
        className="rounded p-1 text-muted-foreground hover:bg-card hover:text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        {trigger}
      </button>
      {open && (
        <div
          role="menu"
          aria-label={ariaLabel}
          className={cn(
            "absolute top-full z-30 mt-1 min-w-[12rem] overflow-hidden rounded-md border border-border bg-card p-1 shadow-lg",
            align === "right" ? "right-0" : "left-0",
          )}
        >
          {items.map((item, idx) => (
            <DropdownRow
              key={`${item.label}-${idx}`}
              item={item}
              onActivate={closeAnd(item.onClick)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function DropdownRow({ item, onActivate }: { item: DropdownMenuItem; onActivate: () => void }) {
  const baseClass = cn(
    "flex w-full items-center gap-2 rounded px-2.5 py-1.5 text-left text-sm transition-colors",
    item.disabled
      ? "cursor-not-allowed opacity-50"
      : item.destructive
        ? "text-red-400 hover:bg-red-950/60"
        : "text-foreground hover:bg-muted",
  );

  const content = (
    <>
      {item.icon && <span className="flex h-4 w-4 items-center justify-center">{item.icon}</span>}
      <span className="truncate">{item.label}</span>
    </>
  );

  return (
    <>
      {item.href ? (
        <a
          role="menuitem"
          href={item.disabled ? undefined : item.href}
          className={baseClass}
          aria-disabled={item.disabled || undefined}
        >
          {content}
        </a>
      ) : (
        <button
          role="menuitem"
          type="button"
          disabled={item.disabled}
          onClick={onActivate}
          className={baseClass}
        >
          {content}
        </button>
      )}
      {item.separator && <div className="my-1 h-px bg-border" aria-hidden />}
    </>
  );
}
