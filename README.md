# noteful

**Notebook** — a handwriting notebook for iPad and Apple Pencil, inspired by Noteful.
Live at https://notebook-v1-flame.vercel.app

## The app (`app/`)

TypeScript + React web app. Pages scroll natively, each page is drawn on its own
canvas, and Apple Pencil input comes straight from Safari's pointer events
(pressure, coalesced points, palm rejection).

```sh
cd app
npm install
npm run dev      # local development
npm run build    # builds into notebook-v1/web/public (what Vercel serves)
```

Code layout:

```
app/src/core/     document model, storage (IndexedDB), ink engine, geometry,
                  rendering, PDF export, pictures/PDF import, shape presets
app/src/editor/   the writing surface (board.ts), document operations (doc.ts),
                  editor screen, panels, sidebar, dialogs
app/src/library/  the library screen
```

Features: pens (ballpoint, fountain, brush), highlighter, tape, eraser, lasso,
shapes and shape gallery, text boxes, stickers, ruler, laser pointer, layers,
bookmarks, outline, links, photos and PDF import, PDF export, templates, paper
colours and sizes, night mode, study timer, folders, nested tags, search, trash,
and backup / restore (Settings in the library).

Notebooks are saved automatically in the browser (IndexedDB). The storage format
is the same as the earlier Rust version, so existing notebooks open unchanged.

## Design guide (`docs/guide/`)

`BUILD-SPEC-for-Claude.md`, `Noteful-Guide-Book.html` and the reference screenshots
the app was built from.

## Earlier version (`notebook-v1/`)

The first version was written in Rust (egui, compiled to WebAssembly). It is kept
for reference; the web app no longer uses it. `notebook-v1/web/public` now holds
the build output of `app/`.
