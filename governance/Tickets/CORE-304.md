# CORE-304 — Voice: Migrar Siren Protocol de WebSockets a WebRTC/WebTransport

## Metadata
- **Epic:** 52 — Voice Quality
- **Tipo:** feat
- **Prioridad:** Alta
- **Asignado a:** Kernel Engineer + Shell Engineer
- **Estado:✅ Done

## Problema
Actualmente, el protocolo Siren de voz bidireccional streamea audio sobre WebSockets puros. Al operar sobre TCP, cualquier pérdida de paquetes de red provoca un bloqueo de cabeza de línea (Head-of-Line Blocking), generando tirones, latencia acumulativa, y audio robotizado en conexiones móviles o inestables. Además, el manejo de audio en crudo dificulta la supresión de ruido nativa.

## Solución propuesta
Migrar la capa de transporte de Siren Protocol para utilizar tecnologías nativas de streaming de baja latencia basadas en UDP:

1. **Uso de WebRTC (Audio Channels) / WebTransport:**
   * Reemplazar la conexión de WebSocket dedicada a audio por una sesión WebRTC (usando `RTCPeerConnection` en el frontend y una biblioteca compatible en el backend en Rust como `webrtc-rs`).
   * Alternativamente, evaluar WebTransport para streaming de audio asíncrono con control directo de transporte.
2. **Establecimiento de Gateways de Audio Dedicados:**
   * Utilizar canales WebRTC optimizados para audio Opus a baja tasa de bits y baja latencia.
3. **Manejo de Jitter y Pérdidas de Paquetes:**
   * Aprovechar los algoritmos integrados de WebRTC para mitigar el jitter de red (Jitter Buffers) y la ocultación de pérdidas de paquetes (Packet Loss Concealment).
4. **Simplificación en el Frontend:**
   * Reemplazar buffers manuales complejos en React/JS por el procesamiento nativo del navegador para la reproducción y captura de flujos de audio (`MediaStreamTrack`).

## Criterios de aceptación
- [x] La latencia de audio bidireccional se reduce a niveles sub-segundo (<500ms en condiciones normales).
- [x] La transmisión se mantiene fluida y recuperable incluso con pérdidas moderadas de paquetes de red (hasta un 10%).
- [x] El canal de audio Opus se negocia correctamente entre el backend en Rust y el navegador del cliente.
- [x] Se remueve el uso de WebSockets en el protocolo Siren, conservándolos únicamente para el flujo de eventos de texto del chat.
