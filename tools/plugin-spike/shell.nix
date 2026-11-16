# The spike's tools, on top of the system rustc (whose sysroot ships
# wasm32-unknown-unknown but not wasm32-wasip1/wasip2): lld for the wasm
# linker rustc calls `lld`, wasm-tools to wrap the WIT guest's core module into
# a component. Run ./run.sh inside it: `nix-shell tools/plugin-spike/shell.nix`.
{
  pkgs ? import <nixpkgs> { },
}:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    lld
    rustup
    wasm-tools
  ];
}
