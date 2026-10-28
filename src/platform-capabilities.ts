import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface PlatformCapabilities {
  recordPrompt: boolean;
  backgroundTranscription: boolean;
  snapshots: boolean;
}

export const unavailableCapabilities: PlatformCapabilities = {
  recordPrompt: false,
  backgroundTranscription: false,
  snapshots: false,
};

let cached: Promise<PlatformCapabilities> | undefined;

export function platformCapabilities(): Promise<PlatformCapabilities> {
  return cached ??= invoke<PlatformCapabilities>("platform_capabilities").catch(() => unavailableCapabilities);
}

export function usePlatformCapabilities(): PlatformCapabilities {
  const [capabilities, setCapabilities] = useState(unavailableCapabilities);
  useEffect(() => {
    let active = true;
    void platformCapabilities().then((value) => { if (active) setCapabilities(value); });
    return () => { active = false; };
  }, []);
  return capabilities;
}
