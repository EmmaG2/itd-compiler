# Feature 06: Control de flujo y funciones

## Objetivo

Permitir programas estructurados con bloques, decisiones, ciclos y funciones verificadas estáticamente.

## Tareas de control de flujo

- [x] Implementar bloques con scope propio.
- [x] Implementar `if` y `else`.
- [x] Implementar `while` y la construcción elegida para repetición.
- [x] Implementar `break` y `continue` con validación de contexto.
- [ ] Implementar `each` cuando exista al menos un tipo iterable definido.
- [x] Añadir análisis de flujo para detectar sentencias inalcanzables básicas.

## Tareas de funciones

- [x] Implementar declaraciones de función, parámetros y tipo de retorno.
- [x] Resolver llamadas y validar aridad y tipos de argumentos.
- [x] Implementar `return` y comprobar que aparece dentro de una función.
- [x] Verificar que todas las rutas de una función no `void` retornan un valor.
- [x] Implementar scopes locales, recursión y stack de llamadas en el intérprete temporal.
- [x] Añadir funciones nativas mínimas, empezando por salida de texto.
- [x] Mantener errores de usuario como diagnósticos, no como `panic`.

## Pruebas mínimas

- [ ] `if/else` con ramas anidadas.
- [ ] Ciclo con `break` y `continue`.
- [ ] Uso inválido de `break`, `continue` y `return` fuera de contexto.
- [ ] Función recursiva pequeña.
- [ ] Aridad y tipos incorrectos.
- [ ] Función no `void` con una ruta sin retorno.
- [ ] Variables locales aisladas entre llamadas.

## PRs sugeridos

1. `feat(control): add blocks and conditional execution`
2. `feat(control): add loops and loop control`
3. `feat(functions): parse and resolve function declarations`
4. `feat(functions): type-check calls and returns`
5. `feat(runtime): execute functions with call frames`

## Criterios de aceptación

- Los scopes se crean y destruyen correctamente en ramas, ciclos y llamadas.
- Una función no puede leer locales de otra llamada.
- Los errores de control de flujo se detectan antes de ejecutar.
- La recursión queda limitada por el runtime para evitar agotar el proceso host.

## Fuera de alcance

Closures, lambdas, async/await, excepciones y generadores.
