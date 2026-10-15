import { invoke } from "@tauri-apps/api/core";

export interface AppInfo { version: string }

export const appInfo = () => invoke<AppInfo>("app_info");

export type OutsideLink = "privacy" | "support" | "groq" | "gemini" | "report_problem";
export const openLink = (link: OutsideLink) => invoke<void>("open_link", { link });
