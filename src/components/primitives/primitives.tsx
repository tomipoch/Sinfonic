// Settings primitives — building blocks for the settings window.
//
// The visual language is borrowed from the Cursor settings page:
//   - Section: a labeled region (small uppercase label + content slot)
//   - Card: rounded panel that hosts one logical group of controls
//   - ChoiceCard: a wide pill that can be selected among siblings
//   - ToggleCard: a row with title + description + a toggle on the right
//   - SliderCard: a row with label + value + slider
//   - Title: bold page heading + muted subtitle
//
// Tokens used throughout: `bg-card/40` (semi-transparent panels layered
// over `bg-background`), `border-border`, `text-muted-foreground`.

import type { ReactNode } from "react";

export function SettingsTitle({ title, subtitle }: { title: string; subtitle?: string }) {
  return (
    <header className="flex flex-col gap-1 pb-1">
      <h1 className="text-2xl font-semibold tracking-tight text-foreground">{title}</h1>
      {subtitle ? <p className="text-sm text-muted-foreground">{subtitle}</p> : null}
    </header>
  );
}

export function SettingsSection({ label, children }: { label: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2.5">
      <div className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
        {label}
      </div>
      <div className="flex flex-col gap-2">{children}</div>
    </section>
  );
}

export function SettingsCard({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={`rounded-xl border border-border bg-card/40 backdrop-blur-sm ${className}`}>
      {children}
    </div>
  );
}

export function ChoiceCard({
  selected,
  onClick,
  icon,
  label,
}: {
  selected: boolean;
  onClick: () => void;
  icon?: ReactNode;
  label: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={selected}
      className={
        "flex flex-1 flex-col items-center justify-center gap-2 rounded-xl border px-4 py-6 transition-all " +
        (selected
          ? "border-foreground/40 bg-card shadow-[inset_0_0_0_1px_rgba(255,255,255,0.04)]"
          : "border-border bg-card/30 hover:border-muted-foreground/40 hover:bg-card/60")
      }
    >
      {icon ? (
        <span className="text-foreground/90" aria-hidden>
          {icon}
        </span>
      ) : null}
      <span className="text-sm font-medium text-foreground">{label}</span>
    </button>
  );
}

export function ToggleCard({
  title,
  description,
  checked,
  onChange,
  disabled,
}: {
  title: string;
  description?: string;
  checked: boolean;
  onChange: (next: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <SettingsCard>
      <div className="flex items-start justify-between gap-6 px-4 py-4">
        <div className="flex min-w-0 flex-col gap-0.5">
          <div className="text-sm font-medium text-foreground">{title}</div>
          {description ? <div className="text-xs text-muted-foreground">{description}</div> : null}
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={checked}
          aria-label={title}
          disabled={disabled}
          onClick={() => onChange(!checked)}
          className={
            "relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border transition-colors " +
            (checked ? "border-foreground/30 bg-foreground" : "border-border bg-muted")
          }
        >
          <span
            className={
              "pointer-events-none inline-block h-5 w-5 transform rounded-full bg-background shadow-sm transition-transform " +
              (checked ? "translate-x-5" : "translate-x-0.5")
            }
          />
        </button>
      </div>
    </SettingsCard>
  );
}

export function SliderCard({
  label,
  value,
  displayValue,
  min,
  max,
  step,
  onChange,
  disabled,
}: {
  label: string;
  value: number;
  displayValue: string;
  min: number;
  max: number;
  step: number;
  onChange: (next: number) => void;
  disabled?: boolean;
}) {
  return (
    <SettingsCard>
      <div className="flex flex-col gap-3 px-4 py-4">
        <div className="flex items-center justify-between">
          <div className="text-sm font-medium text-foreground">{label}</div>
          <div className="text-sm tabular-nums text-muted-foreground">{displayValue}</div>
        </div>
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          disabled={disabled}
          onChange={(e) => onChange(Number(e.currentTarget.value))}
          className="h-1 w-full cursor-pointer appearance-none rounded-full bg-border accent-foreground disabled:cursor-not-allowed disabled:opacity-40"
          aria-label={label}
        />
      </div>
    </SettingsCard>
  );
}
