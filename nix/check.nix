{ pkgs, toolchain }:

let
  inherit (pkgs) writeShellApplication;
in
{

  # Format
  check-rustfmt = (writeShellApplication {
    name = "check-rustfmt";
    runtimeInputs = [ toolchain ];
    text = "cargo fmt --check";
  });

  # Spelling
  check-spelling = (writeShellApplication {
    name = "check-spelling";
    runtimeInputs = with pkgs; [ git typos ];
    text = ''
      typos
    '';
  });

  # NixFormatting
  check-nixpkgs-fmt = (writeShellApplication {
    name = "check-nixpkgs-fmt";
    runtimeInputs = with pkgs; [ git nixpkgs-fmt findutils ];
    text = ''
      nixpkgs-fmt --check .
    '';
  });

  # EditorConfig
  check-editorconfig = (writeShellApplication {
    name = "check-editorconfig";
    runtimeInputs = with pkgs; [ eclint ];
    text = ''
      eclint .
    '';
  });

  # Clippy
  check-clippy = (writeShellApplication {
    name = "check-clippy";
    runtimeInputs = [ toolchain ];
    text = ''
      cargo clippy --all-features --all-targets -- -D warnings
    '';
  });

}
