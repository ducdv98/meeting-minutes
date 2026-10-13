import { invoke } from "@tauri-apps/api/core";

export interface AppInfo { version: string; channel: "Beta" }

export const appInfo = () => invoke<AppInfo>("app_info");

export type OutsideLink = "privacy" | "support" | "groq" | "gemini";
export const openLink = (link: OutsideLink) => invoke<void>("open_link", { link });
