# CORE-283 — chat_agent: honestidad cuando el supervisor no retorna datos

**Tipo:** fix  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer (modificación de chat_agent.md)  
**Componentes:**
- `kernel/config/agents/chat_agent.md`
**Estado:** ✅ Completo

---

## Contexto

El log muestra este patrón repetido:

```
Chat agent: "En breve tendrás más información sobre el proyecto. 🚀"
[supervisor trabajando...]
[supervisor completa sin datos o queda bloqueado]
Chat agent (siguiente mensaje): "Voy a pedirle al supervisor que me prepare
un informe actualizado... En cuanto me responda, te comparto todos los detalles."
```

El chat_agent promete resultados que nunca llegan. El usuario pregunta "¿está activo
el supervisor?" y el chat_agent responde "no hay supervisor activo" — contradiciendo
su mensaje anterior donde dijo que estaba trabajando.

El problema es una combinación de:
1. El chat_agent no tiene feedback real de si el supervisor completó con datos
2. Las instrucciones actuales dicen "informá al usuario que el supervisor está
   trabajando" pero no dicen qué hacer si el supervisor no retorna nada

---

## Cambios requeridos

### `kernel/config/agents/chat_agent.md` — nueva sección de manejo de incertidumbre

Agregar después de la sección "Gestión de Proyectos con Supervisores":

```markdown
## Cuándo NO prometer resultados

Cuando creás un supervisor con `spawn_agent`, el sistema te confirma que fue
creado — pero NO te confirma que está trabajando ni que va a retornar datos.

**Reglas de comunicación honesta:**

✓ Decile al usuario que despachaste la tarea:
  "Le pedí al equipo de Aegis que revise el estado del proyecto."

✗ NO prometas resultados futuros:
  ~~"En breve te comparto todos los detalles"~~
  ~~"En cuanto el supervisor me responda, te lo envío"~~

✓ Si el usuario pregunta por el estado y no tenés datos:
  "No tengo un reporte del supervisor todavía. ¿Querés que lo reactive?"

✓ Si spawneaste hace más de 2 turnos y no recibiste nada:
  "El equipo no respondió aún. Puede estar trabajando en segundo plano,
  o puede que necesite ser reactivado."

✗ NO inventes que el supervisor está "trabajando en segundo plano" si no
  tenés confirmación de eso.

## Una sola promesa por tarea

Cuando despachás trabajo, hacé UNA confirmación breve y no la repitas.
Si el usuario vuelve a preguntar sin que hayas recibido datos nuevos,
respondé honestamente sobre el estado real:

✓ "Todavía no tengo respuesta del equipo."
✗ "El supervisor sigue trabajando y pronto te envío los detalles." (repetición)
```

---

## Criterios de aceptación

- [x] El chat_agent no repite promesas de "en breve te comparto" en mensajes sucesivos
- [x] Cuando el usuario pregunta por el estado y no hay datos, el chat_agent
  responde honestamente que no tiene información
- [x] El chat_agent no afirma que el supervisor "está trabajando en segundo plano"
  sin tener confirmación de eso
- [x] El tono sigue siendo cálido — la honestidad no implica frialdad

## Dependencias

- CORE-281 (deduplicación) — evita que el chat_agent cree supervisores repetidos
  ante la incertidumbre, lo cual agrava el problema
