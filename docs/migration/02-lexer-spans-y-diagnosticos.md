# Feature 02: Lexer, spans y diagnósticos

## Objetivo

Tokenizar fuentes completas sin perder información y producir diagnósticos precisos en lugar de excepciones genéricas.

## Modelo mínimo

```text
SourceId  identifica un archivo
Span      contiene offsets start..end
Token     contiene kind, span y lexema cuando sea necesario
Diagnostic contiene code, severity, message, source y labels
```

## Tareas

- [x] Implementar `SourceMap` para registrar fuentes y convertir offsets a línea/columna.
- [x] Implementar `TokenKind` como enum Rust.
- [x] Mantener el lexema original de números para evitar pérdida decimal.
- [x] Reconocer identificadores y palabras reservadas según la especificación.
- [x] Reconocer enteros, decimales y sufijos numéricos aprobados.
- [x] Implementar strings y chars con escapes válidos y errores de cierre.
- [x] Implementar operadores y delimitadores, incluidos los de varios caracteres.
- [x] Implementar comentarios de línea y, si la especificación los conserva, comentarios de bloque.
- [x] Emitir un token EOF incluso después de identificadores o números sin separador final.
- [x] Reportar caracteres inválidos y continuar cuando sea seguro.
- [x] Añadir códigos estables como `E0001` para errores léxicos.

## Pruebas mínimas

- [x] Identificador y número justo antes de EOF.
- [x] String o comentario de bloque sin cerrar.
- [x] Entradas UTF-8 conforme a la regla elegida para identificadores.
- [x] Saltos de línea `LF` y `CRLF`.
- [x] Decimal con punto, exponente o sufijo válido e inválido.
- [x] Dos errores léxicos recuperables en el mismo archivo.
- [x] Span exacto de cada token y diagnóstico.

## Criterios de aceptación

- Ningún input causa `panic`.
- Los lexemas decimales llegan intactos al parser.
- Línea y columna se calculan desde `Span`, no se mantienen como estado duplicado en cada fase.
- Los errores serializables no contienen backtraces ni detalles internos.

## Fuera de alcance

Construir AST, resolver nombres o asignar tipos.
