export const PROVIDER_PRESETS = {
  openai: {
    label: "OpenAI",
    url: "https://api.openai.com/v1/chat/completions",
    model: "gpt-4o",
    keyLink: "https://platform.openai.com/api-keys",
    provider: "openai"
  },
  anthropic: {
    label: "Anthropic",
    url: "https://api.anthropic.com/v1/messages",
    model: "claude-sonnet-4-5",
    keyLink: "https://console.anthropic.com/settings/keys",
    provider: "anthropic"
  },
  groq: {
    label: "Groq",
    url: "https://api.groq.com/openai/v1/chat/completions",
    model: "llama-3.3-70b-versatile",
    keyLink: "https://console.groq.com/keys",
    provider: "groq"
  },
  grok: {
    label: "Grok (xAI)",
    url: "https://api.x.ai/v1/chat/completions",
    model: "grok-3",
    keyLink: "https://console.x.ai",
    provider: "grok"
  },
  openrouter: {
    label: "OpenRouter",
    url: "https://openrouter.ai/api/v1/chat/completions",
    model: "meta-llama/llama-3.3-70b-instruct",
    keyLink: "https://openrouter.ai/keys",
    provider: "openrouter"
  },
  ollama: {
    label: "Ollama",
    url: "http://localhost:11434/api/chat",
    model: "llama3.2",
    keyLink: null,
    provider: "ollama"
  },
  gemini: {
    label: "Gemini",
    url: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
    model: "gemini-2.0-flash",
    keyLink: "https://aistudio.google.com/app/apikey",
    provider: "gemini"
  },
  custom: {
    label: "Custom",
    url: "",
    model: "",
    keyLink: null,
    provider: "custom"
  },
} as const;

export type ProviderType = keyof typeof PROVIDER_PRESETS;
