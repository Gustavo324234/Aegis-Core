#!/usr/bin/env python3
import os
import re
import sys
import argparse

# Configurar encoding UTF-8 para evitar caídas en Windows
if sys.platform.startswith("win"):
    try:
        sys.stdout.reconfigure(encoding='utf-8')
        sys.stderr.reconfigure(encoding='utf-8')
    except Exception:
        pass


# Configuración de rutas
ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TICKETS_DIR = os.path.join(ROOT_DIR, "governance", "Tickets")
MASTER_PATH = os.path.join(ROOT_DIR, "governance", "TICKETS_MASTER.md")

# Secciones del master en orden estándar de renderizado
ACTIVE_SECTIONS = [
    "EPIC 49 — Cognitive Loop",
    "EPIC 51 — Model Intelligence",
    "EPIC 52 — Voice Quality",
    "EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure",
    "EPIC 54 — Aegis Connect: Persistent WebSocket Tunneling",
    "Otras Características Consolidadas",
    "EPIC 55 — Mobile App (Orion ID & Web Redirection)",
    "Governance & Tooling"
]

def parse_ticket_file(filepath):
    """Parsea el archivo markdown de un ticket y extrae sus metadatos del header."""
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
    except Exception as e:
        print(f"Error leyendo {filepath}: {e}")
        return None
        
    lines = content.splitlines()
    if not lines:
        return None
        
    # Extraer ID y título del primer encabezado # CORE-XXX — Título
    m = re.match(r"^#\s*(CORE-\d+(?:-[a-zA-Z0-9]+)?)\s*[-—:]\s*(.+)$", lines[0])
    if not m:
        # Re-intentar con formato simple # CORE-XXX
        m = re.match(r"^#\s*(CORE-\d+(?:-[a-zA-Z0-9]+)?)(.*)$", lines[0])
        if not m:
            return None

            
    ticket_id = m.group(1).strip()
    title = m.group(2).strip(" —:-")
    
    metadata = {
        "ID": ticket_id,
        "Título": title,
        "Tipo": "chore",
        "Prioridad": "Media",
        "Épica": "",
        "Estado": "📥 Todo",
        "Asignado a": ""
    }
    
    # Parsear campos de metadatos hasta llegar a '---'
    for line in lines[1:]:
        if line.strip() == "---":
            break
            
        match_kv = re.match(r"^\s*[-\s]*\**([a-zA-Z\s]+)\**\s*:\s*(.+)$", line)
        if match_kv:

            key = match_kv.group(1).strip("* ").lower()
            val = match_kv.group(2).strip("* ")
            # Limpiar formato de comillas y links
            val = re.sub(r"^[`'\"]+", "", val)
            val = re.sub(r"[`'\"]+$", "", val)
            val = re.sub(r"\[(.*?)\]\(.*?\)", r"\1", val)

            
            if key == "tipo":
                metadata["Tipo"] = val
            elif key == "prioridad":
                metadata["Prioridad"] = val
            elif key in ["épica", "epica", "epic"]:
                metadata["Épica"] = val
            elif key == "estado":
                # Normalización del estado para la leyenda estándar
                val_lower = val.lower()
                if "done" in val_lower or "completo" in val_lower or "✅ done" in val_lower or "✅ completo" in val_lower:
                    metadata["Estado"] = "✅ Done"
                elif "progress" in val_lower or "curso" in val_lower or "🚧 in progress" in val_lower:
                    metadata["Estado"] = "🚧 In Progress"
                elif "todo" in val_lower or "pendiente" in val_lower or "📥 todo" in val_lower:
                    metadata["Estado"] = "📥 Todo"
                elif "blocked" in val_lower or "bloqueado" in val_lower or "❌ blocked" in val_lower:
                    metadata["Estado"] = "❌ Blocked"
                elif "verificar" in val_lower or "revisar" in val_lower or "⚠️ revisar" in val_lower or "⚠️ verificar" in val_lower:
                    metadata["Estado"] = "⚠️ Revisar"
                else:
                    metadata["Estado"] = val
            elif key in ["asignado a", "asignado", "assignee"]:
                metadata["Asignado a"] = val
                
    return metadata

