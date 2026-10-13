import { invoke } from "@tauri-apps/api/core";

export const getDiagnostics = () => invoke<string>("get_diagnostics");
