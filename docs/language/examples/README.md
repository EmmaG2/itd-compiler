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
