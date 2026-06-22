{ pkgs, toolchain }:

let
  inherit (pkgs) writeShellApplication;
in
{

  # Format
  check-rustfmt = (
    writeShellApplication {
      name = "check-rustfmt";
      runtimeInputs = [ toolchain ];
      text = "cargo fmt --check";
    }
  );

  # Spelling
  check-spelling = (
    writeShellApplication {
      name = "check-spelling";
      runtimeInputs = with pkgs; [
        git
        typos
      ];
      text = ''
        typos
      '';
    }
  );

  # NixFormatting
  check-nix-fmt = (
    writeShellApplication {
      name = "check-nix-fmt";
      runtimeInputs = with pkgs; [
        nixfmt-tree
      ];
      text = ''
        treefmt --ci
      '';
    }
  );

  # EditorConfig
  check-editorconfig = (
    writeShellApplication {
      name = "check-editorconfig";
      runtimeInputs = with pkgs; [ eclint ];
      text = ''
        eclint .
      '';
    }
  );

  # Clippy
  check-clippy = (
    writeShellApplication {
      name = "check-clippy";
      runtimeInputs = [ toolchain ];
      text = ''
        cargo clippy --all-features --all-targets -- -D warnings
      '';
    }
  );

}
