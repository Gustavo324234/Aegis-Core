import * as Contacts from 'expo-contacts';
import * as Notifications from 'expo-notifications';
import { Platform, Alert } from 'react-native';

export async function requestContactsPermission(): Promise<boolean> {
  const { status } = await Contacts.requestPermissionsAsync();
  if (status !== 'granted') {
    Alert.alert(
      'Permission Denied',
      'Aegis needs access to contacts to identify people in your messages.'
    );
    return false;
  }
  return true;
}

export async function requestNotificationsPermission(): Promise<boolean> {
  const { status } = await Notifications.requestPermissionsAsync();
  if (status !== 'granted') {
    Alert.alert(
      'Permission Denied',
      'Aegis needs notification access to monitor incoming messages.'
    );
    return false;
  }
  return true;
}

export async function checkPermissions() {
  // Silent check
  const contacts = await Contacts.getPermissionsAsync();
  const notes = await Notifications.getPermissionsAsync();

  return {
    contacts: contacts.status === 'granted',
    notifications: notes.status === 'granted',
  };
}
