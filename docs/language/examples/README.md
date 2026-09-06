# Matriz de ejemplos

| Regla                | Válido                                            | Inválido                         | Fase que rechaza |
| -------------------- | ------------------------------------------------- | -------------------------------- | ---------------- |
| Identificador        | `let año := 1;`                                   | `let @x := 1;`                   | Lexer            |
| Número               | `12.50dec`                                        | `12.5dinero`                     | Lexer            |
| String               | `"hola\\n"`                                       | `"sin cierre`                    | Lexer            |
| Char                 | `'ñ'`                                             | `'ab'`                           | Lexer            |
| Comentario           | `/* texto */`                                     | `/* texto`                       | Lexer            |
| Declaración          | `let x := 1;`                                     | `let := 1;`                      | Parser           |
| Precedencia          | `2 + 3 * 4;`                                      | `2 + * 3;`                       | Parser           |
| Mutabilidad          | `let mut x := 1; x = 2;`                          | `let x := 1; x = 2;`             | Tipos            |
| Inicialización       | `let x := 1;`                                     | `let x: int;`                    | Parser           |
| Cast                 | `1 as decimal`                                    | `true as decimal`                | Tipos            |
| División             | `4 / 2`                                           | `4 / 0`                          | Runtime          |
| Módulo               | `use tienda.pagos;`                               | `use ..secretos;`                | Resolución       |
| Función              | `fn uno() -> int { return 1; }`                   | `fn uno( {}`                     | Parser           |
| Clase                | `class Caja { priv let total: decimal := 0dec; }` | `this.total;` fuera de clase     | Resolución       |
| Trait                | `trait Nombre { fn nombre() -> str; }`            | Método con campo dentro de trait | Parser           |
| Scope                | `{ let x := 1; }`                                 | `{ let x := 1; let x := 2; }`    | Resolución       |
| Tipo decimal/binario | `1dec + 2dec`                                     | `1dec + 2f64`                    | Tipos            |

## Programas completos

Los diez programas del catálogo son autónomos: cárgalos desde **Ejemplos → Cargar ejemplo** y pulsa **Ejecutar**. La app muestra una descripción y permite consultar su salida esperada. Si modificaste el editor, puedes conservarlo o descargar una copia antes de reemplazarlo.

| Programa                            | Conceptos                           | Salida principal              |
| ----------------------------------- | ----------------------------------- | ----------------------------- |
| [Expresiones](expressions.itd)      | Precedencia y agrupación            | `0.6`, `0.16`, `0.09`         |
| [Factorial](factorial.itd)          | Recursión y caso base               | `720`                         |
| [Fibonacci](fibonacci.itd)          | Acumuladores y ciclos               | `55`                          |
| [MCD](gcd.itd)                      | Algoritmo de Euclides               | `6`                           |
| [Primos](primes.itd)                | Divisibilidad y funciones           | Primos entre 2 y 30           |
| [Cuenta bancaria](bank-account.itd) | Encapsulación y decimal             | Retiro rechazado, saldo `115` |
| [Carrito](cart.itd)                 | Composición de dos productos        | `19.75`                       |
| [Figuras](shapes.itd)               | Traits y dispatch dinámico          | `12`, `12.56`                 |
| [Inventario](inventory.itd)         | Validación y estados independientes | Stock `7` y `5`               |
| [Contador](instance-counter.itd)    | Estáticos e identidad               | Dos instancias                |

Cada programa tiene un archivo vecino `.expected.txt`. `catalog.json` es el índice compartido por la app; el código se importa directamente de los `.itd`, sin copias. Las pruebas ejecutan todos los ejemplos y comparan la salida exacta en VM y, cuando se habilita, intérprete.

Los ejemplos numéricos usan `to_string(valor)`, que acepta `byte`, `short`, `int`, `long` y `decimal` sin cast previo. Factorial y Fibonacci usan `int` con overflow comprobado: no son algoritmos de precisión arbitraria. El carrito tiene exactamente dos productos y el círculo usa pi aproximado a 3.14.

### Facturación entre módulos

Selecciona **Abrir proyecto**, elige la carpeta `docs/language/examples/invoice`, deja `main` como módulo inicial y pulsa **Ejecutar proyecto**. `product.itd` exporta el producto; `invoice.itd` compone la factura; `main.itd` los usa con un alias. La salida es `25.50`.

`valid.itd` se conserva como fixture histórico de sintaxis: importa un módulo que no está incluido, por lo que no forma parte del catálogo ejecutable.
