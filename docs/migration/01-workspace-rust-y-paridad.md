# Feature 01: Workspace Rust y paridad Java

## Objetivo

Crear el núcleo Rust y congelar el comportamiento útil de la base Java antes de sustituirla.

## Tareas

- [x] Crear un workspace Cargo en la raíz.
- [x] Crear `crates/compiler-core` como biblioteca sin dependencias de Tauri.
- [x] Fijar la versión estable de Rust mediante `rust-toolchain.toml`.
- [x] Configurar `rustfmt` y Clippy con warnings tratados como errores.
- [x] Crear fixtures para declaraciones, precedencia aritmética, variables y errores existentes.
- [x] Ejecutar esos fixtures con Java y guardar únicamente entradas y resultados esperados legibles.
- [x] Definir una API inicial pura: `analyze(source)` y `run(source)`.
- [x] Crear tipos públicos mínimos para resultado, salida y diagnóstico.
- [x] Configurar pruebas unitarias y de integración de `compiler-core`.
- [x] Documentar divergencias intencionales cuando Rust corrija un bug Java.

## PRs sugeridos

1. `chore: initialize rust compiler workspace`
2. `test: capture java compiler baseline`
3. `feat(core): add compiler result contracts`

## Criterios de aceptación

- `cargo check --workspace` pasa.
- Los casos base Java tienen resultados esperados versionados.
- `compiler-core` puede compilarse y probarse sin una ventana Tauri.
- No se ha duplicado en Rust la UI Swing ni sus textos de presentación.

## Limpieza posterior

Eliminar `build/`, la copia duplicada de `MIcompiladordejava` y finalmente `src/` Java solo después de que todas las pruebas de paridad relevantes pasen. Esa eliminación debe ser un PR independiente y recuperable desde Git.
