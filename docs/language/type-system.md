# Sistema de tipos

`let` es inmutable y `let mut` permite reasignar. `const` exige inicializador constante. Un nombre puede ocultar otro de un scope exterior, pero no redeclararse en el mismo scope. Leer antes de inicializar es error.

Tipos enteros: `byte` (`i8`), `short` (`i16`), `int` (`i32`) y `long` (`i64`), con overflow comprobado. `float` y `double` son IEEE-754; `decimal` es base 10 exacta. También existen `bool`, `str`, `char` y `void`.

Solo se infiere el tipo local del inicializador. No hay conversiones numéricas implícitas salvo ensanchamiento entre enteros cuando el valor cabe. La aritmética y las comparaciones de orden entre entero y `decimal` promueven el entero a `decimal` (la potencia mixta exige cast). La asignación entero/decimal y todo cambio con tipos binarios requieren `as`; un cast fuera de rango falla. La igualdad requiere tipos iguales. División o módulo por cero fallan. División entera trunca hacia cero.

Los resultados `decimal` usan precisión máxima de 28 cifras significativas, operaciones comprobadas y redondeo bancario (mitad al par) solo cuando sea necesario ajustar escala. Nunca pasan por `float` o `double`.

Precedencia, de mayor a menor: postfix, cast, unarios, potencia (asociativa a derecha), multiplicación/división/módulo, suma/resta, comparación, igualdad, `and`, `or`, asignación (asociativa a derecha).

La suma y resta son asociativas a izquierda: `a + b + c` forma `(a + b) + c`. `0.1dec + 0.2dec * 0.3dec` produce `0.16`; con paréntesis, `(0.1dec + 0.2dec) * 0.3dec` produce `0.09`.

`to_string(valor)` acepta `byte`, `short`, `int`, `long` y `decimal` y devuelve `str`. Los enteros se convierten directamente, sin cast a decimal ni pérdida de precisión; los decimales conservan su escala. Por ejemplo, `print(to_string(42));` imprime `42`. Se requiere exactamente un argumento; otros tipos no se aceptan. `print` sigue recibiendo `str`.
