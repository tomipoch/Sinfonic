// Vitest test setup. Runs once per test file.

import "@testing-library/jest-dom/vitest";

// Mock the Tauri JS API. Components reach for `invoke()` from
// `@tauri-apps/api/core` and `listen()` from `@tauri-apps/api/event`;
// tests get a controllable stub instead of a real IPC bridge.

import { vi } from "vitest";

const invokeMock = vi.fn(async () => undefined);
const listenMock = vi.fn(async () => () => undefined);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  convertFileSrc: vi.fn((path: string) => path),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
  emit: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(async () => null),
  save: vi.fn(async () => null),
}));

vi.mock("@tauri-apps/plugin-store", () => ({
  LazyStore: class {
    get = vi.fn(async () => undefined);
    set = vi.fn(async () => undefined);
    save = vi.fn(async () => undefined);
    onChange = vi.fn(async () => () => undefined);
  },
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: vi.fn(async () => "macos"),
}));

export { invokeMock, listenMock };
