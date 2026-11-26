# CORE-340 — Windows: `aegis update` canal stable descargaba de un tag "latest" inexistente (404)

**Tipo:** fix
**Prioridad:** Alta
**Épica:** EPIC 56 — Public MVP / Thesis Validation
**Estado:** ✅ Done
**Asignado a:** DevOps Engineer

---

## Problema

`cmd_update` en `installer/aegis.ps1` construía la URL de descarga stable como `releases/download/latest/ank-server-windows-x86_64.zip`. GitHub no tiene un tag literal `latest` en este repo, así que la descarga devolvía 404 y el update abortaba (con restart del servicio, pero sin actualizar). El canal stable de Windows **nunca funcionó**: antes de CORE-339 era inalcanzable (bug de binding de `--stable`), y tras CORE-339 pasó a ser el default, exponiendo el 404.

El endpoint `/releases/latest` de GitHub tampoco sirve directo: está contaminado por los releases por-crate de release-please (`ank-server-vX.Y.Z`, `shell-ui-vX.Y.Z`), que no llevan binarios del OS — solo los tags `v*` los tienen. El wrapper bash ya resolvía esto (CORE-329) filtrando el tag `v*` no-prerelease más nuevo vía API.

## Solución aplicada

`cmd_update` (aegis.ps1) replica la lógica del wrapper bash: consulta `api.github.com/repos/.../releases`, toma el primer release no-prerelease con tag `v*`, y usa ese tag para las URLs de binario/UI/agents. Fallback pineado a `v0.2.0` si la API es inalcanzable o rate-limiteada. El canal y tag resueltos se muestran en la salida del comando.

## Criterios de aceptación

- [x] `aegis update` (Windows, canal stable) resuelve un tag real `v*` y descarga assets existentes.
- [x] Fallback funcional sin API (tag pineado).
- [x] Parseo PS 5.1 limpio + BOM intacto (gate CI `installer-windows`).
- [x] Resolución verificada contra la API real (devuelve `v0.2.0`; tomará `v0.2.1` al publicarse).
