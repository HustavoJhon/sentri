# ADR-0003: Relevancia basada en reglas explicables

- Estado: aceptada
- Fecha: 2026-11-13

## Decisión

La puntuación de una oportunidad es la suma de contribuciones de reglas
(intereses, palabras clave, ubicación, modalidad, deadline, beneficios,
afinidad aprendida de likes/guardados/descartes). Cada contribución guarda su
motivo, de forma que la TUI puede mostrar *por qué* se recomienda algo.

Los pesos son configurables en `config.toml`. Ninguna oportunidad se elimina
por puntuación baja: solo se ordena más abajo.

## Alternativas descartadas

Modelos de ML/embeddings: más opacos, requieren datos que todavía no existen.
Se podrán añadir como una regla más en el futuro.
