import { getApiKey } from '@/services/secureStorage';
import type { ChatMessage } from '@/types/chat';
import { PROVIDERS } from '@/constants/providers';
import * as contactService from '@/services/contactService';
import * as whatsappService from '@/services/whatsappService';
import * as notificationService from '@/services/notificationService';
import { useChatStore } from '@/stores/chatStore';

interface StreamCallbacks {
  onToken: (token: string) => void;
  onDone: () => void;
  onError: (message: string) => void;
}

type CloudMessage = { role: string; content: string };

// ── ANK Tools Definition ────────────────────────────────────────────────

const ANK_TOOLS = [
  {
    name: "search_contacts",
    description: "Search for people in the device contacts by name. Returns phone numbers and emails.",
    parameters: {
      type: "object",
      properties: {
        name: { type: "string", description: "The name of the contact to search for" }
      },
      required: ["name"]
    }
  },
  {
    name: "get_all_contacts",
    description: "Retrieve a list of all contacts on the device.",
    parameters: { type: "object", properties: {} }
  },
  {
    name: "send_whatsapp",
    description: "Open WhatsApp to send a message to a specific phone number.",
    parameters: {
      type: "object",
      properties: {
        phone: { type: "string", description: "The phone number with country code (e.g., +1234567890)" },
        message: { type: "string", description: "The text message to send" }
      },
      required: ["phone", "message"]
    }
  },
  {
    name: "read_notifications",
    description: "Retrieve a list of the most recent notifications received on the device.",
    parameters: {
      type: "object",
      properties: {
        limit: { type: "number", description: "The maximum number of notifications to retrieve (max 5)." }
      }
    }
  }
];

// ── ANK Tool Execution Bridge ───────────────────────────────────────────

async function executeAnkTool(name: string, args: any) {
  const { setProcessingTool } = useChatStore.getState();
  setProcessingTool(true);
  
  console.log(`[ANK Bridge] Executing tool: ${name}`, args);
  try {
    let result;
    switch (name) {
      case 'search_contacts':
        result = await contactService.searchContacts(args.name);
        break;
      case 'get_all_contacts':
        result = await contactService.getAllContacts();
        break;
      case 'send_whatsapp':
        result = await whatsappService.sendWhatsAppMessage(args.phone, args.message);
        break;
      case 'read_notifications':
        result = await notificationService.getRecentNotifications(args.limit || 5);
        break;
      default:
        result = { error: "Unknown tool" };
    }
    return result;
  } catch (err: any) {
    return { error: err.message };
  }
}

// ── Gemini Engine (with Tool Support) ──────────────────────────────────

async function runGeminiWithTools(
  apiKey: string,
  model: string,
  messages: CloudMessage[],
  callbacks: StreamCallbacks
) {
  const contents = messages.map((m) => ({
    role: m.role === 'user' ? 'user' : 'model',
    parts: [{ text: m.content }],
  }));

  const url = `https://generativelanguage.googleapis.com/v1beta/models/${model}:generateContent?key=${apiKey}`;

  const payload = {
    contents,
    tools: [{ function_declarations: ANK_TOOLS }],
    tool_config: { function_calling_config: { mode: "AUTO" } }
  };

  try {
    const response = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });

    const json = await response.json();
    if (!response.ok) {
      callbacks.onError(`Gemini Error: ${json?.error?.message || response.statusText}`);
      return;
    }

    const firstCandidate = json?.candidates?.[0];
    const call = firstCandidate?.content?.parts?.find((p: any) => p.functionCall);

    if (call) {
      // 1. ANK Bridge intercepts the call
      const toolResult = await executeAnkTool(call.functionCall.name, call.functionCall.args);
      
      // 2. Feed the result back to Gemini to get the final natural language answer
      const updatedContents = [
        ...contents,
        firstCandidate.content, // The assistant message with the function call
        {
          role: 'function',
          parts: [{
            functionResponse: {
              name: call.functionCall.name,
              response: { content: toolResult }
            }
          }]
        }
      ];

      const secondResponse = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ contents: updatedContents }),
      });

      const finalJson = await secondResponse.json();
      const finalContent = finalJson?.candidates?.[0]?.content?.parts?.[0]?.text || '';
      
      if (finalContent) {
        callbacks.onToken(finalContent);
        callbacks.onDone();
      } else {
        callbacks.onError('Failed to generate final response after tool use');
      }
    } else {
      // No tool needed, just normal text
      const text = firstCandidate?.content?.parts?.[0]?.text || '';
      callbacks.onToken(text);
      callbacks.onDone();
    }
  } catch (err: any) {
    callbacks.onError(err.message || 'Gemini Bridge failed');
  }
}

// ── Shared Standard Fetch ─────────────────────────────────────────────

async function fetchNonStreaming(
  url: string,
  options: RequestInit,
  providerFormat: string,
  callbacks: StreamCallbacks
) {
  try {
    const response = await fetch(url, options);
    const json = await response.json();
    
    if (!response.ok) {
      callbacks.onError(`Error: ${json?.error?.message || response.statusText}`);
      return;
    }

    let content = '';
    if (providerFormat === 'gemini') {
      content = json?.candidates?.[0]?.content?.parts?.[0]?.text || '';
    } else if (providerFormat === 'anthropic') {
      content = json?.content?.[0]?.text || '';
    } else {
      content = json?.choices?.[0]?.message?.content || '';
    }

    if (content) {
      callbacks.onToken(content);
      callbacks.onDone();
    } else {
      callbacks.onError('Empty response from provider');
    }
  } catch (err: any) {
    callbacks.onError(err.message || 'Request failed');
  }
}

// ── Public API ─────────────────────────────────────────────────────────

export async function streamCloud(
  providerId: string,
  model: string,
  messages: ChatMessage[],
  callbacks: StreamCallbacks
): Promise<void> {
  const provider = PROVIDERS.find((p) => p.id === providerId);
  if (!provider) {
    callbacks.onError(`Unknown provider: ${providerId}`);
    return;
  }
  
  const apiKey = await getApiKey(providerId);
  if (!apiKey) {
    callbacks.onError(`No API key configured for ${provider.name}`);
    return;
  }

  const cloudMessages: CloudMessage[] = messages.map((m) => ({
    role: m.role === 'user' ? 'user' : 'assistant',
    content: m.content,
  }));

  try {
    if (provider.format === 'gemini') {
      await runGeminiWithTools(apiKey, model, cloudMessages, callbacks);
    } else if (provider.format === 'anthropic') {
      await fetchNonStreaming('https://api.anthropic.com/v1/messages', {
        method: 'POST',
        headers: {
          'x-api-key': apiKey,
          'anthropic-version': '2023-06-01',
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ model, max_tokens: 4096, messages: cloudMessages, stream: false }),
      }, 'anthropic', callbacks);
    } else {
      await fetchNonStreaming(`${provider.baseUrl.replace(/\/$/, '')}/v1/chat/completions`, {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${apiKey}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ model, messages: cloudMessages, stream: false }),
      }, 'openai', callbacks);
    }
  } catch (err: unknown) {
    callbacks.onError(err instanceof Error ? err.message : 'Network error');
  }
}
