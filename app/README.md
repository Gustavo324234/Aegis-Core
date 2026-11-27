# app/

Aegis Mobile Client — React Native / Expo

Cross-platform mobile app (Android + iOS) with two modes:

- **Satellite mode** — connects to a running Aegis server via HTTP/WebSocket
- **Cloud mode** — talks directly to cloud AI providers (no server needed)

## Android Default Assistant (`ROLE_ASSISTANT`) + Escucha Continua 24/7 + Auto-Updater (`EPIC 63`)

Aegis incluye el plugin nativo [plugins/withAegisAssistant.js](plugins/withAegisAssistant.js) que configura:
1. **Asistente Digital Predeterminado de Android (`ACTION_ASSIST` / `VOICE_COMMAND`)**: al mantener apretado el botón de encendido, deslizar desde la esquina inferior o pulsar auriculares Bluetooth, se abre Aegis listo para escucharte.
2. **Servicio en primer plano (`AegisVoiceForegroundService`)**: escucha pasiva local con captura 16 kHz PCM, validación biométrica contra tus voces registradas en el servidor y frase `"Aegis escuchame"` (o el nombre personalizado del asistente).
3. **VAD tolerante a pausas de respiración (`2.4s`)**: grabás y hablás natural sin que corte al tomar aire.
4. **Actualizador integrado (`appUpdater.ts`)**: busca versiones en **GitHub Releases**, valida checksum **SHA-256** e instala la nueva APK desde adentro de la app conservando sesión y datos (se desactiva automáticamente cuando compilás para Play Store con `PLAY_STORE_BUILD=1`).

## Descarga Automática vía GitHub Actions

El workflow [.github/workflows/publish-native.yml](../.github/workflows/publish-native.yml) compila la APK automáticamente al fusionar en `main`:
- **Nightly:** `https://github.com/Gustavo324234/Aegis-Core/releases/download/nightly/aegis-android.apk`
- **Estable:** `https://github.com/Gustavo324234/Aegis-Core/releases/latest/download/aegis-android.apk`

### Firma estable (una sola vez — obligatoria para actualizar encima sin desinstalar)

Generá tu keystore único en PC:

```bash
keytool -genkeypair -v -storetype PKCS12 -keystore aegis-release.keystore -alias aegis -keyalg RSA -keysize 2048 -validity 10000
```

Convertilo a Base64 (`[Convert]::ToBase64String([IO.File]::ReadAllBytes("aegis-release.keystore")) | Set-Clipboard` en Windows) y cargá estos 4 secrets en GitHub (**Settings → Secrets and variables → Actions**):
- `ANDROID_KEYSTORE_B64`
- `ANDROID_KEYSTORE_PASSWORD`
- `ANDROID_KEY_ALIAS` (`aegis`)
- `ANDROID_KEY_PASSWORD`

## Compilar APK manualmente en Android Studio

```bash
cd app
npm install
npx expo prebuild --platform android
```

Luego abrí **`app/android`** desde **Android Studio**, esperá Gradle Sync y ejecutá **Build → Build APK(s)** o **Run (`▶`)**.

Para preparar un bundle `.aab` destinado a **Google Play Store** (sin `REQUEST_INSTALL_PACKAGES`):

```bash
cd app
PLAY_STORE_BUILD=1 npx expo prebuild --platform android
```

## Development

```bash
npm install
npx expo start           # development
npx eas build            # cloud EAS build (optional)
```
