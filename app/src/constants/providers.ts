export type ProviderFormat = 'openai-compat' | 'anthropic' | 'gemini';

export interface Provider {
  id: string;
  name: string;
  baseUrl: string;
  format: ProviderFormat;
  defaultModel: string;
  models: string[];
}

export const PROVIDERS: Provider[] = [
  {
    id: 'openai', 
    name: 'OpenAI',
    baseUrl: 'https://api.openai.com',
    format: 'openai-compat',
    defaultModel: 'gpt-4o-mini',
    models: ['gpt-4o', 'gpt-4o-mini', 'o1-preview', 'o1-mini', 'gpt-4-turbo'],
  },
  {
    id: 'groq', 
    name: 'Groq',
    baseUrl: 'https://api.groq.com/openai',
    format: 'openai-compat',
    defaultModel: 'llama-3.3-70b-versatile',
    models: [
      'llama-3.3-70b-versatile',
      'llama-3.1-405b-reasoning',
      'llama-3.1-70b-versatile',
      'llama-3.1-8b-instant',
      'mixtral-8x7b-32768',
      'gemma2-9b-it'
    ],
  },
  {
    id: 'grok', 
    name: 'Grok (xAI)',
    baseUrl: 'https://api.x.ai',
    format: 'openai-compat',
    defaultModel: 'grok-beta',
    models: ['grok-beta', 'grok-vision-beta'],
  },
  {
    id: 'openrouter', 
    name: 'OpenRouter',
    baseUrl: 'https://openrouter.ai/api',
    format: 'openai-compat',
    defaultModel: 'google/gemini-2.0-flash-001',
    models: [
      'google/gemini-3.1-pro-preview',
      'google/gemini-2.5-flash',
      'google/gemini-2.0-flash-001',
      'anthropic/claude-3.5-sonnet',
      'meta-llama/llama-3.1-405b-instruct',
      'meta-llama/llama-3.3-70b-instruct',
      'mistralai/pixtral-12b',
      'openrouter/auto'
    ],
  },
  {
    id: 'anthropic', 
    name: 'Anthropic',
    baseUrl: 'https://api.anthropic.com',
    format: 'anthropic',
    defaultModel: 'claude-3-5-sonnet-20241022',
    models: [
      'claude-3-5-sonnet-20241022',
      'claude-3-5-haiku-20241022',
      'claude-3-opus-20240229'
    ],
  },
  {
    id: 'gemini', 
    name: 'Gemini',
    baseUrl: 'https://generativelanguage.googleapis.com/v1beta',
    format: 'gemini',
    defaultModel: 'gemini-2.0-flash',
    models: [
      'gemini-3.1-pro-preview',
      'gemini-3.1-flash-lite-preview',
      'gemini-3-flash-preview',
      'gemini-2.5-pro',
      'gemini-2.5-flash',
      'gemini-2.5-flash-lite',
      'gemini-2.0-flash',
      'gemini-2.0-flash-lite',
      'gemini-1.5-pro',
      'gemini-1.5-flash',
      'gemini-1.5-flash-8b'
    ],
  },
];
