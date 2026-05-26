import React, { useState, useEffect } from 'react';
import {
  View,
  Text,
  TouchableOpacity,
  StyleSheet,
  ActivityIndicator,
  Alert,
  ScrollView,
  Platform
} from 'react-native';
import { useRouter } from 'expo-router';
import { useAuthStore } from '@/stores/authStore';
import { getOAuthStatus, ConnectedAccountStatus } from '@/services/oauthService';
import * as WebBrowser from 'expo-web-browser';
import { Ionicons, MaterialCommunityIcons } from '@expo/vector-icons';

const PROVIDERS = [
  {
    id: 'google' as const,
    name: 'Google',
    description: 'YouTube Music · Calendar · Drive · Gmail',
    color: '#4285F4',
    icon: 'google' as const,
  },
  {
    id: 'spotify' as const,
    name: 'Spotify',
    description: 'Reproducción de música · Playlists',
    color: '#1DB954',
    icon: 'spotify' as const,
  },
];

export default function ConnectedAccountsScreen() {
  const router = useRouter();
  const { serverUrl, tenantId, sessionKey } = useAuthStore();
  const [status, setStatus] = useState<Record<string, ConnectedAccountStatus>>({});
  const [loading, setLoading] = useState(true);

  const fetchStatus = async () => {
    if (!serverUrl || !tenantId || !sessionKey) return;
    try {
      const s = await getOAuthStatus(serverUrl, tenantId, sessionKey);
      setStatus(s);
    } catch (e) {
      console.warn('Failed to fetch OAuth status:', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchStatus();
  }, []);

  const handleOpenWebDashboard = async () => {
    if (!serverUrl) return;
    try {
      // Aegis server stores web assets in / settings tab is at server root
      await WebBrowser.openBrowserAsync(serverUrl);
    } catch (e) {
      Alert.alert('Error', 'No se pudo abrir el navegador del sistema.');
    }
  };

  if (loading) {
    return (
      <View style={styles.center}>
        <ActivityIndicator color="#00E5CC" />
      </View>
    );
  }

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.scrollContent}>
      <View style={styles.header}>
        <TouchableOpacity onPress={() => router.back()} style={styles.backBtn}>
          <Text style={styles.back}>←</Text>
        </TouchableOpacity>
        <Text style={styles.headerTitle}>VINCULAR SERVICIOS</Text>
      </View>
      
      {/* Informative Banner */}
      <View style={styles.infoBanner}>
        <Ionicons name="shield-checkmark-outline" size={24} color="#00E5CC" />
        <View style={styles.infoBannerTextContainer}>
          <Text style={styles.infoBannerTitle}>Vinculación Segura (Citadel)</Text>
          <Text style={styles.infoBannerDesc}>
            Para cumplir con las normas de seguridad y redireccionamiento de tokens, 
            todas las vinculaciones de cuentas deben gestionarse directamente a través del portal web de Aegis OS.
          </Text>
        </View>
      </View>

      <TouchableOpacity style={styles.primaryWebBtn} onPress={handleOpenWebDashboard}>
        <Ionicons name="open-outline" size={18} color="#000" />
        <Text style={styles.primaryWebBtnText}>ABRIR PANEL DE CONFIGURACIÓN WEB</Text>
      </TouchableOpacity>

      <Text style={styles.sectionLabel}>ESTADO DE LAS INTEGRACIONES</Text>

      {PROVIDERS.map((provider) => {
        const s = status[provider.id];
        const isConnected = s?.connected ?? false;

        return (
          <View key={provider.id} style={[
            styles.card,
            isConnected && { borderColor: provider.color + '40', backgroundColor: 'rgba(255,255,255,0.02)' }
          ]}>
            <View style={styles.cardInfo}>
              <View style={styles.providerHeader}>
                <MaterialCommunityIcons name={provider.icon} size={20} color={provider.color} style={{ marginRight: 8 }} />
                <Text style={styles.providerName}>{provider.name}</Text>
              </View>
              <Text style={styles.providerDesc}>{provider.description}</Text>
              {isConnected && s?.email && (
                <Text style={[styles.email, { color: provider.color }]}>
                  ● Conectado como: {s.email}
                </Text>
              )}
            </View>

            <View style={styles.statusBadgeContainer}>
              {isConnected ? (
                <View style={[styles.statusBadge, { borderColor: provider.color + '50' }]}>
                  <Text style={[styles.statusBadgeText, { color: provider.color }]}>ACTIVO</Text>
                </View>
              ) : (
                <TouchableOpacity onPress={handleOpenWebDashboard} style={styles.configureBtn}>
                  <Text style={styles.configureBtnText}>CONFIGURAR</Text>
                </TouchableOpacity>
              )}
            </View>
          </View>
        );
      })}
      
      <Text style={styles.footerInfo}>
        Una vez que vincules tu cuenta de Google o Spotify en la página web, el estado se sincronizará de forma automática en este terminal móvil.
      </Text>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#050505' },
  scrollContent: { padding: 24, paddingBottom: 48 },
  center: { flex: 1, backgroundColor: '#050505', justifyContent: 'center', alignItems: 'center' },
  header: { flexDirection: 'row', alignItems: 'center', marginBottom: 24 },
  backBtn: { padding: 8, marginRight: 12 },
  back: { color: '#7C6FE0', fontSize: 24, fontWeight: '300' },
  headerTitle: { color: '#FFF', fontSize: 13, fontWeight: '900', letterSpacing: 2 },
  infoBanner: { flexDirection: 'row', backgroundColor: '#0F0D1A', padding: 16, borderRadius: 12, borderWidth: 1, borderColor: '#7C6FE033', marginBottom: 24, gap: 14 },
  infoBannerTextContainer: { flex: 1 },
  infoBannerTitle: { color: '#00E5CC', fontSize: 13, fontWeight: 'bold', marginBottom: 4 },
  infoBannerDesc: { fontSize: 12, color: 'rgba(255,255,255,0.6)', lineHeight: 18 },
  primaryWebBtn: { flexDirection: 'row', backgroundColor: '#00E5CC', padding: 16, borderRadius: 8, alignItems: 'center', justifyContent: 'center', gap: 8, marginBottom: 32 },
  primaryWebBtnText: { color: '#000', fontWeight: '900', fontSize: 12, letterSpacing: 1 },
  sectionLabel: { color: '#555', fontSize: 10, fontWeight: 'bold', letterSpacing: 1, marginBottom: 16 },
  card: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    backgroundColor: '#0A0A0A',
    borderRadius: 12, borderWidth: 1, borderColor: '#1A1A1A',
    padding: 16, marginBottom: 12,
  },
  cardInfo: { flex: 1, marginRight: 12 },
  providerHeader: { flexDirection: 'row', alignItems: 'center', marginBottom: 6 },
  providerName: { fontSize: 15, fontWeight: 'bold', color: '#fff' },
  providerDesc: { fontSize: 11, color: 'rgba(255,255,255,0.4)', lineHeight: 16 },
  email: { fontSize: 11, marginTop: 8, fontWeight: '600' },
  statusBadgeContainer: { marginLeft: 8 },
  statusBadge: { paddingHorizontal: 12, paddingVertical: 6, borderRadius: 20, borderWidth: 1, backgroundColor: 'rgba(255,255,255,0.02)' },
  statusBadgeText: { fontSize: 10, fontWeight: 'bold', letterSpacing: 1 },
  configureBtn: { paddingHorizontal: 12, paddingVertical: 8, borderRadius: 6, borderWidth: 1, borderColor: '#7C6FE050', backgroundColor: 'rgba(124, 111, 224, 0.05)' },
  configureBtnText: { color: '#7C6FE0', fontSize: 10, fontWeight: 'bold', letterSpacing: 1 },
  footerInfo: { color: '#333', fontSize: 11, textAlign: 'center', marginTop: 32, lineHeight: 18 },
});