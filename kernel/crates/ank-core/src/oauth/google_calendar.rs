use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

const CALENDAR_EVENTS_BASE: &str =
    "https://www.googleapis.com/calendar/v3/calendars/primary/events";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CalendarEventInput {
    pub summary: String,
    pub start: String,
    pub end: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub attendees: Option<Vec<String>>,
    pub all_day: Option<bool>,
}

/// Normalizes user/LLM date-time strings (`YYYY-MM-DD`, `YYYY-MM-DD HH:MM`,
/// `YYYY-MM-DDTHH:MM`, `YYYY-MM-DDTHH:MM:SS`, or full RFC3339) into Google
/// Calendar API `{ "date": ... }` or `{ "dateTime": ... }` objects.
pub fn normalize_calendar_datetime(
    raw: &str,
    all_day: bool,
) -> Result<(Value, Option<NaiveDateTime>, bool)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("La fecha/hora no puede estar vacía"));
    }

    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        if all_day {
            return Ok((
                json!({ "date": date.format("%Y-%m-%d").to_string() }),
                None,
                true,
            ));
        }
        // Date-only without explicit time defaults to an all-day entry unless caller requires timed.
        return Ok((
            json!({ "date": date.format("%Y-%m-%d").to_string() }),
            None,
            true,
        ));
    }

    if DateTime::parse_from_rfc3339(trimmed).is_ok() {
        return Ok((json!({ "dateTime": trimmed }), None, false));
    }

    let naive = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M"))
        .or_else(|_| NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S"))
        .or_else(|_| NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M"))
        .map_err(|_| {
            anyhow!(
                "Formato de fecha inválido '{}'. Usá RFC3339 o 'YYYY-MM-DD HH:MM' o 'YYYY-MM-DD'.",
                trimmed
            )
        })?;

    if all_day {
        let d = naive.date().format("%Y-%m-%d").to_string();
        return Ok((json!({ "date": d }), None, true));
    }

    let local_dt = Local
        .from_local_datetime(&naive)
        .single()
        .or_else(|| Local.from_local_datetime(&naive).earliest())
        .ok_or_else(|| anyhow!("No se pudo resolver zona horaria local para '{}'", trimmed))?;

    Ok((
        json!({ "dateTime": local_dt.to_rfc3339() }),
        Some(naive),
        false,
    ))
}

fn format_event_summary(evt: &Value) -> Value {
    let start = evt.get("start");
    let end = evt.get("end");
    let attendees = evt["attendees"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a["email"].as_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    json!({
        "id": evt["id"].as_str().unwrap_or(""),
        "title": evt["summary"].as_str().unwrap_or("Sin título"),
        "start": start.and_then(|s| s["dateTime"].as_str().or_else(|| s["date"].as_str())),
        "end": end.and_then(|e| e["dateTime"].as_str().or_else(|| e["date"].as_str())),
        "location": evt["location"].as_str().unwrap_or(""),
        "description": evt["description"].as_str().unwrap_or(""),
        "attendees": attendees,
        "status": evt["status"].as_str().unwrap_or("confirmed"),
        "html_link": evt["htmlLink"].as_str().unwrap_or(""),
    })
}

async fn send_calendar_request(
    http_client: &Arc<reqwest::Client>,
    tenant_id: &str,
    session_key: &str,
    method: reqwest::Method,
    url: &str,
    body: Option<&Value>,
) -> Result<Option<Value>> {
    for attempt in 0..2 {
        let force_refresh = attempt == 1;
        let token = super::get_or_refresh_token_for_tenant(
            http_client,
            tenant_id,
            session_key,
            "google",
            force_refresh,
        )
        .await?;

        let mut req = http_client
            .request(method.clone(), url)
            .bearer_auth(&token)
            .timeout(std::time::Duration::from_secs(10));

        if let Some(payload) = body {
            req = req.json(payload);
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status == reqwest::StatusCode::UNAUTHORIZED && attempt == 0 {
            // Token may have been revoked or expired early before local TTL elapsed; try one refresh.
            continue;
        }

        if status == reqwest::StatusCode::NO_CONTENT {
            return Ok(None);
        }

        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            if status == reqwest::StatusCode::FORBIDDEN && text.contains("insufficient") {
                return Err(anyhow!(
                    "Permisos insuficientes en Google OAuth para modificar Calendar (HTTP 403). \
                     Reconectá Google en Settings → Cuentas Conectadas."
                ));
            }
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(anyhow!(
                    "Token de Google expirado o revocado (HTTP 401). \
                     Reconectá tu cuenta en Settings → Cuentas Conectadas."
                ));
            }
            return Err(anyhow!(
                "Google Calendar API error (HTTP {}): {}",
                status,
                text.chars().take(400).collect::<String>()
            ));
        }

        if text.trim().is_empty() {
            return Ok(None);
        }
        let parsed: Value = serde_json::from_str(&text)?;
        return Ok(Some(parsed));
    }

    Err(anyhow!(
        "Google Calendar API request failed after token refresh"
    ))
}

pub async fn list_events(
    http_client: &Arc<reqwest::Client>,
    tenant_id: &str,
    session_key: &str,
    days: u8,
    max_results: u8,
    query: Option<&str>,
) -> Result<Value> {
    let now = chrono::Utc::now();
    let days_clamped = days.clamp(1, 60);
    let max_clamped = max_results.clamp(1, 50);
    let time_max = now + chrono::Duration::days(days_clamped as i64);

    let mut url = format!(
        "{}?timeMin={}&timeMax={}&maxResults={}&singleEvents=true&orderBy=startTime\
         &fields=items(id,summary,start,end,location,description,attendees,status,htmlLink)",
        CALENDAR_EVENTS_BASE,
        urlencoding::encode(&now.to_rfc3339()),
        urlencoding::encode(&time_max.to_rfc3339()),
        max_clamped
    );

    if let Some(q) = query.map(str::trim).filter(|s| !s.is_empty()) {
        url.push_str(&format!("&q={}", urlencoding::encode(q)));
    }

    let data = send_calendar_request(
        http_client,
        tenant_id,
        session_key,
        reqwest::Method::GET,
        &url,
        None,
    )
    .await?
    .unwrap_or_else(|| json!({}));

    let events: Vec<Value> = data["items"]
        .as_array()
        .map(|items| items.iter().map(format_event_summary).collect())
        .unwrap_or_default();

    Ok(json!({
        "status": "ok",
        "days": days_clamped,
        "count": events.len(),
        "events": events,
    }))
}

pub async fn create_event(
    http_client: &Arc<reqwest::Client>,
    tenant_id: &str,
    session_key: &str,
    input: CalendarEventInput,
) -> Result<Value> {
    let summary = input.summary.trim();
    if summary.is_empty() {
        return Err(anyhow!("Falta el título (`summary`) del evento"));
    }

    let force_all_day = input.all_day.unwrap_or(false);
    let (start_val, start_naive, is_all_day) =
        normalize_calendar_datetime(&input.start, force_all_day)?;

    let end_val = if let Some(end_raw) = input.end.as_deref().filter(|s| !s.trim().is_empty()) {
        let (ev, _, _) = normalize_calendar_datetime(end_raw, is_all_day)?;
        ev
    } else if is_all_day {
        let start_date_str = start_val["date"].as_str().unwrap_or("");
        let start_d = NaiveDate::parse_from_str(start_date_str, "%Y-%m-%d")?;
        let next_d = start_d + Duration::days(1);
        json!({ "date": next_d.format("%Y-%m-%d").to_string() })
    } else if let Some(naive_s) = start_naive {
        let end_naive = naive_s + Duration::hours(1);
        let local_end = Local
            .from_local_datetime(&end_naive)
            .single()
            .or_else(|| Local.from_local_datetime(&end_naive).earliest())
            .ok_or_else(|| anyhow!("No se pudo calcular hora de fin"))?;
        json!({ "dateTime": local_end.to_rfc3339() })
    } else if let Some(rfc) = start_val["dateTime"].as_str() {
        let dt = DateTime::parse_from_rfc3339(rfc)?;
        json!({ "dateTime": (dt + Duration::hours(1)).to_rfc3339() })
    } else {
        return Err(anyhow!("Falta fecha de fin (`end`)"));
    };

    let mut payload = json!({
        "summary": summary,
        "start": start_val,
        "end": end_val,
    });

    if let Some(desc) = input
        .description
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        payload["description"] = json!(desc.trim());
    }
    if let Some(loc) = input.location.as_deref().filter(|s| !s.trim().is_empty()) {
        payload["location"] = json!(loc.trim());
    }
    if let Some(attendees) = input.attendees.filter(|a| !a.is_empty()) {
        let mapped: Vec<Value> = attendees
            .into_iter()
            .map(|e| json!({ "email": e.trim() }))
            .collect();
        payload["attendees"] = json!(mapped);
    }

    let created = send_calendar_request(
        http_client,
        tenant_id,
        session_key,
        reqwest::Method::POST,
        CALENDAR_EVENTS_BASE,
        Some(&payload),
    )
    .await?
    .unwrap_or_else(|| json!({}));

    Ok(json!({
        "status": "created",
        "event": format_event_summary(&created),
    }))
}

