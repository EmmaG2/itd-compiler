# Feature 04: Resolución, scopes y tipos

## Objetivo

Resolver cada nombre a una declaración y rechazar programas semánticamente inválidos antes de ejecutarlos.

## Modelo inicial

```text
Type: Int | Decimal | Bool | Str | Char | Void | Function | Error
Symbol: id, name, kind, type, visibility, mutability, declaration_span
Scope: parent + símbolos declarados localmente
```

## Tareas

- [x] Crear identificadores internos para símbolos; no usar el nombre como identidad después de resolverlo.
- [x] Implementar scopes globales, de bloque y de función.
- [x] Detectar declaraciones duplicadas, nombres inexistentes y uso antes de inicialización.
- [x] Implementar shadowing conforme a la especificación.
- [x] Validar mutabilidad durante una asignación.
- [x] Implementar anotaciones de tipo e inferencia local.
- [x] Definir una matriz de tipos permitidos por operador.
- [x] Insertar conversiones permitidas en una representación tipada; rechazar las demás.
- [x] Validar condiciones booleanas y compatibilidad de asignación.
- [x] Continuar el análisis usando `Type::Error` para evitar diagnósticos en cascada.
- [x] Separar AST sintáctico de información semántica mediante IDs o tablas laterales.

## Pruebas mínimas

- [x] Variable inexistente, duplicada e inmutable reasignada.
- [x] Shadowing válido entre bloques.
- [x] Inferencia de literales y expresiones.
- [x] Mezclas numéricas permitidas y prohibidas.
- [x] Condición no booleana.
- [x] Más de un error semántico independiente en el mismo archivo.

## PRs sugeridos

1. `feat(semantic): resolve symbols and lexical scopes`
2. `feat(types): infer variables and check assignments`
3. `feat(types): validate unary and binary operators`

## Criterios de aceptación

- Cada referencia válida apunta a un `SymbolId` único.
- Un programa con errores semánticos no llega al runtime.
- Los diagnósticos apuntan tanto al uso como a la declaración cuando corresponde.
- Resolver y comprobar tipos no depende de Tauri ni del filesystem.

## Fuera de alcance

Imports entre archivos, miembros de clases y generación de bytecode.
