# Lenguaje ITD 0.1

Esta es la especificación normativa de la primera versión del lenguaje. Una incompatibilidad futura requiere incrementar la versión, documentar la migración y conservar fixtures de la versión anterior.

## Decisiones sobre los tokens Java

| Tokens Java                                                                                                    | Decisión en 0.1                                                                               |
| -------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `TOKEN_ID`, `TOKEN_NUM`, `TOKEN_STRING`, `TOKEN_CHAR`, `TOKEN_EOF`, `TOKEN_ERR`                                | Se conservan como `Identifier`, `Number`, `String`, `Char`, `Eof` y `Error`.                  |
| `TOKEN_KW_WHILE`                                                                                               | Se unifica con `KW_WHILE` como `While`.                                                       |
| `KW_PUB`, `KW_PRIV`, `KW_STATIC`                                                                               | Se conservan.                                                                                 |
| `KW_PROTECT`                                                                                                   | Se renombra a `protected`; `protect` deja de ser palabra reservada.                           |
| `KW_INT`, `KW_LONG`, `KW_STR`, `KW_DOUBLE`, `KW_CHAR`, `KW_FLOAT`, `KW_SHORT`, `KW_BYTE`, `KW_BOOL`, `KW_VOID` | Se conservan; se añade `decimal`.                                                             |
| `KW_IF`, `KW_ELSE`, `KW_WHILE`, `KW_EACH`, `KW_IN`                                                             | Se conservan.                                                                                 |
| `KW_EXIST`                                                                                                     | Se elimina: no tenía semántica definida. `exist` es un identificador válido.                  |
| `KW_REPEAT`                                                                                                    | Se pospone y queda reservado.                                                                 |
| `KW_ENUM`, `KW_STRUCT`, `KW_TRAIT`, `KW_CLASS`, `KW_LET`, `KW_CONST`, `KW_MUT`, `KW_STATIC`                    | Se conservan.                                                                                 |
| `KW_FN`, `KW_RETURN`, `KW_BREAK`, `KW_CONTINUE`                                                                | Se conservan.                                                                                 |
| `KW_LAMBDA`, `KW_ASYNC`, `KW_AWAIT`                                                                            | Se posponen y quedan reservados.                                                              |
| `KW_AND`, `KW_OR`, `KW_NOT`                                                                                    | Se conservan como operadores lógicos de palabra.                                              |
| `KW_USE`, `KW_FROM`, `KW_EXPORT`, `KW_MODULE`                                                                  | Se conservan; `from` queda reservado para una futura forma de importación.                    |
| `KW_PACKAGE`                                                                                                   | Se elimina; `package` es un identificador válido.                                             |
| `KW_TRY`, `KW_CATCH`, `KW_THROW`, `KW_FINALLY`, `KW_RAISE`                                                     | Se posponen y quedan reservados; `raise` será alias incompatible y no se implementará en 0.1. |
| `TOKEN_PLUS`, `TOKEN_MINUS`, `TOKEN_MULT`, `TOKEN_DIV`, `TOKEN_POW`, `TOKEN_MOD`                               | Se conservan.                                                                                 |
| `TOKEN_ARITH`                                                                                                  | Se elimina por ser un token genérico sin lexema propio.                                       |
| `TOKEN_REL_EQ`, `TOKEN_REL_NE`, `TOKEN_REL_LT`, `TOKEN_REL_GT`, `TOKEN_REL_LE`, `TOKEN_REL_GE`                 | Se conservan.                                                                                 |
| `TOKEN_ASSIGN`, `TOKEN_ASSIGN_DEF`, `TOKEN_COLON`, `TOKEN_SEMICOLON`, `TOKEN_COMMA`, `TOKEN_DOT`, `TOKEN_EXCL` | Se conservan; `!` no reemplaza a `not`.                                                       |
| Todos los delimitadores `TOKEN_LBRACE`…`TOKEN_RBRACKET`                                                        | Se conservan.                                                                                 |
| `TOKEN_COMMENT`                                                                                                | Los comentarios se conservan como trivia y no llegan al parser.                               |

Los tokens pospuestos se reconocen para producir un error claro del parser. No implican que la característica exista.

## Ejemplos

- [Gramática](grammar.ebnf)
- [Módulos](modules.md)
- [Modelo de objetos](object-model.md)
- [Bytecode y VM](bytecode.md)

- [Programa válido](examples/valid.itd): debe superar lexer y, cuando existan, parser y análisis semántico.
- [Errores léxicos](examples/invalid-lexical.itd): rechazado por el lexer.
- [Errores semánticos](examples/invalid-semantic.itd): aceptado sintácticamente y rechazado por tipos/resolución.
