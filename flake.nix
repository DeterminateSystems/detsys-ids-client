{
  description = "detsys-ids-client";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0";

    fenix = {
      url = "https://flakehub.com/f/nix-community/fenix/0";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      naersk,
      ...
    }@inputs:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      forAllSystems = f: nixpkgs.lib.genAttrs supportedSystems (system: (forSystem system f));

      forSystem =
        system: f:
        f rec {
          inherit system;
          pkgs = nixpkgs.legacyPackages.${system};
          lib = pkgs.lib;
        };

      fenixToolchain =
        system:
        with fenix.packages.${system};
        combine (
          [
            stable.clippy
            stable.rustc
            stable.cargo
            stable.rustfmt
            stable.rust-src
            stable.rust-analyzer
          ]
          ++ nixpkgs.lib.optionals (system == "x86_64-linux") [
            targets.x86_64-unknown-linux-musl.stable.rust-std
          ]
          ++ nixpkgs.lib.optionals (system == "aarch64-linux") [
            targets.aarch64-unknown-linux-musl.stable.rust-std
          ]
        );
    in
    {
      devShells = forAllSystems (
        { system, pkgs, ... }:
        let
          toolchain = fenixToolchain system;
          check = import ./nix/check.nix { inherit pkgs toolchain; };

        in
        {
          default = pkgs.mkShell.override { stdenv = pkgs.clangStdenv; } {
            name = "detsys-ids-client-shell";

            env.RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";

            packages = with pkgs; [
              toolchain
              cargo-outdated
              cacert
              cargo-audit
              cargo-watch
              cargo-nextest
              cargo-machete
              self.formatter.${system}
              check.check-rustfmt
              check.check-spelling
              check.check-nix-fmt
              check.check-editorconfig
              check.check-clippy
              libiconv
            ];
          };
        }
      );

      formatter = forAllSystems ({ pkgs, ... }: pkgs.nixfmt-tree);

      checks = forAllSystems (
        { system, pkgs, ... }:
        let
          toolchain = fenixToolchain system;
          check = import ./nix/check.nix { inherit pkgs toolchain; };
        in
        {
          check-rustfmt = pkgs.runCommand "check-rustfmt" { buildInputs = [ check.check-rustfmt ]; } ''
            cd ${./.}
            check-rustfmt
            touch $out
          '';
          check-spelling = pkgs.runCommand "check-spelling" { buildInputs = [ check.check-spelling ]; } ''
            cd ${./.}
            check-spelling
            touch $out
          '';
          check-nix-fmt = pkgs.runCommand "check-nix-fmt" { buildInputs = [ check.check-nix-fmt ]; } ''
            cd ${./.}
            check-nix-fmt
            touch $out
          '';
          check-editorconfig =
            pkgs.runCommand "check-editorconfig"
              {
                buildInputs = [
                  pkgs.git
                  check.check-editorconfig
                ];
              }
              ''
                cd ${./.}
                check-editorconfig
                touch $out
              '';
        }
      );
    };
}
