import React from 'react';
import { View, TouchableOpacity, StyleSheet } from 'react-native';
import { Ionicons } from '@expo/vector-icons';

interface VoiceButtonProps {
  isListening: boolean;
  onPress: () => void;
}

export default function VoiceButton({ isListening, onPress }: VoiceButtonProps) {
  return (
    <TouchableOpacity 
      style={[styles.button, isListening && styles.active]} 
      onPress={onPress}
    >
      <Ionicons 
        name={isListening ? "mic" : "mic-outline"} 
        size={22} 
        color={isListening ? "#FFF" : "#00E5CC"} 
      />
      {isListening && <View style={styles.pulse} />}
    </TouchableOpacity>
  );
}

const styles = StyleSheet.create({
  button: { padding: 8, position: 'relative' },
  active: { backgroundColor: '#FF4444', borderRadius: 20 },
  pulse: { 
    position: 'absolute', 
    top: 0, left: 0, right: 0, bottom: 0, 
    borderRadius: 20, 
    borderWidth: 2, 
    borderColor: '#FF4444', 
    opacity: 0.5 
  },
});
