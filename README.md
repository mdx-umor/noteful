# Notebook

A handwriting notebook for iPad and Apple Pencil, inspired by Noteful.
Live at https://notebook-v1-flame.vercel.app

Built with TypeScript and React. Notebooks are saved automatically in the
browser (IndexedDB).

## Run it

```sh
npm install
npm run dev      # local development
npm run build    # production build into dist/
```

Vercel builds and deploys the site from this repo (`npm run build`, output `dist`).

## Code layout

```
src/core/      document model, storage, ink engine, geometry, rendering,
               PDF export, photo/PDF import, audio, handwriting recognition
src/editor/    the writing surface (board.ts), document operations (doc.ts),
               editor screen, panels, sidebar, dialogs, zoom window, audio
src/library/   the library screen
docs/guide/    the design guide: build spec, guide book and screenshots
```

## Features

Pens (ballpoint, fountain, brush), highlighter, study tape, eraser (whole and
precise), lasso, shapes and shape gallery, text boxes, stickers, ruler, laser
pointer, layers, bookmarks, outline, links, photo and PDF import, image cropping,
PDF export, 17 templates, paper colours and sizes, zoom window, night mode,
study timer, audio recording linked to your writing, handwriting search and
convert to text, folders, nested tags, search, trash, automatic daily backups,
and backup download / restore.
