# CORE-117 — Agregar step de validación de stubs gRPC Python en CI

**Epic:** Epic 35 — Hardening Pre-Launch
**Agente:** Kernel Engineer (CI) / DevOps Engineer
**Prioridad:** 🟡 MEDIA — Calidad CI
**Estado:** TODO
**Origen:** REC-011 / Auditoría multi-modelo 2026-04-16

---

## Contexto

Los stubs Python `kernel_pb2.py` y `kernel_pb2_grpc.py` (generados desde
`kernel/proto/kernel.proto`) deben regenerarse manualmente cuando el proto
cambia. No existe ningún gate en CI que detecte divergencia entre el proto
fuente y los stubs commiteados.

Historial: en el smoke test de 2026-04-06 se encontraron 3 bugs críticos de
alineación protocolar en el punto BFF→Kernel, todos atribuibles a stubs
desactualizados o headers incorrectos.

Aunque el BFF Python ya no existe en `Aegis-Core` (ADR-031: "BFF Python es
legacy"), el proto sigue siendo el contrato gRPC y puede haber consumidores
externos (CLI tools, integraciones) que dependan de stubs actualizados.

**Objetivo:** ningún cambio a `kernel.proto` puede mergearse sin regenerar
los stubs correspondientes.

## Cambios requeridos

**Archivo:** `.github/workflows/pr_check.yml`

### Opción A — Step de validación en CI (recomendada)

Agregar un job que regenere los stubs y falle si hay diff:

```yaml
proto-stubs-check:
  name: Proto stubs sync check
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4

    - name: Install protoc + grpc_tools
      run: |
        sudo apt-get install -y protobuf-compiler
        pip install grpcio-tools

    - name: Regenerate Python stubs
      run: |
        python -m grpc_tools.protoc \
          -I kernel/proto \
          --python_out=/tmp/stubs_check \
          --grpc_python_out=/tmp/stubs_check \
          kernel/proto/kernel.proto

    - name: Check for diff (if stubs are tracked)
      run: |
        # Solo aplica si los stubs están commiteados en el repo
        # Si no están, este step documenta cómo regenerarlos
        echo "Proto stubs check: if you changed kernel.proto, regenerate stubs."
        echo "See CONTRIBUTING.md for instructions."
```

### Opción B — Documentar en `CONTRIBUTING.md` (mínimo viable)

Si los stubs Python no están en el repo (porque el BFF Python está deprecado),
agregar en `CONTRIBUTING.md` una sección:

```markdown
## Cambios al contrato gRPC

Si modificás `kernel/proto/kernel.proto` o `kernel/proto/siren.proto`:

1. Regenerar stubs Python (para referencias y herramientas externas):
   ```bash
   pip install grpcio-tools
   python -m grpc_tools.protoc \
     -I kernel/proto \
     --python_out=./stubs \
     --grpc_python_out=./stubs \
     kernel/proto/kernel.proto
   ```

2. Verificar que el BFF (si aplica) usa los nuevos stubs antes del push.

3. Agregar en el commit message: `chore(proto): update kernel.proto - <descripción>`
```

**Implementar Opción B como mínimo. Opción A si los stubs están trackeados en el repo.**

## Criterios de aceptación

- [ ] `CONTRIBUTING.md` tiene una sección explicando qué hacer cuando cambia `kernel.proto`
- [ ] Si los stubs están commiteados: el CI falla cuando hay divergencia entre proto y stubs
- [ ] Si los stubs no están commiteados: el CI tiene un step que documenta/valida que el proto compiló correctamente con `protoc`
- [ ] El step no agrega más de 30 segundos al tiempo de CI

## Dependencias

Ninguna bloqueante. Puede implementarse de forma independiente.
