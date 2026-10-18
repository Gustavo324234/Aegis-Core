#!/usr/bin/env python3
"""
Aegis OS — Generador de Dataset para Fine-Tuning (Citadel Tenant-Isolated)
Lee los logs locales de historial de chat del tenant activo (`chat_history.log`),
redacta secretos/API keys, agrupa las líneas en sesiones de conversación y genera
un archivo JSONL con el protocolo Tool Use v2 listo para el entrenamiento.
"""

import os
import re
import sys
import json
import argparse
from datetime import datetime

DEFAULT_SYSTEM_PROMPT = (
    "Sos el Asistente Maestro de Aegis OS. Hablás de forma concisa, cálida y natural. "
    "Cuando necesites invocar una herramienta del kernel, emití una llamada estructurada "
    "con el prefijo __TOOL_CALL__ seguido del objeto JSON {\"name\": \"...\", \"arguments\": {...}}."
)

# Expresiones regulares para sanitizar claves de API, tokens Bearer y setup_tokens (CORE-350)
SECRET_PATTERNS = [
    (re.compile(r"\bsk-[A-Za-z0-9_-]{16,}\b"), "[REDACTED_API_KEY]"),
    (re.compile(r"\bgsk_[A-Za-z0-9_-]{16,}\b"), "[REDACTED_GROQ_KEY]"),
    (re.compile(r"\bAIza[0-9A-Za-z_-]{30,}\b"), "[REDACTED_GEMINI_KEY]"),
    (re.compile(r"Bearer\s+[A-Za-z0-9\-\._~\+\/]+=*", re.IGNORECASE), "Bearer [REDACTED_TOKEN]"),
    (re.compile(r"setup_token=[A-Za-z0-9_-]+"), "setup_token=[REDACTED]"),
    (re.compile(r"x-citadel-key:\s*[^\s\"']+", re.IGNORECASE), "x-citadel-key: [REDACTED]"),
]


def redact_secrets(text: str) -> str:
    """Elimina claves de API y credenciales sensibles antes de persistir el dataset."""
    sanitized = text
    for pattern, replacement in SECRET_PATTERNS:
        sanitized = pattern.sub(replacement, sanitized)
    return sanitized


def is_safe_tenant_id(tenant_id: str) -> bool:
    """Valida que el tenant_id sea seguro y no permita path traversal."""
    return bool(tenant_id) and bool(re.match(r"^[A-Za-z0-9_-]+$", tenant_id))


def get_default_data_dir():
    """Detecta el directorio de datos por defecto de Aegis según el sistema operativo."""
    home = os.path.expanduser("~")
    if sys.platform.startswith("win"):
        appdata = os.environ.get("APPDATA")
        if appdata:
            return os.path.join(appdata, "aegis")
        return os.path.join(home, "AppData", "Roaming", "aegis")
    elif sys.platform.startswith("darwin"):
        return os.path.join(home, "Library", "Application Support", "aegis")
    else:
        xdg_data = os.environ.get("XDG_DATA_HOME")
        if xdg_data:
            return os.path.join(xdg_data, "aegis")
        return os.path.join(home, ".local", "share", "aegis")


def parse_args():
    parser = argparse.ArgumentParser(description="Generar dataset de entrenamiento desde logs de Aegis.")
    parser.add_argument(
        "--data_dir",
        type=str,
        default=os.environ.get("AEGIS_DATA_DIR", get_default_data_dir()),
        help="Ruta al directorio de datos de Aegis (por defecto se detecta según el OS)."
    )
    parser.add_argument(
        "--tenant_id",
        type=str,
        default=None,
        help="ID del tenant para aislar estrictamente sus logs de conversación (Citadel)."
    )
    parser.add_argument(
        "--output_file",
        type=str,
        default="dataset.jsonl",
        help="Nombre del archivo JSON Lines de salida (por defecto: dataset.jsonl)."
    )
    parser.add_argument(
        "--threshold_minutes",
        type=int,
        default=30,
        help="Tiempo en minutos de inactividad para segmentar una nueva conversación (por defecto: 30)."
    )
    parser.add_argument(
        "--system_prompt",
        type=str,
        default=DEFAULT_SYSTEM_PROMPT,
        help="System Prompt que se inyectará al inicio de cada conversación."
    )
    return parser.parse_args()


def parse_log_line(line):
    """
    Parsea una línea del historial de chat.
    Formato esperado: [ISO8601] ROLE: contenido
    Ejemplo: [2026-06-16T08:08:55Z] USER: Hola
    """
    pattern = r"^\[(.*?)\]\s+(USER|ASSISTANT):\s+(.*)$"
    match = re.match(pattern, line.strip())
    if not match:
        return None

    timestamp_str, role_raw, content = match.groups()
    role = "user" if role_raw.upper() == "USER" else "assistant"

    try:
        ts_clean = timestamp_str.replace("Z", "")
        if "." in ts_clean:
            dt = datetime.strptime(ts_clean, "%Y-%m-%dT%H:%M:%S.%f")
        else:
            dt = datetime.strptime(ts_clean, "%Y-%m-%dT%H:%M:%S")
    except ValueError:
        dt = None

    return {
        "timestamp": dt,
        "role": role,
        "content": redact_secrets(content.strip())
    }


