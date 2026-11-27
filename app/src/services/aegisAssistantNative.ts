import { NativeModules, Platform } from 'react-native';

export interface AppVersionInfo {
  versionName: string;
  versionCode: number;
  supportsInAppUpdate: boolean;
}

interface AegisNativeInterface {
  openAssistantSettings(): Promise<boolean>;
  consumeAssistLaunchTrigger(): Promise<boolean>;
  consumePendingVoiceCommand(): Promise<string | null>;
  startContinuousListening(
    serverUrl: string,
    tenantId: string,
    sessionKey: string,
    agentName: string,
    hangoverMs: number
  ): Promise<boolean>;
  stopContinuousListening(): Promise<boolean>;
  isContinuousListening(): Promise<boolean>;
  recordPcm16Utterance(
    hangoverMs: number,
    fixedDurationMs: number
  ): Promise<{ pcmB64: string; sampleRate: number; hadSpeech: boolean }>;
  getAppVersionInfo(): Promise<AppVersionInfo>;
  downloadAndInstallApk(apkUrl: string, expectedSha256: string | null): Promise<string>;
}

const nativeModule: AegisNativeInterface | undefined =
  Platform.OS === 'android' ? NativeModules.AegisAssistantModule : undefined;

export function hasNativeAssistantModule(): boolean {
  return Boolean(nativeModule);
}

export async function openAndroidAssistantSettings(): Promise<boolean> {
  if (!nativeModule) return false;
  return nativeModule.openAssistantSettings();
}

export async function consumeAssistLaunchTrigger(): Promise<boolean> {
  if (!nativeModule) return false;
  return nativeModule.consumeAssistLaunchTrigger();
}

export async function consumePendingVoiceCommand(): Promise<string | null> {
  if (!nativeModule) return null;
  return nativeModule.consumePendingVoiceCommand();
}

export async function startAlwaysOnListening(
  serverUrl: string,
  tenantId: string,
  sessionKey: string,
  agentName: string,
  hangoverMs = 2400
): Promise<boolean> {
  if (!nativeModule) return false;
  return nativeModule.startContinuousListening(
    serverUrl,
    tenantId,
    sessionKey,
    agentName || 'Aegis',
    hangoverMs
  );
}

export async function stopAlwaysOnListening(): Promise<boolean> {
  if (!nativeModule) return false;
  return nativeModule.stopContinuousListening();
}

export async function isAlwaysOnListening(): Promise<boolean> {
  if (!nativeModule) return false;
  return nativeModule.isContinuousListening();
}

export async function recordNativePcm16Utterance(
  hangoverMs = 2400,
  fixedDurationMs = 0
): Promise<{ pcmB64: string; sampleRate: number; hadSpeech: boolean } | null> {
  if (!nativeModule) return null;
  return nativeModule.recordPcm16Utterance(hangoverMs, fixedDurationMs);
}

export async function getAppVersionInfo(): Promise<AppVersionInfo> {
  if (!nativeModule) {
    return { versionName: '1.0.0', versionCode: 1, supportsInAppUpdate: false };
  }
  return nativeModule.getAppVersionInfo();
}

export async function downloadAndInstallApk(
  apkUrl: string,
  expectedSha256: string | null
): Promise<string | null> {
  if (!nativeModule) return null;
  return nativeModule.downloadAndInstallApk(apkUrl, expectedSha256);
}
