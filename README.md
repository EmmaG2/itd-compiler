# ITD Compiler 0.1

Compilador ITD en Rust con aplicación de escritorio Tauri 2. No requiere Java.

## Desarrollo

Requisitos: Rust estable, Node.js 22 y las [dependencias del sistema de Tauri 2](https://v2.tauri.app/start/prerequisites/).

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

## Verificación

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
pnpm test
pnpm build
```

Los ejemplos de sintaxis están en [`docs/language/examples`](docs/language/examples). Los límites y el modelo de seguridad están en [`docs/security.md`](docs/security.md).
