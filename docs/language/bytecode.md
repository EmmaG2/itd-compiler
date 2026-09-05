# Bytecode y VM

La transición inicial usa dos instrucciones deliberadamente pequeñas:

- `Execute(index)`: ejecuta el item validado almacenado en `constants[index]`.
- `Halt`: termina el programa y debe ser la última instrucción.

Un `Chunk` contiene instrucciones, items constantes y un span de código fuente por instrucción. Antes de ejecutar, la VM valida el mapa de spans, todos los índices y la posición de `Halt`. `run` siempre compila y valida un `Chunk`; la evaluación interna conserva temporalmente el intérprete de AST como referencia.

Cada ejecución limita instrucciones, profundidad de llamadas y objetos del heap, y acepta una señal atómica de cancelación. Los errores conservan el span original y agregan las llamadas activas como etiquetas. El heap pertenece a una ejecución y se libera completo al terminar.

Este formato es de transición. Instrucciones escalares para operandos y saltos se añadirán cuando se retire el intérprete de referencia.
