use crate::enclave::TenantDB;
use chrono::Local;
use serde_json::{json, Value};
use uuid::Uuid;

fn now_iso() -> String {
    Local::now().format("%Y-%m-%d %H:%M").to_string()
}

fn new_short_id(prefix: &str) -> String {
    let raw = Uuid::new_v4().simple().to_string();
    format!("{}-{}", prefix, &raw[..8])
}

fn read_state(db: &TenantDB, module_id: &str, default_state: Value) -> Result<Value, String> {
    let key = format!("module_data:{}", module_id);
    match db
        .get_kv(&key)
        .map_err(|e| format!("DB read error: {}", e))?
    {
        Some(raw) if !raw.trim().is_empty() => {
            Ok(serde_json::from_str::<Value>(&raw).unwrap_or(default_state))
        }
        _ => Ok(default_state),
    }
}

fn write_state(db: &TenantDB, module_id: &str, state: &Value) -> Result<(), String> {
    let key = format!("module_data:{}", module_id);
    db.set_kv(&key, &state.to_string())
        .map_err(|e| format!("DB write error: {}", e))
}

fn find_item_index(
    items: &[Value],
    id_opt: Option<&str>,
    key_fields: &[&str],
    match_str: Option<&str>,
) -> Option<usize> {
    if let Some(id) = id_opt.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(pos) = items
            .iter()
            .position(|it| it.get("id").and_then(Value::as_str) == Some(id))
        {
            return Some(pos);
        }
    }
    if let Some(query) = match_str.map(str::trim).filter(|s| !s.is_empty()) {
        let q_lower = query.to_lowercase();
        for field in key_fields {
            if let Some(pos) = items.iter().position(|it| {
                it.get(*field)
                    .and_then(Value::as_str)
                    .map(|v| v.trim().to_lowercase() == q_lower)
                    .unwrap_or(false)
            }) {
                return Some(pos);
            }
        }
        for field in key_fields {
            let matches: Vec<usize> = items
                .iter()
                .enumerate()
                .filter_map(|(idx, it)| {
                    let val = it.get(*field).and_then(Value::as_str)?.to_lowercase();
                    if val.contains(&q_lower) {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();
            if matches.len() == 1 {
                return Some(matches[0]);
            }
        }
    }
    None
}

fn set_str_if_present(obj: &mut serde_json::Map<String, Value>, args: &Value, field: &str) {
    if let Some(v) = args.get(field).and_then(Value::as_str) {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            obj.insert(field.to_string(), json!(trimmed));
        }
    }
}

/// Returns `true` if `module_id` has an embedded native SQLCipher runtime in the kernel.
pub fn supports_native_runtime(module_id: &str) -> bool {
    matches!(
        module_id,
        "aegis.domain.creator"
            | "aegis.domain.video"
            | "aegis.domain.dev"
            | "aegis.domain.business"
    )
}

/// Executes a tool belonging to a native/built-in domain module inside the tenant's encrypted SQLCipher DB.
pub fn execute_native_module_tool(
    module_id: &str,
    tool_name: &str,
    args_json: &str,
    tenant_id: &str,
    session_key: &str,
) -> Result<String, String> {
    let args: Value = if args_json.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(args_json)
            .map_err(|e| format!("Invalid tool arguments JSON: {}", e))?
    };

    let db = TenantDB::open(tenant_id, session_key)
        .map_err(|e| format!("Failed to open TenantDB for module '{}': {}", module_id, e))?;

    let result = match module_id {
        "aegis.domain.creator" => execute_creator_tool(&db, tool_name, &args)?,
        "aegis.domain.dev" => execute_dev_tool(&db, tool_name, &args)?,
        "aegis.domain.business" => execute_business_tool(&db, tool_name, &args)?,
        _ => {
            return Err(format!(
                "Module '{}' does not have a native local runtime",
                module_id
            ));
        }
    };

    Ok(result.to_string())
}

// ── 1. Aegis Creator Studio (`aegis.domain.creator`) ──────────────────────────

fn creator_default_state() -> Value {
    json!({ "ideas": [], "scripts": [], "calendar": [] })
}

