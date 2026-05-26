{ lib, rustPlatform, fetchFromGitHub, pkg-config, openssl, sqlite, libsqlcipher-dev ? null }:

rustPlatform.buildRustPackage rec {
  pname = "ank-server";
  version = "1.0.0";

  src = ../../.;

  cargoLock = {
    lockFile = ../../Cargo.lock;
  };

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ openssl sqlite ];

  # Force features for standalone static compilation of SQLCipher and SQLite
  buildFeatures = [ "bundled-sqlcipher-vendored-openssl" ];

  meta = with lib; {
    description = "Aegis OS Cognitive Kernel Server";
    homepage = "https://github.com/Gustavo324234/Aegis-Core";
    license = licenses.mit;
    maintainers = [ "Gustavo324234" ];
  };
}