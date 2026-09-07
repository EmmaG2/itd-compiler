.DEFAULT_GOAL := help

.PHONY: help install dev run web build macos windows compile test test-core test-ui benchmark lint format format-check check

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
	  'make test-ui       Ejecuta las pruebas del frontend' \
	  'make benchmark     Compara el rendimiento de la VM y el intérprete' \
	  'make lint          Ejecuta Clippy y comprueba tipos del frontend' \
	  'make format        Aplica rustfmt y Prettier' \
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

format-check:
	cargo fmt --check
	pnpm format:check

check: format-check lint test compile
