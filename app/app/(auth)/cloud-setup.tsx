import React, { useState, useEffect } from 'react';
import {
  View,
  Text,
  TextInput,
  TouchableOpacity,
  StyleSheet,
  ScrollView,
  SafeAreaView,
  Alert,
} from 'react-native';
import { useRouter } from 'expo-router';
import { useSettingsStore } from '@/stores/settingsStore';
import { useAuthStore } from '@/stores/authStore';
import { PROVIDERS } from '@/constants/providers';
import * as secureStorage from '@/services/secureStorage';

export default function CloudSetupScreen() {
  const router = useRouter();
  const { setMode } = useAuthStore();
  const { 
    selectedProviderId, 
    selectedModel, 
    apiKeys,
    loadSettings, 
    setProvider, 
    setModel,
    refreshApiKeys
  } = useSettingsStore();

  const [apiKeyInput, setApiKeyInput] = useState('');
  const [customModelInput, setCustomModelInput] = useState('');
  const [showCustomModel, setShowCustomModel] = useState(false);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  const handleSaveApiKey = async () => {
    if (!apiKeyInput.trim()) return;
    
    try {
      await secureStorage.saveApiKey(selectedProviderId, apiKeyInput.trim());
      setApiKeyInput('');
      await refreshApiKeys();
      Alert.alert('Success', `API Key for ${selectedProviderId} saved securely.`);
    } catch (err: unknown) {
      console.error('Save API key error:', err);
      Alert.alert('Error', 'Failed to save API key');
    }
  };

  const handleRemoveApiKey = async () => {
    Alert.alert(
      'Remove API Key',
      `Are you sure you want to delete the key for ${selectedProviderId}?`,
      [
        { text: 'Cancel', style: 'cancel' },
        { 
          text: 'Delete', 
          style: 'destructive',
          onPress: async () => {
            await secureStorage.removeApiKey(selectedProviderId);
            await refreshApiKeys();
          }
        }
      ]
    );
  };

  const handleCustomModelSave = async () => {
    if (!customModelInput.trim()) return;
    await setModel(customModelInput.trim());
    setCustomModelInput('');
    setShowCustomModel(false);
  };

  const handleStartChatting = async () => {
    // Check if at least one API key exists
    const hasKeys = Object.values(apiKeys).some(key => !!key);
    if (!hasKeys) {
      Alert.alert('Missing API Key', 'Please save at least one API key to use Cloud Mode.');
      return;
    }

    await setMode('cloud');
    router.replace('/(main)/chat');
  };

  const currentProvider = PROVIDERS.find(p => p.id === selectedProviderId) || PROVIDERS[0];
  const isModelSelected = (m: string) => selectedModel === m;

  return (
    <SafeAreaView style={styles.container}>
      <ScrollView contentContainerStyle={styles.scrollContent}>
        <Text style={styles.title}>CLOUD MODE SETUP</Text>
        <Text style={styles.description}>
          Configure your direct AI provider keys. These are stored locally in your device's secure enclave.
        </Text>

        <View style={styles.section}>
          <Text style={styles.label}>SELECT PROVIDER</Text>
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
                ]}>
                  {p.name}
                </Text>
                {apiKeys[p.id] && <Text style={styles.keyIndicator}>KEY ✓</Text>}
              </TouchableOpacity>
            ))}
          </View>
        </View>

        <View style={styles.section}>
          <Text style={styles.label}>API KEY FOR {currentProvider.name}</Text>
          <View style={styles.keyActionContainer}>
            <TextInput
              style={styles.input}
              value={apiKeyInput}
              onChangeText={setApiKeyInput}
              placeholder={apiKeys[selectedProviderId] ? "••••••••••••••••" : "Paste your key here"}
              placeholderTextColor="#444"
              secureTextEntry
              autoCapitalize="none"
              autoCorrect={false}
            />
            <TouchableOpacity 
              style={styles.saveButton}
              onPress={handleSaveApiKey}
            >
              <Text style={styles.saveButtonText}>SAVE</Text>
            </TouchableOpacity>
          </View>
          {apiKeys[selectedProviderId] && (
            <TouchableOpacity onPress={handleRemoveApiKey}>
              <Text style={styles.removeText}>REMOVE EXISTING KEY</Text>
            </TouchableOpacity>
          )}
        </View>

        <View style={styles.section}>
          <View style={styles.sectionHeader}>
            <Text style={styles.label}>DEFAULT MODEL</Text>
            <TouchableOpacity onPress={() => setShowCustomModel(!showCustomModel)}>
              <Text style={styles.customLink}>{showCustomModel ? 'CANCEL' : 'CUSTOM MODEL'}</Text>
            </TouchableOpacity>
          </View>
          
          {showCustomModel ? (
            <View style={styles.keyActionContainer}>
              <TextInput
                style={styles.input}
                value={customModelInput}
                onChangeText={setCustomModelInput}
                placeholder="e.g. meta-llama/llama-3.1-405b"
                placeholderTextColor="#444"
                autoCapitalize="none"
              />
              <TouchableOpacity 
                style={styles.saveButton}
                onPress={handleCustomModelSave}
              >
                <Text style={styles.saveButtonText}>SET</Text>
              </TouchableOpacity>
            </View>
          ) : (
            <View style={styles.modelList}>
              {currentProvider.models.map((m) => (
                <TouchableOpacity
                  key={m}
                  style={[
                    styles.modelItem,
                    isModelSelected(m) && styles.modelItemActive
                  ]}
                  onPress={() => setModel(m)}
                >
                  <Text style={[
                    styles.modelText,
                    isModelSelected(m) && styles.modelTextActive
                  ]}>
                    {m}
                  </Text>
                  {isModelSelected(m) && <View style={styles.selectedDot} />}
                </TouchableOpacity>
              ))}
            </View>
          )}
          
          {!showCustomModel && !currentProvider.models.includes(selectedModel) && (
            <View style={styles.customModelIndicator}>
              <Text style={styles.customModelLabel}>CURRENT CUSTOM MODEL:</Text>
              <Text style={styles.customModelValue}>{selectedModel}</Text>
            </View>
          )}
        </View>

        <TouchableOpacity 
          style={styles.finishButton}
          onPress={handleStartChatting}
        >
          <Text style={styles.finishButtonText}>START CHATTING</Text>
        </TouchableOpacity>

        <TouchableOpacity 
          style={styles.backButton}
          onPress={() => router.back()}
        >
          <Text style={styles.backButtonText}>BACK TO LOGIN</Text>
        </TouchableOpacity>
      </ScrollView>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#050505' },
  scrollContent: { padding: 24 },
  title: { color: '#7C6FE0', fontSize: 20, fontWeight: '900', letterSpacing: 2, marginBottom: 12 },
  description: { color: '#666', fontSize: 13, lineHeight: 20, marginBottom: 32 },
  section: { marginBottom: 32 },
  sectionHeader: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 },
  label: { color: '#AAA', fontSize: 11, fontWeight: 'bold', letterSpacing: 1 },
  customLink: { color: '#7C6FE0', fontSize: 10, fontWeight: 'bold' },
  providerGrid: { flexDirection: 'row', flexWrap: 'wrap', marginHorizontal: -4, marginTop: 16 },
  providerCard: { width: '48%', backgroundColor: '#0A0A0A', borderWidth: 1, borderColor: '#1A1A1A', borderRadius: 8, padding: 16, margin: '1%', alignItems: 'center' },
  providerCardActive: { borderColor: '#7C6FE0', backgroundColor: '#0F0D1A' },
  providerName: { color: '#666', fontSize: 14, fontWeight: '600' },
  providerNameActive: { color: '#FFF' },
  keyIndicator: { color: '#00E5CC', fontSize: 9, fontWeight: 'bold', marginTop: 4 },
  keyActionContainer: { flexDirection: 'row' },
  input: { flex: 1, backgroundColor: '#111', borderWidth: 1, borderColor: '#333', borderRadius: 8, color: '#FFF', padding: 14, fontSize: 14, marginRight: 8 },
  saveButton: { backgroundColor: '#7C6FE0', paddingHorizontal: 20, borderRadius: 8, justifyContent: 'center' },
  saveButtonText: { color: '#FFF', fontWeight: 'bold', fontSize: 12 },
  removeText: { color: '#FF4444', fontSize: 11, marginTop: 12, textAlign: 'center' },
  modelList: { backgroundColor: '#0A0A0A', borderRadius: 8, borderWidth: 1, borderColor: '#1A1A1A', overflow: 'hidden' },
  modelItem: { padding: 16, flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', borderBottomWidth: 1, borderBottomColor: '#1A1A1A' },
  modelItemActive: { backgroundColor: '#111' },
  modelText: { color: '#444', fontSize: 14 },
  modelTextActive: { color: '#00E5CC', fontWeight: 'bold' },
  selectedDot: { width: 6, height: 6, borderRadius: 3, backgroundColor: '#00E5CC' },
  customModelIndicator: { marginTop: 12, padding: 12, backgroundColor: '#0F0D1A', borderRadius: 8, borderWidth: 1, borderColor: '#7C6FE033' },
  customModelLabel: { color: '#666', fontSize: 9, fontWeight: 'bold', marginBottom: 4 },
  customModelValue: { color: '#7C6FE0', fontSize: 13, fontWeight: '600' },
  finishButton: { backgroundColor: '#00E5CC', padding: 18, borderRadius: 8, alignItems: 'center', marginTop: 16 },
  finishButtonText: { color: '#000', fontWeight: '900', fontSize: 14 },
  backButton: { padding: 16, alignItems: 'center', marginTop: 8 },
  backButtonText: { color: '#666', fontSize: 12 },
});
