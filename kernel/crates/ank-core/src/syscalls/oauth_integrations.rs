use crate::syscalls::{SyscallError, SyscallExecutor};

impl SyscallExecutor {
    pub(super) async fn search_spotify(
        &self,
        token: &str,
        query: &str,
        max_results: u8,
        _tenant_id: &str,
    ) -> Result<String, SyscallError> {
        let token = match (!token.is_empty()).then(|| token.to_string()) {
            Some(t) => t,
            None => {
                return Ok(
                    "[SYSTEM_RESULT: Spotify token expired. Please reconnect in Settings.]"
                        .to_string(),
                )
            }
        };

        let url = format!(
            "https://api.spotify.com/v1/search?q={}&type=track&limit={}",
            urlencoding::encode(query),
            max_results.clamp(1, 10)
        );

        let resp = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| SyscallError::IOError(format!("Spotify API request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(SyscallError::IOError(format!(
                "Spotify API error: {}",
                resp.status()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SyscallError::IOError(e.to_string()))?;

        let results: Vec<serde_json::Value> = data["tracks"]["items"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "provider": "spotify",
                            "track_id": t["id"].as_str().unwrap_or(""),
                            "track_uri": t["uri"].as_str().unwrap_or(""),
                            "title": t["name"].as_str().unwrap_or(""),
                            "artist": t["artists"][0]["name"].as_str().unwrap_or(""),
                            "album": t["album"]["name"].as_str().unwrap_or(""),
                            "duration_ms": t["duration_ms"].as_u64().unwrap_or(0),
                            "thumbnail": t["album"]["images"][0]["url"].as_str().unwrap_or(""),
                            "preview_url": t["preview_url"].as_str().unwrap_or(""),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(format!(
            "[SYSTEM_RESULT: {}]",
            serde_json::to_string(&serde_json::json!({
                "results": results,
                "provider": "spotify"
            }))
            .unwrap_or_default()
        ))
    }

    pub(super) async fn search_youtube_oauth(
        &self,
        token: &str,
        query: &str,
        max_results: u8,
    ) -> Result<String, SyscallError> {
        let token = match (!token.is_empty()).then(|| token.to_string()) {
            Some(t) => t,
            None => {
                return Ok(
                    "[SYSTEM_RESULT: Google token expired. Please reconnect in Settings.]"
                        .to_string(),
                )
            }
        };

        let url = format!(
            "https://www.googleapis.com/youtube/v3/search\
             ?part=snippet&type=video&videoCategoryId=10\
             &q={}&maxResults={}",
            urlencoding::encode(query),
            max_results.clamp(1, 5)
        );

        let resp = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| SyscallError::IOError(format!("YouTube API request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(SyscallError::IOError(format!(
                "YouTube API error: {}",
                resp.status()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SyscallError::IOError(e.to_string()))?;

        let results: Vec<serde_json::Value> = data["items"]
            .as_array()
            .map(|items| {
                items.iter()
                    .map(|item| {
                        serde_json::json!({
                            "provider": "youtube",
                            "video_id": item["id"]["videoId"].as_str().unwrap_or(""),
                            "title": item["snippet"]["title"].as_str().unwrap_or(""),
                            "channel": item["snippet"]["channelTitle"].as_str().unwrap_or(""),
                            "thumbnail": item["snippet"]["thumbnails"]["default"]["url"].as_str().unwrap_or("")
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(format!(
            "[SYSTEM_RESULT: {}]",
            serde_json::to_string(&serde_json::json!({
                "results": results,
                "provider": "youtube"
            }))
            .unwrap_or_default()
        ))
    }

    pub(super) async fn google_calendar(
        &self,
        token: Option<String>,
        days: u8,
        max_results: u8,
    ) -> Result<String, SyscallError> {
        let token = match token {
            Some(t) => t,
            None => {
                return Ok(
                    "[SYSTEM_RESULT: Google token expired. Please reconnect in Settings.]"
                        .to_string(),
                )
            }
        };

        let now = chrono::Utc::now();
        let time_max = now + chrono::Duration::days(days as i64);
        let time_min_str = now.to_rfc3339();
        let time_max_str = time_max.to_rfc3339();

        let url = format!(
            "https://www.googleapis.com/calendar/v3/calendars/primary/events\
             ?timeMin={}&timeMax={}&maxResults={}&singleEvents=true&orderBy=startTime\
             &fields=items(summary,start,end,location,description,attendees)",
            urlencoding::encode(&time_min_str),
            urlencoding::encode(&time_max_str),
            max_results.clamp(1, 20)
        );

        let resp = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| {
                SyscallError::IOError(format!("Google Calendar API request failed: {}", e))
            })?;

        if !resp.status().is_success() {
            return Err(SyscallError::IOError(format!(
                "Google Calendar API error: {}",
                resp.status()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SyscallError::IOError(e.to_string()))?;

        let events: Vec<serde_json::Value> = data["items"]
            .as_array()
            .map(|items| {
                items.iter().map(|evt| {
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

                    serde_json::json!({
                        "title": evt["summary"].as_str().unwrap_or("Sin título"),
                        "start": start.and_then(|s| s["dateTime"].as_str().or_else(|| s["date"].as_str())),
                        "end": end.and_then(|e| e["dateTime"].as_str().or_else(|| e["date"].as_str())),
                        "location": evt["location"].as_str().unwrap_or(""),
                        "description": evt["description"].as_str().unwrap_or(""),
                        "attendees": attendees
                    })
                }).collect()
            })
            .unwrap_or_default();

        Ok(format!(
            "[SYSTEM_RESULT: {}]",
            serde_json::to_string(&serde_json::json!({ "events": events })).unwrap_or_default()
        ))
    }

    pub(super) async fn google_drive(
        &self,
        token: Option<String>,
        query: &str,
        max_results: u8,
    ) -> Result<String, SyscallError> {
        let token = match token {
            Some(t) => t,
            None => {
                return Ok(
                    "[SYSTEM_RESULT: Google token expired. Please reconnect in Settings.]"
                        .to_string(),
                )
            }
        };

        let q = if query.is_empty() {
            "trashed=false".to_string()
        } else {
            format!(
                "name contains '{}' and trashed=false",
                urlencoding::encode(query)
            )
        };

        let url = format!(
            "https://www.googleapis.com/drive/v3/files\
             ?q={}&fields=files(id,name,mimeType,modifiedTime,webViewLink,size)\
             &orderBy=modifiedTime desc&pageSize={}",
            q,
            max_results.clamp(1, 10)
        );

        let resp = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| {
                SyscallError::IOError(format!("Google Drive API request failed: {}", e))
            })?;

        if !resp.status().is_success() {
            return Err(SyscallError::IOError(format!(
                "Google Drive API error: {}",
                resp.status()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SyscallError::IOError(e.to_string()))?;

        let files: Vec<serde_json::Value> = data["files"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|f| {
                        let mime = f["mimeType"].as_str().unwrap_or("");
                        let file_type = match mime {
                            "application/vnd.google-apps.spreadsheet" => "spreadsheet",
                            "application/vnd.google-apps.document" => "document",
                            "application/vnd.google-apps.presentation" => "presentation",
                            "application/vnd.google-apps.folder" => "folder",
                            "application/pdf" => "pdf",
                            _ if mime.starts_with("image/") => "image",
                            _ if mime.starts_with("video/") => "video",
                            _ => "file",
                        };

                        serde_json::json!({
                            "id": f["id"].as_str().unwrap_or(""),
                            "name": f["name"].as_str().unwrap_or(""),
                            "type": file_type,
                            "modified": f["modifiedTime"].as_str().unwrap_or(""),
                            "url": f["webViewLink"].as_str().unwrap_or("")
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(format!(
            "[SYSTEM_RESULT: {}]",
            serde_json::to_string(&serde_json::json!({ "files": files })).unwrap_or_default()
        ))
    }

    pub(super) async fn gmail(
        &self,
        token: Option<String>,
        query: &str,
        max_results: u8,
    ) -> Result<String, SyscallError> {
        let token = match token {
            Some(t) => t,
            None => {
                return Ok(
                    "[SYSTEM_RESULT: Google token expired. Please reconnect in Settings.]"
                        .to_string(),
                )
            }
        };

        let url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages\
             ?q={}&maxResults={}",
            urlencoding::encode(query),
            max_results.clamp(1, 10)
        );

        let resp = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| SyscallError::IOError(format!("Gmail API request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(SyscallError::IOError(format!(
                "Gmail API error: {}",
                resp.status()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SyscallError::IOError(e.to_string()))?;

        let message_ids: Vec<String> = data["messages"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let mut emails = Vec::new();
        for id in message_ids {
            let msg_url = format!(
                "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}\
                 ?fields=payload/headers,snippet",
                id
            );

            let msg_resp = self
                .http_client
                .get(&msg_url)
                .bearer_auth(&token)
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await;

            if let Ok(msg_resp) = msg_resp {
                if msg_resp.status().is_success() {
                    if let Ok(msg_data) = msg_resp.json::<serde_json::Value>().await {
                        let headers = msg_data["payload"]["headers"].as_array();
                        let get_header = |name: &str| -> Option<String> {
                            headers?
                                .iter()
                                .find(|h| {
                                    h["name"]
                                        .as_str()
                                        .map(|n| n.to_lowercase() == name.to_lowercase())
                                        .unwrap_or(false)
                                })
                                .and_then(|h| h["value"].as_str().map(String::from))
                        };

                        let from = get_header("from").unwrap_or_default();
                        let subject = get_header("subject").unwrap_or_default();
                        let date = get_header("date").unwrap_or_default();
                        let unread = get_header("status")
                            .map(|s| s.to_lowercase() == "unread")
                            .unwrap_or(false);

                        emails.push(serde_json::json!({
                            "from": from,
                            "subject": subject,
                            "snippet": msg_data["snippet"].as_str().unwrap_or(""),
                            "date": date,
                            "unread": unread
                        }));
                    }
                }
            }
        }

        Ok(format!(
            "[SYSTEM_RESULT: {}]",
            serde_json::to_string(&serde_json::json!({ "emails": emails })).unwrap_or_default()
        ))
    }
}
