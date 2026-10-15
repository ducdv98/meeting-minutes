import { invoke } from "@tauri-apps/api/core";

export type ProviderKind = "ollama" | "openai" | "anthropic" | "groq" | "gemini";
export interface AiStatus { provider: ProviderKind | null; model: string | null; configured: boolean; has_openai_key: boolean; has_anthropic_key: boolean; has_groq_key: boolean; has_gemini_key: boolean; summary_provider?: ProviderKind | null; summary_model?: string | null; summary_configured?: boolean; ask_provider?: ProviderKind | null; ask_model?: string | null; ask_configured?: boolean }
export interface AiSettings { provider: ProviderKind | null; ollama_model: string | null; openai_model: string | null; anthropic_model: string | null; gemini_model: string | null }
export interface LanguageModel { provider: Exclude<ProviderKind, "groq">; model: string }
export interface LanguageModelSettings { models: LanguageModel[]; correction: LanguageModel | null; summary: LanguageModel | null; ask: LanguageModel | null; confirmed_correction_providers: ProviderKind[]; legacy_correction_pending: boolean }
export interface ManagedStatus { connected: boolean; revoked: boolean; email?: string | null; enabled?: boolean }
export interface Message { role: "user" | "assistant"; content: string }
export interface Disclosure { provider: ProviderKind; model: string; endpoint: string; system: string; messages: Message[]; characters: number }
export const PROVIDER_NAMES: Record<ProviderKind, string> = { ollama: "Ollama", openai: "OpenAI", anthropic: "Anthropic", groq: "Groq", gemini: "Gemini" };
export const PROVIDERS = [
  { kind: "ollama", label: "Ollama (on this PC)" },
  { kind: "openai", label: "OpenAI (your API key)" },
  { kind: "anthropic", label: "Anthropic (your API key)" },
  { kind: "gemini", label: "Gemini (your API key)" },
] as const;
export const aiStatus = () => invoke<AiStatus>("ai_status");
export const managedStatus = () => invoke<ManagedStatus>("managed_status");
export const managedSendCode = (email: string) => invoke<void>("managed_send_code", { email });
export const managedSignIn = (email: string, code: string) => invoke<ManagedStatus>("managed_sign_in", { email, code });
export const managedDisconnect = () => invoke<ManagedStatus>("managed_disconnect");
export const managedSetEnabled = (enabled: boolean) => invoke<ManagedStatus>("managed_set_enabled", { enabled });
export const managedDeleteAccount = () => invoke<ManagedStatus>("managed_delete_account");
export const listAiModels = (provider: ProviderKind) => invoke<string[]>("list_ai_models", { provider });
export const saveApiKey = (provider: ProviderKind, key: string) => invoke<AiStatus>("save_api_key", { provider, key });
export const deleteApiKey = (provider: ProviderKind) => invoke<AiStatus>("delete_api_key", { provider });
export const updateAiSettings = (provider: ProviderKind | null, model: string | null) => invoke<AiStatus>("update_ai_settings", { provider, model });
export const testAiProvider = () => invoke<string>("test_ai_provider");
export const updateLanguageModels = (languageModels: LanguageModelSettings) => invoke<import("./processing").Settings>("update_language_models", { languageModels });
export const testLanguageModel = (model: LanguageModel) => invoke<string>("test_language_model", { model });
export const testCloudApiKey = (provider: "groq" | "gemini") => invoke<void>("test_cloud_api_key", { provider });
