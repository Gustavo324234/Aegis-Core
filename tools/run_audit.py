#!/usr/bin/env python3
import os
import sys
import subprocess
import shutil
import json

# Configurar encoding UTF-8 para evitar caídas en Windows
if sys.platform.startswith("win"):
    try:
        sys.stdout.reconfigure(encoding='utf-8')
        sys.stderr.reconfigure(encoding='utf-8')
    except Exception:
        pass

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SHELL_UI_DIR = os.path.join(ROOT_DIR, "shell", "ui")

def run_command(cmd, cwd=ROOT_DIR):
    """Ejecuta un comando en la consola y retorna exit_code, stdout, stderr."""
    try:
        process = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            shell=True,
            cwd=cwd,
            text=True,
            encoding='utf-8',
            errors='replace'
        )
        stdout, stderr = process.communicate()
        return process.returncode, stdout, stderr
    except Exception as e:
        return -1, "", str(e)

def check_cargo_deny():
    """Verifica si cargo-deny está instalado y ejecuta su chequeo."""
    if not shutil.which("cargo"):
        return {"status": "skipped", "reason": "cargo not found", "issues": []}
    
    # Verificar si cargo-deny está disponible
    code, stdout, stderr = run_command("cargo deny --version")
    if code != 0:
        return {"status": "skipped", "reason": "cargo-deny not installed", "issues": []}

    print("Ejecutando cargo deny check...")
    code, stdout, stderr = run_command("cargo deny check")
    
    issues = []
    if code != 0:
        issues.append(f"cargo deny reportó problemas o advertencias:\n{stdout[:1000]}")
    
    return {
        "status": "success" if code == 0 else "failed",
        "reason": "completed",
        "issues": issues,
        "raw": stdout[:2000]
    }

def check_cargo_clippy():
    """Ejecuta cargo clippy para buscar advertencias en Rust."""
    if not shutil.which("cargo"):
        return {"status": "skipped", "reason": "cargo not found", "issues": []}

    print("Ejecutando cargo clippy...")
    # Ejecutamos cargo clippy en workspace sin forzar error para recopilar advertencias
    code, stdout, stderr = run_command("cargo clippy --workspace --all-targets")
    
    warnings = []
    errors = []
    
    # Simple parseo de la salida
    lines = (stdout + "\n" + stderr).splitlines()
    for line in lines:
        if "warning:" in line:
            warnings.append(line.strip())
        elif "error:" in line or "error[" in line:
            errors.append(line.strip())

    # Agrupar issues
    issues = []
    if errors:
        issues.append(f"Errores encontrados en Clippy ({len(errors)}):\n" + "\n".join(errors[:5]))
    if warnings:
        issues.append(f"Advertencias encontradas en Clippy ({len(warnings)}):\n" + "\n".join(warnings[:5]))

    return {
        "status": "success" if not errors else "failed",
        "reason": "completed",
        "warnings_count": len(warnings),
        "errors_count": len(errors),
        "issues": issues,
        "raw": (stdout + "\n" + stderr)[:2000]
    }

def check_npm_audit():
    """Ejecuta npm audit en la UI."""
    if not os.path.exists(SHELL_UI_DIR):
        return {"status": "skipped", "reason": "shell/ui dir not found", "issues": []}
    
    if not shutil.which("npm"):
        return {"status": "skipped", "reason": "npm not found", "issues": []}

    print("Ejecutando npm audit...")
    code, stdout, stderr = run_command("npm audit --json", cwd=SHELL_UI_DIR)
    
    issues = []
    try:
        data = json.loads(stdout)
        metadata = data.get("metadata", {}).get("vulnerabilities", {})
        total_vulns = sum(metadata.values())
        if total_vulns > 0:
            summary = ", ".join([f"{k}: {v}" for k, v in metadata.items() if v > 0])
            issues.append(f"npm audit detectó vulnerabilidades: {summary}")
    except Exception:
        # Caída si npm audit falla o la salida no es JSON estructurado normal
        if "vulnerability" in stdout or "vulnerabilities" in stdout:
            issues.append("npm audit reportó vulnerabilidades en dependencias. Revisar logs.")
        elif code != 0 and stderr:
            issues.append(f"npm audit falló al ejecutarse:\n{stderr[:500]}")

    return {
        "status": "success" if code == 0 else "failed",
        "reason": "completed",
        "issues": issues,
        "raw": stdout[:2000]
    }

import datetime

