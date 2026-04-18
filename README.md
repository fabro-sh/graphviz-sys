# graphviz-sys

Rust crate that statically links a vendored [Graphviz](https://graphviz.org/) 14.1.5
and exposes a single function for rendering DOT source to SVG.

No system dependency on `libgraphviz`, no `bindgen` at build time — the C sources
are compiled via [`cc`](https://crates.io/crates/cc) and the eight FFI symbols we
need are declared by hand in `src/lib.rs`.

## Usage

```toml
[dependencies]
graphviz-sys = { git = "https://github.com/fabro-sh/graphviz-sys" }
```

```rust
let svg = graphviz_sys::render_dot_to_svg("digraph { a -> b }")?;
std::fs::write("graph.svg", svg)?;
```

`render_dot_to_svg` returns the SVG bytes, or an error string if the DOT source
is invalid or Graphviz fails to lay out or render it.

## What's included

Only what's needed to go from DOT to SVG:

- `dot` layout engine (`plugin/dot_layout`)
- Core renderer plugin (`plugin/core`) — includes the SVG backend
- Supporting libraries: `cdt`, `cgraph`, `common`, `dotgen`, `gvc`, `label`,
  `ortho`, `pack`, `pathplan`, `util`, `xdot`

Other layout engines (neato, fdp, sfdp, twopi, circo, osage, patchwork) and
other output formats are not compiled in.

## Thread safety

Graphviz has non-thread-safe global state (string interning, error counters,
layout state). `render_dot_to_svg` serializes all calls through a global
`Mutex`. Callers should run it on a blocking thread pool (e.g. Tokio's
`spawn_blocking`).

## Build

```
cargo build --release
cargo test --release
```

The build script compiles ~12 static archives and links them into the final
binary. On Linux the archives are merged with `ar -M` to resolve circular
symbol references between `common` and `gvc`; macOS's linker handles cycles
natively.

### Supported targets

CI builds and tests:

- `aarch64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`

musl builds require `musl-tools` (`musl-gcc`).

## Layout

```
build.rs              compiles vendored Graphviz C into static archives
builtins.c            registers the statically-linked plugins with Graphviz
config.h              minimal replacement for the autotools-generated config
src/lib.rs            FFI declarations and render_dot_to_svg
vendor/graphviz-14.1.5  upstream Graphviz source (pruned)
```

## License

Graphviz is licensed under the [Eclipse Public License 2.0](https://www.eclipse.org/legal/epl-2.0/).
This crate is distributed under the same license.
