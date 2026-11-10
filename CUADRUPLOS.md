# Cuádruplos del lenguaje ITD

## Propósito y estado

ITD implementa una representación intermedia de tres direcciones, tipada y validada, para el backend Darwin ARM64. El flujo nativo es `fuente → análisis semántico para cuádruplos → cuádruplos → optimización → ensamblador`; no pasa por bytecode ni ejecuta la VM. La API pública correspondiente es `compile_quadruples(source)`, `emit_arm64(program)` y `compile_arm64(source)`. Véase la [guía del backend Darwin ARM64](docs/language/arm64.md) para su alcance y uso.

Las tablas didácticas de más abajo conservan la propuesta de 31 operaciones y el contrato general del lenguaje. **El inventario implementado está en la sección siguiente**: no todas las operaciones didácticas existen como variantes de la IR ni todas las variantes nativas tienen equivalente en la propuesta. Se prefieren cuádruplos a triplos porque el resultado tiene un nombre explícito (`t1`, `t2`), independiente de la posición de la instrucción. Los triplos ahorran ese campo, pero referencian resultados por posición y complican la reorganización de instrucciones. No se mantiene una segunda representación equivalente.

## Inventario actual de la IR nativa

Estas son **todas las 30 variantes** de `Operation` definidas en [`quadruple.rs`](crates/compiler-core/src/quadruple.rs). Las formas indican los campos reales de cada variante; `t` es un temporal tipado, `v` un `Operand`, `L` una etiqueta, `local` un slot resuelto y `call` un identificador de sitio de llamada. Cada `Quadruple` también lleva un `Span` fuente. El optimizador puede sustituir o quitar instrucciones, por lo que un programa concreto no necesariamente contiene todas las variantes.

- **Aritmética binaria (6):** `Add { left: v, right: v, result: t }`, `Sub { … }`, `Mul { … }`, `Div { … }`, `Mod { … }`, `Pow { … }`. Suma, resta, producto, división, resto y potencia.
- **Unarias (2):** `Neg { operand: v, result: t }`, `Not { operand: v, result: t }`. Negación numérica y lógica.
- **Comparaciones (5):** `Eq { left: v, right: v, result: t }`, `Ne { … }`, `Lt { … }`, `Le { … }`, `Gt { … }`. Producen un temporal `bool`.
- **Valores y locales (4):** `Copy { source: v, result: t }`, `LoadLocal { local, result: t }`, `StoreLocal { local, value: v }`, `I32ToF64 { source: v, result: t }`. Copia tipada, lectura/escritura de slot y conversión de entero a decimal.
- **Control de flujo (3):** `Label(L)`, `Jump(L)`, `JumpIfFalse { condition: v, target: L }`. Destino y saltos condicional o incondicional.
- **Llamadas (3):** `Param { call, index, value: v }`, `Call { call, target, result: Option<t> }`, `Return(Option<v>)`. Argumento por posición, invocación y retorno opcional.
- **Miembros (5):** `GetField { owner: v, field, result: t }`, `SetField { owner: v, field, value: v }`, `GetStatic { static_, result: t }`, `SetStatic { static_, value: v }`, `BindMethod { receiver: v, method, result: t }`. Campos y métodos enlazados.
- **Terminación (2):** `Halt`, `Trap(Diagnostic)`. Fin de la función principal y error diferido con diagnóstico.

Los operandos reales son `I32`, `F64`, `Bool`, `String`, `Receiver`, `Parameter` y `Temporary`. Los tipos de valor de esta IR son `I32`, `F64`, `Bool`, `Str`, `Object`, `BoundMethod` y `Void`. `Call.target` distingue `Function`, `Constructor`, `Runtime` y `BoundMethod`; las firmas, clases, campos, estáticos y cadenas residen en la metadata de `QuadrupleProgram`, no son cuádruplos adicionales.

**Diferencias con las tablas didácticas:** no existen variantes `Pos`, `Ge`, `JumpIfTrue` ni `Cast` genérico; el lowering representa esas formas con otras instrucciones o, si exceden el subconjunto nativo, las rechaza. En cambio, `LoadLocal`, `StoreLocal` e `I32ToF64` son variantes reales que las tablas generales no enumeran por separado. La presencia de `Mod` y `Pow` en la IR tampoco promete emisión ARM64 para sus versiones decimales: el emisor las rechaza. Véase [alcance ARM64](docs/language/arm64.md).

