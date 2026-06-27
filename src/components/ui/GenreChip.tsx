// GenreChip — clickable pill for a music genre.

import { cn } from "@/lib/cn";

type Props = {
  label: string;
  onClick?: () => void;
  className?: string;
};

export function GenreChip({ label, onClick, className }: Props) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "shrink-0 rounded-full border border-border bg-card px-4 py-1.5 text-sm font-medium text-foreground transition-colors hover:border-primary hover:text-primary",
        className,
      )}
    >
      {label}
    </button>
  );
}
