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

const windowsCapabilities: PlatformCapabilities = {
  recordPrompt: true,
  backgroundTranscription: true,
  snapshots: true,
};

export function isWindowsPlatform(): boolean {
  return typeof navigator !== "undefined" && navigator.userAgent.includes("Windows");
}

export function guessedCapabilities(): PlatformCapabilities {
  return isWindowsPlatform() ? windowsCapabilities : unavailableCapabilities;
}

// Wording follows the Windows capability set: only Windows has every capability today.
export function platformWording(text: string, capabilities?: PlatformCapabilities): string {
  return (capabilities ? capabilities.recordPrompt : isWindowsPlatform()) ? text : text.replaceAll("this PC", "this computer").replaceAll("Windows Credential Manager", "macOS Keychain");
}

let resolved: PlatformCapabilities | undefined;
let pending: Promise<PlatformCapabilities> | undefined;

export function platformCapabilities(): Promise<PlatformCapabilities> {
  if (resolved) return Promise.resolve(resolved);
  if (pending) return pending;
  pending = invoke<PlatformCapabilities>("platform_capabilities").then(
    (value) => { resolved = value; pending = undefined; return value; },
    () => { pending = undefined; return guessedCapabilities(); },
  );
  return pending;
}

export function usePlatformCapabilities(): PlatformCapabilities {
  const [capabilities, setCapabilities] = useState(() => resolved ?? guessedCapabilities());
  useEffect(() => {
    let active = true;
    void platformCapabilities().then((value) => { if (active) setCapabilities(value); });
    return () => { active = false; };
  }, []);
  return capabilities;
}