#[allow(clippy::too_many_arguments)]
pub async fn update_event(
    http_client: &Arc<reqwest::Client>,
    tenant_id: &str,
    session_key: &str,
    event_id: &str,
    summary: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
    description: Option<&str>,
    location: Option<&str>,
) -> Result<Value> {
    let trimmed_id = event_id.trim();
    if trimmed_id.is_empty() {
        return Err(anyhow!("Falta `event_id` para actualizar el evento"));
    }

    let mut patch = serde_json::Map::new();
    if let Some(s) = summary.map(str::trim).filter(|s| !s.is_empty()) {
        patch.insert("summary".into(), json!(s));
    }
    if let Some(d) = description {
        patch.insert("description".into(), json!(d.trim()));
    }
    if let Some(l) = location {
        patch.insert("location".into(), json!(l.trim()));
    }
    if let Some(st) = start.map(str::trim).filter(|s| !s.is_empty()) {
        let (sv, _, _) = normalize_calendar_datetime(st, false)?;
        patch.insert("start".into(), sv);
    }
    if let Some(en) = end.map(str::trim).filter(|s| !s.is_empty()) {
        let (ev, _, _) = normalize_calendar_datetime(en, false)?;
        patch.insert("end".into(), ev);
    }

    if patch.is_empty() {
        return Err(anyhow!("No se enviaron campos a modificar"));
    }

    let url = format!(
        "{}/{}",
        CALENDAR_EVENTS_BASE,
        urlencoding::encode(trimmed_id)
    );
    let updated = send_calendar_request(
        http_client,
        tenant_id,
        session_key,
        reqwest::Method::PATCH,
        &url,
        Some(&Value::Object(patch)),
    )
    .await?
    .unwrap_or_else(|| json!({}));

    Ok(json!({
        "status": "updated",
        "event": format_event_summary(&updated),
    }))
}

