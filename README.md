# ITD Compiler 0.1

Compilador ITD en Rust con aplicación de escritorio Tauri 2, Svelte 5 y TypeScript. No requiere Java.

## Requisitos

Instala [Nix](https://nixos.org/download/) y las [dependencias del sistema de Tauri 2](https://v2.tauri.app/start/prerequisites/). El archivo [`shell.nix`](shell.nix) declara Rust, Node.js, pnpm y GNU Make para el proyecto.

## Cargar el entorno Nix

Desde la raíz del repositorio, ejecuta:

```sh
nix-shell
```

Nix detecta `shell.nix` y abre un shell con las herramientas necesarias. Mantén esa terminal abierta mientras trabajas. Para ejecutar una única tarea sin entrar al shell interactivo:

```sh
nix-shell --run 'make check'
```

## Inicio rápido

```sh
nix-shell
make install
make dev
```

`make install` instala las dependencias bloqueadas en `pnpm-lock.yaml` y `make dev` abre la aplicación Tauri.

## Comandos disponibles

Ejecuta todas las tareas mediante `make` desde la raíz del repositorio.

| Comando                 | Acción                                                                   |
| ----------------------- | ------------------------------------------------------------------------ |
| `make` / `make help`    | Mostrar los comandos disponibles.                                        |
| `make install`          | Instalar las dependencias del frontend respetando el lockfile.           |
| `make dev` / `make run` | Abrir la aplicación Tauri en desarrollo.                                 |
| `make web`              | Servir el frontend en el navegador.                                      |
| `make build`            | Compilar y empaquetar la aplicación Tauri.                               |
| `make compile`          | Comprobar tipos y compilar el frontend.                                  |
| `make test`             | Ejecutar todas las pruebas del compilador y del frontend.                |
| `make test-core`        | Probar el compilador y comparar la VM con el intérprete de referencia.   |
| `make test-ui`          | Ejecutar las pruebas unitarias del frontend.                             |
| `make benchmark`        | Comparar el rendimiento de la VM y el intérprete de referencia.          |
| `make lint`             | Ejecutar el análisis estático y la comprobación de tipos.                |
| `make format`           | Aplicar el formato del proyecto.                                         |
| `make format-check`     | Comprobar el formato sin modificar archivos.                             |
| `make check`            | Ejecutar formato, análisis estático, pruebas y compilación del frontend. |

Antes de publicar cambios, ejecuta:

```sh
make check
```

Los ejemplos de sintaxis están en [`docs/language/examples`](docs/language/examples). Los límites y el modelo de seguridad están en [`docs/security.md`](docs/security.md).

## Ejemplos e inspección

El catálogo de la app carga diez programas con salida esperada: algoritmos, cuentas, inventario, composición y traits. Los archivos viven en `docs/language/examples`; la carpeta `invoice` demuestra ejecución entre módulos.

**Analizar** muestra AST y bytecode; **Ejecutar** usa la VM de pila. **Ampliar AST** abre el grafo a pantalla completa, con controles adaptados al tema oscuro. **Analizar proyecto** y **Ejecutar proyecto** son acciones distintas.

Detalles: [bytecode y API](docs/language/bytecode.md), [benchmark](docs/language/performance.md) y [verificación de la interfaz](apps/desktop/tests/README.md).
