import React, { useEffect, useState, useCallback } from 'react';
import {
  View, Text, ScrollView, TouchableOpacity,
  TextInput, StyleSheet, Alert, SafeAreaView, Platform
} from 'react-native';
import { useRouter } from 'expo-router';
import { useAuthStore } from '@/stores/authStore';
import { useSettingsStore } from '@/stores/settingsStore';
import * as secureStorage from '@/services/secureStorage';
import { PROVIDERS } from '@/constants/providers';
import { TRANSLATIONS, Language } from '@/constants/i18n';

export default function SettingsScreen() {
  const router = useRouter();
  const { logout, mode, serverUrl } = useAuthStore();
  const {
    selectedProviderId,
    selectedModel,
    language,
    setProvider,
    setModel,
    setLanguage,
    loadSettings,
    refreshApiKeys,
    apiKeys: hasApiKeys,
    agentPersona,
    isPersonaConfigured,
    fetchAgentPersona
  } = useSettingsStore();

  const t = TRANSLATIONS[language];

  const [editingKey, setEditingKey] = useState<string | null>(null);
  const [keyInput, setKeyInput] = useState('');
  const [showCustomModel, setShowCustomModel] = useState(false);
  const [customModelInput, setCustomModelInput] = useState('');

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  useEffect(() => {
    if (mode === 'satellite') {
      fetchAgentPersona();
    }
  }, [mode, fetchAgentPersona]);

  const handleSaveKey = async (providerId: string) => {
    if (!keyInput.trim()) return;
    try {
      await secureStorage.saveApiKey(providerId, keyInput.trim());
      await refreshApiKeys();
      setEditingKey(null);
      setKeyInput('');
      Alert.alert('Success', 'API Key saved successfully');
    } catch (err) {
      Alert.alert('Error', 'Failed to save API Key');
    }
  };

  const handleRemoveKey = async (providerId: string) => {
    Alert.alert('Remove Key', 'Are you sure?', [
      { text: 'Cancel' },
      {
        text: 'Remove',
        style: 'destructive',
        onPress: async () => {
          await secureStorage.removeApiKey(providerId);
          await refreshApiKeys();
        }
      }
    ]);
  };

  const handleCustomModelSave = async () => {
    if (!customModelInput.trim()) return;
    await setModel(customModelInput.trim());
    setCustomModelInput('');
    setShowCustomModel(false);
  };

  const handleLogout = () => {
    Alert.alert('Logout', 'Disconnect and clear session?', [
      { text: 'Cancel', style: 'cancel' },
      {
        text: 'Logout',
        style: 'destructive',
        onPress: async () => {
          await logout();
          router.replace('/(auth)/login');
        }
      },
    ]);
  };

  const currentProvider = PROVIDERS.find(p => p.id === selectedProviderId) || PROVIDERS[0];

  return (
    <SafeAreaView style={styles.container}>
      <View style={styles.header}>
        <TouchableOpacity onPress={() => router.back()} style={styles.backBtn}>
          <Text style={styles.back}>←</Text>
        </TouchableOpacity>
        <Text style={styles.title}>{t.settings_title}</Text>
      </View>

      <ScrollView contentContainerStyle={styles.scrollContent}>

        {/* SECTION: LANGUAGE */}
        <View style={styles.sectionHeader}>
          <Text style={styles.sectionTitle}>{t.language}</Text>
        </View>
        <View style={styles.langGrid}>
          {(['en', 'es'] as Language[]).map((l) => (
            <TouchableOpacity
              key={l}
              style={[styles.langBtn, language === l && styles.langBtnActive]}
              onPress={() => setLanguage(l)}
            >
              <Text style={[styles.langText, language === l && styles.langTextActive]}>
                {l === 'en' ? 'ENGLISH' : 'ESPAÑOL'}
              </Text>
            </TouchableOpacity>
          ))}
        </View>

        <View style={styles.sectionHeader}>
          <Text style={styles.sectionTitle}>SATELLITE CONNECTION</Text>
        </View>
        <View style={styles.card}>
          <Text style={styles.cardLabel}>CONNECTED TO</Text>
          <Text style={styles.serverValue}>{serverUrl || 'None'}</Text>
          <TouchableOpacity 
            style={styles.relinkBtn}
            onPress={() => {
              Alert.alert('Re-link Server', 'This will logout and allow you to scan a new QR code.', [
                { text: 'Cancel', style: 'cancel' },
                { text: 'Continue', onPress: async () => { await logout(); router.replace('/(auth)/login'); } }
              ]);
            }}
          >
            <Text style={styles.relinkBtnText}>RE-LINK VIA QR SCAN</Text>
          </TouchableOpacity>
        </View>

        {/* SECTION: ACTIVE MODE */}
        <View style={styles.sectionHeader}>
          <Text style={styles.sectionTitle}>{t.active_mode}</Text>
        </View>
        <View style={styles.modeCard}>
          <Text style={styles.modeValue}>{mode.toUpperCase()}</Text>
          <Text style={styles.modeDesc}>Operating in {mode} neural link.</Text>
        </View>

        {/* SECTION: AGENT PERSONA (Satellite only) */}
        {mode === 'satellite' && (
          <View style={styles.sectionHeader}>
            <Text style={styles.sectionTitle}>IDENTIDAD DEL AGENTE</Text>
          </View>
        )}
        {mode === 'satellite' && (
          <View style={styles.card}>
            {isPersonaConfigured ? (
              <>
                <View style={styles.personaBadge}>
                  <Text style={styles.personaBadgeText}>✓ Persona configurada por el operador</Text>
                </View>
                <Text style={styles.personaPreview} numberOfLines={2}>
                  {agentPersona.slice(0, 120)}{agentPersona.length > 120 ? '...' : ''}
                </Text>
              </>
            ) : (
              <Text style={styles.personaEmpty}>Sin identidad personalizada — usando Aegis por defecto</Text>
            )}
          </View>
        )}

        {/* SECTION: CLOUD PROVIDERS */}
        <View style={styles.sectionHeader}>
          <Text style={styles.sectionTitle}>{t.cloud_providers}</Text>
        </View>

        <View style={styles.providerGrid}>
          {PROVIDERS.map((p) => (
            <TouchableOpacity
              key={p.id}
              style={[
                styles.providerCard,
                selectedProviderId === p.id && styles.providerCardActive
              ]}
              onPress={() => setProvider(p.id)}
            >
              <Text style={[
                styles.providerName,
                selectedProviderId === p.id && styles.providerNameActive
              ]}>{p.name}</Text>
              {hasApiKeys[p.id] && <View style={styles.keyIndicator} />}
            </TouchableOpacity>
          ))}
        </View>

        {/* SECTION: API KEY FOR SELECTED */}
        <View style={styles.card}>
          <Text style={styles.cardLabel}>API KEY: {currentProvider.name}</Text>
          {editingKey === selectedProviderId ? (
            <View style={styles.inputGroup}>
              <TextInput
                style={styles.input}
                value={keyInput}
                onChangeText={setKeyInput}
                placeholder="Paste key here..."
                placeholderTextColor="#444"
                secureTextEntry
                autoCapitalize="none"
              />
              <TouchableOpacity style={styles.actionBtnPrimary} onPress={() => handleSaveKey(selectedProviderId)}>
                <Text style={styles.actionBtnText}>SAVE</Text>
              </TouchableOpacity>
              <TouchableOpacity onPress={() => setEditingKey(null)}>
                <Text style={styles.cancelLink}>CANCEL</Text>
              </TouchableOpacity>
            </View>
          ) : (
            <View style={styles.keyStatusRow}>
              <Text style={hasApiKeys[selectedProviderId] ? styles.keyOk : styles.keyMissing}>
                {hasApiKeys[selectedProviderId] ? 'KEY CONFIGURED ✓' : 'NO KEY DETECTED'}
              </Text>
              <View style={styles.row}>
                <TouchableOpacity onPress={() => setEditingKey(selectedProviderId)}>
                  <Text style={styles.linkText}>{hasApiKeys[selectedProviderId] ? 'REPLACE' : 'ADD'}</Text>
                </TouchableOpacity>
                {hasApiKeys[selectedProviderId] && (
                  <TouchableOpacity onPress={() => handleRemoveKey(selectedProviderId)}>
                    <Text style={styles.removeLink}>REMOVE</Text>
                  </TouchableOpacity>
                )}
              </View>
            </View>
          )}
        </View>

        {/* SECTION: MODEL SELECTION */}
        <View style={styles.sectionHeader}>
          <Text style={styles.sectionTitle}>{t.model_selection}</Text>
          <TouchableOpacity onPress={() => setShowCustomModel(!showCustomModel)}>
            <Text style={styles.linkText}>{showCustomModel ? 'LIST' : 'CUSTOM ID'}</Text>
          </TouchableOpacity>
        </View>

        {showCustomModel ? (
          <View style={styles.card}>
            <Text style={styles.cardLabel}>ENTER CUSTOM MODEL ID</Text>
            <View style={styles.inputGroup}>
              <TextInput
                style={styles.input}
                value={customModelInput}
                onChangeText={setCustomModelInput}
                placeholder="e.g. gpt-4-turbo"
                placeholderTextColor="#444"
                autoCapitalize="none"
              />
              <TouchableOpacity style={styles.actionBtnPrimary} onPress={handleCustomModelSave}>
                <Text style={styles.actionBtnText}>SET</Text>
              </TouchableOpacity>
            </View>
          </View>
        ) : (
          <View style={styles.modelList}>
            {currentProvider.models.map((m) => (
              <TouchableOpacity
                key={m}
                style={[
                  styles.modelItem,
                  selectedModel === m && styles.modelItemActive
                ]}
                onPress={() => setModel(m)}
              >
                <Text style={[
                  styles.modelText,
                  selectedModel === m && styles.modelTextActive
                ]}>{m}</Text>
                {selectedModel === m && <View style={styles.activeDot} />}
              </TouchableOpacity>
            ))}
          </View>
        )}

        <TouchableOpacity style={styles.logoutBtn} onPress={handleLogout}>
          <Text style={styles.logoutText}>{t.terminate_session}</Text>
        </TouchableOpacity>

        <TouchableOpacity style={styles.oauthLink} onPress={() => router.push('/(main)/connected-accounts')}>
          <Text style={styles.oauthLinkText}>CUENTAS CONECTADAS</Text>
        </TouchableOpacity>

      </ScrollView>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#050505' },
  header: { flexDirection: 'row', alignItems: 'center', padding: 20, borderBottomWidth: 1, borderBottomColor: '#1A1A1A' },
  backBtn: { padding: 8, marginRight: 12 },
  back: { color: '#7C6FE0', fontSize: 24, fontWeight: '300' },
  title: { color: '#FFF', fontSize: 13, fontWeight: '900', letterSpacing: 2 },
  scrollContent: { padding: 20 },
  sectionHeader: { flexDirection: 'row', justifyContent: 'space-between', marginTop: 24, marginBottom: 12 },
  sectionTitle: { color: '#555', fontSize: 10, fontWeight: 'bold', letterSpacing: 1 },
  langGrid: { flexDirection: 'row', gap: 10, marginBottom: 10 },
  langBtn: { flex: 1, padding: 12, backgroundColor: '#0A0A0A', borderRadius: 8, borderWidth: 1, borderColor: '#1A1A1A', alignItems: 'center' },
  langBtnActive: { borderColor: '#7C6FE0', backgroundColor: '#0F0D1A' },
  langText: { color: '#444', fontSize: 11, fontWeight: 'bold' },
  langTextActive: { color: '#FFF' },
  modeCard: { backgroundColor: '#0A0A0A', padding: 16, borderRadius: 8, borderWidth: 1, borderColor: '#1A1A1A' },
  modeValue: { color: '#00E5CC', fontSize: 16, fontWeight: 'bold', letterSpacing: 1 },
  modeDesc: { color: '#444', fontSize: 11, marginTop: 4 },
  serverValue: { color: '#AAA', fontSize: 13, fontFamily: Platform.OS === 'ios' ? 'Menlo' : 'monospace', marginBottom: 12 },
  relinkBtn: { backgroundColor: 'rgba(124, 111, 224, 0.1)', padding: 12, borderRadius: 6, borderWidth: 1, borderColor: '#7C6FE033', alignItems: 'center' },
  relinkBtnText: { color: '#7C6FE0', fontSize: 10, fontWeight: 'bold', letterSpacing: 1 },
  providerGrid: { flexDirection: 'row', flexWrap: 'wrap', gap: 8, marginBottom: 16 },
  providerCard: { flex: 1, minWidth: '30%', backgroundColor: '#0A0A0A', padding: 12, borderRadius: 8, borderWidth: 1, borderColor: '#1A1A1A', alignItems: 'center', position: 'relative' },
  providerCardActive: { borderColor: '#7C6FE0', backgroundColor: '#0F0D1A' },
  providerName: { color: '#666', fontSize: 12, fontWeight: 'bold' },
  providerNameActive: { color: '#FFF' },
  keyIndicator: { position: 'absolute', top: 6, right: 6, width: 4, height: 4, borderRadius: 2, backgroundColor: '#00E5CC' },
  card: { backgroundColor: '#0A0A0A', padding: 16, borderRadius: 8, borderWidth: 1, borderColor: '#1A1A1A' },
  cardLabel: { color: '#555', fontSize: 9, fontWeight: 'bold', marginBottom: 12, textTransform: 'uppercase' },
  keyStatusRow: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center' },
  keyOk: { color: '#00E5CC', fontSize: 12, fontWeight: 'bold' },
  keyMissing: { color: '#FF4444', fontSize: 12, fontWeight: 'bold' },
  row: { flexDirection: 'row', gap: 16 },
  linkText: { color: '#7C6FE0', fontSize: 11, fontWeight: 'bold' },
  removeLink: { color: '#FF4444', fontSize: 11, fontWeight: 'bold' },
  cancelLink: { color: '#555', fontSize: 11, fontWeight: 'bold', marginLeft: 10 },
  inputGroup: { flexDirection: 'row', alignItems: 'center', gap: 10 },
  input: { flex: 1, backgroundColor: '#111', borderWidth: 1, borderColor: '#333', borderRadius: 6, color: '#FFF', padding: 10, fontSize: 14 },
  actionBtnPrimary: { backgroundColor: '#7C6FE0', paddingHorizontal: 15, paddingVertical: 10, borderRadius: 6 },
  actionBtnText: { color: '#FFF', fontWeight: 'bold', fontSize: 11 },
  modelList: { backgroundColor: '#0A0A0A', borderRadius: 8, borderWidth: 1, borderColor: '#1A1A1A', overflow: 'hidden' },
  modelItem: { padding: 14, flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', borderBottomWidth: 1, borderBottomColor: '#1A1A1A' },
  modelItemActive: { backgroundColor: '#111' },
  modelText: { color: '#444', fontSize: 13 },
  modelTextActive: { color: '#00E5CC', fontWeight: 'bold' },
  activeDot: { width: 6, height: 6, borderRadius: 3, backgroundColor: '#00E5CC' },
  logoutBtn: { marginTop: 40, marginBottom: 40, padding: 16, borderRadius: 8, borderWidth: 1, borderColor: '#FF444433', alignItems: 'center' },
  logoutText: { color: '#FF4444', fontWeight: 'bold', fontSize: 11, letterSpacing: 2 },
  personaBadge: { flexDirection: 'row', alignItems: 'center', marginBottom: 8 },
  personaBadgeText: { color: '#00E5CC', fontSize: 11, fontWeight: 'bold' },
  personaPreview: { color: '#888', fontSize: 12, lineHeight: 18 },
  personaEmpty: { color: '#444', fontSize: 12, fontStyle: 'italic' },
  oauthLink: { marginTop: 10, padding: 16, borderRadius: 8, borderWidth: 1, borderColor: '#7C6FE033', backgroundColor: '#0A0A0A', alignItems: 'center' },
  oauthLinkText: { color: '#7C6FE0', fontWeight: 'bold', fontSize: 11, letterSpacing: 2 }
});
