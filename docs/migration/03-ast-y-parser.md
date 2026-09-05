# Feature 03: AST y parser

## Objetivo

Transformar tokens en un AST pequeño, tipado y preparado para análisis semántico.

## Tareas

- [x] Modelar `Program`, `Decl`, `Stmt` y `Expr` con enums y structs Rust.
- [x] Asociar un `Span` a cada nodo relevante.
- [x] Implementar parser Pratt para operadores prefijos, infijos y postfix.
- [x] Implementar literales, variables, agrupación y operadores aritméticos.
- [x] Implementar declaraciones `let`, asignación y expresiones terminadas según la gramática.
- [x] Añadir comparaciones y operadores lógicos.
- [x] Añadir sintaxis postfix necesaria: llamadas, acceso a miembro e indexación.
- [x] Separar errores sintácticos de errores léxicos.
- [x] Recuperarse en `;`, `}` o inicio de declaración para reportar múltiples errores.
- [x] Producir un resultado parcial solo para análisis; nunca ejecutarlo si contiene errores fatales.

## Pruebas mínimas

- [x] Precedencia: `1 + 2 * 3`.
- [x] Asociatividad de asignación y operadores aprobados.
- [x] Expresiones unarias y agrupadas.
- [x] Llamadas y cadenas de acceso a miembros.
- [x] Declaración incompleta seguida de otra válida.
- [x] Delimitadores faltantes con diagnóstico sobre el rango correcto.
- [x] Snapshot o representación estable del AST para fixtures pequeños.

## PRs sugeridos

1. `feat(parser): add expression ast and pratt parser`
2. `feat(parser): add declarations and assignments`
3. `feat(parser): recover from syntax errors`

## Criterios de aceptación

- El parser consume todo el archivo o explica por qué no puede hacerlo.
- Los errores incluyen lo esperado, lo encontrado y un `Span`.
- No se convierten números a valores de runtime durante el parseo.
- La gramática implementada coincide con `grammar.ebnf`.

## Fuera de alcance

Comprobar tipos, ejecutar el programa o resolver archivos importados.
