// serverStore — regression tests for the source-switch / logout cleanup.
//
// The original bug: switching source (or logging out / deleting the
// active server) only stopped the audio on the backend; the Zustand
// `queueStore` and the playback snapshot kept their old contents, so
// the QueuePanel and PlayerBar briefly rendered tracks from a server
// that no longer existed. The fix: `setActive`, `logout`, and
// `deleteServer` (when active) call `resetSessionState`, which
// eagerly clears the library / queue / playback snapshot before the
// backend event lands.

import { beforeEach, describe, expect, it } from "vitest";

import { registerPlaybackReset } from "@/lifecycle/resetSession";
import { useLibraryStore } from "@/stores/libraryStore";
import { useServerStore } from "./serverStore";
import { useQueueStore } from "./queueStore";
import { invokeMock } from "@/test/setup";

const setQueueSnapshot = (serverId: string | null) => {
  useQueueStore.setState({
    serverId,
    entries: serverId
      ? [
          {
            id: "queue-0",
            trackId: { as_str: () => "track-old" } as never,
            entry_seq: 0,
            title: "Old",
            artist: "Old Artist",
            album: "Old Album",
            duration_seconds: 180,
            origin: { Source: { shuffle_key: 0 } },
          } as never,
        ]
      : [],
    currentIndex: serverId ? 0 : null,
    repeat: "off",
    shuffle: false,
    shuffleSeed: 0,
  });
};

let playbackResetCalls = 0;

beforeEach(() => {
  useServerStore.setState({
    servers: [],
    activeServerId: null,
    discovered: [],
    lastSync: "idle",
    error: null,
    pendingConnection: null,
  });
  setQueueSnapshot("server-old");
  useLibraryStore.setState({
    albums: [],
    artists: [],
    genres: [],
    tracks: [],
    loading: false,
    loaded: false,
    error: null,
  });
  invokeMock.mockReset();
  playbackResetCalls = 0;
  // Simulate the PlaybackProvider registering its reset callback.
  registerPlaybackReset(() => {
    playbackResetCalls += 1;
  });
});

describe("serverStore — source switch / logout cleanup", () => {
  it("logout clears the queue, library, and triggers the playback reset", async () => {
    invokeMock.mockResolvedValueOnce(undefined); // providerLogout

    await useServerStore.getState().logout();

    // queue + library were eagerly wiped by resetSessionState.
    expect(useQueueStore.getState().entries).toHaveLength(0);
    expect(useQueueStore.getState().currentIndex).toBeNull();
    expect(useQueueStore.getState().serverId).toBeNull();
    expect(useLibraryStore.getState().albums).toHaveLength(0);
    // Playback snapshot reset was triggered through the registered callback.
    expect(playbackResetCalls).toBe(1);
  });

  it("setActive clears the queue, library, and triggers the playback reset", async () => {
    useServerStore.setState({
      servers: [
        { id: "server-jellyfin", kind: "jellyfin", name: "Jellyfin", baseUrl: "http://jellyfin.local" },
      ],
      activeServerId: "server-jellyfin",
    });
    invokeMock.mockResolvedValueOnce({
      serverId: "server-jellyfin",
      kind: "jellyfin",
      name: "Jellyfin",
      baseUrl: "http://jellyfin.local",
    } as never);

    await useServerStore.getState().setActive("server-jellyfin");

    expect(useQueueStore.getState().entries).toHaveLength(0);
    expect(useQueueStore.getState().currentIndex).toBeNull();
    expect(useLibraryStore.getState().albums).toHaveLength(0);
    expect(playbackResetCalls).toBe(1);
  });

  it("deleteServer on the active server clears state and resets playback", async () => {
    useServerStore.setState({
      servers: [
        { id: "server-jellyfin", kind: "jellyfin", name: "Jellyfin", baseUrl: "http://jellyfin.local" },
      ],
      activeServerId: "server-jellyfin",
    });
    invokeMock.mockResolvedValueOnce(undefined); // providerDelete

    await useServerStore.getState().deleteServer("server-jellyfin");

    expect(useQueueStore.getState().entries).toHaveLength(0);
    expect(useServerStore.getState().activeServerId).toBeNull();
    expect(playbackResetCalls).toBe(1);
  });
});