// playlistsStore — exercise the CRUD / playback / error paths so
// regressions in the playlist flow surface before users hit them.

import { beforeEach, describe, expect, it } from "vitest";

import { usePlaylistsStore } from "./playlistsStore";
import { useServerStore } from "./serverStore";
import { invokeMock } from "@/test/setup";

const makePlaylist = (id: string, name = `Playlist ${id}`) => ({
  id,
  name,
  trackCount: 0,
  durationSeconds: 0,
  owner: null,
  public: false,
  imageRef: null,
});

const makeTrack = (id: string) => ({
  id,
  albumId: `album-${id}`,
  title: `Track ${id}`,
  artist: `Artist ${id}`,
  artistId: null,
  album: `Album ${id}`,
  durationSeconds: 100,
  trackNumber: 1,
  discNumber: 1,
  favorite: false,
  imageRef: null,
});

beforeEach(() => {
  usePlaylistsStore.setState({
    playlists: [],
    loading: false,
    error: null,
    detail: null,
    detailLoading: false,
    detailError: null,
  });
  useServerStore.setState({
    servers: [],
    activeServerId: "server-test",
    discovered: [],
    lastSync: "idle",
    error: null,
    pendingConnection: null,
  });
  invokeMock.mockReset();
});

describe("playlistsStore — list + detail", () => {
  it("loadPlaylists populates the list and clears loading", async () => {
    invokeMock.mockResolvedValueOnce([makePlaylist("p-1"), makePlaylist("p-2")]);
    await usePlaylistsStore.getState().loadPlaylists();

    expect(usePlaylistsStore.getState().playlists).toHaveLength(2);
    expect(usePlaylistsStore.getState().loading).toBe(false);
    expect(usePlaylistsStore.getState().error).toBeNull();
  });

  it("loadPlaylists surfaces errors via extractError fallback", async () => {
    invokeMock.mockRejectedValueOnce(new Error(""));

    await usePlaylistsStore.getState().loadPlaylists();

    expect(usePlaylistsStore.getState().error).toBe("couldn't load playlists");
    expect(usePlaylistsStore.getState().loading).toBe(false);
  });

  it("loadPlaylistDetail populates detail and clears loading", async () => {
    invokeMock.mockResolvedValueOnce({
      playlist: makePlaylist("p-1", "Focus"),
      tracks: [makeTrack("t-0"), makeTrack("t-1")],
    });

    await usePlaylistsStore.getState().loadPlaylistDetail("p-1");

    const detail = usePlaylistsStore.getState().detail;
    expect(detail?.playlist.id).toBe("p-1");
    expect(detail?.tracks).toHaveLength(2);
    expect(usePlaylistsStore.getState().detailLoading).toBe(false);
  });

  it("loadPlaylistDetail surfaces errors", async () => {
    invokeMock.mockRejectedValueOnce(new Error(""));

    await usePlaylistsStore.getState().loadPlaylistDetail("missing");

    expect(usePlaylistsStore.getState().detail).toBeNull();
    expect(usePlaylistsStore.getState().detailError).toBe("couldn't load playlist");
  });
});

