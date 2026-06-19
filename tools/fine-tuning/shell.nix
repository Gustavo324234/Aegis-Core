{ pkgs ? import <nixpkgs> { config.allowUnfree = true; } }:

let
  pythonEnv = pkgs.python311.withPackages (ps: with ps; [
    pip
    virtualenv
    setuptools
    wheel
  ]);
in
pkgs.mkShell {
  name = "aegis-ml-env";
  
  buildInputs = with pkgs; [
    pythonEnv
    git
    pkg-config
    openssl
    zlib
    stdenv.cc.cc.lib
  ];

  shellHook = ''
    echo "=========================================================="
    echo "  Aegis OS — Entorno de Aprendizaje Automático (Nix Shell)"
    echo "=========================================================="
    
    # Crear y activar entorno virtual si no existe
    if [ ! -d ".venv" ]; then
      echo "Creando entorno virtual .venv..."
      virtualenv .venv
    fi
    source .venv/bin/activate

    echo ""
    echo "Para instalar las dependencias ejecute:"
    echo "  pip install -r requirements.txt"
    echo "=========================================================="
  '';

  # Crucial para que librerías nativas de PyTorch y bitsandbytes encuentren CUDA y C++ libs en NixOS
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [
    stdenv.cc.cc.lib
    zlib
    glibc
    libffi
    openssl
  ]) + ":/run/opengl-driver/lib:/run/opengl-driver-32/lib";
}
