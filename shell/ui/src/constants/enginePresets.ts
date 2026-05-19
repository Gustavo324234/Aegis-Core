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
    // CORE-FIX: route Anthropic through OpenRouter's OpenAI-compat endpoint.
    // The backend CloudProxyDriver only speaks OpenAI-compat shape; it does
    // NOT know how to talk to api.anthropic.com/v1/messages (different
    // request format). Bundled models.yaml already routes anthropic/* via
    // OpenRouter — this preset now matches that. The user pastes their
    // Anthropic key into OpenRouter's "Anthropic" provider settings (or
    // uses an OpenRouter key directly).
    url: "https://openrouter.ai/api/v1/chat/completions",
    model: "anthropic/claude-sonnet-4-6",
    keyLink: "https://openrouter.ai/keys",
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
    // CORE-FIX: switched to /v1/chat/completions (OpenAI-compat shim) because
    // the cloud driver speaks OpenAI's SSE format. The native /api/chat
    // endpoint returns NDJSON without the `data: ` prefix, so the streaming
    // parser silently produced 0 tokens — the exact "model returned empty"
    // symptom from the smoke test with cogito-2.1:671b.
    label: "Ollama",
    url: "http://localhost:11434/v1/chat/completions",
    model: "llama3.2",
    keyLink: null,
    provider: "ollama"
  },
  ollama_cloud: {
    // CORE-FIX: same as Ollama local — point at the OpenAI-compat endpoint
    // (Ollama Cloud serves it at /v1) so streaming responses parse correctly.
    label: "Ollama Cloud",
    url: "https://ollama.com/v1/chat/completions",
    model: "llama3.3:70b-cloud",
    keyLink: "https://ollama.com/settings/api-keys",
    provider: "ollama_cloud"
  },
  gemini: {
    label: "Gemini",
    url: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
    // CORE-FIX: default is gemini-2.5-flash. The previous default
    // (gemini-2.0-flash) was already a generation behind, and discovery now
    // pulls the real list from Google so the user picks the actual model.
    // This default is only used when discovery hasn't returned yet.
    model: "gemini-2.5-flash",
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
