{
  description = "";
  inputs = {
    nixpkgs.url = github:NixOS/nixpkgs/nixpkgs-unstable;
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }@inputs: let
    shell-overlays = [
      (import rust-overlay)
      (final: prev: {
        # Use final so we can benefit from rust-overlay.
        # This will include cargo, rustfmt, and other things from Rust.
        rust-pinned = prev.rust-bin.stable.latest.default.override {
          extensions = [
            # For rust-analyzer and others.  See
            # https://nixos.wiki/wiki/Rust#Shell.nix_example for some details.
            "rust-src"
            "rust-analyzer"
            "rustfmt-preview"
          ];
        };
      })
    ];
  in {

    devShells.aarch64-darwin.default = let
      system = "aarch64-darwin";
      pkgs = import nixpkgs {
        overlays = shell-overlays;
        inherit system;
      };
    in pkgs.mkShell {
      buildInputs = [
        pkgs.cargo
        pkgs.clang
        pkgs.darwin.apple_sdk.frameworks.Security
        pkgs.darwin.apple_sdk.frameworks.CoreFoundation
        pkgs.openssl
        pkgs.libssh2
        # To help with finding openssl.
        pkgs.pkg-config
        pkgs.rust-pinned
      ];
      shellHook = ''
      '';
    };

    overlays.default = final: prev: {
      nix-remote-builder-doctor = prev.callPackage ./derivation.nix {};
    };

  };
}