def process_log_file(filepath, threshold_minutes):
    """Procesa un archivo de log y agrupa las líneas en sesiones de conversación."""
    if not os.path.exists(filepath):
        return []

    print(f"Leyendo log de chat: {filepath}")

    parsed_messages = []
    with open(filepath, "r", encoding="utf-8", errors="ignore") as f:
        for line in f:
            parsed = parse_log_line(line)
            if parsed:
                parsed_messages.append(parsed)

    if not parsed_messages:
        return []

    sessions = []
    current_session = []
    last_dt = None

    for msg in parsed_messages:
        dt = msg["timestamp"]

        if last_dt is None or dt is None:
            current_session.append(msg)
        else:
            diff = (dt - last_dt).total_seconds() / 60.0
            if diff > threshold_minutes:
                if len(current_session) >= 2:
                    sessions.append(current_session)
                current_session = [msg]
            else:
                current_session.append(msg)

        if dt:
            last_dt = dt

    if len(current_session) >= 2:
        sessions.append(current_session)

    return sessions


def main():
    args = parse_args()
    print("==========================================================")
    print(" Generando Dataset de Entrenamiento desde Logs de Aegis OS")
    print(f" Carpeta de Datos: {args.data_dir}")
    print(f" Tenant Aislado:   {args.tenant_id or 'ALL (CLI manual)'}")
    print(f" Umbral de Sesión: {args.threshold_minutes} minutos")
    print(f" Archivo de Salida: {args.output_file}")
    print("==========================================================")

    if args.tenant_id is not None and not is_safe_tenant_id(args.tenant_id):
        print(f"❌ Error de seguridad Citadel: tenant_id inválido '{args.tenant_id}'.")
        sys.exit(1)

    users_dir = os.path.join(args.data_dir, "users")
    all_sessions = []

    # 1. Cargar sesiones plantilla de Aegis (Q&A y formato de herramientas Tool Use v2)
    script_dir = os.path.dirname(os.path.abspath(__file__))
    template_path = os.path.join(script_dir, "dataset_template.jsonl")
    template_sessions_count = 0

    if os.path.exists(template_path):
        print(f"Cargando plantilla de sistema y Q&A de Aegis: {template_path}")
        with open(template_path, "r", encoding="utf-8") as f:
            for line in f:
                try:
                    data = json.loads(line)
                    if "messages" in data:
                        messages = data["messages"]
                        session_msgs = []
                        for m in messages:
                            if m["role"] == "system":
                                continue
                            session_msgs.append({
                                "role": m["role"],
                                "content": redact_secrets(m["content"]),
                                "timestamp": None
                            })
                        if len(session_msgs) >= 2:
                            all_sessions.append(session_msgs)
                            template_sessions_count += 1
                except Exception as e:
                    print(f"⚠️ Error al leer línea de plantilla: {e}")
        print(f"Loaded {template_sessions_count} template sessions.")

    # 2. Buscar el archivo chat_history.log respetando el aislamiento de tenant (CORE-350)
    user_sessions_count = 0
    if os.path.exists(users_dir):
        if args.tenant_id:
            log_path = os.path.join(users_dir, args.tenant_id, "workspace", "chat_history.log")
            if os.path.exists(log_path):
                sessions = process_log_file(log_path, args.threshold_minutes)
                all_sessions.extend(sessions)
                user_sessions_count += len(sessions)
            else:
                print(f"ℹ️ El tenant '{args.tenant_id}' aún no posee chat_history.log; usando plantillas base.")
        else:
            for tid in os.listdir(users_dir):
                if not is_safe_tenant_id(tid):
                    continue
                tenant_path = os.path.join(users_dir, tid)
                if os.path.isdir(tenant_path):
                    log_path = os.path.join(tenant_path, "workspace", "chat_history.log")
                    if os.path.exists(log_path):
                        sessions = process_log_file(log_path, args.threshold_minutes)
                        all_sessions.extend(sessions)
                        user_sessions_count += len(sessions)
    else:
        print(f"ℹ️ No se encontró la carpeta 'users' en {args.data_dir}; generando con plantillas base.")

    if not all_sessions:
        print("\n⚠️ No se encontraron logs de conversación ni plantillas válidas.")
        print("El archivo 'dataset.jsonl' no ha sido generado.")
        sys.exit(1)

    print(f"Mezcla de datos completada: {template_sessions_count} plantillas de Aegis + {user_sessions_count} sesiones del usuario.")

    valid_sessions_count = 0
    total_messages_count = 0

    with open(args.output_file, "w", encoding="utf-8") as out_f:
        for session in all_sessions:
            while session and session[0]["role"] == "assistant":
                session.pop(0)

            if len(session) < 2:
                continue

            formatted_messages = [{"role": "system", "content": args.system_prompt}]
            for msg in session:
                formatted_messages.append({
                    "role": msg["role"],
                    "content": msg["content"]
                })

            out_f.write(json.dumps({"messages": formatted_messages}, ensure_ascii=False) + "\n")
            valid_sessions_count += 1
            total_messages_count += len(session)

    if valid_sessions_count == 0:
        print("❌ Ninguna sesión válida tras normalización.")
        sys.exit(1)

    print("\n==========================================================")
    print(" DATASET GENERADO CON ÉXITO")
    print(f" Sesiones Procesadas:  {valid_sessions_count}")
    print(f" Mensajes Totales:     {total_messages_count}")
    print(f" Promedio de Mensajes: {total_messages_count / valid_sessions_count:.1f} por sesión")
    print(f" Destino:              {os.path.abspath(args.output_file)}")
    print("==========================================================")


if __name__ == "__main__":
    main()
