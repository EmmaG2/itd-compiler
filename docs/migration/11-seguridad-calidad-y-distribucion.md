# Feature 11: Seguridad, calidad y distribución

## Objetivo

Preparar una primera versión distribuible, reproducible y resistente a código fuente malicioso o defectuoso.

## Seguridad

- [x] Limitar tamaño de archivo, número de módulos y profundidad de imports.
- [x] Limitar instrucciones, stack, heap, output y tiempo por ejecución.
- [ ] Hacer cancelables lexer, compilación de proyectos grandes y VM cuando sea necesario.
- [x] Mantener filesystem, red y procesos fuera de la biblioteca estándar inicial.
- [x] Auditar todas las rutas canonicalizadas y contratos IPC.
- [x] Rechazar bytecode inválido incluso si fue creado fuera del compilador.
- [x] Revisar dependencias y evitar crates innecesarios.
- [x] No incluir secretos, rutas locales ni backtraces en builds de release.

## Biblioteca estándar mínima

- [x] Salida de texto controlada por el host.
- [x] Operaciones esenciales de strings.
- [x] Conversiones explícitas entre tipos primitivos.
- [x] Funciones decimales documentadas, incluyendo redondeo.
- [x] Colección iterable mínima solo si `each` forma parte del MVP (no forma parte de 0.1).

## Calidad

- [x] Ejecutar `cargo fmt --check`.
- [x] Ejecutar `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] Ejecutar `cargo test --workspace`.
- [x] Ejecutar type check y tests del frontend.
- [x] Añadir pruebas end-to-end de programas completos.
- [ ] Añadir fuzzing del lexer/parser después de estabilizar su API.
- [x] Verificar que errores esperados no usan `panic`, `unwrap` o `expect` en rutas de usuario.
- [x] Documentar formato, gramática, tipos, módulos y modelo de objetos.

## CI y distribución

- [x] Crear CI para checks Rust y frontend en cada PR.
- [x] Configurar la prueba de build Tauri en Windows, macOS y Linux soportados.
- [x] Definir versión de lenguaje, compilador y aplicación.
- [ ] Generar artefactos firmados cuando existan certificados del proyecto.
- [x] Publicar notas de versión con cambios incompatibles y limitaciones conocidas.
- [x] Incluir ejemplos ejecutables y una guía de inicio rápido.

## Pruebas de aceptación del producto

- [x] Proyecto de varios módulos con clase, trait, funciones y decimal.
- [x] Diagnósticos léxicos, sintácticos, semánticos y de runtime visibles en Tauri.
- [x] Programa infinito cancelado sin cerrar la aplicación.
- [x] Intento de acceso fuera del proyecto rechazado.
- [x] Resultado decimal exacto conservado en UI.
- [ ] Instalación limpia y ejecución en cada plataforma objetivo.

## Criterios de aceptación

- Todos los checks automáticos pasan desde una instalación limpia.
- Los límites de recursos tienen valores predeterminados documentados y configurables por el host.
- La aplicación puede distribuirse sin Java instalado.
- Las limitaciones fuera del MVP aparecen en la documentación del release.

## Fuera de alcance

Marketplace de paquetes, telemetría, actualizaciones automáticas, depurador y language server. Se crean como features independientes cuando sean requeridos.
