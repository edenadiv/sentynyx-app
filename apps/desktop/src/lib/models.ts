import type { Model, Conversation } from "./types";

// Built-in models. Four BYOK cloud providers (keys live in the OS keychain) +
// the bundled on-device model. Ollama models are discovered at runtime and
// merged in by the app (see `ollamaModel` below), so they're not listed here.
export const MODELS: Model[] = [
  { id:"gpt-5",          name:"GPT-5",            provider:"OpenAI",     ctx:"400K", flash:"Latest",   color:"#10a37f" },
  { id:"gpt-5-mini",     name:"GPT-5 mini",       provider:"OpenAI",     ctx:"200K", flash:"Fast",     color:"#10a37f" },
  { id:"o4",             name:"o4 reasoning",     provider:"OpenAI",     ctx:"200K", flash:"Reasoning",color:"#10a37f" },
  { id:"claude-opus-4",  name:"Claude Opus 4.5",  provider:"Anthropic",  ctx:"500K", flash:"Top",      color:"#d97757" },
  { id:"claude-sonnet",  name:"Claude Sonnet 4.5",provider:"Anthropic",  ctx:"200K", flash:"Balanced", color:"#d97757" },
  { id:"claude-haiku",   name:"Claude Haiku 4.5", provider:"Anthropic",  ctx:"200K", flash:"Fast",     color:"#d97757" },
  { id:"gemini-2-5-pro", name:"Gemini 2.5 Pro",   provider:"Google",     ctx:"2M",   flash:"Long ctx", color:"#4285f4" },
  { id:"gemini-flash",   name:"Gemini Flash 2.5", provider:"Google",     ctx:"1M",   flash:"Fast",     color:"#4285f4" },
  { id:"grok-4",         name:"Grok 4",           provider:"xAI",        ctx:"256K", flash:"Realtime", color:"#ffffff" },
  // One OpenRouter key unlocks all of these (ids verified against the live
  // openrouter.ai catalog). Routed by the `openrouter:` prefix.
  { id:"openrouter:meta-llama/llama-4-maverick",  name:"Llama 4 Maverick",  provider:"OpenRouter", ctx:"1M",   flash:"Open",     color:"#1877f2" },
  { id:"openrouter:deepseek/deepseek-v4-pro",     name:"DeepSeek V4 Pro",   provider:"OpenRouter", ctx:"1M",   flash:"Reasoning",color:"#4d6bfe" },
  { id:"openrouter:mistralai/mistral-large-2512", name:"Mistral Large",     provider:"OpenRouter", ctx:"262K", flash:"EU",       color:"#ff7000" },
  { id:"openrouter:qwen/qwen3.7-plus",            name:"Qwen3.7 Plus",      provider:"OpenRouter", ctx:"1M",   flash:"Long ctx", color:"#8b5cf6" },
  { id:"openrouter:cohere/command-a",             name:"Command A",         provider:"OpenRouter", ctx:"256K", flash:"RAG",      color:"#ff6b9d" },
  { id:"sentynyx-local", name:"Sentynyx Local",   provider:"On-device",  ctx:"32K",  flash:"Private",  color:"#f2ff2b" },
];

export const PROVIDER_GLYPHS: Record<string, string> = {
  "OpenAI": "◎", "Anthropic": "✦", "Google": "◆", "xAI": "𝕏",
  "OpenRouter": "⌬", "Ollama": "⊙", "On-device": "●",
};

/** Tauri provider key by model id. null = no key needed (Ollama / on-device). */
export function providerKey(modelId: string): "openai" | "anthropic" | "google" | "xai" | "openrouter" | null {
  if (modelId.startsWith("openrouter:")) return "openrouter";
  if (modelId.startsWith("gpt") || modelId === "o4") return "openai";
  if (modelId.startsWith("claude")) return "anthropic";
  if (modelId.startsWith("gemini")) return "google";
  if (modelId === "grok-4") return "xai";
  return null;
}

/**
 * Build a `Model` entry for a model discovered on the local Ollama server.
 * The id is prefixed with `ollama:` so the Rust router recognizes it; the
 * "Ollama" provider gives it its own group + glyph in the picker. A loopback
 * Ollama server runs entirely on-device, so these are zero-egress like the
 * bundled local model.
 */
export function ollamaModel(name: string): Model {
  return {
    id: `ollama:${name}`,
    name,
    provider: "Ollama",
    ctx: "local",
    flash: "Local",
    color: "#a78bfa",
  };
}

export const SAMPLE_CONVERSATIONS: Conversation[] = [
  { id:"c1", title:"Q4 board memo — draft",         time:"now",   pinned:true, shield:true },
  { id:"c2", title:"Competitor analysis: Northwind", time:"12m",   shield:true },
  { id:"c3", title:"Refactor billing webhook",       time:"1h" },
  { id:"c4", title:"HR policy rewrite — EU",         time:"3h",    shield:true },
  { id:"c5", title:"Pitch deck: Series C",           time:"Yday",  shield:true },
  { id:"c6", title:"Prod incident post-mortem",      time:"Mon" },
  { id:"c7", title:"SOC 2 control mapping",          time:"Aug 12" },
];

export const SUGGESTIONS = [
  { t:"Summarize this earnings call transcript", k:"Analyze" },
  { t:"Draft a customer churn retention email",   k:"Write" },
  { t:"Review this contract for red flags",       k:"Legal" },
  { t:"Turn these notes into an exec memo",       k:"Synthesize" },
];