## Convenciones

Cada instrucción tiene la forma `(operación, argumento1, argumento2, resultado)`. `—` significa campo no utilizado, no un valor del lenguaje. Los temporales son locales a una función; las etiquetas identifican destinos dentro de esa misma función. El campo resultado también puede contener el destino de un salto o de una escritura, según la operación.

Los operandos distinguen constantes tipadas, temporales, símbolos resueltos, campos, clases, funciones y etiquetas. Los nombres legibles de las tablas representan identificadores internos: dos variables de scopes distintos no se confunden por tener el mismo nombre. Cada instrucción conserva su ubicación fuente (`Span`); los temporales conservan su tipo. Esa información no añade columnas al cuádruplo.

En estas tablas, `N` describe el sistema numérico general del lenguaje. No debe leerse como el tipo del backend: la IR Darwin ARM64 solo representa `int` como `i32` con signo y `decimal` como `f64` IEEE-754, además de `bool`, `str`, objetos y métodos enlazados. El backend no convierte silenciosamente `byte`, `short`, `long`, `float` ni `double`; los rechaza.

Los ejemplos de expresiones terminan en `;` porque son sentencias ITD válidas. Sus cuádruplos omiten la finalización del programa y muestran solo la operación relevante; las lecturas, conversiones y llamadas necesarias se detallan cuando afectan el significado. Los ejemplos de cada sección son independientes, salvo las referencias al contexto de objetos de la sección 25.

## Aritmética

### 1. ADD — suma

Ejemplo ITD: `2 + 3;`. Suma los operandos; ambos pertenecen a `N` y el resultado tiene el tipo numérico común.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| ADD       | 2           | 3           | t1: int   |

No admite concatenación de `str`: aunque existe soporte interno en runtime, el analizador semántico actual rechaza esa expresión.

### 2. SUB — resta

Ejemplo ITD: `9 - 4;`. Operandos numéricos normalizados al mismo tipo; devuelve ese tipo.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| SUB       | 9           | 4           | t1: int   |

### 3. MUL — multiplicación

Ejemplo ITD: `3 * 4;`. Acepta `N × N` tras las coerciones autorizadas; devuelve el tipo común.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| MUL       | 3           | 4           | t1: int   |

### 4. DIV — división

Ejemplo ITD: `7 / 2;`. Acepta operandos numéricos compatibles; devuelve su tipo común. La división entera trunca hacia cero: el ejemplo produce `3`. Dividir entre cero falla.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| DIV       | 7           | 2           | t1: int   |

### 5. MOD — resto

Ejemplo ITD: `7 % 3;`. Acepta operandos numéricos compatibles, incluidos decimales y flotantes; devuelve su tipo común. El divisor no puede ser cero. No se debe sustituir por un módulo matemático siempre positivo.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| MOD       | 7           | 3           | t1: int   |

### 6. POW — potencia

Ejemplo ITD: `2 ** 3;`. Admite tipos numéricos iguales o enteros ensanchados; entero/decimal mixtos requieren `as`. Devuelve el tipo común.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| POW       | 2           | 3           | t1: int   |

Con base entera, el exponente debe ser no negativo y caber en `u32`; con base decimal, debe tener valor entero representable en `i32`, y puede ser negativo. Con flotantes se admite exponente fraccionario, pero no resultados no finitos. Se conservan las comprobaciones de rango.

## Operadores unarios

### 7. POS — identidad numérica

Ejemplo ITD: `+5;`. Acepta un valor de `N` y conserva su tipo y valor.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| POS       | 5           | —           | t1: int   |

### 8. NEG — negación numérica

Ejemplo ITD: `-5;`. Acepta `N` y devuelve el mismo tipo. La negación debe comprobar rango, especialmente para el mínimo entero representable.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| NEG       | 5           | —           | t1: int   |

### 9. NOT — negación lógica

Ejemplo ITD: `not true;` (también `!true;`). Acepta únicamente `bool` y devuelve `bool`.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| NOT       | true        | —           | t1: bool  |

## Comparaciones

### 10. EQ — igualdad

Ejemplo ITD: `4 == 4;`. Devuelve `bool`. Los valores escalares deben tener el mismo tipo (`N`, `bool`, `str` o `char`); objetos compatibles de clase/trait se comparan por identidad, no por contenido.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| EQ        | 4           | 4           | t1: bool  |

