#!/usr/bin/env python3
import os
import sys
import time
import socket
import subprocess
import urllib.request
import urllib.error
import json

# Colores para salida de consola premium
class Colors:
    HEADER = '\033[95m'
    OKBLUE = '\033[94m'
    OKGREEN = '\033[92m'
    WARNING = '\033[93m'
    FAIL = '\033[91m'
    ENDC = '\033[0m'
    BOLD = '\033[1m'
    UNDERLINE = '\033[4m'

def print_header(title):
    print(f"\n{Colors.HEADER}{Colors.BOLD}=== {title} ==={Colors.ENDC}")

def print_success(message):
    print(f"{Colors.OKGREEN}[OK] {message}{Colors.ENDC}")

def print_fail(message):
    print(f"{Colors.FAIL}[FAIL] {message}{Colors.ENDC}")

def print_info(message):
    print(f"{Colors.OKBLUE}[INFO] {message}{Colors.ENDC}")

def print_warning(message):
    print(f"{Colors.WARNING}[WARN] {message}{Colors.ENDC}")

def run_command(command, cwd=None, env=None):
    """Ejecuta un comando en consola de forma síncrona y devuelve True si tiene éxito."""
    try:
        process = subprocess.run(
            command,
            cwd=cwd,
            env=env,
            shell=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True
        )
        if process.returncode == 0:
            return True, process.stdout
        else:
            return False, process.stdout
    except Exception as e:
        return False, str(e)

