# ADR-0004: Sin scraping de plataformas que lo prohíben

- Estado: aceptada
- Fecha: 2026-11-13

## Decisión

SENTRI solo usa APIs oficiales, feeds RSS/Atom públicos o páginas cuyo acceso
automatizado está permitido. No se hace scraping de LinkedIn ni de X, no se usan
cookies de sesión de cuentas personales y no se automatizan inicios de sesión.

Para esas plataformas se ofrece: añadir enlaces manualmente (`sentri add`),
APIs oficiales cuando el usuario disponga de credenciales, y fuentes
alternativas que republican las mismas oportunidades.

Ver [docs/sources.md](../sources.md).