def get_target_section(ticket):
    """Asigna a qué sección del master corresponde un ticket basándose en su Épica e ID."""
    epic = ticket.get("Épica", "")
    ticket_id = ticket["ID"]
    
    if "EPIC 49" in epic or "Cognitive Loop" in epic:
        return "EPIC 49 — Cognitive Loop"
    elif "EPIC 51" in epic or "Model Intelligence" in epic:
        return "EPIC 51 — Model Intelligence"
    elif "EPIC 52" in epic or "Voice Quality" in epic:
        return "EPIC 52 — Voice Quality"
    elif "EPIC 53" in epic or "Stabilization" in epic:
        return "EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure"
    elif "EPIC 54" in epic or "Aegis Connect" in epic:
        return "EPIC 54 — Aegis Connect: Persistent WebSocket Tunneling"
    elif "EPIC 55" in epic or "Mobile App" in epic:
        return "EPIC 55 — Mobile App (Orion ID & Web Redirection)"
    elif "Governance" in epic or "Tooling" in epic or ticket_id == "CORE-313":
        return "Governance & Tooling"
    elif ticket_id in ["CORE-150", "CORE-151"]:
        return "Otras Características Consolidadas"
        
    # Clasificación por rangos numéricos si no especifica épica
    num = int(ticket_id.split("-")[1])
    if 290 <= num <= 306:
        # En base a models.yaml / update_models / SirenRouter / CMR v2 / Asymmetric routing
        if num in [295, 302, 304]:
            return "EPIC 52 — Voice Quality"
        else:
            return "EPIC 51 — Model Intelligence"
    elif 307 <= num <= 309:
        return "EPIC 54 — Aegis Connect: Persistent WebSocket Tunneling"
    elif 310 <= num <= 312:
        return "EPIC 55 — Mobile App (Orion ID & Web Redirection)"
    elif num == 313:
        return "Governance & Tooling"
    elif 245 <= num <= 258:
        return "EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure"
    elif 259 <= num <= 289:
        # Tickets del cognitive loop o deduplicación/timeouts/ledgers
        if num in [263, 265, 266, 281, 286]:
            return "EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure"
        else:
            return "EPIC 49 — Cognitive Loop"
            
    return "Otras Características Consolidadas"

def parse_master_file(master_path):
    """Parsea el archivo TICKETS_MASTER.md y extrae la lista de tickets por sección activa."""
    if not os.path.exists(master_path):
        return {}
        
    with open(master_path, 'r', encoding='utf-8') as f:
        content = f.read()
        
    lines = content.splitlines()
    sections = {}
    current_section = None
    in_table = False
    headers = []
    
    for line in lines:
        line_strip = line.strip()
        if line_strip.startswith("## "):
            current_section = line_strip[3:].strip()
            sections[current_section] = []
            in_table = False
            continue
            
        if current_section and line_strip.startswith("|"):
            if not in_table:
                if "ID" in line_strip and "Tipo" in line_strip:
                    headers = [h.strip() for h in line_strip.split("|")[1:-1]]
                    in_table = True
                continue
            else:
                if "---" in line_strip:
                    continue
                cols = [c.strip() for c in line_strip.split("|")[1:-1]]
                if len(cols) >= len(headers) and cols[0].startswith("CORE-"):
                    ticket_info = {}
                    for i, h in enumerate(headers):
                        if i < len(cols):
                            val = cols[i]
                            # Limpiar formato de links
                            if h == "ID":
                                val = re.sub(r"\[(CORE-\d+)\]\(.*?\)", r"\1", val)
                                val = re.sub(r"[`'\"]+", "", val)
                            elif h == "Título":
                                val = re.sub(r"^[`'\"]+", "", val)
                                val = re.sub(r"[`'\"]+$", "", val)
                            ticket_info[h] = val
                    sections[current_section].append(ticket_info)
        else:
            in_table = False
            
    return sections

