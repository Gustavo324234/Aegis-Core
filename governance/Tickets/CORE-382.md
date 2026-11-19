# CORE-382: Motor de Voz Natural (Femenina/Masculina), Simplificación de Audio en Chat y Estudio de Avatar/Identidad (`shell/ui`)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 59
- **Asignado a:** Shell Engineer

---

## Description
La experiencia de voz original en `shell/ui` presentaba múltiples barreras de usabilidad: un selector de 4 modos (`[Text | Audio | Conv | Wake]`) que ocultaba el micrófono, bloqueo del botón de micrófono cuando no existía el modelo local `models/ggml-base.bin` incluso usando WebSpeech del navegador, uso de la voz SAPI5 por defecto de escritorio (tono robótico y lectura literal de símbolos Markdown), y paneles duplicados con opciones rotas (`Voxtral`, `Fish Speech`). Además, el usuario requería poder elegir entre **voces femeninas y masculinas**, personalizar el **avatar** del asistente y ver su identidad reflejada en el chat.

## Archivos Creados / Modificados
1. **`shell/ui/src/audio/naturalVoice.ts`** (Nuevo):
   - Puntuación inteligente de naturalidad (`scoreVoiceNaturalness`) que prioriza voces Neurales/Online (`Microsoft Elena/Tomás/Dalia/Álvaro Online (Natural)`, `Google español`, Apple Enhanced) y penaliza voces SAPI5 `Desktop`.
   - Detección de género (`detectVoiceGender`: `'female' | 'male' | 'neutral'`) y 4 perfiles rápidos (`VOICE_STYLE_PRESETS`: *Femenina Cálida*, *Femenina Dinámica*, *Masculina Serena*, *Masculina Profunda*), con ajuste automático de timbre grave (`pitch`) si el navegador no expone una voz masculina nativa.
   - Limpieza de Markdown (`cleanMarkdownForSpeech`) y partición por oraciones (`splitIntoNaturalChunks`) para evitar el corte de 15s de Chrome.
2. **`shell/ui/src/components/AssistantAvatar.tsx`** (Nuevo):
   - Componente reutilizable con 6 avatares diseñados (`sol-amber`, `orb-cyan`, `nova-purple`, `atlas-emerald`, `crimson-flame`, `minimal-bot`) y soporte para imagen personalizada subida por el usuario (recortada y comprimida en WebP vía Canvas).
3. **`shell/ui/src/components/ChatTerminal.tsx`**:
   - Eliminado `<InputModeSelector />`; los botones de Parlante (`Volume2`/`VolumeX`) y Micrófono (`Mic`/`MicOff`) quedan siempre visibles y activos.
   - Renderizado del `<AssistantAvatar />` y del nombre real del asistente (`agentName`) en cada burbuja de respuesta y en la píldora de acceso rápido del header superior.
4. **`shell/ui/src/components/SettingsPanel.tsx` y `shell/ui/src/components/SirenConfigTab.tsx`**:
   - Unificado el Estudio de Personalización del Asistente en `PersonaTab` (Nombre + Avatar + Tipo de Voz Femenina/Masculina con botón de prueba en vivo + Chips de Personalidad).
   - Reemplazado el antiguo `VozTab` duplicado en `SettingsPanel.tsx` por `<SirenConfigTab />`.
5. **`shell/ui/src/store/useAegisStore.ts`**:
   - Integrado `speakNaturalText`, `stopSpeaking`, idioma `es-AR` por defecto en `SpeechRecognition`, `sttAvailable = true` para el proveedor `browser`, y refresco automático de `fetchAgentName()` al conectar y tras el onboarding.

## Acceptance Criteria
- [x] El botón de micrófono y el toggle de voz funcionan directamente en la barra de chat sin cambiar modos ni requerir modelos locales Whisper.
- [x] El usuario puede elegir entre voz femenina y masculina (4 estilos preconfigurados + selector de voces detectadas) y probarla en vivo.
- [x] El usuario puede elegir entre 6 avatares o subir su propia imagen, viéndose reflejado junto al nombre del asistente en los mensajes del chat y en el header.
- [x] `npm run build` en `shell/ui` compila con 0 errores de TypeScript.
