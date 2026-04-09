import { useEffect } from 'react';
import { Slot, useRouter } from 'expo-router';
import { useAuthStore } from '@/stores/authStore';
import { GestureHandlerRootView } from 'react-native-gesture-handler';

export default function RootLayout() {
  const { initFromStorage, isAuthenticated } = useAuthStore();
  const router = useRouter();

  useEffect(() => {
    initFromStorage().then(() => {
      if (isAuthenticated) {
        router.replace('/(main)/chat');
      } else {
        router.replace('/(auth)/login');
      }
    });
  }, [initFromStorage, isAuthenticated, router]);

  return (
    <GestureHandlerRootView style={{ flex: 1 }}>
      <Slot />
    </GestureHandlerRootView>
  );
}