def generate_markdown_report(deny_results, clippy_results, npm_results, all_issues):
    date_str = datetime.date.today().strftime("%Y-%m-%d")
    report_dir = os.path.join(ROOT_DIR, "governance", "Tickets")
    os.makedirs(report_dir, exist_ok=True)
    report_path = os.path.join(report_dir, f"CORE-AUDIT-{date_str}.md")
    
    # Intenta obtener el hash de commit actual para mostrarlo
    commit_hash = "Desconocido"
    try:
        process = subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True, check=True, cwd=ROOT_DIR)
        commit_hash = process.stdout.strip()
    except Exception:
        pass

    lines = []
    lines.append(f"# Auditoría de Seguridad & UX — {date_str}")
    lines.append("")
    lines.append("| Métrica | Resultado | Notas |")
    lines.append("|---|---|---|")
    lines.append(f"| Commits Analizados | `{commit_hash}` | Últimas actualizaciones locales |")
    
    total_vulns = len(deny_results.get("issues", [])) + len(npm_results.get("issues", []))
    lines.append(f"| Vulnerabilidades detectadas | {total_vulns} reportadas | `cargo-deny` y `npm audit` |")
    lines.append(f"| Advertencias de Clippy | {clippy_results.get('warnings_count', 0)} advertencias | Linter estático de Rust |")
    lines.append("| Problemas de UX identificados | *Pendiente* | Se completará con la auditoría de UX del agente de IA |")
    lines.append("")
    
    lines.append("## 🔒 Hallazgos de Seguridad")
    
    # Cargo Deny
    if deny_results.get("status") == "skipped":
        lines.append(f"- **[Info] cargo-deny omitido:** {deny_results.get('reason')}")
    elif not deny_results.get("issues"):
        lines.append("- **[Info] cargo-deny:** No se encontraron vulnerabilidades ni problemas de licencias en Rust.")
    else:
        for issue in deny_results.get("issues"):
            lines.append(f"- **[Alto] Problema en dependencias de Rust:**")
            lines.append(f"  * {issue}")
            
    # NPM Audit
    if npm_results.get("status") == "skipped":
        lines.append(f"- **[Info] npm audit omitido:** {npm_results.get('reason')}")
    elif not npm_results.get("issues"):
        lines.append("- **[Info] npm audit:** No se encontraron vulnerabilidades en dependencias de Node.")
    else:
        for issue in npm_results.get("issues"):
            lines.append(f"- **[Medio] Vulnerabilidad en dependencias de UI (npm):**")
            lines.append(f"  * {issue}")
            
    lines.append("")
    lines.append("## 🦀 Advertencias de Calidad (Rust Clippy)")
    if clippy_results.get("status") == "skipped":
        lines.append(f"- **[Info] cargo clippy omitido:** {clippy_results.get('reason')}")
    elif clippy_results.get("errors_count", 0) == 0 and clippy_results.get("warnings_count", 0) == 0:
        lines.append("- **[Info] Clippy:** Código Rust 100% limpio de advertencias.")
    else:
        lines.append(f"- Se encontraron {clippy_results.get('warnings_count', 0)} advertencias y {clippy_results.get('errors_count', 0)} errores.")
        if clippy_results.get("issues"):
            for issue in clippy_results.get("issues"):
                lines.append("```")
                lines.append(issue)
                lines.append("```")
                
    lines.append("")
    lines.append("## 🎨 Hallazgos de UX / UI")
    lines.append("*Pendiente de Auditoría del Agente. El agente de IA evaluará el diseño y la consistencia de UI durante la madrugada.*")
    lines.append("")
    lines.append("## 🌐 Investigación Web & Propuestas de Mejora")
    lines.append("*Pendiente de Auditoría del Agente. El agente buscará en internet novedades de dependencias y mejoras.*")
    lines.append("")
    lines.append("## 📋 Lista de Tickets Propuestos")
    lines.append("*Pendiente de consolidación por el agente de IA.*")
    lines.append("")

    with open(report_path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))
    print(f"Reporte de gobernanza guardado en {report_path}")

def main():
    print("Iniciando escaneo local estático de Aegis-Core...")
    
    deny_results = check_cargo_deny()
    clippy_results = check_cargo_clippy()
    npm_results = check_npm_audit()
    
    # Consolidar reporte
    all_issues = []
    all_issues.extend(deny_results.get("issues", []))
    all_issues.extend(clippy_results.get("issues", []))
    all_issues.extend(npm_results.get("issues", []))
    
    report_path = os.path.join(ROOT_DIR, "scratch", "last_static_audit.json")
    os.makedirs(os.path.dirname(report_path), exist_ok=True)
    
    report_data = {
        "cargo_deny": {k: v for k, v in deny_results.items() if k != "raw"},
        "cargo_clippy": {k: v for k, v in clippy_results.items() if k != "raw"},
        "npm_audit": {k: v for k, v in npm_results.items() if k != "raw"},
        "total_issues": len(all_issues),
        "issues_summary": all_issues
    }
    
    with open(report_path, "w", encoding="utf-8") as f:
        json.dump(report_data, f, indent=2, ensure_ascii=False)
        
    generate_markdown_report(deny_results, clippy_results, npm_results, all_issues)
        
    print("\n" + "=" * 50)
    print("=== RESUMEN DE AUDITORÍA ESTÁTICA LOCAL ===")
    print("=" * 50)
    print(f"Total de problemas encontrados: {len(all_issues)}")
    for issue in all_issues:
        print(f"- {issue.splitlines()[0]}")
    print("=" * 50)
    print(f"Reporte guardado en scratch/last_static_audit.json")
    
    return 0 if len(all_issues) == 0 else 1

if __name__ == "__main__":
    sys.exit(main())
