{
  description = "Dev environment for event-timers (GW2 Nexus addon, x86_64-pc-windows-gnu)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, fenix }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      mingw = pkgs.pkgsCross.mingwW64;

      # Stable Rust for the host (native `cargo test` on nexus-free crates)
      # plus the std library for the Windows target (the addon dll itself).
      # Explicit component list: defaultToolchain would pull in rust-docs (~1GB).
      toolchain = fenix.packages.${system}.combine [
        (fenix.packages.${system}.stable.withComponents [
          "cargo"
          "clippy"
          "rustc"
          "rustfmt"
          "rust-std"
        ])
        fenix.packages.${system}.targets.x86_64-pc-windows-gnu.stable.rust-std
      ];
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = [
          toolchain
          mingw.stdenv.cc
        ];

        # The addon only compiles for the Windows target (nexus pulls in
        # windows-only crates). Tests live in nexus-free workspace crates and
        # run natively, so the default target stays the host; build the dll
        # with `cargo build --target x86_64-pc-windows-gnu`.
        CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "x86_64-w64-mingw32-gcc";
        # mingw pthreads is not on the cross-gcc's default search path in nix.
        CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS =
          "-L native=${mingw.windows.pthreads}/lib";

        # mkShell exports host CC/CXX, which the `cc` crate can pick up and
        # silently produce ELF objects (arcdps-imgui-sys). Target-scoped vars
        # take precedence and force the cross compilers.
        CC_x86_64_pc_windows_gnu = "x86_64-w64-mingw32-gcc";
        CXX_x86_64_pc_windows_gnu = "x86_64-w64-mingw32-g++";
        AR_x86_64_pc_windows_gnu = "x86_64-w64-mingw32-ar";
      };
    };
}
