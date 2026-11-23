use super::*;

impl CognitiveHAL {
    // --- CHAT AGENT LEVEL 0 DELEGATION & TASK HELPERS ---

    pub(super) fn normalize_text(text: &str) -> String {
        text.to_lowercase()
            .chars()
            .map(|c| match c {
                'á' | 'à' | 'ä' => 'a',
                'é' | 'è' | 'ë' => 'e',
                'í' | 'ì' | 'ï' => 'i',
                'ó' | 'ò' | 'ö' => 'o',
                'ú' | 'ù' | 'ü' => 'u',
                _ => c,
            })
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub(super) fn assistant_claims_delegation(text: &str) -> bool {
        let norm = Self::normalize_text(text);
        norm.contains("pidiendo al equipo")
            || norm.contains("pidiendo al equipazo")
            || norm.contains("pido al equipo")
            || norm.contains("pedi al equipo")
            || norm.contains("le pedi a")
            || norm.contains("le he pedido a")
            || norm.contains("equipo de aegis")
            || norm.contains("equipazo de aegis")
            || norm.contains("equipo ya esta")
            || norm.contains("equipo esta")
            || norm.contains("equipazo ya esta")
            || norm.contains("equipazo esta")
            || norm.contains("asignando al supervisor")
            || norm.contains("asigne al supervisor")
            || norm.contains("asigne la tarea")
            || norm.contains("asignar la tarea")
            || norm.contains("empujoncito al equipo")
            || norm.contains("empujoncito")
            || norm.contains("supervisor de proyecto")
            || norm.contains("supervisor esta trabajando")
            || norm.contains("te aviso cuando este listo")
            || norm.contains("te avisare cuando")
            || norm.contains("estoy creando tu nuevo proyecto")
            || norm.contains("encargare de generar")
            || norm.contains("manos a la obra con tu nuevo")
            || norm.contains("programar eso para")
            || norm.contains("programando eso para")
            || norm.contains("en cuanto tenga el script listo")
            || norm.contains("en cuanto este listo el script")
            || norm.contains("en cuanto tenga el codigo listo")
            || norm.contains("en cuanto este listo el codigo")
    }

    pub fn is_video_production_request(prompt: &str) -> bool {
        if Self::is_status_query(prompt) {
            return false;
        }
        let lower = prompt.to_lowercase();
        let norm = Self::normalize_text(prompt);

        let explicit_video_markers = [
            "video_produce_auto",
            "video_create_project",
            "video_render_mp4",
            "gemini_studio",
            "free_builtin",
            "hybrid_pro",
            "video con ia",
            "video ia",
            "exportar mp4",
            "exportalo a mp4",
            "exporta el mp4",
        ];
        if explicit_video_markers
            .iter()
            .any(|m| lower.contains(m) || norm.contains(m))
        {
            return true;
        }

        let has_video_noun = norm.split_whitespace().any(|w| {
            matches!(
                w,
                "video" | "videos" | "mp4" | "reel" | "reels" | "short" | "shorts" | "cortometraje"
            )
        }) || lower.contains("9:16")
            || lower.contains("16:9")
            || norm.contains("video studio");

        if !has_video_noun {
            return false;
        }

        let action_verbs = [
            "crea",
            "creame",
            "crear",
            "genera",
            "generame",
            "generar",
            "hace",
            "haceme",
            "hacer",
            "arma",
            "armame",
            "armar",
            "edita",
            "editar",
            "renderiza",
            "renderizar",
            "exporta",
            "exportar",
            "produci",
            "producir",
            "promocionando",
            "promocional",
        ];
        norm.split_whitespace().any(|w| action_verbs.contains(&w))
            || lower.contains("4 escenas")
            || lower.contains("escenas")
    }

    pub fn assistant_claims_video_execution(text: &str) -> bool {
        let norm = Self::normalize_text(text);
        let markers = [
            "aqui va tu video",
            "aqui tienes tu video",
            "aqui tenes tu video",
            "aqui va el intento",
            "aqui va la llamada a la herramienta",
            "va la herramienta",
            "llamar a la herramienta",
            "invocar la herramienta",
            "preparando la invocacion",
            "a punto de invocar la herramienta",
            "video produce auto",
            "videoproduceauto",
            "video create project",
            "voy a crearlo con la herramienta",
            "estoy creando tu nuevo proyecto",
            "creando tu video",
        ];
        markers.iter().any(|m| norm.contains(m))
    }

    pub fn is_video_followup_nudge(prompt: &str) -> bool {
        let norm = Self::normalize_text(prompt);
        let markers = [
            "lo vas a hacer",
            "vas a seguir durmiendo",
            "y donde esta",
            "donde esta el video",
            "no muestra que este haciendo nada",
            "no hace nada",
            "no hizo nada",
            "estas inventando",
            "no enviaste nada",
            "cuanto vas a tardar",
            "hace 10 minutos",
            "hacelo",
            "si hacelo",
            "dale hacelo",
            "generalo",
            "terminalo",
            "proba de nuevo",
            "vamos a que",
        ];
        markers.iter().any(|m| norm.contains(m))
    }

    pub fn resolve_video_request_prompt(
        prompt: &str,
        history: &[ChatMessage],
        assistant_text: &str,
    ) -> Option<String> {
        let trimmed = prompt.trim();
        if Self::is_video_production_request(trimmed) {
            return Some(trimmed.to_string());
        }
        if Self::is_status_query(trimmed) || Self::is_video_advisory_or_followup_query(trimmed) {
            return None;
        }
        if Self::is_video_followup_nudge(trimmed)
            || (!assistant_text.trim().is_empty()
                && Self::assistant_claims_video_execution(assistant_text))
        {
            for msg in history.iter().rev() {
                if msg.role == ChatRole::User {
                    if let Some(ref content) = msg.content {
                        let c = content.trim();
                        if Self::is_video_production_request(c) {
                            return Some(c.to_string());
                        }
                    }
                }
            }
            if !assistant_text.trim().is_empty()
                && Self::assistant_claims_video_execution(assistant_text)
                && !trimmed.is_empty()
            {
                return Some(trimmed.to_string());
            }
        }
        None
    }

    pub fn has_video_context(prompt: &str, history: &[ChatMessage]) -> bool {
        Self::resolve_video_request_prompt(prompt, history, "").is_some()
    }

    pub(super) fn is_status_query(prompt: &str) -> bool {
        let norm = Self::normalize_text(prompt);
        norm == "que paso"
            || norm.starts_with("que paso")
            || norm == "como va"
            || norm.starts_with("como va")
            || norm.contains("novedades")
            || norm.contains("alguna novedad")
            || norm == "estado"
            || norm == "status"
            || norm.starts_with("como venimos")
            || norm.starts_with("termino")
            || norm == "terminaste"
            || norm == "ya esta"
            || norm.starts_with("ya esta listo")
            || norm.starts_with("ya esta terminad")
            || norm.starts_with("ya estan listo")
            || norm.starts_with("ya termino")
    }

    pub fn is_video_advisory_or_followup_query(prompt: &str) -> bool {
        if Self::is_video_production_request(prompt) {
            return false;
        }
        let norm = Self::normalize_text(prompt);
        let advisory_markers = [
            "que necesitas",
            "que necesitaria",
            "salga mejor",
            "salio mal",
            "otras apis",
            "otra api",
            "cual api",
            "que api",
            "como mejorar",
            "por que salio",
            "que motor",
            "cual motor",
            "mejor calidad",
            "conviene api",
            "api o local",
            "gemini pago",
            "tier 1",
        ];
        if advisory_markers.iter().any(|m| norm.contains(m)) {
            return true;
        }
        let media_terms = [
            "video",
            "videos",
            "mp4",
            "reel",
            "escena",
            "escenas",
            "freebuiltin",
            "geministudio",
            "hybridpro",
            "elevenlabs",
            "pexels",
        ];
        norm.contains("calidad") && media_terms.iter().any(|m| norm.contains(m))
    }

    pub(super) fn is_technical_request(prompt: &str) -> bool {
        if Self::is_status_query(prompt)
            || Self::is_video_production_request(prompt)
            || Self::is_video_advisory_or_followup_query(prompt)
        {
            return false;
        }
        let norm = Self::normalize_text(prompt);
        let technical_keywords = [
            "script",
            "codigo",
            "python",
            "rust",
            "javascript",
            "typescript",
            "programa",
            "programar",
            "desarrollar",
            "desarrolla",
            "crea un",
            "crear un",
            "hace un",
            "hacer un",
            "implementa",
            "implementar",
            "escribe un",
            "escribir un",
            "analiza",
            "analizar",
            "corrige",
            "corregir",
            "revisa",
            "revisar",
            "refactor",
            "bug",
            "endpoint",
            "clona",
            "clonar",
            "clones",
            "repositorio",
            "compila",
            "compilar",
            "depura",
            "depurar",
            "pull request",
        ];
        technical_keywords.iter().any(|kw| norm.contains(kw))
    }

    pub(super) fn has_technical_context(prompt: &str, history: &[ChatMessage]) -> bool {
        if Self::is_status_query(prompt)
            || Self::is_video_production_request(prompt)
            || Self::is_video_advisory_or_followup_query(prompt)
        {
            return false;
        }
        if Self::is_technical_request(prompt) {
            return true;
        }
        let short_reply = prompt.trim().len() < 50;
        if short_reply {
            for msg in history.iter().rev() {
                if msg.role == ChatRole::User {
                    if let Some(ref content) = msg.content {
                        if Self::is_technical_request(content) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub(super) fn derive_task_scope(prompt: &str, history: &[ChatMessage]) -> String {
        let trimmed = prompt.trim();
        if trimmed.len() < 50 {
            for msg in history.iter().rev() {
                if msg.role == ChatRole::User {
                    if let Some(ref content) = msg.content {
                        if Self::is_technical_request(content) {
                            return format!("{}. Parámetros/Detalles: {}", content.trim(), trimmed);
                        }
                    }
                }
            }
        }
        trimmed.to_string()
    }

    pub(super) fn derive_task_project_name(scope: &str) -> String {
        let lower = scope.to_lowercase();
        if lower.contains("python") && (lower.contains("disco") || lower.contains("espacio")) {
            "Script Espacio Disco".to_string()
        } else if lower.contains("python") {
            "Script Python".to_string()
        } else if lower.contains("script") {
            "Script Automatización".to_string()
        } else {
            let clean: String = scope
                .chars()
                .take(35)
                .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
                .collect();
            let trimmed = clean.trim();
            if trimmed.is_empty() {
                "Proyecto".to_string()
            } else {
                trimmed.to_string()
            }
        }
    }

    /// Detecta si un token parece una clave API real (Groq, Gemini, OpenRouter, OpenAI, Anthropic, xAI).
    pub(super) fn is_likely_api_key_token(token: &str) -> bool {
        let clean = token.trim_matches(|c: char| {
            matches!(
                c,
                '"' | '\'' | '`' | ',' | '.' | ';' | ':' | '(' | ')' | '[' | ']'
            )
        });
        if clean.len() < 20 {
            return false;
        }
        clean.starts_with("gsk_")
            || clean.starts_with("AIza")
            || clean.starts_with("sk-or-")
            || clean.starts_with("sk-ant-")
            || clean.starts_with("xai-")
            || clean.starts_with("sk-")
    }

    /// Extrae la primera clave API reconocible dentro de un texto libre del usuario.
    pub fn extract_api_key_from_text(text: &str) -> Option<String> {
        Self::extract_all_api_keys_from_text(text)
            .into_iter()
            .map(|(_, k)| k)
            .next()
    }

    /// Extrae todas las claves API reconocibles dentro del texto junto con su proveedor inferido.
    pub fn extract_all_api_keys_from_text(text: &str) -> Vec<(String, String)> {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for line in text.lines() {
            let line_trim = line.trim();
            for word in line_trim.split_whitespace() {
                let clean = word.trim_matches(|c: char| {
                    matches!(
                        c,
                        '"' | '\''
                            | '`'
                            | ','
                            | '.'
                            | ';'
                            | ':'
                            | '('
                            | ')'
                            | '['
                            | ']'
                            | '{'
                            | '}'
                            | '='
                    )
                });
                let is_key = Self::is_likely_api_key_token(clean)
                    || (clean.len() >= 24
                        && (line_trim.to_lowercase().contains("ollama")
                            || line_trim.to_lowercase().contains("elevenlabs")
                            || (clean.contains('.') && clean.len() >= 30)));

                if is_key && seen.insert(clean.to_string()) {
                    let prov = Self::infer_provider_from_key_or_text("", clean, line_trim);
                    results.push((prov, clean.to_string()));
                }
            }
        }
        results
    }

    /// Enmascara una clave API individual (`gsk_••••1234`) para no exponerla en logs ni respuestas.
    pub fn mask_single_key(key: &str) -> String {
        let trimmed = key.trim();
        let chars: Vec<char> = trimmed.chars().collect();
        if chars.len() <= 8 {
            return "••••".to_string();
        }
        let prefix: String = chars[..4].iter().collect();
        let suffix: String = chars[chars.len() - 4..].iter().collect();
        format!("{}••••{}", prefix, suffix)
    }

    /// Enmascara cualquier clave API presente en un texto antes de persistirlo en historial o memoria.
    pub fn mask_api_keys_in_text(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut chars = text.char_indices().peekable();

        while let Some(&(start, ch)) = chars.peek() {
            let is_ws = ch.is_whitespace();
            let mut end = text.len();
            while let Some(&(idx, next_ch)) = chars.peek() {
                if next_ch.is_whitespace() != is_ws {
                    end = idx;
                    break;
                }
                chars.next();
            }
            let slice = &text[start..end];
            if is_ws {
                out.push_str(slice);
            } else {
                let clean = slice.trim_matches(|c: char| {
                    matches!(
                        c,
                        '"' | '\'' | '`' | ',' | '.' | ';' | ':' | '(' | ')' | '[' | ']'
                    )
                });
                if Self::is_likely_api_key_token(clean)
                    || (clean.len() >= 30
                        && (clean.contains('.') || clean.chars().all(|c| c.is_ascii_hexdigit())))
                {
                    out.push_str(&slice.replace(clean, &Self::mask_single_key(clean)));
                } else {
                    out.push_str(slice);
                }
            }
        }

        out
    }

    /// Infiere el ID canónico del proveedor a partir del prefijo de la API key o del texto.
    pub fn infer_provider_from_key_or_text(
        raw_provider: &str,
        api_key: &str,
        hint_text: &str,
    ) -> String {
        let k = api_key.trim();
        if k.starts_with("gsk_") {
            return "groq".to_string();
        }
        if k.starts_with("AIza") {
            return "gemini".to_string();
        }
        if k.starts_with("sk-or-") {
            return "openrouter".to_string();
        }
        if k.starts_with("sk-ant-") {
            return "anthropic".to_string();
        }
        if k.starts_with("xai-") {
            return "xai".to_string();
        }

        if !raw_provider.trim().is_empty() {
            return crate::router::normalize_provider_id(raw_provider);
        }

        let lower = hint_text.to_lowercase();
        for (kw, prov) in [
            ("elevenlabs", "elevenlabs"),
            ("openrouter", "openrouter"),
            ("gemini", "gemini"),
            ("google", "gemini"),
            ("groq", "groq"),
            ("anthropic", "anthropic"),
            ("claude", "anthropic"),
            ("deepseek", "deepseek"),
            ("mistral", "mistral"),
            ("ollama", "ollama_cloud"),
            ("xai", "xai"),
            ("grok", "xai"),
            ("openai", "openai"),
            ("gpt", "openai"),
        ] {
            if lower.contains(kw) {
                return prov.to_string();
            }
        }

        if k.starts_with("sk-") {
            return "openai".to_string();
        }

        String::new()
    }

    /// Retorna `(default_model, default_api_url)` para cada proveedor soportado.
    pub fn default_model_and_url_for_provider(provider: &str) -> (&'static str, &'static str) {
        match provider {
            "groq" => ("openai/gpt-oss-120b", "https://api.groq.com/openai/v1"),
            "gemini" => (
                "gemini-2.5-flash",
                "https://generativelanguage.googleapis.com/v1beta/openai",
            ),
            "openrouter" => ("openrouter/free", "https://openrouter.ai/api/v1"),
            "ollama_cloud" => ("gpt-oss:120b", "https://ollama.com"),
            "ollama" => ("llama3.2", "http://localhost:11434/v1"),
            "openai" => ("gpt-4o-mini", "https://api.openai.com/v1"),
            "anthropic" => ("claude-3-5-sonnet-latest", "https://api.anthropic.com/v1"),
            "deepseek" => ("deepseek-chat", "https://api.deepseek.com/v1"),
            "xai" => ("grok-2-latest", "https://api.x.ai/v1"),
            "mistral" => ("mistral-small-latest", "https://api.mistral.ai/v1"),
            _ => ("gpt-4o-mini", "https://api.openai.com/v1"),
        }
    }
}
