# Backend Darwin ARM64

El backend nativo genera texto de ensamblador Darwin ARM64 para Apple Silicon. Su ruta es independiente de la VM:

```text
fuente → análisis semántico para cuádruplos → IR tipada y validada
       → optimización segura → ensamblador Darwin ARM64
```

No ejecuta el ensamblador desde la aplicación. La acción **Generar ARM64** solo solicita una inspección nativa y muestra diagnósticos, la tabla de cuádruplos y, si la emisión es válida, el texto del ensamblador.

## Uso en la aplicación

1. Abrí la aplicación de escritorio con `make dev` dentro de `nix-shell`.
2. Escribí o cargá un programa y elegí **Generar ARM64**.
3. Abrí la pestaña **Cuádruplos y ARM64**.

La tabla es la IR **antes de optimizar**, con operación, dos argumentos, resultado, función, tipo y ubicación fuente. El bloque de ensamblador es el resultado **optimizado**. Por ejemplo, `2 + 3 * 4` puede reducirse a una constante, y una rama estáticamente inalcanzable no aporta funciones, cadenas ni código alcanzable al ensamblador. La inspección está limitada a 20 000 filas; no es una interfaz para compilar ni ejecutar binarios.

La API del núcleo ofrece:

- `compile_quadruples(source)`: baja fuente a `QuadrupleProgram` tipado, sin compilar bytecode.
- `emit_arm64(program)`: valida y optimiza el programa antes de devolver el ensamblador o un diagnóstico.
- `compile_arm64(source)`: atajo de fuente a ensamblador, conservando diagnósticos.

No hay un comando de línea de comandos para emitir fuente a ARM64. Usá la UI o esas APIs; las tareas del repositorio se invocan exclusivamente mediante `make`.

## Alcance numérico y diferencias con la VM

La IR nativa admite `int` como `i32` con signo y `decimal` como `f64` IEEE-754. Esto diverge deliberadamente de la VM, que conserva enteros `i64` y `decimal` exacto de base 10. Por eso resultados, precisión, NaN y límites pueden diferir; el backend no promete equivalencia total con la VM.

`byte`, `short`, `long`, `float` y `double` no están soportados por este backend. Tampoco se convierten silenciosamente a un tipo nativo compatible: el lowering los rechaza con un diagnóstico. Para `i32`, suma, resta, multiplicación, negación, división, módulo y potencia verifican overflow cuando corresponde. La división y el módulo verifican divisor distinto de cero; la potencia entera rechaza exponentes negativos y desborda mediante un trap nativo. Las comprobaciones dinámicas producen una salida no nula del programa nativo, no un diagnóstico de la UI después de ejecutarlo.

`decimal` usa `f64` para suma, resta, multiplicación, división, comparaciones y conversión desde `int`. La implementación no admite `POW` ni `MOD` decimal en el emisor, y los rechaza. Las cadenas que contienen NUL también se rechazan antes de emitir, porque el runtime nativo las representa como cadenas C terminadas en NUL.

## Ejemplos pequeños

Este balance, basado en el material [PDF de cuádruplos](../../files/Cuadruplos_corregidos.docx%20%282%29.pdf), está dentro del alcance porque usa `int`, `decimal` y `print`/`to_string`:

```itd
let mut balance: decimal := 100dec;
let mut month: int := 1;
while month <= 3 {
    balance = balance + 50dec;
    print(to_string(balance));
    month = month + 1;
}
```

También se soportan construcción, campos, estáticos y métodos enlazados locales:

```itd
class Counter {
    pub let mut value: int := 0;
    pub static let mut total: int := 0;
    pub fn this(initial: int) { this.value = initial; }
    pub fn read() -> int { return this.value; }
}
let counter := Counter(3);
let read := counter.read;
Counter.total = read();
print(to_string(Counter.total));
```

## ABI, llamadas y límites

