import React, { useState, useEffect } from 'react';
import {
  View,
  Text,
  TextInput,
  TouchableOpacity,
  StyleSheet,
  Image,
  ActivityIndicator,
  KeyboardAvoidingView,
  Platform,
  ScrollView,
  Alert,
} from 'react-native';
import { useRouter } from 'expo-router';
import { useAuthStore } from '@/stores/authStore';
import * as bffClient from '@/services/bffClient';
import * as secureStorage from '@/services/secureStorage';
import { CameraView, useCameraPermissions } from 'expo-camera';
import { Ionicons } from '@expo/vector-icons';

export default function LoginScreen() {
  const router = useRouter();
  const { loginSuccess } = useAuthStore();

  const [serverUrl, setServerUrl] = useState('http://');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [permission, requestPermission] = useCameraPermissions();
  const [showScanner, setShowScanner] = useState(false);

  useEffect(() => {
    // Load last used server URL
    secureStorage.getServerUrl().then((url) => {
      if (url) setServerUrl(url);
    });
  }, []);

  const handleLogin = async () => {
    if (!serverUrl || !email || !password) {
      Alert.alert('Error', 'Please fill in all fields');
      return;
    }

    setIsLoading(true);
    try {
      const response = await bffClient.login(serverUrl, email, password);
      
      await loginSuccess({
        sessionKey: response.session_key,
        tenantId: response.tenant_id,
        serverUrl: serverUrl,
      });

      router.replace('/(main)/chat');
    } catch (error: any) {
      console.error('Login error:', error);
      const message = error.message === 'AUTH_FAILURE' 
        ? 'Invalid email or password' 
        : `Connection error: ${error.message}`;
      Alert.alert('Login Failed', message);
    } finally {
      setIsLoading(false);
    }
  };

  const handleScanPress = async () => {
    if (!permission?.granted) {
      const res = await requestPermission();
      if (!res.granted) {
        Alert.alert('Permission required', 'Camera access is needed to scan the QR code');
        return;
      }
    }
    setShowScanner(true);
  };

  const handleBarCodeScanned = ({ data }: { data: string }) => {
    try {
      // Validate it's a URL
      if (data.startsWith('http')) {
        const url = new URL(data);
        setServerUrl(url.origin);
        setShowScanner(false);
      } else {
        Alert.alert('Invalid QR', 'The scanned code is not a valid URL');
      }
    } catch (e) {
      Alert.alert('Invalid QR', 'The scanned code is not a valid URL');
    }
  };

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
      style={styles.container}
    >
      <ScrollView contentContainerStyle={styles.scrollContent}>
        <View style={styles.header}>
          <Image
            source={require('../../Logo/Logoconnombre.png')}
            style={styles.logo}
            resizeMode="contain"
          />
          <Text style={styles.subtitle}>SATELLITE TERMINAL</Text>
        </View>

        <View style={styles.form}>
          <Text style={styles.label}>Server URL</Text>
          <View style={styles.inputWithAction}>
            <TextInput
              style={[styles.input, { flex: 1, marginBottom: 0 }]}
              value={serverUrl}
              onChangeText={setServerUrl}
              placeholder="http://192.168.1.x:8000"
              placeholderTextColor="#666"
              autoCapitalize="none"
              autoCorrect={false}
              keyboardType="url"
            />
            <TouchableOpacity 
              onPress={handleScanPress} 
              style={styles.qrButton}
              activeOpacity={0.7}
            >
              <Ionicons name="qr-code-outline" size={22} color="#00E5CC" />
            </TouchableOpacity>
          </View>

          <Text style={styles.label}>Email / Tenant ID</Text>
          <TextInput
            style={styles.input}
            value={email}
            onChangeText={setEmail}
            placeholder="admin@aegis"
            placeholderTextColor="#666"
            autoCapitalize="none"
            autoCorrect={false}
            keyboardType="email-address"
          />

          <Text style={styles.label}>Password</Text>
          <View style={styles.passwordContainer}>
            <TextInput
              style={[styles.input, { flex: 1, marginBottom: 0 }]}
              value={password}
              onChangeText={setPassword}
              placeholder="••••••••"
              placeholderTextColor="#666"
              secureTextEntry={!showPassword}
              autoCapitalize="none"
            />
            <TouchableOpacity 
              onPress={() => setShowPassword(!showPassword)}
              style={styles.toggle}
            >
              <Text style={styles.toggleText}>{showPassword ? 'HIDE' : 'SHOW'}</Text>
            </TouchableOpacity>
          </View>

          <TouchableOpacity
            style={[styles.button, isLoading && styles.buttonDisabled]}
            onPress={handleLogin}
            disabled={isLoading}
          >
            {isLoading ? (
              <ActivityIndicator color="#000" />
            ) : (
              <Text style={styles.buttonText}>CONNECT TO CITADEL</Text>
            )}
          </TouchableOpacity>

          <TouchableOpacity
            style={styles.secondaryButton}
            onPress={() => router.push('/(auth)/cloud-setup')}
          >
            <Text style={styles.secondaryButtonText}>USE CLOUD MODE (API KEYS)</Text>
          </TouchableOpacity>
        </View>

        <Text style={styles.footer}>AEGIS OS v2.3 • SECURE ENCLAVE</Text>
      </ScrollView>

      {showScanner && (
        <View style={styles.scannerOverlay}>
          <CameraView
            style={StyleSheet.absoluteFill}
            onBarcodeScanned={handleBarCodeScanned}
            barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
          />
          <View style={styles.scannerHeader}>
            <Text style={styles.scannerTitle}>SCAN CONNECTION QR</Text>
          </View>
          <View style={styles.scannerFrame}>
            <View style={styles.scannerCornerTL} />
            <View style={styles.scannerCornerTR} />
            <View style={styles.scannerCornerBL} />
            <View style={styles.scannerCornerBR} />
          </View>
          <TouchableOpacity 
            style={styles.closeScanner} 
            onPress={() => setShowScanner(false)}
          >
            <Ionicons name="close-circle" size={64} color="rgba(255,255,255,0.8)" />
          </TouchableOpacity>
        </View>
      )}
    </KeyboardAvoidingView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#050505' },
  scrollContent: { flexGrow: 1, justifyContent: 'center', padding: 24 },
  header: { alignItems: 'center', marginBottom: 48 },
  logo: { width: 200, height: 60, marginBottom: 8 },
  subtitle: { color: '#00E5CC', fontSize: 12, letterSpacing: 4, fontWeight: 'bold' },
  form: { width: '100%' },
  label: { color: '#AAA', fontSize: 12, marginBottom: 8, fontWeight: '600', textTransform: 'uppercase' },
  input: { backgroundColor: '#111', borderWidth: 1, borderColor: '#333', borderRadius: 8, color: '#FFF', padding: 16, marginBottom: 20, fontSize: 16 },
  passwordContainer: { flexDirection: 'row', alignItems: 'center', backgroundColor: '#111', borderWidth: 1, borderColor: '#333', borderRadius: 8, marginBottom: 24 },
  toggle: { paddingHorizontal: 16 },
  toggleText: { color: '#7C6FE0', fontSize: 10, fontWeight: 'bold' },
  button: { backgroundColor: '#00E5CC', padding: 18, borderRadius: 8, alignItems: 'center', marginTop: 8 },
  buttonDisabled: { opacity: 0.6 },
  buttonText: { color: '#000', fontWeight: '900', fontSize: 14, letterSpacing: 1 },
  secondaryButton: { marginTop: 20, padding: 12, alignItems: 'center' },
  secondaryButtonText: { color: '#7C6FE0', fontSize: 13, fontWeight: '600' },
  footer: { color: '#333', fontSize: 10, textAlign: 'center', marginTop: 48, letterSpacing: 1 },
  inputWithAction: { flexDirection: 'row', alignItems: 'center', backgroundColor: '#111', borderWidth: 1, borderColor: '#333', borderRadius: 8, marginBottom: 20 },
  qrButton: { padding: 12, borderLeftWidth: 1, borderLeftColor: '#222' },
  scannerOverlay: { ...StyleSheet.absoluteFillObject, backgroundColor: '#000', zIndex: 1000 },
  scannerHeader: { position: 'absolute', top: 60, left: 0, right: 0, alignItems: 'center', zIndex: 1001 },
  scannerTitle: { color: '#00E5CC', fontSize: 14, fontWeight: '900', letterSpacing: 2, textShadowColor: 'rgba(0,0,0,0.5)', textShadowOffset: { width: 0, height: 2 }, textShadowRadius: 4 },
  scannerFrame: { position: 'absolute', top: '25%', left: '10%', right: '10%', height: '40%', borderWidth: 0, alignItems: 'center', justifyContent: 'center' },
  scannerCornerTL: { position: 'absolute', top: 0, left: 0, width: 40, height: 40, borderTopWidth: 4, borderLeftWidth: 4, borderColor: '#00E5CC' },
  scannerCornerTR: { position: 'absolute', top: 0, right: 0, width: 40, height: 40, borderTopWidth: 4, borderRightWidth: 4, borderColor: '#00E5CC' },
  scannerCornerBL: { position: 'absolute', bottom: 0, left: 0, width: 40, height: 40, borderBottomWidth: 4, borderLeftWidth: 4, borderColor: '#00E5CC' },
  scannerCornerBR: { position: 'absolute', bottom: 0, right: 0, width: 40, height: 40, borderBottomWidth: 4, borderRightWidth: 4, borderColor: '#00E5CC' },
  closeScanner: { position: 'absolute', bottom: 60, left: 0, right: 0, alignItems: 'center', zIndex: 1001 },
});
