import { Linking } from 'react-native';
import * as secureStorage from '@/services/secureStorage';
import {
  downloadAndInstallApk,
  getAppVersionInfo,
  hasNativeAssistantModule,
} from '@/services/aegisAssistantNative';

const GITHUB_RELEASES_API =
  'https://api.github.com/repos/Gustavo324234/Aegis-Core/releases?per_page=8';
const LAST_RELEASE_KEY = 'aegis_last_apk_release_sig';

export const DIRECT_NIGHTLY_APK_URL =
  'https://github.com/Gustavo324234/Aegis-Core/releases/download/nightly/aegis-android.apk';

export interface AppUpdateCheckResult {
  available: boolean;
  tagName: string;
  releaseName: string;
  publishedAt: string;
  releaseSignature: string;
  apkUrl: string;
  sha256: string | null;
  installedVersion: string;
  supportsInAppUpdate: boolean;
}

interface GitHubAsset {
  name: string;
  browser_download_url: string;
  updated_at?: string;
  id?: number;
}

interface GitHubRelease {
  tag_name: string;
  name?: string;
  published_at?: string;
  prerelease?: boolean;
  draft?: boolean;
  assets?: GitHubAsset[];
}

function normalizeSemver(tag: string): string {
  return tag.replace(/^v/i, '').trim();
}

function isNewerSemver(remoteTag: string, localVer: string): boolean {
  const rParts = normalizeSemver(remoteTag)
    .split('.')
    .map((n) => parseInt(n, 10));
  const lParts = normalizeSemver(localVer)
    .split('.')
    .map((n) => parseInt(n, 10));
  if (rParts.some(Number.isNaN) || lParts.some(Number.isNaN)) {
    return false;
  }
  const len = Math.max(rParts.length, lParts.length);
  for (let i = 0; i < len; i++) {
    const r = rParts[i] ?? 0;
    const l = lParts[i] ?? 0;
    if (r > l) return true;
    if (r < l) return false;
  }
  return false;
}

export async function checkForAppUpdate(): Promise<AppUpdateCheckResult> {
  const versionInfo = await getAppVersionInfo();
  const resp = await fetch(GITHUB_RELEASES_API, {
    headers: { Accept: 'application/vnd.github+json' },
  });
  if (!resp.ok) {
    throw new Error(`No se pudo consultar GitHub Releases (HTTP ${resp.status})`);
  }

  const releases = (await resp.json()) as GitHubRelease[];
  const chosen = releases.find(
    (rel) =>
      !rel.draft &&
      (rel.assets || []).some((a) => a.name === 'aegis-android.apk')
  );

  if (!chosen) {
    return {
      available: false,
      tagName: versionInfo.versionName,
      releaseName: 'Sin APK publicada aún',
      publishedAt: '',
      releaseSignature: '',
      apkUrl: DIRECT_NIGHTLY_APK_URL,
      sha256: null,
      installedVersion: versionInfo.versionName,
      supportsInAppUpdate: versionInfo.supportsInAppUpdate,
    };
  }

  const apkAsset = chosen.assets!.find((a) => a.name === 'aegis-android.apk')!;
  const shaAsset = chosen.assets!.find((a) => a.name === 'aegis-android.apk.sha256');

  let sha256: string | null = null;
  if (shaAsset?.browser_download_url) {
    try {
      const shaResp = await fetch(shaAsset.browser_download_url);
      if (shaResp.ok) {
        const text = await shaResp.text();
        const m = text.match(/[a-fA-F0-9]{64}/);
        if (m) sha256 = m[0].toLowerCase();
      }
    } catch {
      // Checksum asset optional
    }
  }

  const releaseSignature = `${chosen.tag_name}:${apkAsset.updated_at || chosen.published_at || apkAsset.id || ''}`;
  const lastInstalledSig = await secureStorage.getItem(LAST_RELEASE_KEY);

  const available =
    isNewerSemver(chosen.tag_name, versionInfo.versionName) ||
    (Boolean(releaseSignature) &&
      lastInstalledSig !== null &&
      lastInstalledSig !== releaseSignature) ||
    (lastInstalledSig === null && chosen.tag_name === 'nightly');

  return {
    available,
    tagName: chosen.tag_name,
    releaseName: chosen.name || chosen.tag_name,
    publishedAt: chosen.published_at || apkAsset.updated_at || '',
    releaseSignature,
    apkUrl: apkAsset.browser_download_url,
    sha256,
    installedVersion: versionInfo.versionName,
    supportsInAppUpdate: versionInfo.supportsInAppUpdate,
  };
}

export async function installAppUpdate(update: AppUpdateCheckResult): Promise<void> {
  if (!hasNativeAssistantModule() || !update.supportsInAppUpdate) {
    await Linking.openURL(update.apkUrl);
    return;
  }
  await downloadAndInstallApk(update.apkUrl, update.sha256);
  if (update.releaseSignature) {
    await secureStorage.setItem(LAST_RELEASE_KEY, update.releaseSignature);
  }
}