No se define aquí igualdad ejecutable de funciones, clases como valores invocables o `void`: el runtime no implementa esas comparaciones, aunque la regla semántica de tipos iguales es más amplia.

### 11. NE — desigualdad

Ejemplo ITD: `4 != 5;`. Tiene las mismas restricciones que `EQ` y devuelve `bool`.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| NE        | 4           | 5           | t1: bool  |

### 12. LT — menor que

Ejemplo ITD: `2 < 3;`. Admite tipos numéricos compatibles y devuelve `bool`; no ordena cadenas, caracteres ni booleanos.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| LT        | 2           | 3           | t1: bool  |

### 13. LE — menor o igual

Ejemplo ITD: `2 <= 3;`. Tipos permitidos: numéricos compatibles; resultado `bool`.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| LE        | 2           | 3           | t1: bool  |

### 14. GT — mayor que

Ejemplo ITD: `3 > 2;`. Tipos permitidos: numéricos compatibles; resultado `bool`.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| GT        | 3           | 2           | t1: bool  |

### 15. GE — mayor o igual

Ejemplo ITD: `3 >= 2;`. Tipos permitidos: numéricos compatibles; resultado `bool`.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| GE        | 3           | 2           | t1: bool  |

## Transferencia y conversión

### 16. COPY — transferencia de valor

Ejemplo ITD: `let mut x: int := 4; x = 7;`. Inicializa o actualiza un símbolo; también captura valores en temporales. Admite valores almacenables del lenguaje con tipo compatible, después de cualquier conversión requerida. Copiar un objeto copia su referencia, no duplica su contenido.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| COPY      | 4           | —           | x: int    |
| COPY      | 7           | —           | x: int    |

No permite escribir una variable inmutable ya inicializada. Una asignación es expresión: su valor es el valor almacenado, que debe conservarse para un uso posterior.

### 17. CAST — conversión comprobada

Ejemplo ITD: `3 as decimal;`. El argumento 2 identifica el tipo destino; el resultado tiene ese tipo. Se utiliza para conversiones numéricas explícitas y coerciones numéricas autorizadas por semántica, no para inventar conversiones entre cadenas, booleanos u objetos.

| Operación | Argumento 1 | Argumento 2 | Resultado   |
| --------- | ----------- | ----------- | ----------- |
| CAST      | 3           | decimal     | t1: decimal |

Las conversiones fuera de rango o con pérdida de precisión fallan; convertir decimal a entero no equivale a truncarlo. La compatibilidad clase/trait conserva la referencia y no exige una conversión numérica.

## Flujo de control

### 18. LABEL — destino simbólico

Ejemplo ITD: `while false { break; }`. El generador crea una etiqueta para la prueba del ciclo; `LABEL` no es una palabra del lenguaje. No recibe valores tipados: resultado es un identificador de etiqueta.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| LABEL     | —           | —           | L_test    |

### 19. JUMP — salto incondicional

Ejemplo ITD: `while true { break; }`. El `break` se traduce al salto mostrado, cuyo destino es una etiqueta de salida existente. No produce valor; `continue` salta a la prueba del ciclo correspondiente.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| JUMP      | —           | —           | L_exit    |

### 20. JUMP_IF_FALSE — salto si falso

Ejemplo ITD: `if false { print("No"); }`. El argumento 1 debe ser `bool`; resultado es una etiqueta. Si la condición es verdadera, continúa en la instrucción siguiente.

| Operación     | Argumento 1 | Argumento 2 | Resultado |
| ------------- | ----------- | ----------- | --------- |
| JUMP_IF_FALSE | false       | —           | L_end     |

### 21. JUMP_IF_TRUE — salto si verdadero

Ejemplo ITD: `true or false;`. Permite omitir el segundo operando. Acepta `bool` y una etiqueta de destino; no produce valor.

| Operación    | Argumento 1 | Argumento 2 | Resultado |
| ------------ | ----------- | ----------- | --------- |
| JUMP_IF_TRUE | true        | —           | L_end     |

## Funciones y llamadas

Cada función tiene su propia lista de cuádruplos y metadata de parámetros, retorno y receptor. No se añaden operaciones `FUNC` o `END_FUNC`. Los parámetros formales se asignan al entrar a la función según esa metadata.

### 22. PARAM — argumento de llamada

