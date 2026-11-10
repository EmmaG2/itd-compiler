.DEFAULT_GOAL := help

.PHONY: help install dev run web build macos windows compile test test-core test-arm64 arm64-metrics test-ui benchmark lint format format-rust-files format-check check

help:
	@printf '%s\n' \
	  'make install       Instala las dependencias de pnpm' \
	  'make dev / run     Abre la aplicación Tauri en desarrollo' \
	  'make web           Inicia Vite en el navegador' \
	  'make build         Compila y empaqueta la aplicación Tauri' \
	  'make macos         Genera el instalador DMG para macOS' \
	  'make windows       Genera el instalador NSIS (.exe) para Windows x64' \
	  'make compile       Comprueba tipos y compila el frontend' \
	  'make test          Ejecuta las pruebas de Rust y frontend' \
	  'make test-core     Prueba el compilador y el intérprete de referencia' \
	  'make test-arm64    Ejecuta las pruebas nativas ARM64 ignoradas (solo Apple Silicon)' \
	  'make arm64-metrics Imprime métricas estáticas y ensamblador ARM64 del fixture decimal' \
	  'make test-ui       Ejecuta las pruebas del frontend' \
	  'make benchmark     Compara el rendimiento de la VM y el intérprete' \
	  'make lint          Ejecuta Clippy y comprueba tipos del frontend' \
	  'make format        Aplica rustfmt y Prettier' \
	  'make format-rust-files FILES="<archivos>" Formatea solo los archivos Rust indicados' \
	  'make format-check  Comprueba el formato' \
	  'make check         Comprueba formato, lint, pruebas y frontend'

install:
	pnpm install --frozen-lockfile

dev run:
	pnpm tauri dev

web:
	pnpm dev

build:
	pnpm tauri build

macos:
	pnpm tauri build --bundles dmg

windows:
	pnpm tauri build --target x86_64-pc-windows-msvc --bundles nsis

compile:
	pnpm build

test:
	cargo test --workspace --all-features
	pnpm test

test-core:
	cargo test -p compiler-core --features reference-interpreter

test-arm64:
	cargo test -p compiler-core --test arm64 -- --ignored

arm64-metrics:
	cargo run -p compiler-core --example arm64_metrics

test-ui:
	pnpm test

benchmark:
	cargo run -p compiler-core --release --example benchmark --features reference-interpreter --offline

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	pnpm --dir apps/desktop typecheck

format:
	cargo fmt
	pnpm format

format-rust-files:
	@test -n "$(strip $(FILES))" || { echo "FILES must list at least one Rust file" >&2; exit 2; }
	rustfmt --edition 2024 $(FILES)

format-check:
	cargo fmt --check
	pnpm format:check

check: format-check lint test compile
