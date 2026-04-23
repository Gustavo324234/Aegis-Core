# CORE-148 — Fix: System prompt — conversación natural, sin respuestas robóticas

**Epic:** 41 — UX & Onboarding
**Repo:** Aegis-Core — `kernel/crates/ank-core/src/chal/mod.rs`
**Tipo:** fix
**Prioridad:** Alta
**Asignado a:** Kernel Engineer

---

## Problema

El chat produce respuestas robóticas y vacías. Ejemplo real:

```
Usuario: hola
Agente:  Hola. Puedo reproducir música.       ← anuncia capacidades sin que nadie pregunte

Usuario: bueno reproduce
Agente:  Comprendido.                          ← no hizo nada, respuesta vacía

Usuario: cuánto tengo gastado hasta ahora?
Agente:  No sé.                                ← respuesta de 2 palabras, sin personalidad
```

**Causas:**

1. `SYSTEM_PROMPT_MASTER` dice "Sé directo y conciso" → el modelo interpreta esto
   como "responde lo más corto posible", produciendo "No sé." en lugar de algo útil.

2. La sección de música se inyecta SIEMPRE en el prompt, incluso en el primer "hola".
   Esto hace que el modelo la mencione en el saludo porque la ve como contexto relevante.

3. No hay instrucción sobre tono conversacional ni calidez.

---

## Fix — Reescribir `SYSTEM_PROMPT_MASTER`

```rust
pub const SYSTEM_PROMPT_MASTER: &str = "\
Sos un asistente personal inteligente y cercano. Respondés en el idioma del usuario.\n\
\n\
TONO Y ESTILO:\n\
- Conversá de forma natural y cálida, como un asistente de confianza.\n\
- Respondés con la extensión adecuada al contexto: corto para saludos, \
  más elaborado cuando la pregunta lo requiere.\n\
- Cuando no sabés algo o no podés hacer algo, lo decís de forma amigable \
  y ofrecés alternativas si las hay. Nunca respondés solo \"No sé.\" \
  — siempre agregás contexto útil.\n\
- No anunciés tus capacidades espontáneamente. Si el usuario pregunta \
  qué podés hacer, entonces sí explicás.\n\
\n\
PRECISIÓN:\n\
- Solo afirmás que hiciste algo si una herramienta te devolvió un resultado concreto.\n\
- No inventés datos, cifras ni hechos que no tenés.\n\
- Si ejecutaste una herramienta y no devolvió resultados útiles, lo decís claramente.\n\
";
```

**Cambios clave respecto al anterior:**
- "directo y conciso" → "conversá de forma natural y cálida"
- Regla explícita: nunca responder solo "No sé." — siempre agregar contexto
- Regla explícita: no anunciar capacidades espontáneamente
- Mantiene la honestidad sobre herramientas y datos

---

## Fix — Sección de música solo cuando es relevante

La sección de música hoy se inyecta siempre. Cambiar para que solo se incluya
cuando el plugin `music_search` está activo en el `PluginManager`.

En `build_prompt()`:

```rust
// ANTES — siempre inyecta música:
let music_section = "\n\nMÚSICA — INSTRUCCIONES:...";

// DESPUÉS — solo si el plugin de música está registrado:
let has_music_plugin = self
    .plugin_manager
    .read()
    .await
    .is_plugin_active("music_search");  // o el nombre real del plugin

let music_section = if has_music_plugin {
    "\n\nMÚSICA — INSTRUCCIONES:\
     \n- Para reproducir: [SYS_CALL_PLUGIN(\"music_search\", {\"query\": \"<artista canción>\", \"max_results\": 1})] \
     y luego [MUSIC_PLAY:youtube:<video_id>] (o [MUSIC_PLAY:spotify:<track_id>] si usas Spotify)\
     \n- Para pausar: responde brevemente y termina con [MUSIC_PAUSE]\
     \n- Para continuar: responde brevemente y termina con [MUSIC_RESUME]\
     \n- Para detener: responde brevemente y termina con [MUSIC_STOP]\
     \n- Para cambiar volumen: termina con [MUSIC_VOLUME:<0-100>]\
     \nNunca expliques estos tags al usuario. Solo úsalos.\n"
} else {
    ""
};
```

**Nota para el Kernel Engineer:** verificar cómo `PluginManager` expone la lista
de plugins activos. Si no hay un método `is_plugin_active()`, agregar uno simple:
```rust
pub fn is_plugin_active(&self, name: &str) -> bool {
    self.plugins.contains_key(name)
}
```

Si la verificación es compleja de implementar en este ticket, como fallback aceptable
es incluir la sección de música solo cuando `tool_prompt` no está vacío (si hay
herramientas registradas, es probable que música esté entre ellas).

---

## Resultado esperado con el fix

```
Usuario: hola
Agente:  ¡Hola! ¿En qué te puedo ayudar?

Usuario: bueno reproduce
Agente:  ¿Qué querés escuchar? Decime el artista o canción.

Usuario: cuánto tengo gastado hasta ahora?
Agente:  No tengo acceso a tu información financiera, así que no puedo
         decirte cuánto gastaste. Si querés llevar ese registro, podría
         ayudarte a organizarlo de otra forma. ¿Tenés algún sistema que
         ya estés usando?
```

---

## Criterios de aceptación

- [x] `cargo build --workspace` sin errores
- [x] El test `test_build_prompt_no_tools_is_clean` sigue pasando
- [x] El test `test_build_prompt_with_persona` sigue pasando
- [x] Actualizar el test para que no requiera `MÚSICA` cuando no hay plugin activo
- [x] Con plugin de música activo: la sección de música aparece en el prompt
- [x] Sin plugin de música: la sección de música NO aparece en el prompt
- [x] **EXTRA:** Implementación de Memoria Neuronal (L3) y Historial de Chat (L2).

---

## Dependencias

Ninguna — cambio autónomo en `chal/mod.rs`.

---

## Commit message

```
fix(ank-core): CORE-148 system prompt — natural conversational tone, no spontaneous capability announcements
```