Ejemplo ITD: `print("Hola");`. Registra un valor ya evaluado en la posición indicada, empezando por cero. El tipo debe satisfacer el parámetro formal; `print` requiere `str`.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| PARAM     | "Hola"      | 0           | —         |

Se evalúan primero el invocable y todos los argumentos, de izquierda a derecha, conservando sus valores en temporales cuando sea necesario. Después se emite un grupo contiguo de `PARAM` seguido de `CALL`; así una llamada anidada no consume argumentos de la llamada exterior.

### 23. CALL — invocación

Ejemplo ITD: `fn twice(x: int) -> int { return x * 2; } twice(3);`. Argumento 1 es el invocable capturado; argumento 2 es la cantidad de argumentos explícitos. El resultado tiene el tipo de retorno, o se omite si no se utiliza.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| PARAM     | 3           | 0           | —         |
| CALL      | twice       | 1           | t1: int   |

Admite funciones, nativas, métodos enlazados y constructores. `CALL Counter, 1, t1` puede construir un `Counter`: la metadata del invocable realiza la reserva, inicialización y ejecución del constructor. No requiere añadir una operación `NEW`. La aridad y los tipos se validan; cada llamada consume solo su grupo de argumentos.

### 24. RETURN — retorno de función

Ejemplo ITD: `fn answer() -> int { return 42; }`. Devuelve un valor compatible con el tipo declarado, después de las coerciones necesarias, y finaliza el marco actual.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| RETURN    | 42          | —           | —         |

`return;` en una función `void` usa argumento 1 vacío. Un constructor retorna su receptor, incluso al terminar implícitamente; no finaliza el programa completo.

## Objetos y miembros

### 25. GET_FIELD — lectura de campo

Contexto ITD compartido para las secciones 25–29:

```text
class Counter {
    pub let mut value: int := 0;
    pub static let mut total: int := 0;
    pub fn this(initial: int) { this.value = initial; }
    pub fn read() -> int { return this.value; }
}
let c := Counter(3);
```

Ejemplo: `c.value;`. Argumento 1 es una referencia de instancia; argumento 2, un campo resuelto. El resultado tiene el tipo declarado del campo, aquí `int`.

| Operación | Argumento 1 | Argumento 2   | Resultado |
| --------- | ----------- | ------------- | --------- |
| GET_FIELD | c           | Counter.value | t1: int   |

### 26. SET_FIELD — escritura de campo

Ejemplo ITD: `c.value = 5;`. Argumento 1 es el valor compatible con el campo; argumento 2 identifica el campo; resultado identifica el receptor que se modifica, no un temporal nuevo. Se conserva el valor escrito si la asignación se usa como expresión.

| Operación | Argumento 1 | Argumento 2   | Resultado |
| --------- | ----------- | ------------- | --------- |
| SET_FIELD | 5           | Counter.value | c         |

Se respetan mutabilidad y visibilidad. Para un receptor calculado, ITD evalúa primero el valor derecho y después el receptor; ambos se evalúan una sola vez.

### 27. GET_STATIC — lectura de campo estático

Ejemplo ITD: `Counter.total;`. Argumento 1 identifica la clase y argumento 2 el campo estático; el resultado tiene el tipo declarado del campo. No necesita instancia.

| Operación  | Argumento 1 | Argumento 2   | Resultado |
| ---------- | ----------- | ------------- | --------- |
| GET_STATIC | Counter     | Counter.total | t1: int   |

### 28. SET_STATIC — escritura de campo estático

Ejemplo ITD: `Counter.total = 1;`. Argumento 1 tiene un tipo compatible con el campo; argumento 2 identifica el campo; resultado identifica la clase propietaria. No modifica un campo por instancia.

| Operación  | Argumento 1 | Argumento 2   | Resultado |
| ---------- | ----------- | ------------- | --------- |
| SET_STATIC | 1           | Counter.total | Counter   |

### 29. BIND_METHOD — enlace de receptor y método

Ejemplo ITD: `c.read();`. Captura el receptor y resuelve el método por la clase real, también cuando el receptor está tipado como trait. Produce un invocable con la firma del método; el receptor queda incorporado y no cuenta como argumento explícito.

| Operación   | Argumento 1 | Argumento 2 | Resultado       |
| ----------- | ----------- | ----------- | --------------- |
| BIND_METHOD | c           | read        | t1: fn() -> int |
| CALL        | t1          | 0           | t2: int         |

Los métodos estáticos se resuelven a funciones y se llaman sin enlace de receptor; `GET_STATIC` corresponde a campos, no a métodos.