fn creator_build_overview(state: &Value) -> Value {
    let ideas = state
        .get("ideas")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let scripts = state
        .get("scripts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let calendar = state
        .get("calendar")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let ready_to_film = scripts
        .iter()
        .filter(|s| s.get("status").and_then(Value::as_str) == Some("ready_to_film"))
        .count();
    let scheduled_posts = calendar
        .iter()
        .filter(|c| c.get("status").and_then(Value::as_str) != Some("published"))
        .count();

    let mut by_platform: serde_json::Map<String, Value> = serde_json::Map::new();
    for item in ideas.iter().chain(scripts.iter()) {
        let p = item
            .get("platform")
            .and_then(Value::as_str)
            .unwrap_or("tiktok_reels")
            .to_string();
        let next = by_platform.get(&p).and_then(Value::as_u64).unwrap_or(0) + 1;
        by_platform.insert(p, json!(next));
    }

    json!({
        "success": true,
        "ideas": ideas,
        "scripts": scripts,
        "calendar": calendar,
        "stats": {
            "total_ideas": ideas.len(),
            "total_scripts": scripts.len(),
            "ready_to_film": ready_to_film,
            "scheduled_posts": scheduled_posts,
            "by_platform": Value::Object(by_platform),
        }
    })
}

fn execute_creator_tool(db: &TenantDB, tool_name: &str, args: &Value) -> Result<Value, String> {
    let mut state = read_state(db, "aegis.domain.creator", creator_default_state())?;

    match tool_name {
        "creator_get_overview" => Ok(creator_build_overview(&state)),
        "creator_manage_idea" => {
            let action = args
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("add")
                .trim()
                .to_lowercase();
            let ideas = state["ideas"]
                .as_array_mut()
                .ok_or_else(|| "Corrupt creator ideas state".to_string())?;

            match action.as_str() {
                "list" => {
                    let plat_f = args.get("platform").and_then(Value::as_str);
                    let status_f = args.get("status").and_then(Value::as_str);
                    let filtered: Vec<Value> = ideas
                        .iter()
                        .filter(|it| {
                            plat_f.is_none_or(|p| {
                                it.get("platform").and_then(Value::as_str) == Some(p)
                            }) && status_f
                                .is_none_or(|s| it.get("status").and_then(Value::as_str) == Some(s))
                        })
                        .cloned()
                        .collect();
                    Ok(json!({ "success": true, "ideas": filtered, "count": filtered.len() }))
                }
                "delete" => {
                    let idx = find_item_index(
                        ideas,
                        args.get("id").and_then(Value::as_str),
                        &["title"],
                        args.get("title").and_then(Value::as_str),
                    )
                    .ok_or_else(|| "No se encontró la idea a eliminar".to_string())?;
                    let removed = ideas.remove(idx);
                    write_state(db, "aegis.domain.creator", &state)?;
                    Ok(json!({ "success": true, "deleted": removed }))
                }
                "add" | "update" => {
                    let id_opt = args.get("id").and_then(Value::as_str);
                    let title_opt = args.get("title").and_then(Value::as_str);
                    if let Some(idx) = find_item_index(ideas, id_opt, &["title"], title_opt) {
                        if let Some(obj) = ideas[idx].as_object_mut() {
                            set_str_if_present(obj, args, "title");
                            set_str_if_present(obj, args, "platform");
                            set_str_if_present(obj, args, "hook");
                            set_str_if_present(obj, args, "pillar");
                            set_str_if_present(obj, args, "notes");
                            set_str_if_present(obj, args, "status");
                            set_str_if_present(obj, args, "priority");
                            obj.insert("updated_at".into(), json!(now_iso()));
                        }
                        let updated = ideas[idx].clone();
                        write_state(db, "aegis.domain.creator", &state)?;
                        Ok(json!({ "success": true, "mode": "updated", "idea": updated }))
                    } else {
                        if action == "update" {
                            return Err(
                                "No se encontró la idea especificada para actualizar".into()
                            );
                        }
                        let title = title_opt
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .ok_or_else(|| {
                                "El campo 'title' es requerido para agregar una idea".to_string()
                            })?;
                        let now = now_iso();
                        let item = json!({
                            "id": new_short_id("idea"),
                            "title": title,
                            "platform": args.get("platform").and_then(Value::as_str).unwrap_or("tiktok_reels"),
                            "hook": args.get("hook").and_then(Value::as_str).unwrap_or(""),
                            "pillar": args.get("pillar").and_then(Value::as_str).unwrap_or("General"),
                            "notes": args.get("notes").and_then(Value::as_str).unwrap_or(""),
                            "status": args.get("status").and_then(Value::as_str).unwrap_or("idea"),
                            "priority": args.get("priority").and_then(Value::as_str).unwrap_or("medium"),
                            "created_at": now,
                            "updated_at": now,
                        });
                        ideas.insert(0, item.clone());
                        write_state(db, "aegis.domain.creator", &state)?;
                        Ok(json!({ "success": true, "mode": "created", "idea": item }))
                    }
                }
                _ => Err(format!(
                    "Acción desconocida en creator_manage_idea: {}",
                    action
                )),
            }
        }
        "creator_manage_script" => {
            let action = args
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("save")
                .trim()
                .to_lowercase();
            let linked_idea_id = args
                .get("idea_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);

            let scripts_res = {
                let scripts = state["scripts"]
                    .as_array_mut()
                    .ok_or_else(|| "Corrupt creator scripts state".to_string())?;
                match action.as_str() {
                    "list" => {
                        let plat_f = args.get("platform").and_then(Value::as_str);
                        let filtered: Vec<Value> = scripts
                            .iter()
                            .filter(|it| {
                                plat_f.is_none_or(|p| {
                                    it.get("platform").and_then(Value::as_str) == Some(p)
                                })
                            })
                            .cloned()
                            .collect();
                        return Ok(
                            json!({ "success": true, "scripts": filtered, "count": filtered.len() }),
                        );
                    }
                    "delete" => {
                        let idx = find_item_index(
                            scripts,
                            args.get("id").and_then(Value::as_str),
                            &["title"],
                            args.get("title").and_then(Value::as_str),
                        )
                        .ok_or_else(|| "No se encontró el guion a eliminar".to_string())?;
                        let removed = scripts.remove(idx);
                        write_state(db, "aegis.domain.creator", &state)?;
                        return Ok(json!({ "success": true, "deleted": removed }));
                    }
                    "save" | "update" => {
                        let id_opt = args.get("id").and_then(Value::as_str);
                        let title_opt = args.get("title").and_then(Value::as_str);
                        let existing_idx = find_item_index(scripts, id_opt, &["title"], title_opt);
                        if let Some(idx) = existing_idx {
                            if let Some(obj) = scripts[idx].as_object_mut() {
                                set_str_if_present(obj, args, "idea_id");
                                set_str_if_present(obj, args, "title");
                                set_str_if_present(obj, args, "platform");
                                set_str_if_present(obj, args, "hook_0_3s");
                                set_str_if_present(obj, args, "body_script");
                                set_str_if_present(obj, args, "b_roll_notes");
                                set_str_if_present(obj, args, "cta");
                                set_str_if_present(obj, args, "caption");
                                set_str_if_present(obj, args, "hashtags");
                                set_str_if_present(obj, args, "status");
                                if let Some(dur) =
                                    args.get("estimated_duration_secs").and_then(Value::as_u64)
                                {
                                    obj.insert("estimated_duration_secs".into(), json!(dur));
                                }
                                obj.insert("updated_at".into(), json!(now_iso()));
                            }
                            Ok(("updated", scripts[idx].clone()))
                        } else {
                            if action == "update" {
                                return Err("No se encontró el guion para actualizar".into());
                            }
                            let title = title_opt
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .ok_or_else(|| {
                                    "El campo 'title' es requerido para guardar un guion"
                                        .to_string()
                                })?;
                            let hook = args.get("hook_0_3s").and_then(Value::as_str).unwrap_or("");
                            let body = args
                                .get("body_script")
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            let cta = args.get("cta").and_then(Value::as_str).unwrap_or("");
                            let est_secs = args
                                .get("estimated_duration_secs")
                                .and_then(Value::as_u64)
                                .unwrap_or_else(|| {
                                    let words = format!("{} {} {}", hook, body, cta)
                                        .split_whitespace()
                                        .count();
                                    ((words as f64 / 2.3).ceil() as u64).max(15)
                                });
                            let now = now_iso();
                            let script = json!({
                                "id": new_short_id("scr"),
                                "idea_id": linked_idea_id.clone().unwrap_or_default(),
                                "title": title,
                                "platform": args.get("platform").and_then(Value::as_str).unwrap_or("tiktok_reels"),
                                "hook_0_3s": hook,
                                "body_script": body,
                                "b_roll_notes": args.get("b_roll_notes").and_then(Value::as_str).unwrap_or(""),
                                "cta": cta,
                                "caption": args.get("caption").and_then(Value::as_str).unwrap_or(""),
                                "hashtags": args.get("hashtags").and_then(Value::as_str).unwrap_or(""),
                                "estimated_duration_secs": est_secs,
                                "status": args.get("status").and_then(Value::as_str).unwrap_or("ready_to_film"),
                                "created_at": now,
                                "updated_at": now,
                            });
                            scripts.insert(0, script.clone());
                            Ok(("created", script))
                        }
                    }
                    _ => Err(format!(
                        "Acción desconocida en creator_manage_script: {}",
                        action
                    )),
                }
            }?;

            if let Some(ref iid) = linked_idea_id {
                if let Some(ideas) = state.get_mut("ideas").and_then(Value::as_array_mut) {
                    if let Some(idea) = ideas
                        .iter_mut()
                        .find(|it| it.get("id").and_then(Value::as_str) == Some(iid.as_str()))
                    {
                        if idea.get("status").and_then(Value::as_str) == Some("idea") {
                            if let Some(obj) = idea.as_object_mut() {
                                obj.insert("status".into(), json!("scripting"));
                                obj.insert("updated_at".into(), json!(now_iso()));
                            }
                        }
                    }
                }
            }

            write_state(db, "aegis.domain.creator", &state)?;
            Ok(json!({ "success": true, "mode": scripts_res.0, "script": scripts_res.1 }))
        }
        "creator_schedule_post" => {
            let action = args
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("schedule")
                .trim()
                .to_lowercase();
            let calendar = state["calendar"]
                .as_array_mut()
                .ok_or_else(|| "Corrupt creator calendar state".to_string())?;

            match action.as_str() {
                "list" => {
                    Ok(json!({ "success": true, "calendar": calendar, "count": calendar.len() }))
                }
                "delete" => {
                    let idx = find_item_index(
                        calendar,
                        args.get("id").and_then(Value::as_str),
                        &["title"],
                        args.get("title").and_then(Value::as_str),
                    )
                    .ok_or_else(|| "No se encontró la publicación programada".to_string())?;
                    let removed = calendar.remove(idx);
                    write_state(db, "aegis.domain.creator", &state)?;
                    Ok(json!({ "success": true, "deleted": removed }))
                }
                "schedule" | "update" => {
                    if let Some(idx) = find_item_index(
                        calendar,
                        args.get("id").and_then(Value::as_str),
                        &["title"],
                        args.get("title").and_then(Value::as_str),
                    ) {
                        if let Some(obj) = calendar[idx].as_object_mut() {
                            set_str_if_present(obj, args, "title");
                            set_str_if_present(obj, args, "platform");
                            set_str_if_present(obj, args, "publish_at");
                            set_str_if_present(obj, args, "script_id");
                            set_str_if_present(obj, args, "caption");
                            set_str_if_present(obj, args, "hashtags");
                            set_str_if_present(obj, args, "status");
                            obj.insert("updated_at".into(), json!(now_iso()));
                        }
                        let updated = calendar[idx].clone();
                        write_state(db, "aegis.domain.creator", &state)?;
                        Ok(json!({ "success": true, "mode": "updated", "post": updated }))
                    } else {
                        if action == "update" {
                            return Err("No se encontró la publicación para actualizar".into());
                        }
                        let title = args
                            .get("title")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .ok_or_else(|| "El campo 'title' es requerido".to_string())?;
                        let publish_at = args
                            .get("publish_at")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .unwrap_or("Por definir");
                        let post = json!({
                            "id": new_short_id("post"),
                            "title": title,
                            "platform": args.get("platform").and_then(Value::as_str).unwrap_or("tiktok_reels"),
                            "publish_at": publish_at,
                            "script_id": args.get("script_id").and_then(Value::as_str).unwrap_or(""),
                            "caption": args.get("caption").and_then(Value::as_str).unwrap_or(""),
                            "hashtags": args.get("hashtags").and_then(Value::as_str).unwrap_or(""),
                            "status": args.get("status").and_then(Value::as_str).unwrap_or("scheduled"),
                            "created_at": now_iso(),
                        });
                        calendar.push(post.clone());
                        calendar.sort_by(|a, b| {
                            let da = a.get("publish_at").and_then(Value::as_str).unwrap_or("");
                            let db_s = b.get("publish_at").and_then(Value::as_str).unwrap_or("");
                            da.cmp(db_s)
                        });
                        write_state(db, "aegis.domain.creator", &state)?;
                        Ok(json!({ "success": true, "mode": "scheduled", "post": post }))
                    }
                }
                _ => Err(format!(
                    "Acción desconocida en creator_schedule_post: {}",
                    action
                )),
            }
        }
        _ => Err(format!(
            "Herramienta desconocida en Aegis Creator Studio: {}",
            tool_name
        )),
    }
}

// ── 2. Aegis Dev Studio (`aegis.domain.dev`) ──────────────────────────────────

fn dev_default_state() -> Value {
    json!({ "projects": [], "tasks": [], "snippets": [] })
}

fn dev_build_overview(state: &Value) -> Value {
    let projects = state
        .get("projects")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let tasks = state
        .get("tasks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let snippets = state
        .get("snippets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let open_tasks = tasks
        .iter()
        .filter(|t| t.get("status").and_then(Value::as_str) != Some("done"))
        .count();
    let in_progress_tasks = tasks
        .iter()
        .filter(|t| t.get("status").and_then(Value::as_str) == Some("in_progress"))
        .count();
    let bugs_count = tasks
        .iter()
        .filter(|t| {
            t.get("kind").and_then(Value::as_str) == Some("bug")
                && t.get("status").and_then(Value::as_str) != Some("done")
        })
        .count();

    json!({
        "success": true,
        "projects": projects,
        "tasks": tasks,
        "snippets": snippets,
        "stats": {
            "total_projects": projects.len(),
            "open_tasks": open_tasks,
            "in_progress_tasks": in_progress_tasks,
            "bugs_count": bugs_count,
            "total_snippets": snippets.len(),
        }
    })
}

fn execute_dev_tool(db: &TenantDB, tool_name: &str, args: &Value) -> Result<Value, String> {
    let mut state = read_state(db, "aegis.domain.dev", dev_default_state())?;

    match tool_name {
        "dev_get_overview" => Ok(dev_build_overview(&state)),
        "dev_manage_project" => {
            let action = args
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("add")
                .trim()
                .to_lowercase();
            let projects = state["projects"]
                .as_array_mut()
                .ok_or_else(|| "Corrupt dev projects state".to_string())?;

            match action.as_str() {
                "list" => {
                    Ok(json!({ "success": true, "projects": projects, "count": projects.len() }))
                }
                "delete" => {
                    let idx = find_item_index(
                        projects,
                        args.get("id").and_then(Value::as_str),
                        &["name"],
                        args.get("name").and_then(Value::as_str),
                    )
                    .ok_or_else(|| "Proyecto no encontrado".to_string())?;
                    let removed = projects.remove(idx);
                    write_state(db, "aegis.domain.dev", &state)?;
                    Ok(json!({ "success": true, "deleted": removed }))
                }
                "add" | "update" => {
                    let name_opt = args.get("name").and_then(Value::as_str);
                    if let Some(idx) = find_item_index(
                        projects,
                        args.get("id").and_then(Value::as_str),
                        &["name"],
                        name_opt,
                    ) {
                        if let Some(obj) = projects[idx].as_object_mut() {
                            set_str_if_present(obj, args, "name");
                            set_str_if_present(obj, args, "repo_path");
                            set_str_if_present(obj, args, "stack");
                            set_str_if_present(obj, args, "description");
                            set_str_if_present(obj, args, "default_branch");
                            set_str_if_present(obj, args, "build_cmd");
                            set_str_if_present(obj, args, "test_cmd");
                            set_str_if_present(obj, args, "status");
                            obj.insert("updated_at".into(), json!(now_iso()));
                        }
                        let updated = projects[idx].clone();
                        write_state(db, "aegis.domain.dev", &state)?;
                        Ok(json!({ "success": true, "mode": "updated", "project": updated }))
                    } else {
                        if action == "update" {
                            return Err("Proyecto no encontrado para actualizar".into());
                        }
                        let name = name_opt
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .ok_or_else(|| {
                                "El campo 'name' es requerido para registrar un proyecto"
                                    .to_string()
                            })?;
                        let now = now_iso();
                        let proj = json!({
                            "id": new_short_id("proj"),
                            "name": name,
                            "repo_path": args.get("repo_path").and_then(Value::as_str).unwrap_or(""),
                            "stack": args.get("stack").and_then(Value::as_str).unwrap_or("Full-Stack"),
                            "description": args.get("description").and_then(Value::as_str).unwrap_or(""),
                            "default_branch": args.get("default_branch").and_then(Value::as_str).unwrap_or("main"),
                            "build_cmd": args.get("build_cmd").and_then(Value::as_str).unwrap_or(""),
                            "test_cmd": args.get("test_cmd").and_then(Value::as_str).unwrap_or(""),
                            "status": args.get("status").and_then(Value::as_str).unwrap_or("active"),
                            "created_at": now,
                            "updated_at": now,
                        });
                        projects.insert(0, proj.clone());
                        write_state(db, "aegis.domain.dev", &state)?;
                        Ok(json!({ "success": true, "mode": "created", "project": proj }))
                    }
                }
                _ => Err(format!(
                    "Acción desconocida en dev_manage_project: {}",
                    action
                )),
            }
        }
        "dev_manage_task" => {
            let action = args
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("add")
                .trim()
                .to_lowercase();
            let tasks = state["tasks"]
                .as_array_mut()
                .ok_or_else(|| "Corrupt dev tasks state".to_string())?;

            match action.as_str() {
                "list" => {
                    let status_f = args.get("status").and_then(Value::as_str);
                    let kind_f = args.get("kind").and_then(Value::as_str);
                    let filtered: Vec<Value> = tasks
                        .iter()
                        .filter(|t| {
                            status_f
                                .is_none_or(|s| t.get("status").and_then(Value::as_str) == Some(s))
                                && kind_f.is_none_or(|k| {
                                    t.get("kind").and_then(Value::as_str) == Some(k)
                                })
                        })
                        .cloned()
                        .collect();
                    Ok(json!({ "success": true, "tasks": filtered, "count": filtered.len() }))
                }
                "delete" => {
                    let idx = find_item_index(
                        tasks,
                        args.get("id").and_then(Value::as_str),
                        &["title"],
                        args.get("title").and_then(Value::as_str),
                    )
                    .ok_or_else(|| "Tarea técnica no encontrada".to_string())?;
                    let removed = tasks.remove(idx);
                    write_state(db, "aegis.domain.dev", &state)?;
                    Ok(json!({ "success": true, "deleted": removed }))
                }
                "add" | "update" => {
                    let title_opt = args.get("title").and_then(Value::as_str);
                    if let Some(idx) = find_item_index(
                        tasks,
                        args.get("id").and_then(Value::as_str),
                        &["title"],
                        title_opt,
                    ) {
                        if let Some(obj) = tasks[idx].as_object_mut() {
                            set_str_if_present(obj, args, "project_id");
                            set_str_if_present(obj, args, "title");
                            set_str_if_present(obj, args, "kind");
                            set_str_if_present(obj, args, "priority");
                            set_str_if_present(obj, args, "status");
                            set_str_if_present(obj, args, "file_refs");
                            set_str_if_present(obj, args, "notes");
                            obj.insert("updated_at".into(), json!(now_iso()));
                        }
                        let updated = tasks[idx].clone();
                        write_state(db, "aegis.domain.dev", &state)?;
                        Ok(json!({ "success": true, "mode": "updated", "task": updated }))
                    } else {
                        if action == "update" {
                            return Err("Tarea técnica no encontrada para actualizar".into());
                        }
                        let title = title_opt
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .ok_or_else(|| {
                                "El campo 'title' es requerido para crear una tarea".to_string()
                            })?;
                        let now = now_iso();
                        let task = json!({
                            "id": new_short_id("tsk"),
                            "project_id": args.get("project_id").and_then(Value::as_str).unwrap_or(""),
                            "title": title,
                            "kind": args.get("kind").and_then(Value::as_str).unwrap_or("feature"),
                            "priority": args.get("priority").and_then(Value::as_str).unwrap_or("medium"),
                            "status": args.get("status").and_then(Value::as_str).unwrap_or("backlog"),
                            "file_refs": args.get("file_refs").and_then(Value::as_str).unwrap_or(""),
                            "notes": args.get("notes").and_then(Value::as_str).unwrap_or(""),
                            "created_at": now,
                            "updated_at": now,
                        });
                        tasks.insert(0, task.clone());
                        write_state(db, "aegis.domain.dev", &state)?;
                        Ok(json!({ "success": true, "mode": "created", "task": task }))
                    }
                }
                _ => Err(format!("Acción desconocida en dev_manage_task: {}", action)),
            }
        }
        "dev_manage_snippet" => {
            let action = args
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("save")
                .trim()
                .to_lowercase();
            let snippets = state["snippets"]
                .as_array_mut()
                .ok_or_else(|| "Corrupt dev snippets state".to_string())?;

            match action.as_str() {
                "list" => {
                    Ok(json!({ "success": true, "snippets": snippets, "count": snippets.len() }))
                }
                "delete" => {
                    let idx = find_item_index(
                        snippets,
                        args.get("id").and_then(Value::as_str),
                        &["title"],
                        args.get("title").and_then(Value::as_str),
                    )
                    .ok_or_else(|| "Snippet no encontrado".to_string())?;
                    let removed = snippets.remove(idx);
                    write_state(db, "aegis.domain.dev", &state)?;
                    Ok(json!({ "success": true, "deleted": removed }))
                }
                "save" => {
                    let title_opt = args.get("title").and_then(Value::as_str);
                    if let Some(idx) = find_item_index(
                        snippets,
                        args.get("id").and_then(Value::as_str),
                        &["title"],
                        title_opt,
                    ) {
                        if let Some(obj) = snippets[idx].as_object_mut() {
                            set_str_if_present(obj, args, "title");
                            set_str_if_present(obj, args, "language");
                            set_str_if_present(obj, args, "code");
                            set_str_if_present(obj, args, "tags");
                            set_str_if_present(obj, args, "description");
                            obj.insert("updated_at".into(), json!(now_iso()));
                        }
                        let updated = snippets[idx].clone();
                        write_state(db, "aegis.domain.dev", &state)?;
                        Ok(json!({ "success": true, "mode": "updated", "snippet": updated }))
                    } else {
                        let title = title_opt
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .ok_or_else(|| {
                                "El campo 'title' es requerido para guardar un snippet".to_string()
                            })?;
                        let now = now_iso();
                        let snip = json!({
                            "id": new_short_id("snp"),
                            "title": title,
                            "language": args.get("language").and_then(Value::as_str).unwrap_or("bash"),
                            "code": args.get("code").and_then(Value::as_str).unwrap_or(""),
                            "tags": args.get("tags").and_then(Value::as_str).unwrap_or(""),
                            "description": args.get("description").and_then(Value::as_str).unwrap_or(""),
                            "created_at": now,
                            "updated_at": now,
                        });
                        snippets.insert(0, snip.clone());
                        write_state(db, "aegis.domain.dev", &state)?;
                        Ok(json!({ "success": true, "mode": "created", "snippet": snip }))
                    }
                }
                _ => Err(format!(
                    "Acción desconocida en dev_manage_snippet: {}",
                    action
                )),
            }
        }
        _ => Err(format!(
            "Herramienta desconocida en Aegis Dev Studio: {}",
            tool_name
        )),
    }
}

// ── 3. Aegis Business & POS (`aegis.domain.business`) ─────────────────────────

fn business_default_state() -> Value {
    json!({ "products": [], "sales": [] })
}

fn business_build_overview(state: &Value) -> Value {
    let products = state
        .get("products")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let sales = state
        .get("sales")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut low_stock_count = 0usize;
    let mut inventory_value = 0.0f64;
    for p in &products {
        let stock = p.get("stock").and_then(Value::as_i64).unwrap_or(0);
        let min_stock = p.get("min_stock").and_then(Value::as_i64).unwrap_or(3);
        let price = p.get("price").and_then(Value::as_f64).unwrap_or(0.0);
        if stock <= min_stock {
            low_stock_count += 1;
        }
        if stock > 0 {
            inventory_value += (stock as f64) * price;
        }
    }

    let total_revenue: f64 = sales
        .iter()
        .filter_map(|s| s.get("total").and_then(Value::as_f64))
        .sum();

    json!({
        "success": true,
        "products": products,
        "sales": sales,
        "stats": {
            "total_products": products.len(),
            "low_stock_count": low_stock_count,
            "inventory_value": (inventory_value * 100.0).round() / 100.0,
            "total_sales_count": sales.len(),
            "total_revenue": (total_revenue * 100.0).round() / 100.0,
        }
    })
}

fn execute_business_tool(db: &TenantDB, tool_name: &str, args: &Value) -> Result<Value, String> {
    let mut state = read_state(db, "aegis.domain.business", business_default_state())?;

    match tool_name {
        "biz_get_overview" => Ok(business_build_overview(&state)),
        "biz_add_product" => {
            let products = state["products"]
                .as_array_mut()
                .ok_or_else(|| "Corrupt business products state".to_string())?;
            let id_opt = args.get("id").and_then(Value::as_str);
            let barcode_opt = args.get("barcode").and_then(Value::as_str);
            let name_opt = args.get("name").and_then(Value::as_str);

            let existing_idx = find_item_index(products, id_opt, &["barcode"], barcode_opt)
                .or_else(|| find_item_index(products, id_opt, &["name"], name_opt));

            if let Some(idx) = existing_idx {
                if let Some(obj) = products[idx].as_object_mut() {
                    set_str_if_present(obj, args, "barcode");
                    set_str_if_present(obj, args, "name");
                    set_str_if_present(obj, args, "category");
                    if let Some(price) = args.get("price").and_then(Value::as_f64) {
                        obj.insert("price".into(), json!(price));
                    }
                    if let Some(cost) = args.get("cost").and_then(Value::as_f64) {
                        obj.insert("cost".into(), json!(cost));
                    }
                    if let Some(stock) = args.get("stock").and_then(Value::as_i64) {
                        obj.insert("stock".into(), json!(stock));
                    }
                    if let Some(min_stock) = args.get("min_stock").and_then(Value::as_i64) {
                        obj.insert("min_stock".into(), json!(min_stock));
                    }
                    obj.insert("updated_at".into(), json!(now_iso()));
                }
                let updated = products[idx].clone();
                write_state(db, "aegis.domain.business", &state)?;
                Ok(json!({ "success": true, "mode": "updated", "product": updated }))
            } else {
                let name = name_opt
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        "El campo 'name' es requerido para agregar un producto".to_string()
                    })?;
                let price = args.get("price").and_then(Value::as_f64).unwrap_or(0.0);
                let prod = json!({
                    "id": new_short_id("prd"),
                    "barcode": barcode_opt.unwrap_or("").trim(),
                    "name": name,
                    "category": args.get("category").and_then(Value::as_str).unwrap_or("General"),
                    "price": price,
                    "cost": args.get("cost").and_then(Value::as_f64).unwrap_or(0.0),
                    "stock": args.get("stock").and_then(Value::as_i64).unwrap_or(0),
                    "min_stock": args.get("min_stock").and_then(Value::as_i64).unwrap_or(3),
                    "updated_at": now_iso(),
                });
                products.insert(0, prod.clone());
                write_state(db, "aegis.domain.business", &state)?;
                Ok(json!({ "success": true, "mode": "created", "product": prod }))
            }
        }
        "biz_update_stock" => {
            let delta = args
                .get("quantity_change")
                .and_then(Value::as_i64)
                .ok_or_else(|| {
                    "Falta 'quantity_change' (entero positivo o negativo)".to_string()
                })?;
            let id_opt = args
                .get("product_id")
                .and_then(Value::as_str)
                .or_else(|| args.get("id").and_then(Value::as_str));
            let barcode_opt = args.get("barcode").and_then(Value::as_str);
            let name_opt = args.get("name").and_then(Value::as_str);

            let products = state["products"]
                .as_array_mut()
                .ok_or_else(|| "Corrupt business products state".to_string())?;

            let idx_opt = find_item_index(products, id_opt, &["barcode"], barcode_opt)
                .or_else(|| find_item_index(products, id_opt, &["name"], name_opt));

            let product = if let Some(idx) = idx_opt {
                let current = products[idx]
                    .get("stock")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let next_stock = (current + delta).max(0);
                if let Some(obj) = products[idx].as_object_mut() {
                    obj.insert("stock".into(), json!(next_stock));
                    obj.insert("updated_at".into(), json!(now_iso()));
                }
                products[idx].clone()
            } else if delta > 0 {
                let name = name_opt
                    .or(barcode_opt)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        "Producto no encontrado; indicá 'name' o 'barcode'".to_string()
                    })?;
                let prod = json!({
                    "id": new_short_id("prd"),
                    "barcode": barcode_opt.unwrap_or("").trim(),
                    "name": name,
                    "category": "General",
                    "price": args.get("price").and_then(Value::as_f64).unwrap_or(0.0),
                    "cost": 0.0,
                    "stock": delta,
                    "min_stock": 3,
                    "updated_at": now_iso(),
                });
                products.insert(0, prod.clone());
                prod
            } else {
                return Err("Producto no encontrado en el inventario".into());
            };

            let stock_val = product.get("stock").and_then(Value::as_i64).unwrap_or(0);
            let min_stock = product
                .get("min_stock")
                .and_then(Value::as_i64)
                .unwrap_or(3);
            write_state(db, "aegis.domain.business", &state)?;
            Ok(json!({
                "success": true,
                "product": product,
                "low_stock_alert": stock_val <= min_stock,
            }))
        }
        "biz_record_sale" => {
            let qty = args
                .get("quantity")
                .and_then(Value::as_i64)
                .unwrap_or(1)
                .max(1);
            let id_opt = args
                .get("product_id")
                .and_then(Value::as_str)
                .or_else(|| args.get("id").and_then(Value::as_str));
            let barcode_opt = args.get("barcode").and_then(Value::as_str);
            let name_opt = args
                .get("product_name")
                .and_then(Value::as_str)
                .or_else(|| args.get("name").and_then(Value::as_str));

            let (resolved_pid, resolved_name, unit_price, remaining_stock, low_stock_alert) = {
                let products = state["products"]
                    .as_array_mut()
                    .ok_or_else(|| "Corrupt business products state".to_string())?;
                let idx_opt = find_item_index(products, id_opt, &["barcode"], barcode_opt)
                    .or_else(|| find_item_index(products, id_opt, &["name"], name_opt));

                if let Some(idx) = idx_opt {
                    let catalog_price = products[idx]
                        .get("price")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0);
                    let current = products[idx]
                        .get("stock")
                        .and_then(Value::as_i64)
                        .unwrap_or(0);
                    let min_s = products[idx]
                        .get("min_stock")
                        .and_then(Value::as_i64)
                        .unwrap_or(3);
                    let next_stock = (current - qty).max(0);
                    let pid = products[idx]
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let pname = products[idx]
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("Producto")
                        .to_string();
                    if let Some(obj) = products[idx].as_object_mut() {
                        obj.insert("stock".into(), json!(next_stock));
                        obj.insert("updated_at".into(), json!(now_iso()));
                    }
                    let effective_price = args
                        .get("unit_price")
                        .and_then(Value::as_f64)
                        .filter(|&p| p > 0.0)
                        .unwrap_or(catalog_price);
                    (
                        pid,
                        pname,
                        effective_price,
                        Some(next_stock),
                        next_stock <= min_s,
                    )
                } else {
                    let pname = name_opt
                        .or(barcode_opt)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| {
                            "Indicá qué producto se vendió ('product_name' o 'barcode')".to_string()
                        })?
                        .to_string();
                    let effective_price = args
                        .get("unit_price")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0);
                    (String::new(), pname, effective_price, None, false)
                }
            };

            let total = ((unit_price * (qty as f64)) * 100.0).round() / 100.0;
            let sale = json!({
                "id": new_short_id("sale"),
                "product_id": resolved_pid,
                "product_name": resolved_name,
                "quantity": qty,
                "unit_price": unit_price,
                "total": total,
                "customer": args.get("customer").and_then(Value::as_str).unwrap_or("Consumidor Final"),
                "payment_method": args.get("payment_method").and_then(Value::as_str).unwrap_or("efectivo"),
                "created_at": now_iso(),
            });

            if let Some(sales) = state.get_mut("sales").and_then(Value::as_array_mut) {
                sales.insert(0, sale.clone());
                if sales.len() > 500 {
                    sales.truncate(500);
                }
            }

            write_state(db, "aegis.domain.business", &state)?;
            Ok(json!({
                "success": true,
                "sale": sale,
                "remaining_stock": remaining_stock,
                "low_stock_alert": low_stock_alert,
            }))
        }
        "biz_delete_product" => {
            let products = state["products"]
                .as_array_mut()
                .ok_or_else(|| "Corrupt business products state".to_string())?;
            let idx = find_item_index(
                products,
                args.get("id").and_then(Value::as_str),
                &["barcode", "name"],
                args.get("barcode")
                    .and_then(Value::as_str)
                    .or_else(|| args.get("name").and_then(Value::as_str)),
            )
            .ok_or_else(|| "Producto no encontrado".to_string())?;
            let removed = products.remove(idx);
            write_state(db, "aegis.domain.business", &state)?;
            Ok(json!({ "success": true, "deleted": removed }))
        }
        _ => Err(format!(
            "Herramienta desconocida en Aegis Business & POS: {}",
            tool_name
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;

    #[test]
    fn test_creator_dev_business_native_runtimes() -> anyhow::Result<()> {
        let _guard = crate::enclave::acquire_test_lock();
        let tenant = format!("mod_rt_{}", Uuid::new_v4().simple());
        let skey = "test_module_runtime_secret_key_32b!!";

        // 1. Creator Studio: idea -> script (auto-advances idea status) -> schedule -> overview
        let r_idea: Value = serde_json::from_str(&execute_native_module_tool(
            "aegis.domain.creator",
            "creator_manage_idea",
            r#"{"action":"add","title":"Demo Aegis 30s","platform":"tiktok_reels","hook":"Tu IA propia sin nube"}"#,
            &tenant,
            skey,
        ).map_err(anyhow::Error::msg)?)?;
        assert_eq!(r_idea["success"], true);
        let idea_id = r_idea["idea"]["id"]
            .as_str()
            .context("missing idea id")?
            .to_string();

        let script_args = json!({
            "action": "save",
            "idea_id": idea_id,
            "title": "Demo Aegis 30s",
            "platform": "tiktok_reels",
            "hook_0_3s": "Tu IA propia en tu PC",
            "body_script": "Mirá cómo corre Aegis con tu propio catálogo de módulos.",
            "cta": "Seguime para parte 2"
        });
        let r_script: Value = serde_json::from_str(
            &execute_native_module_tool(
                "aegis.domain.creator",
                "creator_manage_script",
                &script_args.to_string(),
                &tenant,
                skey,
            )
            .map_err(anyhow::Error::msg)?,
        )?;
        assert_eq!(r_script["success"], true);

        let r_cre_ov: Value = serde_json::from_str(
            &execute_native_module_tool(
                "aegis.domain.creator",
                "creator_get_overview",
                "{}",
                &tenant,
                skey,
            )
            .map_err(anyhow::Error::msg)?,
        )?;
        assert_eq!(r_cre_ov["ideas"][0]["status"], "scripting");
        assert_eq!(r_cre_ov["stats"]["total_scripts"], 1);

        // 2. Dev Studio: project + task + snippet
        let r_proj: Value = serde_json::from_str(
            &execute_native_module_tool(
                "aegis.domain.dev",
                "dev_manage_project",
                r#"{"action":"add","name":"Aegis-Core","stack":"Rust + React"}"#,
                &tenant,
                skey,
            )
            .map_err(anyhow::Error::msg)?,
        )?;
        assert_eq!(r_proj["success"], true);

        let r_task: Value = serde_json::from_str(
            &execute_native_module_tool(
                "aegis.domain.dev",
                "dev_manage_task",
                r#"{"action":"add","title":"Fix CORS header","kind":"bug","priority":"high"}"#,
                &tenant,
                skey,
            )
            .map_err(anyhow::Error::msg)?,
        )?;
        assert_eq!(r_task["task"]["kind"], "bug");

        let r_dev_ov: Value = serde_json::from_str(
            &execute_native_module_tool(
                "aegis.domain.dev",
                "dev_get_overview",
                "{}",
                &tenant,
                skey,
            )
            .map_err(anyhow::Error::msg)?,
        )?;
        assert_eq!(r_dev_ov["stats"]["bugs_count"], 1);

        // 3. Business & POS: add product -> update stock -> record sale
        let r_prod: Value = serde_json::from_str(
            &execute_native_module_tool(
                "aegis.domain.business",
                "biz_add_product",
                r#"{"name":"Filtro Secador","price":12500,"stock":10,"min_stock":3}"#,
                &tenant,
                skey,
            )
            .map_err(anyhow::Error::msg)?,
        )?;
        assert_eq!(r_prod["success"], true);

        let r_sale: Value = serde_json::from_str(
            &execute_native_module_tool(
                "aegis.domain.business",
                "biz_record_sale",
                r#"{"product_name":"Filtro Secador","quantity":2}"#,
                &tenant,
                skey,
            )
            .map_err(anyhow::Error::msg)?,
        )?;
        assert_eq!(r_sale["remaining_stock"], 8);
        assert_eq!(r_sale["sale"]["total"], 25000.0);

        let _ = std::fs::remove_dir_all(format!("./users/{}", tenant));
        Ok(())
    }
}
