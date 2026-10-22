# Catálogo de cuádruplos

## Objetivo y autorización

Crear `CUADRUPLOS.md`, solicitado por el usuario, con las 31 operaciones propuestas, tablas, ejemplos de código y tipos permitidos. Es documentación de una IR propuesta, no implementación ni modificación de la gramática.

## Alcance y restricciones

- Preservar todos los cambios preexistentes; modificar únicamente el catálogo y este seguimiento.
- Español profesional; contrastar ejemplos y tipos con AST, semántica, runtime y documentación actuales.
- No confundir operaciones de pila existentes con cuádruplos implementados.
- Ruta delegada: la preparación requiere más de cuatro archivos y precede a una escritura.
- TDD: no aplicable a documentación sin cambios de comportamiento; no se cambia la configuración existente.
- Comprobaciones: cobertura de las 31 operaciones, revisión semántica y formato mediante Make dentro de Nix.
- RDD: herramienta `gentle-ai` no disponible; evaluación nativa pendiente/no disponible, sin aprobación inferida.
- Entrega: `ask-on-risk`; previsión aproximada de 380 líneas añadidas. Sin operaciones remotas.
- Espejo Engram pendiente: el runtime no proporciona identidad registrada; escrituras de memoria bloqueadas.

## Trabajo

- [x] Q01 — Redactar y verificar el catálogo completo. Ruta: delegada por preparación multidocumento. Resultado: 31 secciones con tabla, ejemplo y tipos; convenciones de operandos y restricciones explícitas.

## Evidencia y siguiente paso

- Exploración: operadores en `ast.rs`, instrucciones en `bytecode.rs`, reglas en `semantic.rs` y `runtime.rs`; la suma de cadenas del runtime no está admitida por el análisis semántico.
- Verificación de ejecución: no aplicable al documento; ejemplos se revisarán contra fuentes, sin afirmar que se ejecutaron.
- Reversión: retirar solamente `CUADRUPLOS.md` y este seguimiento.
- Comprobación: `nix-shell --run 'make format-files FILES="CUADRUPLOS.md odd/tasks/quadruples-catalog.md"'` terminó con código 0; segunda pasada sin cambios. Recuento: 31 operaciones y 335 líneas en el catálogo.
- Revisión de fuentes: sintaxis de negación y constructores, errores diferidos de literales, promociones numéricas, resto decimal/flotante y mutabilidad de campos confirmados. Ejemplos no ejecutados; `make check` omitido por alcance exclusivamente documental.
- Entrega local: 363 líneas iniciales, por debajo de la previsión; ningún archivo de implementación modificado por esta tarea. Commit pendiente de registrar inmediatamente después de crearlo.
- Siguiente paso: registrar commit y entregar el documento. Revisión nativa y espejo Engram siguen no disponibles; no se afirma aprobación.
