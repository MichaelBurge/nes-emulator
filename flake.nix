{
    description = "NES Emulator";

    inputs = {
        rust-overlay.url = "github:oxalica/rust-overlay";
        flake-utils.url = "github:numtide/flake-utils";
    };

    outputs = { self, nixpkgs, flake-utils, rust-overlay }:
        let supportedSystems = [ "x86_64-linux" ];
        in 
        flake-utils.lib.eachSystem supportedSystems (system:
            let
                overlays = [ (import rust-overlay)];
                pkgs = import nixpkgs { inherit system overlays; };
                rust = pkgs.rust-bin.nightly."2021-11-20".default;
                rustPlatform = pkgs.makeRustPlatform { cargo = rust; rustc = rust; };
                pkg = rustPlatform.buildRustPackage {
                    name = "nes-emulator";
                    src = ./.;
                    buildInputs = [ pkgs.SDL2 ];
                    cargoSha256 = "sha256-IPZyBBrqi4kroBIfdLXPoHLKfZiwrjmkwC2bKDNw/XA=";
                };
            in  {
                packages = {
                    nes-emulator = pkg;
                };
                defaultPackage = pkg;
                defaultApp = pkg;
        });
} 