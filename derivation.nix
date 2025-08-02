{
  cargo,
  darwin,
  lib,
  libgit2,
  libssh2,
  openssl,
  pkg-config,
  rustPlatform,
  stdenv,
  ...
}:
rustPlatform.buildRustPackage (let
  version = "0.1.0";
in {
  pname = "repo-sync";
  inherit version;
  src = ./.;
  cargoLock = {
    lockFile = ./Cargo.lock;
  };
  LIBGIT2_SYS_USE_PKG_CONFIG = "1";
  # While not strictly documented as such, `buildInputs` appears to be the
  # correct place to put these, even though they aren't needed at runtime.
  # Without these declared here, `pkg-config` won't find them and the build will
  # fail.
  buildInputs = [
    libgit2
    # Might not be needed.  Verify.
    libgit2.dev
    libssh2
    openssl
  ];
  nativeBuildInputs = [
    cargo
    # libgit2
    # # Might not be needed.  Verify.
    # libgit2.dev
    # libssh2
    # openssl
    # So cargo can find our various "lib" packages.  Might not be needed.
    # Verify.
    pkg-config
  ] ++ lib.optionals stdenv.isDarwin [
    # Verified as a libgit2-sys requirement.
    darwin.apple_sdk.frameworks.Security
    darwin.apple_sdk.frameworks.CoreFoundation
  ];
})
