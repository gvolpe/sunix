{ cargo
, clippy
, gtk4
, gtk4-layer-shell
, mkShell
, pkg-config
, rustc
, rustfmt
}:

mkShell {
  buildInputs = [ gtk4 gtk4-layer-shell ];

  nativeBuildInputs = [
    cargo
    clippy
    pkg-config
    rustc
    rustfmt
  ];
}
