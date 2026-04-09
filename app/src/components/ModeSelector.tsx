import React from 'react';
import { View, Text, TouchableOpacity, StyleSheet, Modal } from 'react-native';
import { useAuthStore } from '@/stores/authStore';
import type { AppMode } from '@/types/auth';

interface Props {
  visible: boolean;
  onClose: () => void;
}

const MODES: { id: AppMode; label: string; description: string }[] = [
  { id: 'satellite', label: 'SATELLITE', description: 'Process on Aegis OS server' },
  { id: 'cloud', label: 'CLOUD', description: 'Use your own API keys directly' },
  { id: 'hybrid', label: 'HYBRID', description: 'Server first, fallback to cloud' },
];

export default function ModeSelector({ visible, onClose }: Props) {
  const { mode, setMode } = useAuthStore();

  const handleSelect = async (selected: AppMode) => {
    await setMode(selected);
    onClose();
  };

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <TouchableOpacity style={styles.overlay} onPress={onClose} activeOpacity={1}>
        <View style={styles.sheet}>
          <Text style={styles.title}>CONNECTION MODE</Text>
          {MODES.map((m) => (
            <TouchableOpacity
              key={m.id}
              style={[styles.option, mode === m.id && styles.optionActive]}
              onPress={() => handleSelect(m.id)}
            >
              <Text style={[styles.optionLabel, mode === m.id && styles.optionLabelActive]}>
                {m.label}
              </Text>
              <Text style={styles.optionDesc}>{m.description}</Text>
            </TouchableOpacity>
          ))}
        </View>
      </TouchableOpacity>
    </Modal>
  );
}

const styles = StyleSheet.create({
  overlay: { flex: 1, backgroundColor: 'rgba(0,0,0,0.8)', justifyContent: 'flex-end' },
  sheet: { backgroundColor: '#0D0D0D', borderTopWidth: 1, borderTopColor: '#1A1A1A', padding: 24, paddingBottom: 40 },
  title: { color: '#666', fontSize: 11, letterSpacing: 3, marginBottom: 20, fontWeight: 'bold' },
  option: { padding: 16, borderRadius: 8, borderWidth: 1, borderColor: '#222', marginBottom: 10 },
  optionActive: { borderColor: '#00E5CC', backgroundColor: 'rgba(0,229,204,0.05)' },
  optionLabel: { color: '#AAA', fontSize: 13, fontWeight: 'bold', letterSpacing: 1 },
  optionLabelActive: { color: '#00E5CC' },
  optionDesc: { color: '#555', fontSize: 12, marginTop: 4 },
});
