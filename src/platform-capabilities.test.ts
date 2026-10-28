import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

beforeEach(() => { vi.resetModules(); vi.resetAllMocks(); });
afterEach(() => { vi.restoreAllMocks(); });

const windows = { recordPrompt: true, backgroundTranscription: true, snapshots: true };
const mac = { recordPrompt: false, backgroundTranscription: false, snapshots: false };

function userAgent(value: string) {
  vi.spyOn(navigator, "userAgent", "get").mockReturnValue(value);
}

function deferredCapabilities() {
  let resolve!: (value: typeof windows) => void;
  const promise = new Promise<typeof windows>((done) => { resolve = done; });
  return { promise, resolve };
}

it("fetches platform capabilities only once", async () => {
  vi.mocked(invoke).mockResolvedValue(windows);
  const { platformCapabilities } = await import("./platform-capabilities");
  expect(await platformCapabilities()).toEqual(windows);
  expect(await platformCapabilities()).toEqual(windows);
  expect(invoke).toHaveBeenCalledExactlyOnceWith("platform_capabilities");
});

it.each([["Windows NT 10.0", windows], ["Macintosh; Intel Mac OS X", mac]])("starts with the %s user agent guess before IPC resolves", async (ua, guess) => {
  userAgent(ua);
  const request = deferredCapabilities();
  vi.mocked(invoke).mockReturnValue(request.promise);
  const { usePlatformCapabilities } = await import("./platform-capabilities");
  const { result } = renderHook(() => usePlatformCapabilities());
  expect(result.current).toEqual(guess);
  request.resolve(mac);
  await waitFor(() => expect(result.current).toEqual(mac));
});

it("replaces the Mac guess with the IPC result", async () => {
  userAgent("Macintosh; Intel Mac OS X");
  const request = deferredCapabilities();
  vi.mocked(invoke).mockReturnValue(request.promise);
  const { usePlatformCapabilities } = await import("./platform-capabilities");
  const { result } = renderHook(() => usePlatformCapabilities());
  expect(result.current).toEqual(mac);
  request.resolve(windows);
  await waitFor(() => expect(result.current).toEqual(windows));
});

it.each([
  ["Windows NT 10.0", "On this PC. Keys stay in Windows Credential Manager."],
  ["Macintosh; Intel Mac OS X", "On this computer. Keys stay in macOS Keychain."],
])("uses platform wording for %s", async (ua, expected) => {
  userAgent(ua);
  const { platformWording } = await import("./platform-capabilities");
  expect(platformWording("On this PC. Keys stay in Windows Credential Manager.")).toBe(expected);
});

it("keeps the guess after failure and retries on the next call", async () => {
  userAgent("Windows NT 10.0");
  vi.mocked(invoke).mockRejectedValueOnce(new Error("Tauri unavailable")).mockResolvedValue(mac);
  const { platformCapabilities, usePlatformCapabilities } = await import("./platform-capabilities");
  const { result } = renderHook(() => usePlatformCapabilities());
  expect(result.current).toEqual(windows);
  expect(await platformCapabilities()).toEqual(windows);
  expect(result.current).toEqual(windows);
  expect(await platformCapabilities()).toEqual(mac);
  expect(invoke).toHaveBeenCalledTimes(2);
});
