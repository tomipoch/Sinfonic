// Local "transport busy" state shared between TransportControls
// and SeekBar inside the PlayerBar.
//
// The PlayerBar only needs to know "is any transport action in
// flight?" so it can lock the seek bar. We model it as a tiny
// React context rather than lifting state into the bar (which
// would force two children to share a setter) or pushing state
// into each child (which would double the IPC).
//
// The context is intentionally narrow: provider + hook + setter
// type. Consumers default to "not busy" when no provider is
// mounted, which keeps the components usable in isolation.

import { createContext, type ReactNode, useContext, useMemo, useState } from "react";

type BusyKind = "play" | "prev" | "next";

export interface TransportBusy {
  busy: BusyKind | null;
  setBusy: (next: BusyKind | null) => void;
  isBusy: boolean;
}

const TransportBusyContext = createContext<TransportBusy | null>(null);

export function TransportBusyProvider({ children }: { children: ReactNode }) {
  const [busy, setBusy] = useState<BusyKind | null>(null);
  const value = useMemo<TransportBusy>(() => ({ busy, setBusy, isBusy: busy !== null }), [busy]);
  return <TransportBusyContext.Provider value={value}>{children}</TransportBusyContext.Provider>;
}

export function useTransportBusy(): TransportBusy {
  const ctx = useContext(TransportBusyContext);
  if (!ctx) {
    // Default: never busy. Lets the components be used in tests or
    // previews without the provider wrapper.
    return { busy: null, setBusy: () => {}, isBusy: false };
  }
  return ctx;
}