def generate_markdown_table(tickets):
    """Genera una tabla de tickets formateada en markdown."""
    if not tickets:
        return "*(No hay tickets registrados)*\n"
        
    # Ordenar tickets por número de ID
    tickets_sorted = sorted(
        tickets, 
        key=lambda x: int(x["ID"].split("-")[1])
    )
    
    # Decidir si la tabla necesita columna 'Asignado a'
    has_assignee = any(t.get("Asignado a") for t in tickets_sorted)
    
    if has_assignee:
        hdr = "| ID | Tipo | Título | Estado | Prioridad | Asignado a |\n"
        div = "|---|---|---|---|---|---|\n"
        rows = []
        for t in tickets_sorted:
            rows.append(
                f"| {t['ID']} | {t['Tipo']} | {t['Título']} | {t['Estado']} | {t['Prioridad']} | {t.get('Asignado a', '')} |"
            )
    else:
        hdr = "| ID | Tipo | Título | Estado | Prioridad |\n"
        div = "|---|---|---|---|---|\n"
        rows = []
        for t in tickets_sorted:
            rows.append(
                f"| {t['ID']} | {t['Tipo']} | {t['Título']} | {t['Estado']} | {t['Prioridad']} |"
            )
            
    return hdr + div + "\n".join(rows) + "\n"

