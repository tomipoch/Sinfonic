// Login providers — pluggable per-`ServerKind` login flow.
//
// The `serverStore.login` action dispatches on `req.kind`; previously
// the dispatch was an if/else inside the action itself, which made
// adding a new provider (Plex? Navidrome?) mean editing two files
// (`serverStore.ts` + `lib/tauri.ts`) and bumping a discriminated
// union. The registry pattern keeps the dispatch declarative:
//
//   registerLoginProvider("jellyfin", async (req) => { ... });
//   registerLoginProvider("subsonic", async (req) => { ... });
//
// New providers are one registration call away.
//
// `LoginRequest` is re-exported from `serverStore.ts` so the
// discriminator + payload shape stays in one place.

import { type ConnectedServer, jellyfinLogin, localLogin, subsonicLogin } from "@/lib/tauri";
import type { ServerKind } from "@/types/domain";

import type { LoginRequest } from "./serverStore";

export type { LoginRequest } from "./serverStore";

export type LoginProvider = (req: LoginRequest) => Promise<ConnectedServer>;

const providers: Partial<Record<ServerKind, LoginProvider>> = {};

/**
 * Register (or replace) the login provider for a given `ServerKind`.
 * Idempotent; safe to call multiple times.
 */
export function registerLoginProvider(kind: ServerKind, provider: LoginProvider): void {
  providers[kind] = provider;
}

/**
 * Look up the provider for `kind` and invoke it. Throws if no
 * provider is registered — that means a new `ServerKind` was added
 * to the union without a matching `registerLoginProvider` call.
 */
export async function dispatchLogin(req: LoginRequest): Promise<ConnectedServer> {
  const provider = providers[req.kind];
  if (!provider) {
    throw new Error(`No login provider registered for "${req.kind}"`);
  }
  return provider(req);
}

// ─── Built-in providers ─────────────────────────────────────────

registerLoginProvider("jellyfin", async (req) => {
  if (req.kind !== "jellyfin") throw new Error("unreachable");
  return jellyfinLogin({
    baseUrl: req.baseUrl,
    username: req.username,
    password: req.password,
  });
});

registerLoginProvider("subsonic", async (req) => {
  if (req.kind !== "subsonic") throw new Error("unreachable");
  return subsonicLogin({
    baseUrl: req.baseUrl,
    username: req.username,
    password: req.password,
  });
});

// `local_login` does the scan + provider install + SQLite write as a
// single atomic step and emits the same `library-sync-status`
// progress events the LoadingView listens to. We synthesise the
// `ConnectedServer` shape locally because there's no `local_login`
// return value to mirror.
registerLoginProvider("local", async (req) => {
  if (req.kind !== "local") throw new Error("unreachable");
  await localLogin(req.path);
  return {
    serverId: "server-local",
    kind: "local",
    name: "Local files",
    baseUrl: req.path,
  };
});
