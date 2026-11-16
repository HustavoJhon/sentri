# ADR-0007: Correcciones manuales como capa separada

- Estado: aceptada
- Fecha: 2026-11-15

## Decisión

Separar detectado (snapshot JSON por oportunidad) y manual (override por
campo), materializando el valor efectivo en `opportunities`.

## Por qué no solo una tabla de overrides aplicada al leer

Los filtros y la búsqueda se hacen en SQL. Si el valor corregido solo existiera
al leer, una categoría corregida (p. ej. "beca") no aparecería en la vista
Scholarships. Materializar el efectivo mantiene SQL simple y correcto.

## Consecuencias

- Toda escritura de detectado pasa por `merge_detected` + recálculo.
- Restablecer = borrar override + recálculo.
- Overrides con valores inválidos (p. ej. de una versión futura) se ignoran con
  aviso, sin romper el ítem.
