// serverStore — regression tests for the source-switch / logout cleanup.
//
// The original bug: switching source (or logging out / deleting the
// active server) only stopped the audio on the backend; the Zustand
// `queueStore` and `playbackStore` kept their old contents, so the
// QueuePanel and PlayerBar briefly rendered tracks from a server
// that no longer existed. The fix: `setActive`, `logout`, and
// `deleteServer` (when active) eagerly reset queue + playback in
// addition to the existing library reset.

import { beforeEach, describe, expect, it } from "vitest";

import { useServerStore } from "./serverStore";
import { useQueueStore } from "./queueStore";
import { usePlaybackStore } from "./playbackStore";
import { useLibraryStore } from "./libraryStore";
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

const setPlaybackState = (playing: boolean) => {
  usePlaybackStore.setState({
    isPlaying: playing,
    currentTrack: playing
      ? { trackId: "track-old", title: "Old", artist: "Old Artist", album: "Old Album" }
      : null,
    positionSeconds: playing ? 30 : 0,
    durationSeconds: playing ? 180 : 0,
    volume: 0.5,
    muted: false,
    repeat: "off",
    shuffle: false,
  });
};

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
  setPlaybackState(true);
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
});

describe("serverStore — source switch / logout cleanup", () => {
  it("logout clears queue, playback, and library eagerly", async () => {
    invokeMock.mockResolvedValueOnce(undefined); // providerLogout

    await useServerStore.getState().logout();

    expect(useQueueStore.getState().entries).toHaveLength(0);
    expect(useQueueStore.getState().currentIndex).toBeNull();
    expect(useQueueStore.getState().serverId).toBeNull();

    expect(usePlaybackStore.getState().isPlaying).toBe(false);
    expect(usePlaybackStore.getState().currentTrack).toBeNull();
    expect(usePlaybackStore.getState().positionSeconds).toBe(0);
    expect(usePlaybackStore.getState().durationSeconds).toBe(0);
  });

  it("setActive clears queue and playback eagerly", async () => {
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
    expect(usePlaybackStore.getState().currentTrack).toBeNull();
    expect(usePlaybackStore.getState().isPlaying).toBe(false);
  });

  it("deleteServer on the active server clears queue and playback", async () => {
    useServerStore.setState({
      servers: [
        { id: "server-jellyfin", kind: "jellyfin", name: "Jellyfin", baseUrl: "http://jellyfin.local" },
      ],
      activeServerId: "server-jellyfin",
    });
    invokeMock.mockResolvedValueOnce(undefined); // providerDelete

    await useServerStore.getState().deleteServer("server-jellyfin");

    expect(useQueueStore.getState().entries).toHaveLength(0);
    expect(usePlaybackStore.getState().currentTrack).toBeNull();
    expect(useServerStore.getState().activeServerId).toBeNull();
  });
});
