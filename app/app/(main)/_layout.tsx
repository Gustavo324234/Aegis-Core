import { Stack } from 'expo-router';

export default function MainLayout() {
  return (
    <Stack screenOptions={{ headerShown: false }}>
      <Stack.Screen name="chat" />
      <Stack.Screen name="settings" />
      <Stack.Screen name="connected-accounts" />
    </Stack>
  );
}
