#!/usr/bin/env python3
"""
Aegis OS — Generador de Dataset para Fine-Tuning
Lee los logs locales de historial de chat de los usuarios de Aegis (`chat_history.log`),
los agrupa en sesiones de conversación por intervalos de tiempo, y genera un archivo
dataset.jsonl listo para el entrenamiento del asistente local.
"""

import os
import re
import sys
import json
import argparse
from datetime import datetime

DEFAULT_SYSTEM_PROMPT = (
    "Eres el Asistente de Aegis OS. Hablas de forma concisa y natural para voz. "
    "Cuando ejecutas comandos, envuélvelos estrictamente en <aegis_sys_call> y </aegis_sys_call>."
)

def get_default_data_dir():
    """Detecta el directorio de datos por defecto de Aegis según el sistema operativo."""
    home = os.path.expanduser("~")
    if sys.platform.startswith("win"):
        # Windows: %APPDATA%/aegis
        appdata = os.environ.get("APPDATA")
        if appdata:
            return os.path.join(appdata, "aegis")
        return os.path.join(home, "AppData", "Roaming", "aegis")
    elif sys.platform.startswith("darwin"):
        # macOS: ~/Library/Application Support/aegis
        return os.path.join(home, "Library", "Application Support", "aegis")
    else:
        # Linux: ~/.local/share/aegis (respetando XDG si existe)
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
    
    # Intentar parsear el timestamp ISO8601
    try:
        # Remover 'Z' final si existe y parsear
        ts_clean = timestamp_str.replace("Z", "")
        # Soportar formatos con o sin milisegundos
        if "." in ts_clean:
            dt = datetime.strptime(ts_clean, "%Y-%m-%dT%H:%M:%S.%f")
        else:
            dt = datetime.strptime(ts_clean, "%Y-%m-%dT%H:%M:%S")
    except ValueError:
        dt = None
        
    return {
        "timestamp": dt,
        "role": role,
        "content": content.strip()
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

    # Agrupar en sesiones basadas en el umbral de tiempo
    sessions = []
    current_session = []
    last_dt = None
    
    for msg in parsed_messages:
        dt = msg["timestamp"]
        
        # Si no hay timestamp o es el primer mensaje, inicializar
        if last_dt is None or dt is None:
            current_session.append(msg)
        else:
            # Calcular la diferencia de tiempo entre mensajes consecutivos
            diff = (dt - last_dt).total_seconds() / 60.0
            if diff > threshold_minutes:
                # El tiempo de inactividad supera el umbral -> cerrar sesión vieja e iniciar nueva
                if len(current_session) >= 2:
                    sessions.append(current_session)
                current_session = [msg]
            else:
                current_session.append(msg)
                
        if dt:
            last_dt = dt
            
    # Añadir la última sesión si tiene suficientes mensajes
    if len(current_session) >= 2:
        sessions.append(current_session)
        
    return sessions

def main():
    args = parse_args()
    print("==========================================================")
    print(" Generando Dataset de Entrenamiento desde Logs de Aegis OS")
    print(f" Carpeta de Datos: {args.data_dir}")
    print(f" Umbral de Sesión: {args.threshold_minutes} minutos")
    print(f" Archivo de Salida: {args.output_file}")
    print("==========================================================")

    users_dir = os.path.join(args.data_dir, "users")
    if not os.path.exists(users_dir):
        print(f"❌ Error: No se encontró la carpeta 'users' en: {args.data_dir}")
        print("Asegúrate de que Aegis OS haya corrido en esta máquina o especifica el path correcto con --data_dir.")
        sys.exit(1)

    all_sessions = []
    
    # Buscar en todas las carpetas de tenants el archivo chat_history.log
    for tenant_id in os.listdir(users_dir):
        tenant_path = os.path.join(users_dir, tenant_id)
        if os.path.isdir(tenant_path):
            log_path = os.path.join(tenant_path, "workspace", "chat_history.log")
            if os.path.exists(log_path):
                sessions = process_log_file(log_path, args.threshold_minutes)
                all_sessions.extend(sessions)

    if not all_sessions:
        print("\n⚠️  No se encontraron logs de conversación válidos con al menos 2 turnos.")
        print("El archivo 'dataset.jsonl' no ha sido generado. Chatea un poco en la UI de Aegis primero.")
        sys.exit(0)

    # Escribir el dataset a formato JSONL listo para Fine-Tuning
    valid_sessions_count = 0
    total_messages_count = 0
    
    with open(args.output_file, "w", encoding="utf-8") as out_f:
        for session in all_sessions:
            # Limpiar la sesión para que empiece estrictamente con un rol de "user"
            # (Muchos modelos de chat requieren que el primer mensaje no sea del asistente)
            while session and session[0]["role"] == "assistant":
                session.pop(0)
                
            # Descartar sesiones que se quedaron vacías o con un solo mensaje después de limpiar
            if len(session) < 2:
                continue
                
            # Formatear al estándar ShareGPT / ChatML
            formatted_messages = [{"role": "system", "content": args.system_prompt}]
            for msg in session:
                formatted_messages.append({
                    "role": msg["role"],
                    "content": msg["content"]
                })
                
            # Escribir la línea
            out_f.write(json.dumps({"messages": formatted_messages}, ensure_ascii=False) + "\n")
            valid_sessions_count += 1
            total_messages_count += len(session)

    print("\n==========================================================")
    print(" DATASET GENERADO CON ÉXITO")
    print(f" Sesiones Procesadas:  {valid_sessions_count}")
    print(f" Mensajes Totales:     {total_messages_count}")
    print(f" Promedio de Mensajes: {total_messages_count / valid_sessions_count:.1f} por sesión")
    print(f" Destino:              {os.path.abspath(args.output_file)}")
    print("==========================================================")
    print("Ya puedes ejecutar 'python fine_tune.py' para iniciar el entrenamiento.")

if __name__ == "__main__":
    main()
