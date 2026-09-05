# Changelog

## 0.1.0

- Núcleo Rust con lexer, parser, análisis semántico, módulos, objetos y VM transicional.
- Aplicación Tauri 2 con análisis, ejecución, diagnósticos, símbolos y proyectos autorizados.

Limitaciones: no hay gestor de paquetes, red, shell, herencia, JIT, actualizaciones automáticas ni firma de artefactos. La ejecución de proyectos múltiples analiza el grafo completo; la ejecución enlazada entre módulos queda pendiente del lowering escalar de bytecode.
