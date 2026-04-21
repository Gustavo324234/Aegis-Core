import React, { useState, useEffect } from 'react';
import {
  View, Text, TouchableOpacity, StyleSheet,
  ActivityIndicator, Alert, ScrollView
} from 'react-native';
import { useRouter } from 'expo-router';
import { useAuthStore } from '@/stores/authStore';
import {
  connectProvider, saveTokensToServer,
  getOAuthStatus, disconnectProvider,
  ConnectedAccountStatus
} from '@/services/oauthService';

const PROVIDERS = [
  {
    id: 'google' as const,
    name: 'Google',
    description: 'YouTube Music · Calendar · Drive · Gmail',
    color: '#4285F4',
  },
  {
    id: 'spotify' as const,
    name: 'Spotify',
    description: 'Reproducción de música · Playlists',
    color: '#1DB954',
  },
];

export default function ConnectedAccountsScreen() {
  const router = useRouter();
  const { serverUrl, tenantId, sessionKey } = useAuthStore();
  const [status, setStatus] = useState<Record<string, ConnectedAccountStatus>>({});
  const [loading, setLoading] = useState(true);
  const [connecting, setConnecting] = useState<string | null>(null);

  const fetchStatus = async () => {
    if (!serverUrl || !tenantId || !sessionKey) return;
    try {
      const s = await getOAuthStatus(serverUrl, tenantId, sessionKey);
      setStatus(s);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { fetchStatus(); }, []);

  const handleConnect = async (provider: 'google' | 'spotify') => {
    if (!serverUrl || !tenantId || !sessionKey) return;
    setConnecting(provider);
    try {
      const tokens = await connectProvider(provider);
      await saveTokensToServer(serverUrl, tenantId, sessionKey, tokens);
      await fetchStatus();
      Alert.alert('✓ Conectado', `Tu cuenta de ${provider === 'google' ? 'Google' : 'Spotify'} fue vinculada exitosamente.`);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : 'Error desconocido';
      if (!msg.includes('Cancelado')) {
        Alert.alert('Error', msg);
      }
    } finally {
      setConnecting(null);
    }
  };

  const handleDisconnect = (provider: string) => {
    Alert.alert(
      'Desconectar',
      `¿Querés desconectar tu cuenta de ${provider}?`,
      [
        { text: 'Cancelar', style: 'cancel' },
        {
          text: 'Desconectar',
          style: 'destructive',
          onPress: async () => {
            if (!serverUrl || !tenantId || !sessionKey) return;
            await disconnectProvider(serverUrl, tenantId, sessionKey, provider);
            await fetchStatus();
          },
        },
      ]
    );
  };

  if (loading) {
    return (
      <View style={styles.center}>
        <ActivityIndicator color="#00F2FE" />
      </View>
    );
  }

  return (
    <ScrollView style={styles.container}>
      <View style={styles.header}>
        <TouchableOpacity onPress={() => router.back()} style={styles.backBtn}>
          <Text style={styles.back}>←</Text>
        </TouchableOpacity>
        <Text style={styles.headerTitle}>CUENTAS CONECTADAS</Text>
      </View>
      
      <Text style={styles.subtitle}>
        Conectá tus cuentas para que Aegis pueda reproducir música,
        consultar tu calendario, archivos y más.
      </Text>

      {PROVIDERS.map((provider) => {
        const s = status[provider.id];
        const isConnected = s?.connected ?? false;
        const isBusy = connecting === provider.id;

        return (
          <View key={provider.id} style={[
            styles.card,
            isConnected && { borderColor: provider.color + '50' }
          ]}>
            <View style={styles.cardInfo}>
              <Text style={styles.providerName}>{provider.name}</Text>
              <Text style={styles.providerDesc}>{provider.description}</Text>
              {isConnected && s?.email && (
                <Text style={[styles.email, { color: provider.color }]}>
                  {s.email}
                </Text>
              )}
            </View>

            {isConnected ? (
              <TouchableOpacity
                onPress={() => handleDisconnect(provider.id)}
                style={styles.disconnectBtn}
              >
                <Text style={styles.disconnectText}>Desconectar</Text>
              </TouchableOpacity>
            ) : (
              <TouchableOpacity
                onPress={() => handleConnect(provider.id)}
                disabled={!!connecting}
                style={[styles.connectBtn, { backgroundColor: provider.color }]}
              >
                {isBusy
                  ? <ActivityIndicator color="#fff" size="small" />
                  : <Text style={styles.connectText}>Conectar</Text>
                }
              </TouchableOpacity>
            )}
          </View>
        );
      })}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#000', padding: 20 },
  center: { flex: 1, justifyContent: 'center', alignItems: 'center' },
  header: { flexDirection: 'row', alignItems: 'center', marginBottom: 20 },
  backBtn: { padding: 8, marginRight: 12 },
  back: { color: '#7C6FE0', fontSize: 24, fontWeight: '300' },
  headerTitle: { color: '#FFF', fontSize: 13, fontWeight: '900', letterSpacing: 2 },
  subtitle: { fontSize: 13, color: 'rgba(255,255,255,0.4)', marginBottom: 24, lineHeight: 18 },
  card: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    backgroundColor: 'rgba(255,255,255,0.05)',
    borderRadius: 16, borderWidth: 1, borderColor: 'rgba(255,255,255,0.1)',
    padding: 16, marginBottom: 12,
  },
  cardInfo: { flex: 1, marginRight: 12 },
  providerName: { fontSize: 15, fontWeight: 'bold', color: '#fff' },
  providerDesc: { fontSize: 11, color: 'rgba(255,255,255,0.4)', marginTop: 2 },
  email: { fontSize: 11, marginTop: 4, fontWeight: '600' },
  connectBtn: { paddingHorizontal: 16, paddingVertical: 8, borderRadius: 8, minWidth: 90, alignItems: 'center' },
  connectText: { color: '#fff', fontSize: 13, fontWeight: 'bold' },
  disconnectBtn: { paddingHorizontal: 12, paddingVertical: 8, borderRadius: 8, borderWidth: 1, borderColor: 'rgba(255,255,255,0.2)' },
  disconnectText: { color: 'rgba(255,255,255,0.5)', fontSize: 12 },
});