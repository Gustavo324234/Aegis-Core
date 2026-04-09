# app/

Aegis Mobile Client — React Native / Expo

Cross-platform mobile app (Android + iOS) with two modes:

- **Satellite mode** — connects to a running Aegis server via HTTP/WebSocket
- **Cloud mode** — talks directly to cloud AI providers (no server needed)

## Build

```bash
npm install
npx expo start           # development
npx eas build            # production (requires EAS account)
```

## Source

Migrated from: `Aegis-App` (archived)
