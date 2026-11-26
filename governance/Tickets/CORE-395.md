# CORE-395: CI Verde — Reparar `cargo clippy`, `cargo fmt` y ESLint en `shell/ui`

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 61
- **Asignado a:** Kernel Engineer + Shell Engineer

---

## Description

Devolver el pipeline de integración continua (`pr_check.yml`) a estado verde corrigiendo los errores y advertencias detectados en la auditoría técnica:
- 17 errores de `cargo clippy` en `ank-core` (incluyendo el bug lógico `clippy::overly_complex_bool_expr` en `video_studio/store.rs`, bloques `if` colapsables e inicialización `Default` en `chal/mod.rs` y `syscalls/mod.rs`, `needless_borrow` y `too_many_arguments` en `google_calendar.rs`, `video_studio/providers.rs` y `video_studio/store.rs`, e `if_same_then_else` / `explicit_counter_loop` / `unnecessary_lazy_evaluations`).
- Diferencias de formato reportadas por `cargo fmt --all -- --check`.
- Race condition de entorno global `AEGIS_DATA_DIR` entre tests paralelos de `ank-core` (`vcm::tests::test_vcm_file_omission_on_overflow` y `video_studio::tests::test_video_project_lifecycle_and_creator_script_bridge`), resuelta mediante guard RAII `ScopedTestDataDir`.
- 1 error (`@typescript-eslint/no-explicit-any` en `ProvidersTab.tsx`) y 3 warnings (`react-refresh/only-export-components` en `AssistantAvatar.tsx` y `VideoStudioPanel.tsx`, `react-hooks/exhaustive-deps` en `PersonalAssistantDrawer.tsx`) en `shell/ui` con `--max-warnings 0`.

## Archivos Modificados

- **Rust (`ank-core` / workspace `cargo fmt`):**
  - `kernel/crates/ank-core/src/chal/mod.rs`
  - `kernel/crates/ank-core/src/syscalls/mod.rs`
  - `kernel/crates/ank-core/src/oauth/google_calendar.rs`
  - `kernel/crates/ank-core/src/video_studio/providers.rs`
  - `kernel/crates/ank-core/src/video_studio/store.rs`
  - `kernel/crates/ank-core/src/enclave/mod.rs` (`ScopedTestDataDir` RAII + aislamiento de tests)
  - `kernel/crates/ank-core/src/vcm/mod.rs`, `kernel/crates/ank-core/src/video_studio/mod.rs`, `kernel/crates/ank-core/src/whatsapp/mod.rs`
  - Archivos adicionales formateados por `cargo fmt --all` (`agents/antigravity_runner.rs`, `agents/orchestrator.rs`, `agents/tool_registry.rs`, etc.).
- **UI (`shell/ui`):**
  - `shell/ui/src/constants/avatarPresets.ts` (nuevo, extracción de `AVATAR_PRESETS` / `AvatarPreset`)
  - `shell/ui/src/components/AssistantAvatar.tsx`
  - `shell/ui/src/components/SettingsPanel.tsx`
  - `shell/ui/src/components/ProvidersTab.tsx`
  - `shell/ui/src/components/VideoStudioPanel.tsx`
  - `shell/ui/src/components/PersonalAssistantDrawer.tsx`

## Acceptance Criteria

- [x] `cargo fmt --all -- --check` finaliza con exit code `0`.
- [x] `cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -A clippy::double-must-use -A clippy::result-large-err` finaliza con exit code `0`.
- [x] `cargo test --workspace` finaliza con exit code `0`.
- [x] `npm run lint` (`--max-warnings 0`) y `npm run build` en `shell/ui` finalizan con exit code `0`.
