{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs = { self, nixpkgs }: let
    pkgs = nixpkgs.legacyPackages."x86_64-linux";
  in {

    devShells."x86_64-linux".default = pkgs.mkShell {
      buildInputs = with pkgs; [
        wayland
        libxkbcommon
        vulkan-loader
        libGL
      ];
      nativeBuildInputs = [ pkgs.pkg-config ];

      LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
        pkgs.wayland
        pkgs.libxkbcommon
        pkgs.vulkan-loader
        pkgs.libGL
      ];

      env.RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
    };
  };
}
