import React from 'react';
import { View, Text, TouchableOpacity, StyleSheet } from 'react-native';
import { Ionicons } from '@expo/vector-icons';
import * as voiceService from '@/services/voiceService';

interface ChatBubbleProps {
  message: {
    role: 'user' | 'assistant';
    content: string;
    isStreaming: boolean;
    timestamp: number;
  };
  language: 'en' | 'es';
}

export default function ChatBubble({ message, language }: ChatBubbleProps) {
  const isUser = message.role === 'user';

  return (
    <View style={[styles.bubble, isUser ? styles.userBubble : styles.assistantBubble]}>
      <View style={styles.header}>
        <Text style={[styles.roleText, isUser ? styles.userRole : styles.assistantRole]}>
          {isUser ? 'USER' : 'AEGIS'}
        </Text>
        {!isUser && !message.isStreaming && (
          <TouchableOpacity onPress={() => voiceService.speak(message.content, language === 'es' ? 'es-ES' : 'en-US')}>
            <Ionicons name="volume-medium-outline" size={16} color="#7C6FE0" />
          </TouchableOpacity>
        )}
      </View>
      <Text style={[styles.messageText, isUser ? styles.userText : styles.assistantText]}>
        {message.content}
        {message.isStreaming && <Text style={styles.cursor}> ▍</Text>}
      </Text>
      <Text style={styles.timestamp}>
        {new Date(message.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  bubble: { maxWidth: '85%', padding: 12, borderRadius: 12, marginBottom: 16 },
  header: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 },
  roleText: { fontSize: 9, fontWeight: 'bold', letterSpacing: 1 },
  userRole: { color: '#666' },
  assistantRole: { color: '#7C6FE0' },
  userBubble: { alignSelf: 'flex-end', backgroundColor: '#1A1A1A', borderBottomRightRadius: 2 },
  assistantBubble: { alignSelf: 'flex-start', backgroundColor: '#0A0A0A', borderWidth: 1, borderColor: '#1A1A1A', borderBottomLeftRadius: 2 },
  messageText: { fontSize: 15, lineHeight: 22 },
  userText: { color: '#FFF' },
  assistantText: { color: '#EEE' },
  cursor: { color: '#00E5CC', fontWeight: 'bold' },
  timestamp: { fontSize: 9, color: '#444', marginTop: 6, textAlign: 'right' },
});