def run_report():
    """Ejecuta la comparación entre los archivos de tickets y el archivo master."""
    print("Iniciando análisis de discrepancias de gobernanza...")
    
    # 1. Leer tickets de disco
    disk_tickets = {}
    if os.path.exists(TICKETS_DIR):
        for f in os.listdir(TICKETS_DIR):
            if f.startswith("CORE-") and f.endswith(".md"):
                path = os.path.join(TICKETS_DIR, f)
                meta = parse_ticket_file(path)
                if meta:
                    disk_tickets[meta["ID"]] = meta
                    
    print(f"Total de tickets encontrados en disco: {len(disk_tickets)}")
    
    # 2. Leer master
    master_sections = parse_master_file(MASTER_PATH)
    master_tickets = {}
    for sect, tlist in master_sections.items():
        if sect in ACTIVE_SECTIONS:
            for t in tlist:
                master_tickets[t["ID"]] = (sect, t)
                
    print(f"Total de tickets mapeados en secciones activas del Master: {len(master_tickets)}")
    
    discrepancies = []
    
    # 3. Comprobar tickets en disco que no están en el master
    for tid, tdisk in disk_tickets.items():
        target_sect = get_target_section(tdisk)
        if target_sect in ACTIVE_SECTIONS:
            if tid not in master_tickets:
                discrepancies.append({
                    "tipo": "ticket_faltante_en_master",
                    "mensaje": f"Ticket {tid} ('{tdisk['Título']}') existe en disco pero no está en la sección '{target_sect}' del Master."
                })
            else:
                # Comprobar diferencias en metadatos
                msect, tmaster = master_tickets[tid]
                diffs = []
                
                # Mapear llaves del ticket en master
                m_tipo = tmaster.get("Tipo", "")
                m_title = tmaster.get("Título", "")
                m_estado = tmaster.get("Estado", "")
                m_priority = tmaster.get("Prioridad", "")
                m_assignee = tmaster.get("Asignado a", "")
                
                if tdisk["Tipo"] != m_tipo:
                    diffs.append(f"Tipo: '{tdisk['Tipo']}' vs '{m_tipo}'")
                if tdisk["Estado"] != m_estado:
                    diffs.append(f"Estado: '{tdisk['Estado']}' vs '{m_estado}'")
                if tdisk["Prioridad"] != m_priority:
                    diffs.append(f"Prioridad: '{tdisk['Prioridad']}' vs '{m_priority}'")
                if tdisk.get("Asignado a", "") != m_assignee and has_assignee_mismatch(tdisk.get("Asignado a", ""), m_assignee):
                    diffs.append(f"Asignado a: '{tdisk.get('Asignado a', '')}' vs '{m_assignee}'")
                    
                if diffs:
                    discrepancies.append({
                        "tipo": "metadatos_divergentes",
                        "mensaje": f"Ticket {tid} ('{tdisk['Título']}') tiene discrepancias de metadatos: {', '.join(diffs)}."
                    })
                    
    # 4. Comprobar tickets en el master que no tienen archivo en disco
    for tid, (msect, tmaster) in master_tickets.items():
        if tid not in disk_tickets:
            discrepancies.append({
                "tipo": "archivo_faltante_en_disco",
                "mensaje": f"Ticket {tid} ('{tmaster['Título']}') está listado en la sección '{msect}' del Master pero no tiene archivo {tid}.md en disco."
            })

            
    # 5. Comprobar tickets no completados bajo Epics marcados como completos (Riesgo de vaporware)
    # Leemos la tabla principal de epics para ver cuáles están declarados como 100% o ✅ Completa
    with open(MASTER_PATH, 'r', encoding='utf-8') as f:
        master_content = f.read()
        
    epics_table = []
    # Parsea la tabla principal de Epics
    for line in master_content.splitlines():
        if "EPIC " in line and "✅" in line:
            # Encontrar el ID del epic
            m_epic = re.search(r"EPIC\s*(\d+)", line)
            if m_epic:
                epics_table.append(f"EPIC {m_epic.group(1)}")
                
    for tid, tdisk in disk_tickets.items():
        epic_field = tdisk.get("Épica", "")
        # Extraer número de épica del ticket
        m_num = re.search(r"EPIC\s*(\d+)", epic_field)
        if m_num:
            epic_id = f"EPIC {m_num.group(1)}"
            if epic_id in epics_table and tdisk["Estado"] != "✅ Done":
                discrepancies.append({
                    "tipo": "ticket_activo_bajo_epic_completo",
                    "mensaje": f"⚠️ ALERTA DE CREDIBILIDAD: El ticket {tid} está en estado '{tdisk['Estado']}' (NO Done) pero pertenece al epic '{epic_id}' que está declarado como '✅ Completa' en el master principal."
                })
                
    # Mostrar resultados
    print("\n" + "=" * 60)
    print("=== REPORTE DE DISCREPANCIAS DE GOBERNANZA ===")
    print("=" * 60)
    
    if not discrepancies:
        print("🎉 ¡Excelente! No se encontraron discrepancias de gobernanza. Todo alineado.")
        print("=" * 60)
        return 0
    else:
        for d in discrepancies:
            print(f"- {d['mensaje']}")
        print("=" * 60)
        print(f"Total de discrepancias encontradas: {len(discrepancies)}")
        print("Ejecuta 'python tools/sync_tickets_master.py --write' para sincronizar automáticamente el Master.")
        print("=" * 60)
        return 1

def has_assignee_mismatch(disk_val, master_val):
    """Devuelve True si hay una diferencia real entre el asignado del disco y el master."""
    if not disk_val and not master_val:
        return False
    # Normalizar strings
    d = re.sub(r"\s+", "", disk_val.lower())
    m = re.sub(r"\s+", "", master_val.lower())
    return d != m

