{
  rustPlatform,
  ...
}:
rustPlatform.buildRustPackage (let
  version = "0.1.0";
in {
  pname = "my-program";
  inherit version;
  src = ./.;
  cargoLock = {
    lockFile = ./Cargo.lock;
  };
  buildInputs = [ ];
})
