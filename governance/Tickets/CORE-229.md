# CORE-229 — fix(installer): agents config no se despliega en producción

**Tipo:** fix  
**Prioridad:** Alta  
**Asignado a:** DevOps Engineer  
**Epic:** Bugs pre-lanzamiento  
**Estado:** ✅ Completo

---

## Contexto

El `installer/install.sh` descarga el binario `ank-server` y los assets de UI desde
GitHub Releases, pero **nunca copia los archivos `kernel/config/agents/*.md`** al
servidor. Como resultado:

1. El `InstructionLoader` arranca con WARN en lugar de INFO en cada inicio del servicio
2. Los archivos de instrucciones no son editables en producción sin acceso al repo
3. La variable `AEGIS_AGENTS_CONFIG_DIR` nunca se setea en `/etc/aegis/aegis.env`

Los archivos afectados son:
- `kernel/config/agents/chat_agent.md`
- `kernel/config/agents/project_supervisor.md`
- `kernel/config/agents/supervisor.md`
- `kernel/config/agents/specialist.md`

**Nota:** Este bug es secundario respecto a CORE-226 (los `.md` aún no están conectados
al path del Chat Agent). Una vez resuelto CORE-226, este ticket permite que los
archivos sean editables en producción sin recompilar.

---

## Cambios requeridos

### 1. Incluir `agents-config.tar.gz` en el Release workflow

**Archivo:** `.github/workflows/publish-native.yml`

En el job de release/publish, crear y subir un artefacto adicional que contenga los
archivos de configuración de agentes:

```bash
tar -czf agents-config.tar.gz -C kernel/config agents/
```

Incluir este archivo en el upload de artefactos junto con el binario y el UI dist.

### 2. Actualizar `install.sh` — función `install_native()`

**Archivo:** `installer/install.sh`

Después de descargar el binario, agregar:

```bash
# Install agent instruction files (editable without recompile)
AGENTS_CONFIG_DIR="${CONFIG_DIR}/agents"
mkdir -p "${AGENTS_CONFIG_DIR}"

log "Downloading agents config..."
if curl -L --fail --silent \
    "${release_url}/agents-config.tar.gz" -o "/tmp/agents-config.tar.gz"; then
    tar -xzf "/tmp/agents-config.tar.gz" -C "${AGENTS_CONFIG_DIR}" --strip-components=1
    rm "/tmp/agents-config.tar.gz"
    chown -R aegis:aegis "${AGENTS_CONFIG_DIR}"
    chmod 644 "${AGENTS_CONFIG_DIR}"/*.md
    success "Agent configs → ${AGENTS_CONFIG_DIR}"
else
    warn "Could not download agents-config.tar.gz — using compiled fallbacks."
fi
```

En la generación del `aegis.env`, agregar la variable:

```bash
AEGIS_AGENTS_CONFIG_DIR=${AGENTS_CONFIG_DIR}
```

Si el tarball no se pudo descargar, **no fallar** (continuar con el fallback compilado).

### 3. Agregar `AEGIS_AGENTS_CONFIG_DIR` al `.env.example`

**Archivo:** `installer/.env.example`

Agregar:
```
AEGIS_AGENTS_CONFIG_DIR=/etc/aegis/agents
```

---

## Criterios de aceptación

- [x] Después de ejecutar `install.sh`, existe `/etc/aegis/agents/chat_agent.md` en el servidor
- [x] `/etc/aegis/aegis.env` contiene `AEGIS_AGENTS_CONFIG_DIR=/etc/aegis/agents`
- [x] Los logs de arranque muestran INFO (no WARN) para la carga de instrucciones
- [x] Si `agents-config.tar.gz` no está disponible en el release, el installer termina exitosamente usando el fallback compilado

---

## Dependencias

Ninguna. Puede trabajarse en paralelo con CORE-226, 227, 228.
Requiere que el Release workflow genere `agents-config.tar.gz` antes de que este fix
pueda verificarse end-to-end en producción.
