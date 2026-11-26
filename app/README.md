# app/

Aegis Mobile Client — React Native / Expo

Cross-platform mobile app (Android + iOS) with two modes:

- **Satellite mode** — connects to a running Aegis server via HTTP/WebSocket
- **Cloud mode** — talks directly to cloud AI providers (no server needed)

## Android Default Assistant (`ROLE_ASSISTANT`) + Escucha Continua 24/7 (`EPIC 63`)

Aegis incluye el plugin nativo [plugins/withAegisAssistant.js](plugins/withAegisAssistant.js) que configura:
1. **Asistente Digital Predeterminado de Android (`ACTION_ASSIST` / `VOICE_COMMAND`)**: al mantener apretado el botón de encendido, deslizar desde la esquina inferior o pulsar auriculares Bluetooth, se abre Aegis listo para escucharte.
2. **Servicio en primer plano (`AegisVoiceForegroundService`)**: escucha pasiva local con captura 16 kHz PCM, validación biométrica contra tus voces registradas en el servidor y frase `"Aegis escuchame"` (o el nombre personalizado del asistente).
3. **VAD tolerante a pausas de respiración (`2.4s`)**: grabás y hablás natural sin que corte al tomar aire.

## Compilar APK en Android Studio

```bash
cd app
npm install
npx expo prebuild --platform android
```

Luego abrí la carpeta **`app/android`** desde **Android Studio**:
1. Esperá a que finalice Gradle Sync.
2. Conectá tu celular por USB (con Depuración USB) y ejecutá **Run (`▶`)**, o generá la APK con **Build → Build Bundle(s) / APK(s) → Build APK(s)**.
3. En el teléfono, abrí **Ajustes → Aplicaciones → Apps predeterminadas → App de asistencia digital** y elegí **Aegis** (o tocá *"Elegir Aegis como Asistente de Android"* dentro de Ajustes en la app).

## Development

```bash
npm install
npx expo start           # development
npx eas build            # production (requires EAS account)
```

## Source

Migrated from: `Aegis-App` (archived)
