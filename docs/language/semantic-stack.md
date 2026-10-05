# Pila de atributos semánticos

El análisis semántico utiliza una pila real para comprobar expresiones del AST.
Cada atributo contiene el identificador de una expresión (`ExprId`), su tipo
(`Type`) y su ubicación (`Span`). Las reglas consumen estos atributos y apilan
el resultado; la traza registra esas operaciones, no una animación independiente.

## Demostración en la aplicación

1. Abra la aplicación con `nix-shell --run 'make dev'`.
2. Escriba `2 + 3 * 4;`, pulse **Analizar** y abra **Pila semántica**.
3. Use **Paso siguiente**, **Paso anterior**, **Volver al inicio** e **Ir al final**.
4. Observe la altura, la regla, los atributos consumidos y el resultado. La cima
   aparece primero; **Ver código** navega a la expresión correspondiente.

La traza empieza vacía y, cuando está completa, termina vacía. Editar o cargar
otra fuente invalida la inspección anterior. Las acciones de proyecto no muestran
una traza conjunta de módulos. Los árboles sintáctico y semántico se conservan.

## Tres pilas distintas

| Estructura                   | Contenido y responsabilidad                                |
| ---------------------------- | ---------------------------------------------------------- |
| Pila de ámbitos              | Nombres y símbolos visibles dentro de funciones y bloques. |
| Pila de atributos semánticos | Tipos e identidad de expresiones durante su comprobación.  |
| Pila de operandos de la VM   | Valores usados al ejecutar el bytecode.                    |

La pila semántica no calcula el valor `14` del ejemplo: comprueba que los tipos
permiten multiplicar y sumar. La VM calcula los valores durante la ejecución.

**Alcance académico:** las reducciones son ascendentes sobre un AST ya construido.
No son acciones durante reducciones de un parser LR; el parser existente usa
precedencia y llamadas recursivas. Esta implementación no garantiza una rúbrica
que exija específicamente acciones semánticas integradas en un parser LR.

## Recorrido de `2 + 3 * 4`

La tabla muestra atributos por expresión, no valores de ejecución. La cima está
a la derecha para facilitar la lectura del recorrido.

| Evento                                          | Pila después del evento          |
| ----------------------------------------------- | -------------------------------- |
| `Push`: literal `2`                             | `[int de 2]`                     |
| `Push`: literal `3`                             | `[int de 2, int de 3]`           |
| `Push`: literal `4`                             | `[int de 2, int de 3, int de 4]` |
| `Consume`: operando derecho de multiplicación   | `[int de 2, int de 3]`           |
| `Consume`: operando izquierdo de multiplicación | `[int de 2]`                     |
| `Reduce`: regla `multiply`                      | `[int de 2, int de 3 * 4]`       |
| `Consume`: operando derecho de suma             | `[int de 2]`                     |
| `Consume`: operando izquierdo de suma           | `[]`                             |
| `Reduce`: regla `add`                           | `[int de 2 + 3 * 4]`             |
| `Consume`: resultado de la sentencia            | `[]`                             |

`Consume` desapila realmente un atributo. `Reduce` apila el resultado después de
esas extracciones; reproducir la reducción no vuelve a desapilar sus operandos.
`Conversion` observa una conversión implícita registrada, sin modificar la pila.
`Recover` descarta únicamente el segmento del checkpoint afectado por un fallo
interno de invariantes.

## Conversiones, llamadas y errores

Estos ejemplos también se pueden analizar desde el editor:

```itd
let answer: long := 2i8 + 3i16 * 4i16;
let converted := 3 as long;
fn subtract(a: int, b: int) -> int { return a - b; }
subtract(20, 2 + 3 * 4);
```

La primera declaración registra conversiones implícitas por tipos enteros; la
segunda utiliza la regla explícita `cast`. Una llamada visita el callee y sus
argumentos en orden, consume sus atributos y comprueba firma, aridad y tipos.
Las asignaciones conservan el análisis del lado derecho primero; resolver su
destino no equivale a leer una variable antes de inicializarla.

```itd
missing + true;
1[false];
let good := 3;
```

Los diagnósticos incluyen el nombre inexistente y la indexación no disponible
(`E2015`). El atributo `error` permite continuar sin contaminar `good`. Un error
normal del programa no implica necesariamente un evento `Recover`: este último
corresponde a la recuperación de invariantes internas, identificadas con `E2099`.
Los errores léxicos o sintácticos impiden iniciar el análisis semántico.

## API y límites de inspección

- `compiler_core::analyze(source)` conserva su contrato sin capturar la traza.
- `compiler_core::analyze_with_trace(source, event_limit)` devuelve
  `(AnalysisResult, Option<SemanticTrace>)`: `None` si no se inició la semántica;
  una fuente vacía válida produce `Some` con una lista vacía.
- `semantic::analyze_with_imports_and_trace(program, imports, event_limit)` ofrece
  la misma comprobación con símbolos importados. No amplía `ProjectDto`.
- Cada evento incluye ubicación, regla, altura anterior/posterior, cantidad
  consumida y, cuando existe, el atributo producido o tipo de conversión.

El núcleo limita la captura a **4096 eventos**, o al límite inferior solicitado,
antes de construir sus payloads. Las etiquetas contienen como máximo **80
caracteres Unicode**; las funciones usan una descripción resumida, no la expansión
recursiva de su firma. La proyección IPC mantiene estos límites.

`truncated: true` indica que se omitieron eventos. El prefijo conservado se puede
reproducir, pero puede terminar con atributos pendientes. **El análisis continúa**:
truncar la captura no cambia los tipos, diagnósticos ni bytecode. Estos límites son
de inspección, no sustituyen los [límites de ejecución](../security.md).

## Verificación

Desde la raíz: `nix-shell --run 'make test-core'` comprueba equivalencia entre
análisis normal y trazado, bytecode, orden, recuperación y límites. `make test`
incluye pruebas de los DTO nativos y del frontend; `make check` reúne las
verificaciones automatizadas del proyecto. Ejecute también estas tareas dentro
del entorno Nix.

La demostración visual en Tauri debe comprobarse por separado: las pruebas de
reproducción y los mocks IPC no prueban por sí solos el recorrido completo nativo.
Consulte el [alcance de la verificación de interfaz](../../apps/desktop/tests/README.md).
Los programas del [catálogo compartido](examples/README.md), como expresiones y
factorial, permiten explorar casos mayores sin duplicar fuentes en la interfaz.