pub async fn delete_event(
    http_client: &Arc<reqwest::Client>,
    tenant_id: &str,
    session_key: &str,
    event_id: &str,
) -> Result<Value> {
    let trimmed_id = event_id.trim();
    if trimmed_id.is_empty() {
        return Err(anyhow!("Falta `event_id` para eliminar el evento"));
    }
    let url = format!(
        "{}/{}",
        CALENDAR_EVENTS_BASE,
        urlencoding::encode(trimmed_id)
    );
    send_calendar_request(
        http_client,
        tenant_id,
        session_key,
        reqwest::Method::DELETE,
        &url,
        None,
    )
    .await?;

    Ok(json!({
        "status": "deleted",
        "event_id": trimmed_id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_date_only_as_all_day() {
        let (val, naive, all_day) = normalize_calendar_datetime("2026-11-14", false).unwrap();
        assert!(all_day);
        assert!(naive.is_none());
        assert_eq!(val["date"].as_str(), Some("2026-11-14"));
    }

    #[test]
    fn normalizes_local_datetime_into_rfc3339() {
        let (val, naive, all_day) = normalize_calendar_datetime("2026-11-14 17:30", false).unwrap();
        assert!(!all_day);
        assert!(naive.is_some());
        let dt_str = val["dateTime"].as_str().unwrap();
        assert!(dt_str.starts_with("2026-11-14T17:30:00"));
        assert!(DateTime::parse_from_rfc3339(dt_str).is_ok());
    }

    #[test]
    fn preserves_explicit_rfc3339() {
        let raw = "2026-11-14T17:00:00-03:00";
        let (val, _, all_day) = normalize_calendar_datetime(raw, false).unwrap();
        assert!(!all_day);
        assert_eq!(val["dateTime"].as_str(), Some(raw));
    }
}
