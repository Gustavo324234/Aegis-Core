import { Linking } from 'react-native';

export async function sendWhatsAppMessage(phone: string, message: string) {
  // 1. Sanitize phone number (remove +, spaces, etc if needed, but WhatsApp Linking works well with +)
  const cleanPhone = phone.replace(/[^\d+]/g, '');
  
  // 2. Encode message
  const url = `whatsapp://send?phone=${cleanPhone}&text=${encodeURIComponent(message)}`;

  try {
    const supported = await Linking.canOpenURL(url);
    if (supported) {
      await Linking.openURL(url);
      return { success: true, detail: "WhatsApp opened with prepared message." };
    } else {
      // Fallback to web link if app not installed
      const webUrl = `https://wa.me/${cleanPhone.replace('+', '')}?text=${encodeURIComponent(message)}`;
      await Linking.openURL(webUrl);
      return { success: true, detail: "WhatsApp Web opened as fallback." };
    }
  } catch (err: any) {
    throw new Error(`Failed to open WhatsApp: ${err.message}`);
  }
}
