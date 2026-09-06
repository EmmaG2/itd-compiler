# Bytecode y VM

El motor predeterminado ejecuta instrucciones escalares en una pila; no reconstruye ni interpreta el AST. El formato anterior `Execute(index)` / `Halt` era transitorio y ya no se utiliza. El bytecode es interno, no tiene formato de archivo estable y debe recompilarse tras actualizar el compilador.

## Representación

`Chunk` contiene constantes tipadas, funciones, clases, número de globals y nombres de métodos. Cada función conserva instrucciones, spans originales, aridad, cantidad de locales y metadata de receptor/constructor. Las variables y campos usan índices; las funciones usan identificadores, y los métodos de instancia se enlazan mediante la clase real del objeto, incluidos receptores tipados como trait.

| Familia            | Instrucciones                                                                    |
| ------------------ | -------------------------------------------------------------------------------- |
| Valores            | `Constant`, `LoadLocal`, `StoreLocal`, `LoadGlobal`, `StoreGlobal`, `Pop`, `Dup` |
| Operaciones        | `Unary`, `Binary`, `Cast`                                                        |
| Flujo              | `Jump`, `JumpIfFalse`, `JumpIfTrue`, `Return`, `Halt`                            |
| Llamadas y objetos | `Call`, `BindMethod`, `GetField`, `SetField`, `GetStatic`, `SetStatic`           |
| Errores diferidos  | `Trap`                                                                           |

`Call` consume el invocable y sus argumentos. Un constructor crea el objeto y ejecuta sus inicializadores y cuerpo; devuelve la instancia. Los marcos de llamada mantienen locales y base de pila independientes. La inicialización de módulos ocurre una vez, antes de sus dependientes, en el orden de imports declarado.

`and` y `or` usan saltos: cada operando se evalúa como máximo una vez. La aritmética y los casts reutilizan operaciones checked; los decimales nunca pasan por `f64`. Un literal fuera de rango genera `Trap` en su posición: una rama que no se ejecuta no falla, y se conserva cualquier salida anterior al error.

## Validación y límites

Antes de ejecutar se verifican referencias, índices, aridad de metadata, spans, saltos y alturas de pila en los puntos de unión. Las lecturas no inicializadas, receptores inválidos y aridades incorrectas también se comprueban durante la ejecución. Cada función admite como máximo 65,536 posiciones locales; las cantidades declaradas de globals y campos están acotadas por el tamaño del programa compilado.

`RuntimeLimits` conserva cancelación, presupuesto de instrucciones, profundidad de llamadas, objetos, salida y tiempo. El presupuesto ahora cuenta instrucciones de bytecode, no visitas al AST: los valores usados por el intérprete de referencia no son directamente comparables. No se exige que ambos motores fallen exactamente en el mismo punto al agotar ese presupuesto.

## APIs

- `analyze(source)`: tokens, AST, símbolos y diagnósticos.
- `compile_source(source)` / `compile_analysis(analysis)`: fuentes, diagnósticos y `Option<Chunk>`.
- `run_compiled(compilation, limits, cancelled)`: ejecuta un resultado válido sin repetir el análisis.
- `run(source)` y `run_with_options(...)`: conservan sus contratos de salida.
- `Project::compile(entry)` y `Project::run(entry, limits, cancelled)`: enlazan y ejecutan módulos.

El intérprete está detrás de la feature `reference-interpreter`, desactivada por defecto. Las pruebas con `--all-features` comparan ambos motores. No se ofrece selector de motor al usuario.

## Inspección

Las acciones Analizar y Ejecutar muestran instrucciones en la pestaña Bytecode. Cada fila incluye función, posición, instrucción y ubicación fuente; la respuesta se limita a 20,000 filas y 256 caracteres por instrucción. Seleccionar una fila de un documento navega al editor. En proyectos se muestra archivo, línea y columna, sin aplicar offsets de otro archivo al documento abierto.
