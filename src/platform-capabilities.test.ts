import { beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

beforeEach(() => { vi.resetModules(); vi.resetAllMocks(); });

it("fetches platform capabilities only once", async () => {
  const windows = { recordPrompt: true, backgroundTranscription: true, snapshots: true };
  vi.mocked(invoke).mockResolvedValue(windows);
  const { platformCapabilities } = await import("./platform-capabilities");
  expect(await platformCapabilities()).toEqual(windows);
  expect(await platformCapabilities()).toEqual(windows);
  expect(invoke).toHaveBeenCalledExactlyOnceWith("platform_capabilities");
});

it("defaults to unavailable when the command cannot be invoked in tests", async () => {
  vi.mocked(invoke).mockRejectedValue(new Error("Tauri unavailable"));
  const { platformCapabilities } = await import("./platform-capabilities");
  expect(await platformCapabilities()).toEqual({ recordPrompt: false, backgroundTranscription: false, snapshots: false });
});
