# Feature 10: Aplicación Tauri

## Objetivo

Reemplazar Swing por una aplicación Tauri 2 que consuma `compiler-core` mediante contratos pequeños y tipados.

## Estrategia

Crear primero una UI mínima con TypeScript, HTML y CSS. No añadir un framework visual ni un editor avanzado hasta necesitar resaltado, autocompletado o navegación de símbolos.

## Comandos Rust

- [x] `analyze_source(source, virtual_path)` para tokens y diagnósticos sin acceso al filesystem.
- [x] `run_source(source, virtual_path)` para programas de un archivo.
- [x] `open_project()` para elegir y registrar una raíz autorizada.
- [x] `run_project(entry_module)` para compilar desde una ruta relativa validada.
- [x] `cancel_run(execution_id)` para detener una ejecución activa.
- [x] Devolver `Result<T, CommandError>` serializable; no exponer errores internos.
- [x] Mantener los comandos delgados: validación IPC, llamada al núcleo y mapeo de DTOs.

## Interfaz

- [x] Editor de código inicial.
- [x] Botones para analizar, ejecutar, detener y abrir proyecto.
- [x] Panel de diagnósticos ordenado por archivo y posición.
- [x] Navegación desde un diagnóstico hasta su rango en el editor.
- [x] Consola de salida separada de logs internos.
- [x] Indicador de estado: listo, analizando, ejecutando, cancelado o fallido.
- [x] Vista opcional de símbolos para sustituir el volcado actual de `SymbolTable`.
- [x] Añadir todas las cadenas visibles a archivos de traducción desde el inicio.
- [x] Definir DTOs TypeScript sin `any` y mantenerlos sincronizados con Rust.

## Seguridad e IPC

- [x] Configurar capabilities de Tauri con mínimo privilegio.
- [x] No habilitar el plugin shell.
- [x] Restringir filesystem al proyecto seleccionado.
- [x] Validar nuevamente rutas y tamaños de entrada en Rust.
- [x] Serializar decimales como strings.
- [x] Ejecutar compilación y VM fuera del hilo de UI.
- [x] Limitar el tamaño de resultados enviados al frontend.

## Pruebas mínimas

- [ ] Analizar código válido e inválido desde la UI.
- [ ] Mostrar varios diagnósticos y navegar al rango correcto.
- [ ] Ejecutar y cancelar un programa.
- [ ] Abrir un proyecto e importar otro módulo.
- [x] Rechazar una ruta fuera de la raíz.
- [x] Mostrar un decimal largo sin pérdida de precisión.
- [x] Type check del frontend y pruebas de comandos Rust.

## PRs sugeridos

1. `feat(ui): initialize tauri application shell`
2. `feat(tauri): expose source analysis command`
3. `feat(ui): render structured diagnostics and output`
4. `feat(tauri): open and run compiler projects safely`
5. `feat(ui): cancel active program execution`

## Criterios de aceptación

- La UI no contiene lógica léxica, sintáctica o semántica.
- Cerrar o cancelar una ejecución no deja tareas activas.
- El frontend no puede invocar shell ni leer rutas arbitrarias.
- El resultado visual conserva archivos, spans y precisión decimal.

## Referencias

- [Comandos Rust en Tauri 2](https://github.com/tauri-apps/tauri-docs/blob/v2/src/content/docs/develop/calling-rust.mdx)
- [Permisos y capabilities en Tauri 2](https://github.com/tauri-apps/tauri-docs/blob/v2/src/content/docs/security/permissions.mdx)
