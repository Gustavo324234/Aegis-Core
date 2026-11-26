# CORE-188 — fix: Onboarding extrae nombre completo literal en lugar de solo el nombre

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-core + ank-http  
**Tipo:** fix  
**Estado:** ✅ Done  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Dependencias:** ninguna

---

## Problema

Cuando el usuario configura el nombre del asistente diciendo frases naturales como
"quiero llamarte Sol" o "tu nombre es Sol", el sistema guarda el texto completo de la
frase como persona en lugar de extraer solo el nombre.

**Evidencia en producción (logs 2026-04-25):**

```
output: "...Quiero llamarte Sol está acá para asistirte."
output: "¡Claro! Soy **Quiero llamarte Sol**, pero bueno…"
```

El `agent_persona` guardado en SQLite tiene el valor `"Quiero llamarte Sol"` en lugar
de `"Sol"`.

---

## Causa raíz

El flujo de onboarding usa `ONBOARDING_NAME_KEY` para guardar el nombre pendiente
durante el proceso de configuración. Pero el handler de onboarding no está extrayendo
el nombre de la frase — está guardando el texto completo del mensaje del usuario como
nombre.

Adicionalmente, el `PERSONA_SECTION_TEMPLATE` inyecta el valor crudo en el system
prompt sin ninguna sanitización:

```rust
PERSONA_SECTION_TEMPLATE.replace("{persona}", p)
// Si p = "Quiero llamarte Sol", el LLM recibe eso como identidad literal
```

---

## Cambios Requeridos

### 1. Localizar el handler de onboarding en `ank-http`

Buscar con `search_code("onboarding_step")` y `search_code("ONBOARDING")` para
encontrar el handler que procesa las respuestas del usuario durante el onboarding.

### 2. Agregar extracción de nombre en el handler

El handler que procesa la respuesta del usuario al paso `"name"` del onboarding
debe extraer el nombre antes de guardarlo. Agregar función en `enclave/mod.rs` o
en el handler:

```rust
/// Extrae el nombre propio de una frase de configuración.
/// Ejemplos:
///   "quiero llamarte Sol" → "Sol"
///   "tu nombre es Aria"   → "Aria"
///   "llamame Max"         → "Max"
///   "Sol"                 → "Sol"  (sin frase, retorna tal cual)
fn extract_name_from_phrase(input: &str) -> String {
    let lower = input.trim().to_lowercase();

    // Patrones comunes de configuración de nombre (orden importa — más específico primero)
    let patterns = [
        "quiero llamarte ",
        "quiero que te llames ",
        "tu nombre es ",
        "te llamas ",
        "llamame ",
        "llámame ",
        "nombre: ",
        "name: ",
    ];

    for pattern in &patterns {
        if lower.starts_with(pattern) {
            let remainder = &input.trim()[pattern.len()..];
            // Tomar solo la primera palabra (el nombre propio)
            let name = remainder
                .split_whitespace()
                .next()
                .unwrap_or(remainder)
                .trim_matches(|c: char| !c.is_alphanumeric());
            if !name.is_empty() {
                // Capitalizar primera letra
                let mut chars = name.chars();
                return match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + chars.as_str()
                    }
                };
            }
        }
    }

    // Si no matchea ningún patrón, asumir que el input YA es el nombre
    // Capitalizar primera letra por consistencia
    let trimmed = input.trim();
    let mut chars = trimmed.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
```

### 3. Usar `extract_name_from_phrase` al guardar el nombre en el onboarding

En el handler que llama a `db.set_onboarding_name(name)`, reemplazar:

```rust
// ANTES
db.set_onboarding_name(&user_message)?;

// DESPUÉS
let clean_name = extract_name_from_phrase(&user_message);
db.set_onboarding_name(&clean_name)?;
```

### 4. Guardar la persona correctamente al completar el onboarding

Cuando el onboarding ensambla la persona final y llama a `db.set_persona(persona_text)`,
verificar que el nombre extraído (no el texto original) se use en la construcción
del string de persona.

### 5. Test unitario

```rust
#[test]
fn test_extract_name_from_phrase() {
    assert_eq!(extract_name_from_phrase("quiero llamarte Sol"), "Sol");
    assert_eq!(extract_name_from_phrase("tu nombre es Aria"), "Aria");
    assert_eq!(extract_name_from_phrase("llamame Max"), "Max");
    assert_eq!(extract_name_from_phrase("Sol"), "Sol");
    assert_eq!(extract_name_from_phrase("ARIA"), "ARIA");
    assert_eq!(extract_name_from_phrase("quiero llamarte sol más info"), "Sol");
}
```

---

## Criterios de Aceptación

- [ ] Al decir "quiero llamarte Sol", el `agent_persona` guardado contiene "Sol", no el texto completo
- [ ] Al decir "tu nombre es Aria", el `agent_persona` guardado contiene "Aria"
- [ ] Al decir solo "Sol", se guarda "Sol" correctamente
- [ ] El LLM responde usando el nombre correcto en la próxima conversación
- [ ] Test unitario `test_extract_name_from_phrase` pasa
- [ ] `cargo build` sin warnings

---

## Commit message

```
fix(ank-core): CORE-188 onboarding extrae nombre limpio en lugar de frase completa
```

---

*Arquitecto IA — 2026-04-26*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: Extracción de nombre en onboarding implementada: ws/chat.rs:986 (helper que extrae el nombre de frases tipo 'quiero que te llames X').