def is_port_open(port, host='127.0.0.1'):
    """Verifica si un puerto TCP está abierto y escuchando."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.settimeout(1.0)
        try:
            s.connect((host, port))
            return True
        except (socket.timeout, ConnectionRefusedError):
            return False

def http_get(url):
    """Realiza un GET HTTP simple y devuelve el código de estado y el cuerpo."""
    try:
        with urllib.request.urlopen(url, timeout=3.0) as response:
            return response.status, response.read().decode('utf-8')
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode('utf-8')
    except urllib.error.URLError as e:
        return 0, str(e.reason)
    except Exception as e:
        return 0, str(e)

def main():
    print(f"{Colors.HEADER}{Colors.BOLD}*** INICIANDO SMOKE TEST DE AEGIS CORE ***{Colors.ENDC}")
    print("=" * 50)
    
    root_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    print_info(f"Directorio raíz del proyecto: {root_dir}")
    
    results = {}
    
    # ----------------------------------------------------
    # 1. Compilar y validar UI
    # ----------------------------------------------------
    print_header("Fase 1: Compilación de UI (shell/ui)")
    ui_dir = os.path.join(root_dir, "shell", "ui")
    
    print_info("Ejecutando compilación de producción en shell/ui...")
    success, output = run_command("npm run build", cwd=ui_dir)
    if success:
        print_success("UI compilada correctamente (Vite + TypeScript).")
        results["ui_build"] = "PASS"
    else:
        print_fail("Error al compilar la UI.")
        print(output[:1000] + "\n..." if len(output) > 1000 else output)
        results["ui_build"] = "FAIL"
        sys.exit(1)

    # ----------------------------------------------------
    # 2. Tests unitarios del kernel en Rust
    # ----------------------------------------------------
    print_header("Fase 2: Pruebas unitarias de Rust")
    print_info("Ejecutando cargo test --workspace...")
    success, output = run_command("cargo test --workspace --all-targets", cwd=root_dir)
    if success:
        print_success("Todos los tests de Rust pasaron correctamente.")
        results["rust_tests"] = "PASS"
    else:
        print_fail("Fallo en las pruebas unitarias de Rust.")
        print(output[:2000] + "\n..." if len(output) > 2000 else output)
        results["rust_tests"] = "FAIL"
        # Preguntar si continuar o abortar, pero como es script automático salimos
        sys.exit(1)

    # ----------------------------------------------------
    # 3. Compilar kernel con UI embebida
    # ----------------------------------------------------
    print_header("Fase 3: Compilación del Kernel (ank-server)")
    print_info("Compilando ank-server con soporte de UI embebida...")
    success, output = run_command("cargo build -p ank-server --features embed-ui", cwd=root_dir)
    if success:
        print_success("Kernel compilado correctamente.")
        results["kernel_build"] = "PASS"
    else:
        print_fail("Error al compilar el kernel.")
        print(output[:1000] + "\n..." if len(output) > 1000 else output)
        results["kernel_build"] = "FAIL"
        sys.exit(1)

    # ----------------------------------------------------
    # 4. Arrancar servidor en background para pruebas de API
    # ----------------------------------------------------
    print_header("Fase 4: Ejecución en vivo de ank-server")
    print_info("Iniciando ank-server en segundo plano...")
    
    # Buscamos el ejecutable compilado en target/debug o release. Por defecto en debug.
    exe_name = "ank-server.exe" if os.name == 'nt' else "ank-server"
    exe_path = os.path.join(root_dir, "target", "debug", exe_name)
    
    if not os.path.exists(exe_path):
        print_fail(f"No se encontró el ejecutable en: {exe_path}")
        sys.exit(1)
        
    # Establecer variables de entorno de prueba para evitar costes
    test_env = os.environ.copy()
    test_env["HW_PROFILE"] = "2" # Perfil local de hardware
    test_env["RUST_LOG"] = "info"
    test_env["AEGIS_ROOT_KEY"] = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
    
    # Aislar directorio de datos para no contaminar el desarrollo del usuario
    smoke_data_dir = os.path.join(root_dir, "target", "smoke_test_data")
    test_env["AEGIS_DATA_DIR"] = smoke_data_dir
    os.makedirs(smoke_data_dir, exist_ok=True)
    
    # Lanzar el proceso del servidor
    server_process = None
    try:
        server_process = subprocess.Popen(
            [exe_path],
            cwd=root_dir,
            env=test_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True
        )
        print_info(f"Proceso iniciado con PID: {server_process.pid}")
        
        # Esperar a que el puerto HTTP esté disponible
        http_port = 8000
        grpc_port = 50051
        
        print_info("Esperando a que los puertos respondan...")
        max_attempts = 15
        server_ready = False
        
        for attempt in range(1, max_attempts + 1):
            time.sleep(1.0)
            if is_port_open(http_port):
                server_ready = True
                print_success(f"Servidor HTTP listo en puerto {http_port} (intento {attempt}/{max_attempts}).")
                break
            
            # Verificar si el proceso murió prematuramente
            ret = server_process.poll()
            if ret is not None:
                print_fail("El proceso del servidor se cerró inesperadamente durante el inicio.")
                break
                
        if not server_ready:
            print_fail("El servidor no respondió en el puerto 8000 a tiempo.")
            results["server_run"] = "FAIL"
            # Volcar últimas líneas de log
            if server_process:
                server_process.terminate()
            sys.exit(1)
            
        results["server_run"] = "PASS"
        
        # ----------------------------------------------------
        # 5. Validar endpoints
        # ----------------------------------------------------
        print_header("Fase 5: Validación de Endpoints y Puertos")
        
        # A. Chequear /health
        print_info("Verificando endpoint público /health...")
        status, body = http_get(f"http://127.0.0.1:{http_port}/health")
        if status == 200:
            try:
                data = json.loads(body)
                if data.get("status") == "Online":
                    print_success(f"Endpoint /health respondió correctamente: {body.strip()}")
                    results["endpoint_health"] = "PASS"
                else:
                    print_fail(f"Formato incorrecto en /health: {body}")
                    results["endpoint_health"] = "FAIL"
            except json.JSONDecodeError:
                print_fail(f"Respuesta de /health no es JSON válido: {body}")
                results["endpoint_health"] = "FAIL"
        else:
            print_fail(f"Fallo al consultar /health. Código HTTP: {status}. Respuesta: {body}")
            results["endpoint_health"] = "FAIL"
            
        # B. Chequear /api/system/state
        print_info("Verificando endpoint público /api/system/state...")
        status, body = http_get(f"http://127.0.0.1:{http_port}/api/system/state")
        if status == 200:
            print_success(f"Endpoint /api/system/state respondió correctamente: {body.strip()}")
            results["endpoint_state"] = "PASS"
        else:
            print_fail(f"Fallo al consultar /api/system/state. Código HTTP: {status}")
            results["endpoint_state"] = "FAIL"

        # C. Verificar gRPC Port
        print_info(f"Verificando disponibilidad del puerto gRPC ({grpc_port})...")
        if is_port_open(grpc_port):
            print_success(f"Puerto gRPC {grpc_port} respondiendo a nivel TCP.")
            results["grpc_port"] = "PASS"
        else:
            print_fail(f"Puerto gRPC {grpc_port} no está escuchando.")
            results["grpc_port"] = "FAIL"

    finally:
        # Apagado limpio del servidor
        if server_process:
            print_header("Fase 6: Parada y Limpieza")
            print_info("Enviando señal de terminación al servidor...")
            server_process.terminate()
            try:
                # Esperar hasta 5 segundos para que cierre
                server_process.wait(timeout=5.0)
                print_success("Servidor detenido limpiamente.")
            except subprocess.TimeoutExpired:
                print_warning("El servidor no se detuvo a tiempo, forzando apagado (kill)...")
                server_process.kill()
                server_process.wait()

            # Limpiar directorio de datos temporales
            smoke_data_dir = test_env.get("AEGIS_DATA_DIR")
            if smoke_data_dir and os.path.exists(smoke_data_dir):
                import shutil
                try:
                    # Dar un pequeño respiro para que se liberen handles de archivos
                    time.sleep(1.0)
                    shutil.rmtree(smoke_data_dir)
                    print_success("Directorio temporal de datos de prueba eliminado.")
                except Exception as e:
                    print_warning(f"No se pudo eliminar el directorio temporal de datos: {e}")

    # ----------------------------------------------------
    # Reporte Final
    # ----------------------------------------------------
    print("\n" + "=" * 50)
    print(f"{Colors.HEADER}{Colors.BOLD}=== RESUMEN DE RESULTADOS DEL SMOKE TEST ==={Colors.ENDC}")
    print("=" * 50)
    
    all_passed = True
    for key, val in results.items():
        status_color = Colors.OKGREEN if val == "PASS" else Colors.FAIL
        print(f"{key:<25}: {status_color}{Colors.BOLD}{val}{Colors.ENDC}")
        if val != "PASS":
            all_passed = False
            
    print("=" * 50)
    if all_passed:
        print(f"{Colors.OKGREEN}{Colors.BOLD}>>> ¡EL SMOKE TEST DE AEGIS CORE PASO CORRECTAMENTE! listo para produccion. <<<{Colors.ENDC}")
        sys.exit(0)
    else:
        print(f"{Colors.FAIL}{Colors.BOLD}!!! ALGUNAS PRUEBAS FALLARON. Revisa los logs superiores antes del lanzamiento. !!!{Colors.ENDC}")
        sys.exit(1)

if __name__ == "__main__":
    main()
