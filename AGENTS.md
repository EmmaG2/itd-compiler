# Guía del proyecto

ITD Compiler es un compilador en Rust con interfaz de escritorio Tauri 2, Svelte 5 y TypeScript. El editor usa CodeMirror 6; la visualización del AST usa Svelte Flow. La implementación activa no requiere Java.

## Estructura

```text
.
├── AGENTS.md                  Guía para trabajar en el repositorio
├── Makefile                   Entrada única para tareas del proyecto
├── shell.nix                  Entorno de desarrollo, incluido GNU Make
├── README.md                  Inicio y documentación principal
├── CHANGELOG.md               Historial de cambios
├── Cargo.toml / Cargo.lock    Workspace Rust y dependencias fijadas
├── rust-toolchain.toml        Toolchain Rust
├── rustfmt.toml               Configuración de formato Rust
├── package.json               Scripts compartidos del frontend
├── pnpm-workspace.yaml        Workspace del frontend
├── pnpm-lock.yaml             Dependencias del frontend fijadas
├── .prettierrc                Configuración de formato del frontend
├── .prettierignore            Exclusiones de formato
├── .gitignore / .gitattributes
├── .github/workflows/ci.yml   Verificación e integración continua
├── crates/compiler-core/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs             API pública de análisis, compilación y ejecución
│   │   ├── source.rs          Fuentes, spans y ubicaciones
│   │   ├── diagnostic.rs      Diagnósticos y etiquetas
│   │   ├── lexer.rs           Tokens y análisis léxico
│   │   ├── ast.rs             Modelo del árbol de sintaxis
│   │   ├── parser.rs          Análisis sintáctico
│   │   ├── semantic.rs        Símbolos, scopes, tipos y coerciones
│   │   ├── bytecode.rs        Instrucciones y generación de bytecode
│   │   ├── bytecode_validate.rs Validación del bytecode
│   │   ├── vm.rs              Máquina virtual
│   │   ├── runtime.rs         Valores, operaciones, funciones nativas y límites
│   │   ├── reference.rs       Intérprete de referencia opcional para comparación
│   │   ├── project.rs         Carga y ejecución de proyectos con módulos
│   │   └── linker.rs          Enlace de módulos
│   ├── tests/                Lexer, parser, semántica, VM, migración y ejemplos
│   └── examples/benchmark.rs Comparación de rendimiento VM/intérprete
├── apps/desktop/
│   ├── package.json
│   ├── index.html
│   ├── vite.config.ts        Configuración de desarrollo y compilación
│   ├── tsconfig.json         Comprobación de tipos
│   ├── components.json       Configuración de componentes de interfaz
│   ├── src/
│   │   ├── main.ts           Entrada del frontend
│   │   ├── App.svelte        Integración y estado de la aplicación
│   │   ├── styles.css        Estilos globales
│   │   ├── vite-env.d.ts     Declaraciones del entorno
│   │   ├── locales/es.json   Textos de la interfaz en español
│   │   ├── features/
│   │   │   ├── editor/       Editor, resaltado Catppuccin Mocha y adaptadores
│   │   │   ├── ast/          Grafo, nodos y distribución del AST
│   │   │   ├── bytecode/     Vista de instrucciones
│   │   │   └── examples/     Catálogo compartido de ejemplos
│   │   └── lib/
│   │       ├── ipc/          Cliente de comunicación con Tauri y sus pruebas
│   │       ├── types/        Contratos TypeScript del compilador
│   │       ├── errors.ts     ErrorWithCode
│   │       ├── utils.ts      Utilidades compartidas
│   │       └── components/ui/ Componentes reutilizables: botones, tarjetas,
│   │                         tablas, pestañas, inputs, etiquetas y scroll
│   ├── src-tauri/
│   │   ├── Cargo.toml
│   │   ├── build.rs
│   │   ├── tauri.conf.json   Configuración de la aplicación nativa
│   │   ├── capabilities/    Permisos de Tauri
│   │   ├── icons/           Iconos de la aplicación
│   │   └── src/
│   │       ├── main.rs      Entrada nativa
│   │       ├── lib.rs       Comandos IPC, DTO y coordinación del compilador
│   │       └── inspection.rs Datos para inspeccionar AST y bytecode
│   └── tests/
│       ├── README.md         Alcance de la verificación visual
│       └── browser-smoke.mjs Prueba de interfaz con IPC simulado
└── docs/
    ├── security.md           Límites y modelo de seguridad
    ├── language/
    │   ├── README.md         Índice del lenguaje
    │   ├── grammar.ebnf      Gramática
    │   ├── lexical-rules.md  Reglas léxicas
    │   ├── type-system.md    Tipos y conversiones
    │   ├── object-model.md   Clases y traits
    │   ├── modules.md        Módulos y exportaciones
    │   ├── bytecode.md       Bytecode y API
    │   ├── performance.md    Mediciones de rendimiento
    │   └── examples/         Programas .itd, salidas .expected.txt y catalog.json
    │       └── invoice/      Ejemplo de facturación entre módulos
    └── migration/            Plan e historial de migración, etapas 00 a 11
```

