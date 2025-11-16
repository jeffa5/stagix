{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs = {
    self,
    nixpkgs,
  }: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
    cargoNix = pkgs.callPackage ./Cargo.nix {};
    packages = pkgs.lib.mapAttrs (name: crate: crate.build) cargoNix.workspaceMembers;
    fs = pkgs.lib.fileset;
    stagix-assets = pkgs.stdenv.mkDerivation {
      pname = "stagix-assets";
      version = "0.1.0";
      src = fs.toSource {
        root = ./.;
        fileset = fs.unions [./style.css ./logo.png ./favicon.png];
      };
      installPhase = ''
        mkdir -p $out/share/doc/stagix
        cp $src/* $out/share/doc/stagix/
      '';
    };
  in {
    packages.${system} =
      packages
      // {
        inherit stagix-assets;
        stagix = pkgs.symlinkJoin {
          name = "stagix";
          paths = [packages.stagix stagix-assets];
        };
      };

    devShells.${system}.default = pkgs.mkShell {
      packages = [
        pkgs.rustc
        pkgs.cargo
        pkgs.rustfmt
        pkgs.clippy

        pkgs.crate2nix
      ];
    };
  };
}
