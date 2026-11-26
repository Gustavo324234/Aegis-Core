# EPIC 63 — Hands-Free Voice Biometrics, Dynamic Wake-Word & Native Assistant

**Estado:** ✅ Completa

## 🎯 Objetivo

Dotar a Aegis OS de una interfaz de voz verdaderamente fluida, manos libres y multi-dispositivo (Web UI, demonio de PC con navegador cerrado y app Android como Asistente Predeterminado):
1. **Biometría Vocal Multi-Perfil v2 (56 dimensiones MFCC + Δ-MFCC + $F_0$ + forma espectral)** en Rust para distinguir voces autorizadas con precisión, rechazando ruido, TV o terceras personas con 0 tokens de LLM.
2. **Activación por Voz Dinámica (`"<NombreAsistente> escuchame"` / `"Aegis escuchame"`)** respetando el nombre personalizado del asistente y reutilizando Groq Whisper Turbo (`key_pool`) con fallback a Whisper Local.
3. **VAD Natural Tolerante a Respiración (`2.4s`) y Ventana de Repregunta (`8s`)** en `shell/ui`, PC daemon y móvil, eliminando el corte abrupto al tomar aire entre frases.
4. **PC Always-On Daemon** (`tools/desktop_voice/`) con atajo global de teclado y escucha continua aun con el navegador cerrado.
5. **App Android Dual** (`app/`): integración oficial como Asistente Digital de Android (`ROLE_ASSISTANT` / `ACTION_ASSIST`) más servicio `ForegroundService` de escucha activa 24/7.

---

## 🏗️ Tickets de la Épica

1. **`CORE-412`** (`feat`, ✅ Done): Biometría Vocal Multi-Perfil v2, Groq Whisper Zero-Config y Endpoint `/api/siren/utterance`.
2. **`CORE-413`** (`feat`, ✅ Done): VAD Natural Tolerante a Respiración (`smartVadRecorder`) y Estudio de Voces/Wake-Word en `shell/ui`.
3. **`CORE-414`** (`feat`, ✅ Done): Demonio de Voz de Escritorio Siempre Activo con Navegador Cerrado (`tools/desktop_voice/`).
4. **`CORE-415`** (`feat`, ✅ Done): App Android como Asistente Digital Predeterminado (`ROLE_ASSISTANT`) + `ForegroundService` de Escucha Continua.
5. **`CORE-416`** (`test`, ✅ Done): Suite de Pruebas de Biometría/Wake-Word/Daemon, Documentación Android Studio/PC y Sincronización SRE.
