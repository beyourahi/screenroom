This directory is intentionally empty.
Blits' MSDF font generator reads `public/` for font files to convert, and fails if the
directory does not exist. Screenroom ships no font files yet: the app uses the
renderer's default sans-serif, and the private design system's font is not vendored.
When a font is added it goes here, and the generator will produce the SDF map.
