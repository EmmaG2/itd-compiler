# Feature 08: Programación orientada a objetos

## Objetivo

Implementar clases y objetos con encapsulación, métodos y polimorfismo sin introducir herencia innecesaria en la primera versión.

## Alcance del MVP

- Clases con campos y métodos.
- Constructor explícito.
- Referencia `this`.
- Miembros de instancia y estáticos.
- Visibilidad pública y privada.
- Traits como contrato y mecanismo de polimorfismo.
- Composición de objetos.

## Tareas de sintaxis y AST

- [x] Añadir declaración de clase, campo, constructor, método y trait.
- [x] Añadir expresiones de construcción, acceso a miembro y llamada de método.
- [x] Definir si los campos requieren tipo explícito y valor inicial.
- [x] Definir sintaxis de implementación de traits.

## Tareas semánticas

- [x] Crear símbolos para clases, campos, métodos, constructores y traits.
- [x] Resolver `this` únicamente dentro de métodos de instancia.
- [x] Rechazar miembros duplicados y firmas incompatibles.
- [x] Validar visibilidad en el punto de acceso.
- [x] Comprobar argumentos del constructor.
- [x] Resolver métodos de instancia y estáticos sin ambigüedad.
- [x] Validar que una clase implemente todos los miembros requeridos por un trait.
- [x] Detectar ciclos de composición directa imposibles por valor.

## Tareas de runtime

- [x] Representar clases mediante metadata inmutable.
- [x] Representar objetos mediante un identificador y almacenamiento de campos.
- [x] Enlazar `this` al invocar un método.
- [x] Implementar lookup de métodos y dispatch por trait.
- [x] Definir igualdad por identidad para objetos y por valor para tipos primitivos.
- [x] Reportar acceso a objeto inválido como error controlado.

## Pruebas mínimas

- [x] Construcción y lectura/escritura de campos permitidos.
- [x] Método que usa `this`.
- [x] Acceso privado válido desde la clase e inválido desde fuera.
- [x] Método estático invocado desde la clase.
- [x] Constructor con argumentos incorrectos.
- [x] Clase que cumple e incumple un trait.
- [x] Dos instancias mantienen estados independientes.
- [x] Uso de un objeto a través de un tipo trait.

## PRs sugeridos

1. `feat(oop): parse classes and object expressions`
2. `feat(oop): resolve members and enforce visibility`
3. `feat(oop): execute constructors and methods`
4. `feat(oop): add traits and dynamic dispatch`

## Criterios de aceptación

- Clases y traits funcionan entre módulos.
- Los accesos inválidos se rechazan antes del runtime cuando sea posible.
- La representación de objetos no expone detalles de Tauri ni JavaScript.
- Los programas con objetos respetan los límites de memoria del runtime.

## Fuera de alcance

Herencia entre clases, `protected`, sobrecarga múltiple, genéricos y reflexión. La herencia se añade solo si composición y traits no cubren un caso real.
