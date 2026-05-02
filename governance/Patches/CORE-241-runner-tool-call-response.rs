                                                if let Some(tool_call) = tool_call_opt {
                                                    let pcb_snapshot =
                                                        shared_pcb.read().await.clone();
                                                    match executor
                                                        .execute_agent_tool_call(
                                                            &pcb_snapshot,
                                                            tool_call,
                                                        )
                                                        .await
                                                    {
                                                        Ok(result_json) => {
                                                            tracing::info!(pid = %pid, "AgentToolCall ejecutado con éxito");

                                                            // CORE-241 FIX: Enviar confirmación al usuario
                                                            // El result_json contiene {"status":"spawned","project":"Aegis"}
                                                            // o {"acknowledged":true} — lo usamos para generar un mensaje natural.
                                                            let user_msg = if result_json.contains("spawned") {
                                                                // Extraer nombre del proyecto del JSON
                                                                serde_json::from_str::<serde_json::Value>(&result_json)
                                                                    .ok()
                                                                    .and_then(|v| v["project"].as_str().map(String::from))
                                                                    .map(|name| format!("Listo, activé el equipo para el proyecto **{}**.", name))
                                                                    .unwrap_or_else(|| "Listo, activé el equipo.".to_string())
                                                            } else if result_json.contains("acknowledged") {
                                                                "Reporte recibido.".to_string()
                                                            } else if result_json.contains("answer") {
                                                                // Query reply — extraer la respuesta
                                                                serde_json::from_str::<serde_json::Value>(&result_json)
                                                                    .ok()
                                                                    .and_then(|v| v["answer"].as_str().map(String::from))
                                                                    .unwrap_or_else(|| result_json.clone())
                                                            } else {
                                                                result_json.clone()
                                                            };

                                                            full_output.push_str(&user_msg);
                                                            tokens_emitted += 1;
                                                            let _ = event_tx.send(ank_proto::v1::TaskEvent {
                                                                pid: pid.clone(),
                                                                timestamp: None,
                                                                payload: Some(
                                                                    ank_proto::v1::task_event::Payload::Output(user_msg),
                                                                ),
                                                            });
                                                        }
                                                        Err(e) => {
                                                            tracing::error!(pid = %pid, "Error en AgentToolCall: {}", e);

                                                            // Informar al usuario del error
                                                            let err_msg = format!("No pude completar la acción: {}", e);
                                                            full_output.push_str(&err_msg);
                                                            tokens_emitted += 1;
                                                            let _ = event_tx.send(ank_proto::v1::TaskEvent {
                                                                pid: pid.clone(),
                                                                timestamp: None,
                                                                payload: Some(
                                                                    ank_proto::v1::task_event::Payload::Output(err_msg),
                                                                ),
                                                            });
                                                        }
                                                    }
                                                }
                                            }

                                            // CORE-241: No propagar el token de tool call al frontend
                                            continue;
