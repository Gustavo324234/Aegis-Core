# CORE-099 — CI: Detección de Stubs gRPC Python Desincronizados

**Epic:** 35 — Hardening Post-Launch  
**Área:** `.github/workflows/` + `kernel/proto/`  
**Agente:** DevOps Engineer  
**Prioridad:** P2 — Calidad CI  
**Estado:** TODO  
**Origen:** REC-011 / claude-sonnet-4-6 sección 2.3

---

## Contexto

Los stubs Python `kernel_pb2.py` y `kernel_pb2_grpc.py` deben regenerarse manualmente
cuando cambia `kernel/proto/kernel.proto`. No hay gate en CI que detecte divergencia.
Históricamente esto produjo 3 bugs críticos de integración descubiertos solo durante
smoke test manual (2026-04-06).

**Nota:** Aegis-Core eliminó el BFF Python (ADR-031), por lo que actualmente estos
stubs son usados principalmente por `ank-cli` u otras herramientas externas. Si ya
no existen stubs Python en el repo, documentarlo y cerrar este ticket como N/A.
Si existen, implementar el gate.

---

## Trabajo requerido

1. Verificar si existen archivos `*_pb2.py` o `*_pb2_grpc.py` en el repo:
   ```bash
   find . -name "*_pb2*.py" -not -path "./.git/*"
   ```

2. **Si no existen:** Cerrar este ticket como N/A y documentar en
   `governance/AEGIS_CONTEXT.md` que no hay stubs Python en el monorepo.

3. **Si existen:** Agregar un step en el workflow de CI que:
   a. Regenera los stubs con `python -m grpc_tools.protoc`
   b. Ejecuta `git diff --exit-code` sobre los archivos generados
   c. Falla el CI con mensaje claro si hay diferencia:
      `"Proto stubs out of sync. Run: make proto-gen and commit the result."`

4. Agregar target `proto-gen` al `Makefile` raíz que regenera todos los stubs.

5. Documentar en `governance/README.md` o en un `CONTRIBUTING.md` que cualquier
   cambio a `kernel/proto/*.proto` requiere ejecutar `make proto-gen` antes del commit.

---

## Criterios de aceptación

- [ ] CI falla si los stubs Python existen y están desincronizados con el proto
- [ ] `make proto-gen` regenera los stubs correctamente desde proto fuente
- [ ] `shellcheck` pasa en cualquier script nuevo de CI
- [ ] Si no hay stubs Python: `governance/AEGIS_CONTEXT.md` lo documenta

---

## Dependencias

Ninguna.
