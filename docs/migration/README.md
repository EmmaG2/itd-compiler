# Plan de migración de Java a Rust y Tauri

Este directorio divide la migración en features implementables y verificables. El objetivo no es traducir cada clase Java literalmente, sino conservar el comportamiento útil y construir un compilador con núcleo Rust independiente de la interfaz.

## Arquitectura objetivo

```text
Tauri UI (TypeScript)
        │ invoke
        ▼
Adaptador Tauri (Rust)
        │
        ▼
compiler-core
SourceMap → Lexer → Parser → Resolver → Type checker → Bytecode → VM
```

## Orden de implementación

| Orden | Feature                                                                       | Depende de                       | Resultado                                              |
| ----: | ----------------------------------------------------------------------------- | -------------------------------- | ------------------------------------------------------ |
|    00 | [Definición del lenguaje](./00-definicion-del-lenguaje.md)                    | —                                | Gramática y semántica aprobadas                        |
|    01 | [Workspace Rust y paridad Java](./01-workspace-rust-y-paridad.md)             | 00                               | Núcleo Rust compilable y casos base congelados         |
|    02 | [Lexer, spans y diagnósticos](./02-lexer-spans-y-diagnosticos.md)             | 01                               | Tokenización robusta con ubicaciones precisas          |
|    03 | [AST y parser](./03-ast-y-parser.md)                                          | 02                               | Programa convertido en AST con recuperación de errores |
|    04 | [Resolución, scopes y tipos](./04-resolucion-scopes-y-tipos.md)               | 03                               | Análisis semántico estático                            |
|    05 | [Decimales exactos](./05-decimales-exactos.md)                                | 04                               | Aritmética decimal sin pasar por `f64`                 |
|    06 | [Control de flujo y funciones](./06-control-de-flujo-y-funciones.md)          | 04, 05                           | Programas estructurados y funciones ejecutables        |
|    07 | [Sistema de módulos](./07-sistema-de-modulos.md)                              | 04, 06                           | Imports seguros entre archivos                         |
|    08 | [Programación orientada a objetos](./08-programacion-orientada-a-objetos.md)  | 04, 06                           | Clases, objetos, métodos y traits                      |
|    09 | [Bytecode y máquina virtual](./09-bytecode-y-maquina-virtual.md)              | 05–08                            | Compilador y runtime propios                           |
|    10 | [Aplicación Tauri](./10-aplicacion-tauri.md)                                  | 02, 03; integración final con 09 | Editor visual, diagnósticos y ejecución                |
|    11 | [Seguridad, calidad y distribución](./11-seguridad-calidad-y-distribucion.md) | 09, 10                           | Release reproducible y endurecido                      |

## Estrategia de entrega

- Implementar las features en orden, salvo el esqueleto visual de Tauri, que puede comenzar después del parser.
- Mantener cada PR por debajo de 1000 líneas de código y 10 archivos de código.
- Separar refactors de cambios funcionales.
- No eliminar todavía el código Java: se conserva como referencia hasta lograr paridad.
- No mezclar lógica del compilador con comandos Tauri.
- Cada feature termina con pruebas ejecutables y criterios de aceptación cumplidos.

## Entorno de desarrollo con Nix

El archivo `shell.nix` instala Rust, Cargo, Clippy y rustfmt sin requerirlos en el sistema:

```sh
nix-shell
cargo test
```

Para ejecutar todas las verificaciones sin abrir una shell interactiva:

```sh
nix-shell --run "cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test"
```

## Definición global de terminado

- `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test` pasan.
- La comprobación TypeScript y el build de Tauri pasan.
- Los diagnósticos contienen código, severidad, mensaje, archivo y rango.
- No se usa `f64` como intermediario para valores `decimal`.
- Imports no pueden salir del directorio raíz del proyecto.
- La ejecución tiene cancelación y límites de recursos.
- Se documenta cualquier cambio incompatible de sintaxis o semántica.

## Fuera del MVP

Compilación nativa con LLVM/Cranelift, gestor de paquetes remoto, herencia múltiple, async/await, lambdas, depurador e incremental compilation. Se añadirán cuando exista un requisito concreto.
