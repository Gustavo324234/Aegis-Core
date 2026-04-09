import React, { useState, useEffect, useRef } from 'react';
import type { ChatMessage } from '@/types/chat';
import {
  View,
  Text,
  TextInput,
  FlatList,
  TouchableOpacity,
  StyleSheet,
  KeyboardAvoidingView,
  Platform,
  SafeAreaView,
  ActivityIndicator,
  StatusBar,
  Alert,
} from 'react-native';
import { useRouter } from 'expo-router';
import { useAuthStore } from '@/stores/authStore';
import { useChatStore } from '@/stores/chatStore';
import { useSettingsStore } from '@/stores/settingsStore';
import * as bffClient from '@/services/bffClient';
import * as cloudRouter from '@/services/cloudRouter';
import * as voiceService from '@/services/voiceService';
import * as DocumentPicker from 'expo-document-picker';
import { Ionicons } from '@expo/vector-icons';
import ModeSelector from '@/components/ModeSelector';
import ChatBubble from '@/components/ChatBubble';
import VoiceButton from '@/components/VoiceButton';
import { nanoid } from 'nanoid/non-secure';
import { TRANSLATIONS } from '@/constants/i18n';

const TypedFlatList = FlatList as unknown as React.ComponentType<any>;

export default function ChatScreen() {
  const router = useRouter();
  const { serverUrl, tenantId, sessionKey, logout, mode, setMode } = useAuthStore();
  const {
    messages,
    isStreaming,
    isProcessingTool,
    isConversationMode,
    error,
    addUserMessage,
    startAssistantMessage,
    appendToken,
    finalizeMessage,
    setConversationMode,
    setError,
    clearError,
  } = useChatStore();

  const { selectedProviderId, selectedModel, language } = useSettingsStore();
  const t = TRANSLATIONS[language];

  const [inputText, setInputText] = useState('');
  const [isListening, setIsListening] = useState(false);
  const [sirenEngine, setSirenEngine] = useState<string | null>(null);
  const [isUploading, setIsUploading] = useState(false);
  const [showModeSelector, setShowModeSelector] = useState(false);
  
  const wsRef = useRef<WebSocket | null>(null);
  const sirenWsRef = useRef<WebSocket | null>(null);
  const flatListRef = useRef<FlatList<ChatMessage>>(null);
  const hasReceivedMessage = useRef(false);

  // Siren Bridge
  useEffect(() => {
    if (mode === 'cloud' || !serverUrl || !tenantId || !sessionKey) {
      if (sirenWsRef.current) {
        sirenWsRef.current.close(1000);
        sirenWsRef.current = null;
      }
      setSirenEngine(null);
      return;
    }

    const sirenWs = voiceService.connectSiren(
      serverUrl,
      tenantId,
      sessionKey,
      (audioData) => {
        console.log('[Siren] Received audio chunk:', audioData.byteLength);
      },
      (event) => {
        if (event.event_type === 'TTS_AUDIO') {
          setSirenEngine(event.engine_id || 'Voxtral (Local)');
        }
      },
      (err) => console.warn('[Siren Bridge] Error:', err)
    );

    sirenWsRef.current = sirenWs;

    return () => {
      if (sirenWsRef.current) sirenWsRef.current.close(1000);
    };
  }, [mode, serverUrl, tenantId, sessionKey]);

  useEffect(() => {
    if (mode === 'cloud') {
      if (wsRef.current) {
        wsRef.current.close(1000, 'Switched to cloud mode');
        wsRef.current = null;
      }
      return;
    }

    if (!serverUrl || !tenantId || !sessionKey) {
      router.replace('/(auth)/login');
      return;
    }

    const ws = bffClient.connectChat(serverUrl, tenantId, sessionKey, {
      onToken: (token) => {
        hasReceivedMessage.current = true;
        appendToken(token);
      },
      onDone: () => finalizeMessage(),
      onError: (msg) => {
        if (!hasReceivedMessage.current) {
          setMode('cloud');
          setError('Server unreachable — switched to Cloud Mode');
        } else {
          setError(msg);
        }
      },
    });

    wsRef.current = ws;

    return () => {
      if (wsRef.current) wsRef.current.close(1000, 'Screen unmounted');
      voiceService.stopSpeaking();
    };
  }, [serverUrl, tenantId, sessionKey, mode, appendToken, finalizeMessage, router, setError, setMode]);

  useEffect(() => {
    if (messages.length > 0) {
      setTimeout(() => {
        flatListRef.current?.scrollToEnd({ animated: true });
      }, 100);
    }
  }, [messages.length, messages]);

  const handleSendMessage = async (customText?: string) => {
    const textToSend = customText || inputText.trim();
    if (!textToSend || isStreaming) return;
    
    if (!customText) setInputText('');
    clearError();
    
    if (mode === 'cloud') {
      const newUserMsg = {
        id: nanoid(),
        role: 'user' as const,
        content: textToSend,
        timestamp: Date.now(),
        isStreaming: false
      };
      
      const updatedMessagesForApi = [...messages, newUserMsg];

      addUserMessage(textToSend);
      startAssistantMessage();

      try {
        await cloudRouter.streamCloud(selectedProviderId, selectedModel, updatedMessagesForApi, {
          onToken: appendToken,
          onDone: () => {
            finalizeMessage();
            const currentMsgs = useChatStore.getState().messages;
            const lastMsg = currentMsgs[currentMsgs.length - 1];
            
            if (isConversationMode && lastMsg && lastMsg.role === 'assistant') {
              voiceService.speak(lastMsg.content, language === 'es' ? 'es-ES' : 'en-US', () => {
                if (useChatStore.getState().isConversationMode) {
                  toggleVoice();
                }
              });
            }
          },
          onError: setError,
        });
      } catch (e: any) {
        setError(e.message || 'Cloud stream failed');
      }
    } else {
      addUserMessage(textToSend);
      startAssistantMessage();
      try {
        bffClient.sendChatMessage(wsRef.current!, textToSend);
      } catch (e: any) {
        setError(e.message || 'Failed to send message');
      }
    }
  };

  const toggleVoice = async () => {
    if (isListening) {
      setIsListening(false);
    } else {
      const granted = await voiceService.requestPermissions();
      if (!granted) {
        setError('Microphone permission required');
        return;
      }
      setIsListening(true);
      
      // Mock listening period for the bridge
      setTimeout(() => {
        setIsListening(false);
      }, 5000); 
    }
  };

  const toggleConversationMode = () => {
    const newMode = !isConversationMode;
    setConversationMode(newMode);
    if (newMode) {
      Alert.alert(t.hands_free_on, t.hands_free_desc);
      if (!isListening) toggleVoice();
    } else {
      voiceService.stopSpeaking();
    }
  };

  const handlePickDocument = async () => {
    try {
      const result = await DocumentPicker.getDocumentAsync({
        type: '*/*',
        copyToCacheDirectory: true,
      });

      if (result.canceled) return;

      setIsUploading(true);
      const asset = result.assets[0];
      
      const formData = new FormData();
      formData.append('tenant_id', tenantId!);
      formData.append('session_key', sessionKey!);
      // @ts-ignore - FormData expects specific fields in React Native
      formData.append('file', {
        uri: asset.uri,
        name: asset.name,
        type: asset.mimeType || 'application/octet-stream',
      });

      const res = await bffClient.uploadFile(serverUrl!, formData);
      if (res.status === 'success') {
        Alert.alert('Success', 'File injected to workspace');
      } else {
        setError(res.message || 'Upload failed');
      }
    } catch (e: any) {
      setError(e.message || 'File picker error');
    } finally {
      setIsUploading(false);
    }
  };

  const renderMessage = ({ item }: { item: ChatMessage }) => (
    <ChatBubble message={item} language={language} />
  );

  return (
    <SafeAreaView style={styles.container}>
      <StatusBar barStyle="light-content" />
      
      <View style={styles.header}>
        <View>
          <Text style={styles.headerTitle}>{t.terminal_title}</Text>
          <TouchableOpacity onPress={() => setShowModeSelector(true)}>
            <Text style={styles.headerStatus}>
              ● {mode.toUpperCase()} · {selectedProviderId} ({selectedModel})
            </Text>
            {mode !== 'cloud' && (
              <View style={styles.sirenBadge}>
                <Ionicons name="volume-medium" size={10} color="#00E5CC" />
                <Text style={styles.sirenBadgeText}>{sirenEngine || 'Siren Standby'}</Text>
              </View>
            )}
          </TouchableOpacity>
        </View>
        <View style={styles.headerActions}>
          <TouchableOpacity 
            onPress={toggleConversationMode} 
            style={[styles.settingsButton, isConversationMode && styles.activeHandsFree]}
          >
            <Ionicons name={isConversationMode ? "infinite" : "mic-circle-outline"} size={20} color={isConversationMode ? "#00E5CC" : "#7C6FE0"} />
          </TouchableOpacity>
          <TouchableOpacity onPress={() => router.push('/(main)/settings')} style={styles.settingsButton}>
            <Ionicons name="settings-outline" size={18} color="#7C6FE0" />
          </TouchableOpacity>
          <TouchableOpacity onPress={() => logout()} style={styles.logoutButton}>
            <Text style={styles.logoutText}>EXIT</Text>
          </TouchableOpacity>
        </View>
      </View>

      {isProcessingTool && (
        <View style={styles.toolBanner}>
          <ActivityIndicator size="small" color="#7C6FE0" style={{ marginRight: 10 }} />
          <Text style={styles.toolText}>{t.bridge_accessing}</Text>
        </View>
      )}

      <TypedFlatList
        ref={flatListRef}
        data={messages}
        renderItem={renderMessage}
        keyExtractor={(item: ChatMessage) => item.id}
        contentContainerStyle={styles.listContent}
        ListEmptyComponent={
          <View style={styles.emptyContainer}>
            <Text style={styles.emptyText}>{t.status_secure}</Text>
          </View>
        }
      />

      {error && (
        <View style={styles.errorBanner}>
          <Text style={styles.errorText}>{error}</Text>
        </View>
      )}

      <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : 'height'} keyboardVerticalOffset={Platform.OS === 'ios' ? 0 : 20}>
        <View style={styles.inputContainer}>
          <TouchableOpacity style={styles.actionButton} onPress={handlePickDocument} disabled={isUploading}>
            {isUploading ? <ActivityIndicator size="small" color="#00E5CC" /> : <Ionicons name="add-circle-outline" size={24} color="#00E5CC" />}
          </TouchableOpacity>

          <VoiceButton isListening={isListening} onPress={toggleVoice} />
          
          <TextInput
            style={styles.input}
            value={inputText}
            onChangeText={setInputText}
            placeholder={isListening ? t.listening : t.input_placeholder}
            placeholderTextColor="#666"
            multiline
            editable={!isStreaming}
          />
          
          <TouchableOpacity
            style={[styles.sendButton, (!inputText.trim() || isStreaming) && styles.sendButtonDisabled]}
            onPress={() => handleSendMessage()}
            disabled={!inputText.trim() || isStreaming}
          >
            {isStreaming ? <ActivityIndicator size="small" color="#000" /> : <Ionicons name="send" size={18} color="#000" />}
          </TouchableOpacity>
        </View>
      </KeyboardAvoidingView>
      <ModeSelector visible={showModeSelector} onClose={() => setShowModeSelector(false)} />
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#050505' },
  header: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', padding: 16, borderBottomWidth: 1, borderBottomColor: '#1A1A1A' },
  headerTitle: { color: '#00E5CC', fontSize: 14, fontWeight: '900', letterSpacing: 2 },
  headerStatus: { color: '#666', fontSize: 10, marginTop: 2 },
  sirenBadge: { flexDirection: 'row', alignItems: 'center', backgroundColor: 'rgba(0, 229, 204, 0.05)', paddingHorizontal: 6, paddingVertical: 2, borderRadius: 4, marginTop: 4, alignSelf: 'flex-start', borderWidth: 0.5, borderColor: 'rgba(0, 229, 204, 0.2)' },
  sirenBadgeText: { color: '#00E5CC', fontSize: 8, fontWeight: 'bold', marginLeft: 4, textTransform: 'uppercase' },
  headerActions: { flexDirection: 'row', alignItems: 'center' },
  settingsButton: { padding: 8, marginRight: 8 },
  activeHandsFree: { backgroundColor: 'rgba(0, 229, 204, 0.1)', borderRadius: 20 },
  logoutButton: { padding: 8 },
  logoutText: { color: '#7C6FE0', fontSize: 12, fontWeight: 'bold' },
  toolBanner: { flexDirection: 'row', backgroundColor: '#0F0D1A', padding: 10, alignItems: 'center', justifyContent: 'center', borderBottomWidth: 1, borderBottomColor: '#7C6FE033' },
  toolText: { color: '#7C6FE0', fontSize: 10, fontWeight: 'bold', letterSpacing: 1 },
  listContent: { padding: 16, paddingBottom: 32 },
  emptyContainer: { flex: 1, alignItems: 'center', justifyContent: 'center', marginTop: 100 },
  emptyText: { color: '#1A1A1A', fontSize: 12, fontWeight: 'bold', letterSpacing: 3 },
  errorBanner: { backgroundColor: 'rgba(255, 0, 0, 0.1)', padding: 8, alignItems: 'center' },
  errorText: { color: '#FF4444', fontSize: 12 },
  inputContainer: { flexDirection: 'row', padding: 10, backgroundColor: '#0A0A0A', borderTopWidth: 1, borderTopColor: '#1A1A1A', alignItems: 'center' },
  actionButton: { padding: 8, position: 'relative' },
  input: { flex: 1, backgroundColor: '#111', borderRadius: 20, paddingHorizontal: 16, paddingVertical: 8, marginRight: 8, color: '#FFF', fontSize: 15, maxHeight: 100 },
  sendButton: { backgroundColor: '#00E5CC', width: 40, height: 40, borderRadius: 20, justifyContent: 'center', alignItems: 'center' },
  sendButtonDisabled: { backgroundColor: '#1A1A1A' },
});