def run_write():
    """Sincroniza y re-escribe el archivo TICKETS_MASTER.md."""
    print("Sincronizando TICKETS_MASTER.md con los archivos de tickets...")
    
    # 1. Leer todos los tickets de disco
    disk_tickets = []
    if os.path.exists(TICKETS_DIR):
        for f in os.listdir(TICKETS_DIR):
            if f.startswith("CORE-") and f.endswith(".md"):
                path = os.path.join(TICKETS_DIR, f)
                meta = parse_ticket_file(path)
                if meta:
                    disk_tickets.append(meta)
                    
    print(f"Leídos {len(disk_tickets)} tickets desde disco.")
    
    # 2. Agrupar por sección destino
    grouped_tickets = {sect: [] for sect in ACTIVE_SECTIONS}
    for t in disk_tickets:
        sect = get_target_section(t)
        if sect in grouped_tickets:
            grouped_tickets[sect].append(t)
            
    # 3. Leer el contenido actual del Master para conservar el header
    if not os.path.exists(MASTER_PATH):
        print(f"Error: No se encontró el master en {MASTER_PATH}")
        return 1
        
    with open(MASTER_PATH, 'r', encoding='utf-8') as f:
        master_content = f.read()
        
    # Dividir el master. Buscamos la primera sección itemizada
    # (usualmente ## EPIC 49 — Cognitive Loop)
    split_term = "## EPIC 49 — Cognitive Loop"
    if split_term not in master_content:
        # Caída en cascada
        split_term = "## EPIC 51 — Model Intelligence"
        
    parts = master_content.split(split_term)
    header_part = parts[0]
    
    # 4. Limpieza del header
    # Quitar la nota de sincronización temporal. Buscamos el bloque que contiene "Nota de sincronización"
    header_clean = header_part
    pattern = r"\n---\n[\s\S]*?>\s*\*\*\s*🔄\s*Nota de sincronización[\s\S]*?\n---\n"
    if re.search(pattern, header_clean):
        header_clean = re.sub(pattern, "\n", header_clean)
        print("Sincronización: Removida la nota de sincronización temporal del header.")
    else:
        # Intento de remoción por líneas si no coincide la regex exacta
        lines = header_clean.splitlines()
        new_header_lines = []
        skip = False
        for line in lines:
            if "Nota de sincronización" in line or (skip and line.strip().startswith(">")):
                skip = True
                continue
            if skip and line.strip() == "---":
                skip = False
                continue
            new_header_lines.append(line)
        header_clean = "\n".join(new_header_lines)
        
    # Asegurarnos de que no queden múltiples divisores pegados o espacios redundantes
    header_clean = re.sub(r"\n\s*---\s*\n\s*---\s*\n", "\n---\n", header_clean)
    header_clean = header_clean.strip() + "\n\n"

    
    # 5. Re-ensamblar todas las tablas itemizadas en orden limpio
    body_part = ""
    for sect in ACTIVE_SECTIONS:
        sect_tickets = grouped_tickets[sect]
        body_part += f"## {sect}\n\n"
        
        # Anotar si la sección es EPIC 54 y tenía un warning anterior
        if sect == "EPIC 54 — Aegis Connect: Persistent WebSocket Tunneling" and not sect_tickets:
            body_part += "> ⚠️ Estos tickets **no tienen ticket file** en governance/Tickets/ (backfill pendiente — CORE-313).\n\n"
            
        table_md = generate_markdown_table(sect_tickets)
        body_part += table_md + "\n"
        
    # 6. Escribir resultado
    final_content = header_clean + body_part
    # Quitar leyenda final repetida o agregar la leyenda maestra al final
    final_content = final_content.strip() + "\n\n---\n\n*Leyenda: 📥 Todo · 🚧 In Progress · ✅ Done · ❌ Blocked · ⚠️ Revisar*\n"
    
    try:
        with open(MASTER_PATH, 'w', encoding='utf-8') as f:
            f.write(final_content)
        print("🎉 Master sincronizado y actualizado exitosamente.")
        return 0
    except Exception as e:
        print(f"Error escribiendo {MASTER_PATH}: {e}")
        return 1

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Aegis Governance Sync Tool")
    parser.add_argument("--report", action="store_true", help="Reportar discrepancias de gobernanza sin modificar archivos")
    parser.add_argument("--write", action="store_true", help="Sincronizar y escribir el master de tickets")
    
    args = parser.parse_args()
    
    # Por defecto, si no se pasa acción, corremos --report
    if not args.write:
        sys.exit(run_report())
    else:
        sys.exit(run_write())
