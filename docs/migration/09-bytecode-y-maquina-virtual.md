# Feature 09: Bytecode y máquina virtual

## Objetivo

Convertir el lenguaje de intérprete de AST a compilador de bytecode con una máquina virtual controlada.

## Tareas del compilador

- [x] Definir un conjunto mínimo de instrucciones documentado.
- [x] Crear chunks con bytecode, constantes y mapa de spans.
- [ ] Compilar literales, variables, operadores y asignaciones.
- [ ] Compilar saltos condicionales, ciclos y control de flujo.
- [ ] Compilar funciones, llamadas y retornos.
- [ ] Compilar acceso a módulos y globals.
- [ ] Compilar construcción de objetos, campos y métodos.
- [ ] Emitir instrucciones específicas o llamadas internas seguras para decimales.
- [x] Validar límites de operandos y terminación al generar bytecode.

## Tareas de la VM

- [ ] Implementar stack de operandos y call frames.
- [x] Implementar tabla de globals y almacenamiento de objetos.
- [x] Mantener un heap acotado por ejecución.
- [x] Ejecutar aritmética checked y conservar tipos en runtime.
- [x] Implementar instruction budget, límite de recursión y cancelación.
- [x] Convertir fallos en diagnósticos con stack trace y spans del código fuente.
- [x] Impedir que bytecode inválido provoque acceso fuera de límites.
- [x] Liberar todo el heap al terminar una ejecución.

## Estrategia de transición

Conservar temporalmente el intérprete de AST como referencia. Ejecutar los mismos fixtures en ambos runtimes y eliminar el intérprete únicamente cuando los resultados y diagnósticos relevantes coincidan.

## Pruebas mínimas

- [ ] Cada familia de instrucciones tiene al menos un programa que la ejecuta.
- [x] Bytecode malformado se rechaza de manera segura.
- [x] Recursión excesiva e iteración infinita alcanzan un límite controlado.
- [x] Stack trace conserva funciones y spans resolubles a archivo y línea.
- [ ] Funciones, módulos, objetos y decimales producen los mismos resultados que el intérprete de referencia.
- [x] Cancelar una ejecución libera sus recursos.

## PRs sugeridos

1. `feat(bytecode): compile expressions and local variables`
2. `feat(vm): execute core bytecode instructions`
3. `feat(bytecode): compile control flow and functions`
4. `feat(vm): add call frames and runtime limits`
5. `feat(bytecode): compile modules and objects`
6. `refactor(runtime): remove tree-walk interpreter`

## Criterios de aceptación

- Todo programa válido del MVP se transforma en bytecode antes de ejecutarse.
- La VM nunca confía en índices o saltos sin validarlos.
- Un programa de usuario no puede bloquear indefinidamente el proceso host.
- Los diagnósticos de runtime conservan ubicación en el código original.

## Fuera de alcance

JIT, optimizador avanzado, LLVM, Cranelift y ejecutables nativos. El heap puede usar handles en un arena acotado; se añade GC trazador cuando existan ejecuciones persistentes o ciclos que deban sobrevivir largo tiempo.
