{
  description = "";
  inputs = {
    # Forgive me.
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = github:NixOS/nixpkgs/nixpkgs-unstable;
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, flake-utils, nixpkgs, rust-overlay }@inputs: let
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
  in (flake-utils.lib.eachDefaultSystem (system: (let
    pkgs = import nixpkgs {
      overlays = shell-overlays;
      inherit system;
    };
  in {
    devShells.default = pkgs.mkShell {
      # This might be more than what is needed to get libgit2-sys to build, but
      # it does contain the right bits in there.
      buildInputs = [
        pkgs.cargo
        pkgs.libgit2
        # Might not be needed.  Verify.
        pkgs.libgit2.dev
        pkgs.libssh2
        pkgs.openssl
        # So cargo can find our various "lib" packages.  Might not be needed.
        # Verify.
        pkgs.pkg-config
        pkgs.rust-pinned
      ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
        # Verified as a libgit2-sys requirement.
        pkgs.darwin.apple_sdk.frameworks.Security
        pkgs.darwin.apple_sdk.frameworks.CoreFoundation
      ];
      # You know, there's a pre-commit tool out there which handles this very
      # poorly - if you have `core.hooksPath` set, pre-commit bails out.  But
      # really all it's doing is saving us 3 characters from typing `cp`.
      shellHook = ''
        export LIBGIT2_SYS_USE_PKG_CONFIG=1
        cp ./pre-commit-hook ./.git/hooks/pre-commit
      '';
    };

    packages.default = pkgs.callPackage ./derivation.nix {};
    defaultPackage = self.packages.${system}.default;
  }))) // {
    overlays.default = final: prev: {
      repo-sync = prev.callPackage ./derivation.nix {};
    };
  };
}
