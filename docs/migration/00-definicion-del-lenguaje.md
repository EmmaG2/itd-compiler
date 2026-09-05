# Feature 00: Definición del lenguaje

## Objetivo

Definir qué lenguaje se va a implementar antes de migrar código. Los tokens actuales son una intención, no una especificación.

## Decisiones recomendadas

- Tipado estático con inferencia local para `let`.
- Variables inmutables por defecto; `mut` habilita reasignación.
- Tipos iniciales: `int`, `decimal`, `bool`, `str`, `char` y `void`.
- `float` y `double` quedan como tipos binarios explícitos; no se mezclan implícitamente con `decimal`.
- Un archivo representa un módulo.
- OOP inicial mediante clases, encapsulación, composición y traits.
- Compilación a bytecode propio; generación nativa queda fuera del MVP.

## Tareas

- [x] Inventariar todos los tokens de `TokenType.java` y clasificarlos como necesarios, renombrados o pospuestos.
- [x] Resolver inconsistencias como `protect` frente a `protected` y `exist` frente a una construcción conocida.
- [x] Escribir la gramática léxica: identificadores, palabras reservadas, números, strings, chars y comentarios.
- [x] Escribir la gramática EBNF de expresiones, declaraciones, bloques, funciones, módulos y clases.
- [x] Definir precedencia y asociatividad de todos los operadores.
- [x] Definir mutabilidad, shadowing, scopes y reglas de inicialización.
- [x] Definir conversiones implícitas y casts explícitos.
- [x] Definir igualdad, overflow, división, módulo y redondeo decimal.
- [x] Definir sintaxis y resolución de `module`, `use` y `export`.
- [x] Definir clases, constructores, `this`, visibilidad, métodos estáticos y traits.
- [x] Preparar ejemplos válidos e inválidos por cada regla.
- [x] Asignar una versión inicial al lenguaje y documentar cómo se manejarán cambios incompatibles.

## Entregables

```text
docs/language/
  grammar.ebnf
  lexical-rules.md
  type-system.md
  modules.md
  object-model.md
  examples/
```

## Criterios de aceptación

- Cada token del proyecto Java tiene una decisión explícita.
- La gramática permite analizar sin ambigüedad todos los ejemplos válidos.
- Cada ejemplo inválido indica la fase que debe rechazarlo.
- Las reglas de decimales, módulos y objetos no dependen de detalles de Tauri.

## Fuera de alcance

Implementar código, elegir optimizaciones o diseñar funcionalidades marcadas como pospuestas.
