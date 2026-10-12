import { invoke } from "@tauri-apps/api/core";

export type ProviderKind = "ollama" | "openai" | "anthropic";
export interface AiStatus { provider: ProviderKind | null; model: string | null; configured: boolean; has_openai_key: boolean; has_anthropic_key: boolean }
export interface AiSettings { provider: ProviderKind | null; ollama_model: string | null; openai_model: string | null; anthropic_model: string | null }
export interface Message { role: "user" | "assistant"; content: string }
export interface Disclosure { provider: ProviderKind; model: string; endpoint: string; system: string; messages: Message[]; characters: number }
export const PROVIDER_NAMES: Record<ProviderKind, string> = { ollama: "Ollama", openai: "OpenAI", anthropic: "Anthropic" };
export const PROVIDERS = [
  { kind: "ollama", label: "Ollama (on this PC)" },
  { kind: "openai", label: "OpenAI (your API key)" },
  { kind: "anthropic", label: "Anthropic (your API key)" },
] as const;
export const aiStatus = () => invoke<AiStatus>("ai_status");
export const listAiModels = (provider: ProviderKind) => invoke<string[]>("list_ai_models", { provider });
export const saveApiKey = (provider: ProviderKind, key: string) => invoke<AiStatus>("save_api_key", { provider, key });
export const deleteApiKey = (provider: ProviderKind) => invoke<AiStatus>("delete_api_key", { provider });
export const updateAiSettings = (provider: ProviderKind | null, model: string | null) => invoke<AiStatus>("update_ai_settings", { provider, model });
export const testAiProvider = () => invoke<string>("test_ai_provider");