El emisor sigue la convención de llamadas de Darwin ARM64 y separa registros enteros/puntero de registros `f64`; como referencia, consultá el [ABI oficial de Apple para arm64](https://developer.apple.com/documentation/xcode/writing-arm64-code-for-apple-platforms). Las llamadas variádicas de Darwin colocan sus argumentos variádicos en slots de pila: el runtime nativo lo hace para `printf` y `snprintf`, incluso cuando el valor original es `f64`.

El límite confirmado por el emisor (diagnóstico `E7201`) es de hasta ocho argumentos enteros o punteros —incluido el receptor— y hasta ocho argumentos `f64` por firma o llamada. No se implementa el paso de argumentos restantes por pila. La residencia en registros también es limitada: el emisor conserva resultados `f64` de uso único y adyacente en `d16` y carga el otro operando directamente en `d17`, pero derrama ante llamadas, uniones de CFG o usos múltiples. No existe un asignador general de registros.

Un `BoundMethod` puede vivir en una variable local y llamarse, como `let read := counter.read; read();`. No puede ser parámetro ni retorno de una firma nativa: esos casos se rechazan antes de generar ensamblador.

Los objetos nativos se reservan con `malloc`; sus campos son slots de ocho bytes y los estáticos se emiten en datos. El runtime incorporado solo cubre las rutas nativas alcanzables de `print` y `to_string` para `i32`, `f64`, `bool` y `str`. Estas son propiedades de la implementación actual, no una garantía de heap mínimo global ni de soporte exhaustivo de las características del lenguaje.

## Propiedad de cadenas nativas

Las literales y el resultado de `to_string(bool)` son prestados y estáticos. `to_string(i32)` y `to_string(f64)` reservan un búfer de 64 bytes: verifican `malloc` y el resultado de `snprintf`, y hacen trap si la conversión no cabe o falla. El formato decimal continúa siendo `%g`, cuyo valor predeterminado imprime seis cifras significativas; no es una representación decimal exacta ni conserva ceros finales. Cambiar esa precisión sería una propuesta semántica separada, no un ajuste incidental del backend.

`print` no agrega un salto de línea. Para `str`, el runtime pasa el dato a `printf` mediante el formato `%s`; por eso llamadas consecutivas concatenan su salida. El programa decimal `0.1dec`, `0.2dec` y `0.3dec` con las tres expresiones de aceptación imprime exactamente `0.60.160.09` bajo `%g`.

El emisor etiqueta internamente esos búferes dinámicos y mantiene una referencia por cada home local, parámetro o temporal `str`. Copias, aliases, argumentos y `to_string(str)` retienen la referencia; los retornos transfieren una referencia al llamador; la salida de la función libera sus homes. `print(str)` desetiqueta solamente el puntero dinámico antes de pasarlo por la pila de argumentos variádicos de Darwin, sin tomar ni ceder propiedad. Los estáticos `str` retienen al asignar, liberan su valor previo y se liberan al terminar `main`.

Los campos `str` se rechazan para ARM64 (`E7201`): los objetos nativos aún no tienen destrucción, por lo que aceptar una cadena dinámica allí ocultaría una fuga o una liberación insegura. No se liberan literales, booleanos ni punteros prestados.

Los límites de recursos de la VM —instrucciones, profundidad de llamadas, objetos, salida, tiempo y cancelación— forman su sandbox y siguen aplicando al flujo **Ejecutar**. No son límites aplicados al binario Darwin emitido; ese binario usa el runtime nativo y sus traps. El mensaje de trap nativo tiene 20 bytes (`native runtime trap\n`) y el helper solo se emite cuando una ruta alcanzable puede atraparse; no forma parte de cada binario emitido.

## Verificación

En Apple Silicon con Apple clang, ejecutá las pruebas nativas ignoradas mediante el objetivo del proyecto:

```sh
nix-shell --run 'make test-arm64'
```

`make check` ejecuta formato, lint, pruebas regulares y compilación de frontend, pero no incluye estas pruebas ignoradas. La prueba de interfaz usa IPC simulado: confirma que la inspección es de solo lectura y no ejecuta código nativo. No hay prueba visual de navegador registrada para esta vista.

## Key Learnings

1. Las llamadas variádicas Darwin colocan los argumentos variables en la pila.
2. El formato `%g` predeterminado conserva seis cifras significativas como máximo.
3. La residencia decimal actual no constituye un asignador general de registros.
4. El runtime de trap solo se emite para rutas nativas alcanzables.
