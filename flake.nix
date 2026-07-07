{
  description = "IPC fuzzy file finder — nucleo & ignore wrapper";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        packages = {
          goldfish = pkgs.rustPlatform.buildRustPackage {
            pname = "goldfish";
            version = "0.1.0";

            src = ./.;

            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.sqlite ];

            meta = with pkgs.lib; {
              description = "IPC fuzzy file finder — nucleo & ignore wrapper";
              homepage = "https://github.com/sameoldlab/goldfish";
              license = licenses.mpl20;
              mainProgram = "gf";
            };
          };
          default = self.packages.${system}.goldfish;
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ self.packages.${system}.goldfish ];
        };
      }
    );
}
