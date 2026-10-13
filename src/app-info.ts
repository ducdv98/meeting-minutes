import { invoke } from "@tauri-apps/api/core";

export interface AppInfo { version: string; channel: "Beta" }

export const appInfo = () => invoke<AppInfo>("app_info");
