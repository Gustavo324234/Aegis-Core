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

export default function LoginScreen() {
  const router = useRouter();
  const { loginSuccess } = useAuthStore();

  const [serverUrl, setServerUrl] = useState('http://');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [showPassword, setShowPassword] = useState(false);

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
          <TextInput
            style={styles.input}
            value={serverUrl}
            onChangeText={setServerUrl}
            placeholder="http://192.168.1.x:8000"
            placeholderTextColor="#666"
            autoCapitalize="none"
            autoCorrect={false}
            keyboardType="url"
          />

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
});
