# Feature 07: Sistema de módulos

## Objetivo

Compilar proyectos de varios archivos con namespaces, exports e imports deterministas y seguros.

## Reglas recomendadas para el MVP

- Un archivo fuente representa un módulo.
- Los nombres de módulo se resuelven desde una raíz de proyecto explícita.
- Solo los símbolos `export` pueden importarse.
- Los ciclos de importación se rechazan inicialmente con un diagnóstico claro.
- No existen imports desde URLs ni instalación automática de paquetes.

## Tareas

- [x] Fijar la sintaxis exacta de `module`, `use`, `from` y `export` en la gramática.
- [x] Definir extensión de archivo y relación entre nombre de módulo y ruta.
- [x] Implementar una estructura `Project` con raíz canonicalizada.
- [x] Resolver rutas relativas sin permitir escapar de la raíz.
- [x] Cargar cada archivo una sola vez por compilación.
- [x] Construir el grafo de dependencias.
- [x] Detectar módulos inexistentes, imports duplicados y ciclos.
- [x] Crear un scope por módulo y aplicar reglas de exportación.
- [x] Resolver nombres importados antes del type checker global.
- [x] Incluir nombre de archivo y cadena de imports en los diagnósticos.
- [x] Permitir que las pruebas suministren un proyecto temporal sin depender de Tauri.

## Pruebas mínimas

- [x] Import válido entre dos y tres archivos.
- [x] Símbolo existente pero no exportado.
- [x] Módulo inexistente.
- [x] Ciclo directo e indirecto.
- [x] Dos módulos con símbolos privados del mismo nombre.
- [x] Intento de ruta `../` fuera del proyecto.
- [x] El mismo módulo importado desde dos rutas lógicas se carga una vez.

## PRs sugeridos

1. `feat(modules): load project sources from a safe root`
2. `feat(modules): resolve exports and imports`
3. `feat(modules): detect dependency cycles`

## Criterios de aceptación

- El mismo proyecto produce el mismo grafo en cualquier sistema operativo soportado.
- Ningún import puede leer archivos fuera de la raíz permitida.
- Todos los errores de módulos son diagnósticos estructurados.
- El módulo loader pertenece al núcleo o a una capa de proyecto, no a componentes visuales.

## Fuera de alcance

Registro de paquetes, descarga de dependencias, versionado semántico de paquetes y ciclos de módulos permitidos.