## Finalización y errores

### 30. HALT — fin del programa

Ejemplo ITD: `print("Fin");`. Después de la llamada, el generador añade la finalización de la función principal. No acepta operandos ni produce valor. `HALT` no existe como sentencia ITD y no sustituye `RETURN`.

| Operación | Argumento 1 | Argumento 2 | Resultado |
| --------- | ----------- | ----------- | --------- |
| HALT      | —           | —           | —         |

### 31. TRAP — error diferido

Ejemplo ITD: `if false { 999999999999999999999999; }`. Un literal que no puede materializarse produce una instrucción con un diagnóstico, pero solo falla si esa instrucción se ejecuta. La rama del ejemplo no se ejecuta.

| Operación | Argumento 1                            | Argumento 2 | Resultado |
| --------- | -------------------------------------- | ----------- | --------- |
| TRAP      | diagnóstico del literal fuera de rango | —           | —         |

El argumento 1 es un diagnóstico con código, mensaje y ubicación, no un valor ITD; no hay resultado. No reemplaza errores léxicos, sintácticos o semánticos que bloquean la compilación. Las operaciones como `DIV` y `CAST` también pueden fallar por sí mismas durante la ejecución.

## Reglas de traducción y validación

1. **Precedencia:** `let x := 2 + 3 * 4;` genera `(MUL, 3, 4, t1)`, `(ADD, 2, t1, t2)` y `(COPY, t2, —, x)`, en ese orden. El AST determina la agrupación; no se vuelve a analizar texto.
2. **Cortocircuito:** para `A and B`, evaluar `A`, copiarlo a `t`, saltar a `L_end` si `t` es falso, evaluar `B`, copiarlo a `t` y emitir `LABEL L_end`. Para `or`, usar `JUMP_IF_TRUE`. `A` y `B` son metanotación de expresiones booleanas: no se generan operaciones ansiosas `AND`/`OR`.
3. **Efectos y temporales:** una lectura cuyo valor deba sobrevivir a una llamada o asignación se captura antes de esa operación. Por ejemplo, en `x + change()`, conservar el valor original de `x` antes de llamar a `change`. No sustituir valores capturados por lecturas posteriores.
4. **Tipos y errores:** reutilizar las resoluciones, tipos y coerciones del análisis semántico. En la VM, `decimal` conserva representación decimal exacta y no pasa por `f64`; en el backend ARM64 opt-in, `decimal` **sí** se representa como `f64` IEEE-754, con sus puntos de redondeo y diferencias de resultado. Ambos flujos conservan las comprobaciones correspondientes a sus tipos. No adelantar una operación que pueda fallar fuera de su rama ni convertir errores de ejecución en fallos incondicionales de compilación.
5. **Estructura:** verificar etiquetas existentes y locales a cada función, tipos de operandos, temporales inicializados en todas las rutas de lectura, receptores, campos, aridad y grupos `PARAM`/`CALL`. La propuesta didáctica no exige SSA; la IR nativa actual sí exige que cada índice temporal se defina una sola vez, mientras los `LocalSlot` mutables sirven de punto de unión entre ramas de cortocircuito.
6. **Alcance:** declaraciones de módulos, clases y traits producen metadata y enlaces, no una operación por palabra de la gramática. Los inicializadores sí producen instrucciones. La indexación y `each` no son ejecutables actualmente; no se inventan operaciones para habilitarlos.

## Integración y referencias

La integración ejecutable de ARM64 ya es independiente: `AST + semántica para cuádruplos → cuádruplos tipados y validados → optimización → ARM64`. La UI muestra la tabla de cuádruplos antes de optimizar y el ensamblador posterior. El bytecode validado y la VM siguen siendo el flujo predeterminado y no son una etapa del backend nativo; tampoco hay equivalencia total prometida entre ambos.

Fuentes del contrato general: [gramática](docs/language/grammar.ebnf), [tipos](docs/language/type-system.md), [objetos](docs/language/object-model.md), [bytecode](docs/language/bytecode.md) y [guía ARM64](docs/language/arm64.md). Evidencia de implementación: [IR](crates/compiler-core/src/quadruple.rs), [lowering](crates/compiler-core/src/quadruple_lowering.rs), [optimización](crates/compiler-core/src/quadruple_optimize.rs) y [emisor](crates/compiler-core/src/arm64.rs).
