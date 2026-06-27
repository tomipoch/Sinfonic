// debug — gated console logger for the webview.
//
// `makeLogger(scope)` returns a tiny logger that prefixes every
// line with `[scope]`, so DevTools output stacks nicely:
//
//   [serverStore] login: jellyfin http://example.com alice
//   [useSyncProgress] event: { state: 'scanning', progress: 0.42 }
//   [useSyncProgress] event: { state: 'complete', progress: 1 }
//
// All output is unconditional — the cost of console.log in
// production is negligible and the visibility it gives during bug
// hunts is worth more than the tiny bundle-size delta. If a future
// refactor wants to gate it, plumb a level through `import.meta.env`
// and respect it here.

type Level = "log" | "warn" | "error";

export function makeLogger(scope: string) {
  const prefix = `[${scope}]`;
  function emit(level: Level, ...args: unknown[]) {
    // eslint-disable-next-line no-console
    console[level](prefix, ...args);
  }
  return {
    log: (...args: unknown[]) => emit("log", ...args),
    warn: (...args: unknown[]) => emit("warn", ...args),
    error: (...args: unknown[]) => emit("error", ...args),
  };
}
