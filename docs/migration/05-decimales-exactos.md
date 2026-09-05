# Feature 05: Decimales exactos

## Objetivo

Implementar un tipo decimal base 10 predecible y evitar los errores de representación binaria de `double`/`f64`.

## Política recomendada

- Usar `rust_decimal::Decimal` para `decimal`.
- Parsear el lexema directamente con una API exacta; nunca convertir primero a `f64`.
- Mantener `float`/`double` como tipos distintos y explícitos si la especificación los conserva.
- Promover `int + decimal` a `decimal`.
- Exigir cast explícito entre `decimal` y tipos binarios.
- Usar operaciones checked y convertir overflow o división por cero en diagnósticos de runtime.
- Definir `half-even` como redondeo predeterminado donde una operación lo requiera.
- Serializar decimales hacia Tauri como strings.

## Tareas

- [x] Añadir `rust_decimal` únicamente a `compiler-core`.
- [x] Implementar literal decimal desde string conservando signo y escala permitida.
- [x] Añadir `Decimal` al modelo de tipos y al enum de valores del runtime.
- [x] Implementar suma, resta, multiplicación, división, módulo y comparación checked.
- [x] Definir comportamiento para resultados no representables y divisiones periódicas.
- [x] Implementar casts explícitos y sus diagnósticos de pérdida de precisión.
- [x] Implementar funciones mínimas `round`, `scale` y conversión a string si pertenecen al lenguaje.
- [x] Documentar precisión máxima, escala y política de formato.

## Pruebas mínimas

- [ ] `0.1 + 0.2 == 0.3`.
- [ ] Conservación de `1.00` cuando la política de formato lo requiera.
- [ ] Números máximo y mínimo representables.
- [ ] Overflow en suma y multiplicación.
- [ ] División por cero.
- [ ] División que exige redondeo.
- [ ] Cast explícito válido e inválido entre `decimal` y `float`.
- [ ] Ida y vuelta por el contrato Tauri sin perder dígitos.

## Criterios de aceptación

- No existe una ruta que convierta un literal `decimal` a través de `f64`.
- Toda operación potencialmente fallida devuelve un error controlado.
- Las reglas observadas coinciden con la especificación numérica.

## Fuera de alcance

Precisión arbitraria. Si 28–29 dígitos significativos resultan insuficientes, se evaluará `bigdecimal` en una migración separada.