describe("playlistsStore — CRUD", () => {
  it("createPlaylist returns the new id and refreshes the list", async () => {
    invokeMock
      .mockResolvedValueOnce("p-new")
      .mockResolvedValueOnce([makePlaylist("p-new", "New")]);

    const id = await usePlaylistsStore.getState().createPlaylist("New");

    expect(id).toBe("p-new");
    expect(invokeMock).toHaveBeenCalledWith("create_playlist", {
      name: "New",
      trackIds: [],
    });
    expect(usePlaylistsStore.getState().playlists).toHaveLength(1);
  });

  it("renamePlaylist refreshes the list and patches the cached detail", async () => {
    usePlaylistsStore.setState({
      detail: {
        playlist: makePlaylist("p-1", "Old"),
        tracks: [makeTrack("t-0")],
      },
    });
    invokeMock
      .mockResolvedValueOnce(undefined) // rename_playlist
      .mockResolvedValueOnce([makePlaylist("p-1", "New")]); // playlists_get

    await usePlaylistsStore.getState().renamePlaylist("p-1", "New");

    expect(usePlaylistsStore.getState().detail?.playlist.name).toBe("New");
    expect(usePlaylistsStore.getState().playlists[0]?.name).toBe("New");
  });

  it("deletePlaylist clears detail and refreshes the list", async () => {
    usePlaylistsStore.setState({
      detail: { playlist: makePlaylist("p-1"), tracks: [] },
    });
    invokeMock
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce([]);

    await usePlaylistsStore.getState().deletePlaylist("p-1");

    expect(usePlaylistsStore.getState().detail).toBeNull();
    expect(usePlaylistsStore.getState().playlists).toHaveLength(0);
  });

  it("removePlaylistEntries refreshes the detail", async () => {
    invokeMock
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce({
        playlist: makePlaylist("p-1", "P"),
        tracks: [makeTrack("t-0")],
      });

    await usePlaylistsStore
      .getState()
      .removePlaylistEntries("p-1", ["entry-0", "entry-1"]);

    expect(invokeMock).toHaveBeenCalledWith("remove_playlist_entries", {
      playlistId: "p-1",
      entryIds: ["entry-0", "entry-1"],
    });
    expect(usePlaylistsStore.getState().detail?.tracks).toHaveLength(1);
  });
});

describe("playlistsStore — playback", () => {
  it("playPlaylist reuses cached detail when id matches", async () => {
    usePlaylistsStore.setState({
      detail: {
        playlist: makePlaylist("p-1"),
        tracks: [makeTrack("t-0"), makeTrack("t-1")],
      },
    });
    invokeMock.mockResolvedValueOnce(undefined);

    await usePlaylistsStore.getState().playPlaylist("p-1");

    expect(invokeMock).toHaveBeenCalledTimes(1);
    // After the queue-persistence work, `playPlaylist` anchors the
    // auto-fill to the playlist so the queue extends with the
    // remaining entries of the same playlist.
    expect(invokeMock).toHaveBeenCalledWith("play_album_with_context", {
      tracks: [
        expect.objectContaining({ id: "t-0" }),
        expect.objectContaining({ id: "t-1" }),
      ],
      context: {
        kind: "playlist",
        playlistId: "p-1",
        serverId: "server-test",
      },
    });
  });

  it("playPlaylist fetches detail when not cached", async () => {
    invokeMock
      .mockResolvedValueOnce({
        // playlist_detail
        playlist: makePlaylist("p-1"),
        tracks: [makeTrack("t-0")],
      })
      .mockResolvedValueOnce(undefined); // play_album

    await usePlaylistsStore.getState().playPlaylist("p-1");

    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("playPlaylist is a no-op when the fetched detail is empty", async () => {
    invokeMock.mockResolvedValueOnce({
      playlist: makePlaylist("p-1"),
      tracks: [],
    });

    await usePlaylistsStore.getState().playPlaylist("p-1");

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).not.toHaveBeenCalledWith("play_album", expect.anything());
  });
});

describe("playlistsStore — reset", () => {
  it("reset clears every field", () => {
    usePlaylistsStore.setState({
      playlists: [makePlaylist("p-1")],
      loading: true,
      error: "boom",
      detail: { playlist: makePlaylist("p-1"), tracks: [] },
      detailLoading: true,
      detailError: "boom2",
    });

    usePlaylistsStore.getState().reset();

    const state = usePlaylistsStore.getState();
    expect(state.playlists).toHaveLength(0);
    expect(state.loading).toBe(false);
    expect(state.error).toBeNull();
    expect(state.detail).toBeNull();
    expect(state.detailLoading).toBe(false);
    expect(state.detailError).toBeNull();
  });
});