Las pruebas unitarias del frontend también viven junto al código como `*.test.ts`. El catálogo importa directamente los programas y salidas de `docs/language/examples/`; no mantengas copias dentro de la interfaz.

Las carpetas raíz `MIcompiladordejava/`, `src/` y `tests/` contienen material Java heredado y están ignoradas. No son la implementación activa. `target/`, `build/`, `dist/`, `node_modules/`, `.pnpm-store/` y `apps/desktop/src-tauri/gen/` son artefactos o dependencias generadas. `.playwright-mcp/` contiene artefactos locales de navegador.

## Flujo y responsabilidades

- Compilación: fuente → lexer → parser/AST → análisis semántico → bytecode → validación → VM.
- El núcleo del lenguaje vive en `crates/compiler-core/`; conserva su independencia de la interfaz.
- Tauri expone el núcleo mediante comandos y DTO; el frontend los consume desde `src/lib/ipc/` y `src/lib/types/`.
- Al cambiar el lenguaje, revisa semántica y ejecución, añade una prueba pertinente y actualiza sus reglas y ejemplos.
- Al cambiar un contrato IPC, mantén sincronizados los DTO nativos y los tipos TypeScript.

## Comandos: solo Make

Ejecuta las tareas desde la raíz y con el entorno definido en `shell.nix` activo. Usa exclusivamente los objetivos del `Makefile`; no invoques directamente las herramientas subyacentes. Si falta una tarea necesaria, añade un objetivo y documenta aquí su comando.

| Comando                 | Acción                                                                                        |
| ----------------------- | --------------------------------------------------------------------------------------------- |
| `make` / `make help`    | Mostrar los comandos disponibles.                                                             |
| `make install`          | Instalar las dependencias del frontend respetando el lockfile.                                |
| `make dev` / `make run` | Abrir la aplicación Tauri en desarrollo.                                                      |
| `make web`              | Servir el frontend con Vite para acceder desde el navegador.                                  |
| `make build`            | Compilar y empaquetar la aplicación Tauri.                                                    |
| `make macos`            | Generar el instalador DMG para macOS.                                                         |
| `make windows`          | Generar el instalador NSIS (`.exe`) para Windows x64; se ejecuta en Windows.                  |
| `make compile`          | Comprobar tipos y compilar el frontend.                                                       |
| `make test`             | Ejecutar pruebas de todo el workspace Rust con todas las funciones opcionales y del frontend. |
| `make test-core`        | Probar el compilador, incluida la comparación con el intérprete de referencia.                |
| `make test-ui`          | Ejecutar las pruebas unitarias del frontend.                                                  |
| `make benchmark`        | Comparar el rendimiento de la VM y el intérprete de referencia.                               |
| `make lint`             | Ejecutar el análisis estático de Rust y comprobar los tipos del frontend.                     |
| `make format`           | Aplicar formato a Rust y a los archivos cubiertos por el formateador del frontend.            |
| `make format-check`     | Comprobar el formato sin modificar archivos.                                                  |
| `make check`            | Ejecutar formato, análisis estático, pruebas y compilación del frontend.                      |

`make check` no empaqueta la aplicación ni ejecuta la prueba de navegador. `make test-ui` tampoco sustituye una comprobación de la integración nativa. Actualmente no hay un objetivo para la prueba de navegador.

## Reglas de trabajo

- Mantén cambios pequeños y centrados; conserva los cambios existentes del usuario.
- Usa `import type` para importaciones de tipos TypeScript; nunca uses `as any`.
- Prefiere retornos tempranos y reutiliza utilidades y componentes existentes.
- En utilidades y servicios TypeScript, usa `ErrorWithCode`; en el núcleo Rust, conserva el sistema de diagnósticos existente.
- Añade textos de interfaz a `apps/desktop/src/locales/es.json`.
- Escribe comentarios que expliquen el motivo, no que repitan el código.
- No expongas secretos ni debilites permisos, validación de bytecode o límites de ejecución.
- Ejecuta las pruebas pertinentes mediante Make. Antes de publicar cambios, ejecuta `make check` y comunica cualquier fallo o verificación pendiente.
- Usa commits convencionales (`feat:`, `fix:`, `refactor:`) y PR en borrador por defecto. Mantén cada PR por debajo de 1000 líneas cambiadas y de 10 archivos de código; divide trabajos mayores por responsabilidad.
- Consulta Context7 para documentación actual de bibliotecas y herramientas cuando la tarea dependa de sus APIs o configuración.
