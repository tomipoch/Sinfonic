// FavoriteButton — heart toggle with optimistic update.
//
// Uses React 19's `useOptimistic` + `useTransition` so the UI flips
// instantly while the IPC call is in flight. We mock the IPC layer
// (already in `src/test/setup.ts`) so we can drive the toggle without
// Tauri.

import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { FavoriteButton } from "./FavoriteButton";
import { invokeMock } from "@/test/setup";

describe("FavoriteButton", () => {
  it("renders the unfilled heart when initialFavorite is false", () => {
    render(<FavoriteButton kind="track" itemId="t-1" initialFavorite={false} />);
    const button = screen.getByRole("button", { name: "Add to favorites" });
    expect(button.getAttribute("aria-pressed")).toBe("false");
    expect(button.textContent).toBe("♡");
  });

  it("renders the filled heart when initialFavorite is true", () => {
    render(<FavoriteButton kind="track" itemId="t-1" initialFavorite />);
    const button = screen.getByRole("button", { name: "Remove from favorites" });
    expect(button.getAttribute("aria-pressed")).toBe("true");
    expect(button.textContent).toBe("♥");
  });

  it("flips the heart optimistically and calls the IPC", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    render(<FavoriteButton kind="track" itemId="t-1" initialFavorite={false} />);
    const button = screen.getByRole("button", { name: "Add to favorites" });

    fireEvent.click(button);

    // Wait for the transition + IPC to settle. `useOptimistic` reverts
    // to the last committed value when the transition completes, so
    // once the mock IPC has resolved and the transition settles the
    // aria-pressed should track the new state.
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_track_favorite", {
        trackId: "t-1",
        favorite: true,
      });
    });
  });

  it("reverts the optimistic flip and surfaces an error when the IPC fails", async () => {
    invokeMock.mockRejectedValueOnce("network down");

    render(<FavoriteButton kind="track" itemId="t-1" initialFavorite={false} />);
    const button = screen.getByRole("button", { name: "Add to favorites" });

    await act(async () => {
      fireEvent.click(button);
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(button.getAttribute("aria-pressed")).toBe("false");
  });

  it("routes the click to the right IPC for albums", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    render(<FavoriteButton kind="album" itemId="a-1" initialFavorite={false} />);
    act(() => {
      fireEvent.click(screen.getByRole("button", { name: "Add to favorites" }));
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_album_favorite", {
        albumId: "a-1",
        favorite: true,
      });
    });
  });

  it("routes the click to the right IPC for artists", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    render(<FavoriteButton kind="artist" itemId="ar-1" initialFavorite={false} />);
    act(() => {
      fireEvent.click(screen.getByRole("button", { name: "Add to favorites" }));
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_artist_favorite", {
        artistId: "ar-1",
        favorite: true,
      });
    });
  });
});