# CORE-285 — Installer: configuración obligatoria de modelo antes de arrancar

**Tipo:** feat  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer (installer)  
**Componentes:**
- `installer/install.sh`
- `kernel/crates/ank-http/src/routes/` (endpoint de configuración inicial)
**Estado:** ✅ Completo

---

## Contexto

`openrouter/free` es el modelo default cuando el usuario no configura nada.
Ese modelo no soporta tool use nativo, tiene rate limits de ~5 req/min, y es
el origen de casi todos los bugs observados en los logs de producción:
- Genera tokens legacy `[SYS_AGENT_SPAWN(...)]` en lugar de function calls
- Retorna 429 Too Many Requests frecuentemente
- Los agentes no pueden ejecutar work real

El installer no pide una API key — el usuario arranca con un sistema roto sin saberlo.

---

## Cambios requeridos

### 1. Sección obligatoria en el installer: configuración de modelo

Agregar en `show_main_menu()`, después de la selección de inference profile, **antes**
de instalar el servicio:

```bash
echo ""
echo -e "${YELLOW}--- CONFIGURACIÓN DEL MOTOR DE IA ---${NC}"
echo -e "Aegis necesita al menos un proveedor de IA para funcionar."
echo ""
echo "  [1] OpenRouter  — acceso a 100+ modelos con una sola API key"
echo "                    (recomendado: claude-3-haiku, gpt-4o-mini)"
echo "                    Obtené tu key en: https://openrouter.ai/keys"
echo ""
echo "  [2] OpenAI      — GPT-4o, GPT-4o-mini"
echo "                    Obtené tu key en: https://platform.openai.com/api-keys"
echo ""
echo "  [3] Anthropic   — Claude 3.5 Haiku, Claude 3 Opus"
echo "                    Obtené tu key en: https://console.anthropic.com/keys"
echo ""
echo "  [4] Configurar después — el sistema arranca pero los agentes"
echo "                           no van a funcionar hasta que configures"
echo "                           un proveedor desde la UI"
echo ""
read -rp "Selección [1-4]: " provider_choice

AI_PROVIDER=""
AI_API_KEY=""
AI_MODEL=""

case "${provider_choice:-4}" in
    1)
        AI_PROVIDER="openrouter"
        read -rp "OpenRouter API Key (sk-or-...): " AI_API_KEY
        echo ""
        echo "Modelo recomendado: anthropic/claude-3-haiku (rápido, económico)"
        echo "Otros: openai/gpt-4o-mini, meta-llama/llama-3.1-8b-instruct:free"
        read -rp "Modelo [default: anthropic/claude-3-haiku]: " AI_MODEL
        AI_MODEL="${AI_MODEL:-anthropic/claude-3-haiku}"
        ;;
    2)
        AI_PROVIDER="openai"
        read -rp "OpenAI API Key (sk-...): " AI_API_KEY
        read -rp "Modelo [default: gpt-4o-mini]: " AI_MODEL
        AI_MODEL="${AI_MODEL:-gpt-4o-mini}"
        ;;
    3)
        AI_PROVIDER="anthropic"
        read -rp "Anthropic API Key (sk-ant-...): " AI_API_KEY
        read -rp "Modelo [default: claude-3-5-haiku-20241022]: " AI_MODEL
        AI_MODEL="${AI_MODEL:-claude-3-5-haiku-20241022}"
        ;;
    4)
        warn "Sin proveedor configurado. Los agentes no funcionarán hasta configurar uno en la UI."
        AI_PROVIDER="none"
        ;;
esac

if [[ -n "$AI_API_KEY" ]]; then
    log "Proveedor: ${AI_PROVIDER}, Modelo: ${AI_MODEL}"
fi
```

### 2. Escribir configuración al `ENV_FILE`

En `install_native()`, después de generar el `ENV_FILE`, agregar:

```bash
if [[ "$AI_PROVIDER" != "none" && -n "$AI_API_KEY" ]]; then
    cat >> "$ENV_FILE" <<EOF
AEGIS_DEFAULT_PROVIDER=${AI_PROVIDER}
AEGIS_DEFAULT_MODEL=${AI_MODEL}
AEGIS_DEFAULT_API_KEY=${AI_API_KEY}
EOF
    success "Motor de IA configurado: ${AI_PROVIDER} / ${AI_MODEL}"
fi
```

### 3. El kernel usa las vars de entorno como modelo default

En `kernel/crates/ank-core/src/router/catalog.rs` (o donde se configura el modelo
por defecto), agregar lectura de `AEGIS_DEFAULT_PROVIDER`, `AEGIS_DEFAULT_MODEL`,
`AEGIS_DEFAULT_API_KEY` al iniciar.

Si estas vars existen, registrar ese modelo como el modelo por defecto con prioridad
sobre `openrouter/free`. Si no existen, mantener el comportamiento actual pero loguear
un warning visible al startup:

```rust
if std::env::var("AEGIS_DEFAULT_API_KEY").is_err() {
    warn!(
        "⚠️  No AI provider configured. Agents will not work correctly. \
         Configure a provider via the UI (Settings → Engine) or reinstall."
    );
}
```

### 4. Mensaje en el summary final del installer

```bash
if [[ "$AI_PROVIDER" != "none" ]]; then
    echo -e "  Motor de IA: ${CYAN}${AI_PROVIDER} / ${AI_MODEL}${NC}"
else
    echo -e "  ${YELLOW}⚠ Motor de IA: No configurado — configurá en Settings → Engine${NC}"
fi
```

---

## Criterios de aceptación

- [x] El installer pregunta por proveedor y API key antes de instalar
- [x] Si el usuario ingresa API key, se guarda en `ENV_FILE` con permisos 640
- [x] El kernel loguea WARNING al startup si no hay API key configurada
- [x] La opción "Configurar después" funciona — el sistema arranca pero los agentes
  muestran mensaje claro de que necesitan configuración
- [x] `shellcheck installer/install.sh` sin errores

## Dependencias

- Ninguna — cambio en installer + lectura de vars de entorno en kernel
