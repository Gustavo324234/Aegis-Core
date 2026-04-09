import * as Notifications from 'expo-notifications';
import { Platform } from 'react-native';

export interface AegisNotification {
  id: string;
  app: string;
  title: string;
  message: string;
  timestamp: number;
}

// In-memory store for recent notifications (Aegis Memory)
let notificationHistory: AegisNotification[] = [];
const MAX_HISTORY = 20;

export function setupNotificationListener() {
  Notifications.addNotificationReceivedListener(notification => {
    const { title, body, data } = notification.request.content;
    const appName = (data?.appName as string) || 'System';

    const newNote: AegisNotification = {
      id: notification.request.identifier,
      app: appName,
      title: title || 'Unknown',
      message: body || '',
      timestamp: Date.now(),
    };

    notificationHistory = [newNote, ...notificationHistory].slice(0, MAX_HISTORY);
    console.log(`[ANK] New notification captured from ${appName}`);
  });
}

export async function getRecentNotifications(limit: number = 5): Promise<AegisNotification[]> {
  return notificationHistory.slice(0, limit);
}

export async function clearNotifications() {
  notificationHistory = [];
}

/**
 * Note: Real cross-app notification listening on Android requires a
 * Native Module (NotificationListenerService). For this "Mini ANK",
 * we'll monitor notifications that the system routes through the app bridge.
 */
export async function requestNotificationAccess() {
  if (Platform.OS === 'android') {
    // This is a placeholder for the intent to open Notification Access settings
    console.log("Requesting Notification Access Service...");
  }
  const { status } = await Notifications.requestPermissionsAsync();
  return status === 'granted';
}
