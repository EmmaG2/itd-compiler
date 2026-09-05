# Reglas léxicas

- El código fuente es UTF-8. Un identificador empieza con `_` o una letra Unicode y continúa con letras Unicode, dígitos o `_`. Las palabras reservadas son ASCII y sensibles a mayúsculas.
- Los espacios, `LF` y `CRLF` separan tokens. Los offsets de un `Span` son bytes UTF-8 en el rango semiabierto `start..end`.
- Comentarios: `//` hasta el salto de línea y `/* ... */` no anidado.
- Strings usan comillas dobles y chars comillas simples. Escapes válidos: `\n`, `\r`, `\t`, `\0`, `\\`, `\"` y `\'`. Un char contiene exactamente un carácter después de resolver escapes.
- Enteros: dígitos decimales con sufijo opcional `i8`, `i16`, `i32` o `i64`.
- Decimales exactos: dígitos, punto y al menos un dígito fraccionario; exponente decimal opcional; sufijo opcional `dec`. Ejemplos: `1.25`, `1.25dec`, `12e-2dec`.
- Binarios explícitos: cualquier literal numérico con sufijo `f32` o `f64`. Nunca se convierte implícitamente entre binario y `decimal`.
- No se admiten separadores `_`, hexadecimal, `NaN` ni infinito en 0.1.

Palabras reservadas implementables en 0.1: `pub`, `priv`, `protected`, `int`, `long`, `str`, `decimal`, `double`, `char`, `float`, `short`, `byte`, `bool`, `void`, `true`, `false`, `if`, `else`, `while`, `each`, `in`, `enum`, `struct`, `trait`, `class`, `let`, `const`, `mut`, `static`, `fn`, `return`, `break`, `continue`, `and`, `or`, `not`, `use`, `from`, `export`, `module`, `this` y `as`.

Reservadas pero pospuestas: `repeat`, `lambda`, `async`, `await`, `try`, `catch`, `throw`, `finally` y `raise`.
