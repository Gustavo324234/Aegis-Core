# CORE-315 — Kernel/Shell: Exportar/importar configuración de llaves cifrada con contraseña

**Tipo:** feat  
**Prioridad:** Media  
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure  
**Estado:** ✅ Done  
**Asignado a:** Kernel Engineer + UI Engineer  

---

## Problema
Los usuarios avanzados y administradores de Aegis OS configuran múltiples APIs y llaves de diversos proveedores (Gemini, OpenRouter, Anthropic, Ollama, etc.). Configurar estas claves de nuevo en una máquina diferente, tras una reinstalación, o para un nuevo tenant es tedioso. Además, almacenar estas claves en texto plano en archivos no es seguro. El usuario solicita una forma de exportar e importar de forma segura esta configuración en un archivo portable (por ejemplo, para llevar en un pendrive) protegido por contraseña.

## Solución propuesta
Implementar un flujo de exportación/importación seguro utilizando cifrado robusto en el backend y controles correspondientes en el frontend (Shell):

1. **Flujo de Exportación (Export):**
   * El usuario selecciona "Exportar configuración de llaves" en la UI.
   * La UI solicita al usuario definir una contraseña robusta de cifrado.
   * El backend recopila todas las llaves (`ApiKeyEntry`) del KeyPool del tenant o global, omitiendo campos de estado transitorios (ej. cooldowns).
   * Se genera una clave simétrica a partir de la contraseña del usuario utilizando **Argon2id** (o PBKDF2-HMAC-SHA256 como fallback directo) con un salt criptográfico aleatorio.
   * Los datos se cifran mediante **AES-256-GCM** o **ChaCha20-Poly1305** para garantizar confidencialidad e integridad (AEAD).
   * Se descarga un archivo binario portable con la extensión `.aegiskey` o `.enc.json` que incluye el Salt, el Nonce y el texto cifrado.

2. **Flujo de Importación (Import):**
   * El usuario selecciona "Importar configuración de llaves" en la UI y sube el archivo `.aegiskey`.
   * El sistema solicita la contraseña de descifrado.
   * El backend deriva la clave a partir del salt del archivo y la contraseña dada, e intenta descifrar el payload.
   * Si el descifrado es exitoso, valida el esquema de los `ApiKeyEntry` e importa las llaves de forma incremental en el KeyPool del tenant/global (agregándolas o actualizándolas según `key_id` o `provider`).

## Criterios de aceptación
- [ ] Endpoint backend `POST /api/router/keys/export` que toma una contraseña y devuelve un archivo cifrado con AEAD.
- [ ] Endpoint backend `POST /api/router/keys/import` que toma el archivo y la contraseña y registra las llaves de forma segura.
- [ ] Interfaz gráfica en la Shell con botones de Importar/Exportar y campos de contraseña elegantes.
- [ ] Manejo de error claro y seguro si la contraseña ingresada es incorrecta (sin revelar vectores de ataque de tiempo).
- [ ] Pruebas unitarias de cifrado y descifrado de llaves en `key_pool.rs`.
