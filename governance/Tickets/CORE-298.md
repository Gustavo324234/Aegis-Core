# CORE-298 — Kernel: CatalogSyncer actualiza modelos free de OpenRouter al registrar key

## Metadata
- **Epic:** 51 — Model Intelligence
- **Tipo:** feat
- **Prioridad:** Alta
- **Asignado a:** Kernel Engineer
- **Estado:✅ Done

## Contexto

El catálogo de modelos (`models.yaml`) es estático. Cuando el admin registra una
key de OpenRouter, los modelos disponibles en esa key (especialmente los free tier,
que cambian frecuentemente) no se reflejan en el catálogo hasta que se actualiza
manualmente.

Resultado: el CMR no puede rutear hacia modelos free que existen en OpenRouter
pero no están en el catálogo.

## Comportamiento actual

1. Admin registra key de OpenRouter via `POST /api/router/keys/global`
2. El handler verifica la key contra `/api/providers/models`
3. Los modelos verificados se guardan en `active_models` de la `ApiKeyEntry`
4. El catálogo **no se actualiza** — los modelos nuevos son invisibles para el CMR

## Comportamiento deseado

Cuando se registra o actualiza una key de OpenRouter:
1. Consultar `GET https://openrouter.ai/api/v1/models` con la key
2. Filtrar modelos con `pricing.prompt == "0"` (free tier)
3. Para cada modelo free no presente en el catálogo → agregar entrada dinámica
4. El CMR puede rutear hacia esos modelos en el próximo request

## Cambios requeridos

### `kernel/crates/ank-core/src/router/syncer.rs`

Agregar función `sync_openrouter_free_models(key: &str, catalog: &ModelCatalog)`:

```rust
pub async fn sync_openrouter_free_models(
    api_key: &str,
    catalog: &Arc<ModelCatalog>,
) -> anyhow::Result<usize> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://openrouter.ai/api/v1/models")
        .header("Authorization", format!("Bearer {}", api_key))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    let models = resp["data"].as_array().ok_or_else(|| anyhow::anyhow!("no data array"))?;

    let mut added = 0usize;
    for model in models {
        let prompt_price = model["pricing"]["prompt"].as_str().unwrap_or("1");
        if prompt_price != "0" {
            continue; // solo free tier
        }

        let model_id = model["id"].as_str().unwrap_or("").to_string();
        if model_id.is_empty() {
            continue;
        }

        // Si ya existe en el catálogo, no duplicar
        if catalog.find(&model_id).await.is_some() {
            continue;
        }

        let context = model["context_length"].as_u64().unwrap_or(131072) as u32;
        let name = model["name"].as_str().unwrap_or(&model_id).to_string();

        let entry = ModelEntry {
            model_id: model_id.clone(),
            provider: "openrouter".to_string(),
            display_name: format!("{} (free)", name),
            context_window: context,
            cost_input_per_mtok: 0.0,
            cost_output_per_mtok: 0.0,
            supports_tools: false,
            supports_json_mode: false,
            is_local: false,
            avg_latency_ms: Some(2500),
            free_tier_rpm: Some(20),
            free_tier_rpd: Some(200),
            tool_use_support: ToolUseSupport::None,
            task_scores: TaskScores {
                chat: 3, coding: 3, planning: 3,
                analysis: 3, summarization: 3, extraction: 3,
            },
        };

        catalog.add_entry(entry).await;
        added += 1;
    }

    Ok(added)
}
```

### `kernel/crates/ank-http/src/routes/providers.rs`

En el handler `add_global_key` (o `update_global_key`), después de guardar la key
exitosamente, si el provider es `"openrouter"`:

```rust
if entry.provider == "openrouter" {
    let catalog = state.cognitive_router.catalog_ref().await;
    match syncer::sync_openrouter_free_models(&entry.api_key, &catalog).await {
        Ok(n) if n > 0 => info!("CatalogSyncer: added {} free OpenRouter models", n),
        Ok(_) => {},
        Err(e) => warn!("CatalogSyncer: failed to sync OpenRouter free models: {}", e),
    }
}
```

### `kernel/crates/ank-core/src/router/catalog.rs`

Agregar método `add_entry` si no existe:

```rust
pub async fn add_entry(&self, entry: ModelEntry) {
    let mut entries = self.entries.write().await;
    entries.push(entry);
}

pub async fn catalog_ref(&self) -> Arc<ModelCatalog> {
    // Retornar Arc<ModelCatalog> para pasar al syncer
    // Verificar si ya existe o agregar al AppState
}
```

## Criterios de aceptación

- [ ] Al registrar una key de OpenRouter, el catálogo se actualiza con los modelos free disponibles
- [ ] Los modelos ya presentes en el catálogo no se duplican
- [ ] El CMR puede rutear hacia un modelo free recién agregado en el request siguiente
- [ ] Si la sync falla (timeout, error de red), la key se guarda igual — la sync es best-effort
- [ ] `cargo build --workspace` pasa

## Dependencias

- Ninguna
