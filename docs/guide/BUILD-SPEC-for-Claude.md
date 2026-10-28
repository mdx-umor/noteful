# Noteful Clone — iPad Build Spec (for Claude)

> **How to use this file:** Give this file plus the `screenshots/` folder (and optionally `Noteful-Guide-Book.html`) to Claude and say:
> *"Build this iPad app in Swift/SwiftUI exactly as specified. Follow the build phases in order. Use the screenshots as the visual reference."*
>
> Screenshots were taken from Noteful's iPad app running on a Mac, so the screens are the real iPad UI. File names are referenced as `[NN]` → `screenshots/NN-*.jpg`.

---

## 0. Product summary

A handwriting-first notebook app for **iPad + Apple Pencil** (iPadOS 17+). Users keep a **library** of **notebooks** (organised by **folders** and nested **tags**). Each notebook is a stack of **pages**; each page has a **paper template** or is a page of an imported **PDF**. Users write with **pen / highlighter / tape**, erase, lasso-select, and add **text boxes, shapes, images, stickers and links**. Content sits on **layers**. Study features: **tape** that hides content until tapped, hidden answer layers, bookmarks, outlines, audio recording.

Target device: all iPads, landscape and portrait, Split View / Slide Over / Stage Manager, multiple windows. Optimise for 11" and 13" iPad Pro with Apple Pencil (2nd gen / Pro / USB-C). Must also be usable with a finger (setting) and with a hardware keyboard + trackpad.

The free product in this clone should include the "keep" Pro features listed in §14.

---

## 1. Tech stack (required)

| Area | Choice | Notes |
|---|---|---|
| UI shell | **SwiftUI** | `NavigationSplitView` for the library. |
| Drawing canvas | **Custom UIKit view** (`UIViewRepresentable`) rendering with **Metal** (or Core Graphics for v1) | Do **not** use PencilKit: it can't do tape, custom pen types, per-layer ink, lasso filters, or our stroke data model. |
| Pencil input | `UITouch` with `.force`, `.altitudeAngle`, `.azimuthAngle`, **coalesced** and **predicted** touches, `touch.type == .pencil` | Target < 20 ms ink latency. Draw predicted points as a temporary tail. |
| Pencil gestures | `UIPencilInteraction` (double-tap / squeeze), `UIHoverGestureRecognizer` (hover preview of nib) | Double-tap default: switch to eraser and back (setting). |
| PDF | **PDFKit** (`PDFDocument`, render pages to tiles) | Import, render as page background, export flattened PDF. |
| Storage | **SwiftData** (or Core Data) + files on disk for PDFs, images, audio, stroke blobs | Strokes stored as compact binary per page-layer. |
| Sync / backup | **CloudKit** private DB + iCloud Drive file backup | Free in this clone. |
| Handwriting OCR | **Vision** `VNRecognizeTextRequest` on rendered page images | For "Convert to Text" and handwriting search. |
| Audio | **AVFoundation** (`AVAudioRecorder` / `AVAudioPlayer`) | Needs `NSMicrophoneUsageDescription`. |
| Camera | `UIImagePickerController` / PhotosPicker | `NSCameraUsageDescription`, photo library usage. |
| Multi-window | `UIApplicationSupportsMultipleScenes = YES`, `WindowGroup` with notebook ID | "Open in New Window", "Move Tab to New Window". |

---

## 2. Visual style

- Dark UI chrome by default (toolbars/sidebars `#1C1C1E`-ish, secondary `#2C2C2E`), accent **purple** for toggles (`#9B59B6`-ish) and **blue** for selection/primary buttons (`#0A84FF`). See `[01]`, `[17]`.
- Paper default: warm cream `#FFFDF2` with grey rules. See `[16]`.
- Panels are **popovers** with rounded 16 pt corners, dark background, section headers in small grey caps, rows as rounded "cards". Toggles are pill switches (purple when on).
- "Pro" badge = small purple outlined label. (In this clone, remove Pro badges on features we make free.)
- SF Symbols for every icon.
- Themes: System / Light / Dark appearance; toolbar Match appearance / always Dark `[49]`.

---

## 3. App structure (screens)

1. **Library** — `NavigationSplitView` (sidebar + content). `[01]`
2. **New Notebook sheet** — `.sheet`, medium form. `[06]`
3. **Template picker** — full-screen cover. `[07]`–`[12]`
4. **Notebook editor** — full screen, with tab strip. `[16]`, `[52]`
5. **Page manager** — full-screen cover over editor. `[40]`
6. **Settings / About sheet** — opened from the logo. `[48]`
7. **Paywall** — not needed in the clone (or keep for a future "Pro").

---

## 4. Library `[01]–[05], [13]–[15]`

### 4.1 Sidebar (left column, ~280 pt)
- Top: app logo button "N ▾" → Settings/About sheet (§12).
- **Notebooks** (collapsible, chevron) → children: **Untagged** (count), **Unfiled** (count), **Trashed**.
- **Pinned** (count).
- **Folders** (tree; can nest; empty state illustration + "No notebooks" `[13]`).
- Divider, then **Tags** tree with `#` icons and counts, e.g. `# Sample 4` → `# Drawing 2`, `# Markup 1`. Selecting a parent tag shows all descendants. Selected row = rounded grey highlight.
- Swipe / long-press on folder or tag rows: Rename, Change colour, Delete.

### 4.2 Content area
- Large title of current filter, with `#` icon for tags (e.g. "Sample/Drawing").
- **Grid view** (default): portrait thumbnails ~90×120 pt with page-count badge, title below, tag chips below title. A notebook opened partway shows "p.6" after the name (last-opened page).
- **List view** `[03]`: small thumbnail, title, tag chips, relative time ("13 minutes ago"), trailing `•••` button.
- Toolbar (top right), in order: **Search**, **List/Grid toggle**, **Sort**, **Select**, **Quick Note**.
  - **Search** `[14]`: field "Find in library", ✕ to close. Live results grouped: *Document Names*, *Page Text (handwriting/OCR)*, *Tags*. Highlight matched text. Show "N Match(es) Found".
  - **Sort** menu `[02]`: Sort by Name / Date Modified (default, ✓) / Date Created; Zoom In / Zoom Out (thumbnail size, 3 steps). Pinch on grid also changes size.
  - **Select** `[15]`: circles on each item, ✓ Done button top-right, floating bottom bar: Share, Duplicate, Move, Edit Tags, Delete (disabled until something is selected).
  - **Quick Note** (pen icon): immediately creates a notebook titled `Notes on DD/MM/YY` using Quick Create defaults and opens it.
- **+** floating button (bottom right, 44 pt circle) → menu `[05]`: **Notebook** (opens §5), **Quick Create** (creates with defaults; ⚙ beside it edits the defaults), **Import File** (Files picker: PDF, images → new notebook).
- **Long-press a notebook** (or tap `•••`) → context menu `[04]`, header = notebook name: Rename · Add Cover · Duplicate Notebook · Tags · Pin · — · Move · Share · — · Open in New Window · — · Delete Notebook (destructive, moves to Trash).
- **Drag & drop**: drag notebooks onto folders/tags in the sidebar; drag a PDF from Files/other app into the library to import.
- **Trashed**: items show days-left; actions Restore / Delete Permanently; auto-purge after 30 days.

---

## 5. Creating a notebook `[06]–[12]`

### 5.1 New Notebook sheet `[06]`
- Header: ✕ (left), "New Notebook" (centre), **Create** (blue pill, right).
- Row 1: two preview tiles **Cover** and **Paper** (selected one has blue outline). Tapping switches the picker section below between Cover and Paper.
- Right of tiles: **Name** text field (placeholder "Notebook Name"; empty → "Untitled"), **Notebook Cover** toggle, **Tags** chips (pre-filled with the tag currently viewed; tap to edit).
- **Paper section** (or Cover section): horizontal row of 4 templates (Blank ✓, Ruled, Margin, Grid) + a `•••` circle button → full template picker.
- **Background Color**: 12 circular swatches (white, cream (default), dark grey, black, pastel red, pink, lilac, periwinkle, light blue, blue, mint, sage).
- **Paper Size** dropdown (A4 default, Letter, A5, Legal, Square, iPad screen) + Portrait/Landscape segmented control.

### 5.2 Template picker (full screen) `[07]–[12]`
- Top: ✕ (left), segmented **Explore | Saved** (centre), ✓ (blue, right).
- Title "Choose a Template". Filter row (paper only): **Color** ▾, **Size** ▾, **Orientation** toggle, **Line Spacing** ▾ (e.g. "16 (Medium)"), **Line Thickness** ▾ (Thin/Medium/Thick). Covers only show Size + Orientation.
- Scrolling sections, each a grid of tiles with labels:
  - Covers: **Classic** (17 solid colours with spine stripe), **Simple** (7), **Botanical** (4 floral with label box), **Composition** (5 marbled), **Pattern** (7+).
  - Paper: **Basics** (Blank, Ruled, Margin, Grid, Dot, Cornell, 2 Columns), **Cornell** (Cornell, Blank, Grid, Dot), **Ruled** (Ruled, Margin, Margin Right, Grid, Dot, 2 Columns, 3 Columns). *(Music / Storyboard / Handwriting are optional — see §14.)*
- Paper templates must be **generated procedurally** (vector drawing from parameters: line spacing, thickness, colour, margins) — not images — so they look sharp at any zoom.
- **Saved** tab `[12]`: user templates; empty state "No templates saved"; **+** → Import File (PDF page or image becomes a template). Also: any page can be saved as a template from the page menu.

---

## 6. Editor layout `[16]`, `[52]`

```
┌───────────────────────────────────────────────────────────────┐
│ ‹  [tab: Notes on 27/10/26 ✕][tab: Study Notes ✕ ▾]  ▯  ▭  •••│  ← top bar
├──┬────────────────────────────────────────────┬───────────────┤
│▦ │                                            │  Sidebar      │
│✎ │                                            │  (optional)   │
│🖍 │              PAGE CANVAS                   │  Overview /   │
│⌫ │        (vertical scroll of pages)          │  Bookmarks /  │
│◯ │                                            │  Outlines /   │
│✋ │                                            │  Layers       │
│🔦 │                                            │               │
│🎙 │                                            │               │
│★ │                                            │               │
│⊕ │                                        ▭ ⌕ │               │
│↶ │ [↶ Page 5] [88%] [2 of 10]                 │               │
│↷ │                                            │               │
│● │ ← colour favourites of the active tool     │               │
└──┴────────────────────────────────────────────┴───────────────┘
```

- **Top bar**: ‹ back to library · **tab strip** (one tab per open notebook; ✕ closes; active tab title has ▾ menu: Rename, Share, Open in New Window, Close) · **Sidebar toggle** · **View** button (§11) · **More `•••`** (§12).
- **Left toolbar** (vertical, 44 pt buttons, position configurable left/top/right/bottom): Page Manager ▦, Pen, Highlighter/Tape, Eraser, Lasso, Hand, Presentation, Audio, Stickers, Insert ⊕, Undo, Redo, then **colour favourites** for the active tool (circles). Active tool icon tinted blue.
  - **Tap an inactive tool → select it. Tap the active tool again → open its options popover** anchored to the button.
- **Bottom-right floating buttons**: Ruler, Zoom Window (configurable left/right/hidden).
- **Status chips bottom-left**: zoom % (e.g. "88%"), page "2 of 10" (tap → Go to page), and after following a link a **"↶ Page 5"** back chip.
- **Canvas**: pages stacked vertically with 12 pt gaps, centred, dark grey background around paper. Smooth 60/120 fps scrolling; tile-based rendering for PDFs/ink at high zoom. Zoom 25%–800%.
- **Pull-to-add page**: overscroll past the last page shows "Release to add page" → appends page with the same template.

### 6.1 Touch & Pencil rules (very important on iPad)
| Input | Action |
|---|---|
| Apple Pencil | Always draws with the active tool. |
| One finger | Scrolls/pans (unless "Draw with finger" is on). Tap on tape toggles it; tap on link follows it. |
| Two-finger pinch | Zoom; two-finger drag pans. |
| Two-finger tap | Undo (if enabled in Gesture Settings). |
| Three-finger tap | Redo. Alternative mode: three-finger swipe left/right = undo/redo. |
| Finger double-tap | Zoom to fit width (toggle). |
| Pencil double-tap / squeeze | Configurable: switch to eraser ↔ previous tool, show colour palette, or off. |
| Pencil hover (supported models) | Show nib preview dot with tool colour/size. |
| Long-press on an object/selection | Context menu (UIEditMenuInteraction / context menu). |
| Palm | Ignore non-pencil touches while the pencil is down; ignore large-radius touches. |

Hardware keyboard shortcuts (§13) must also work.

---

## 7. Writing tools

### 7.1 Pen `[17]–[19]`
Options popover, two tabs at the bottom (✎ tool | ≡ behaviour):

**Tab 1**
- Header "Pen Tool". Live **stroke preview** box.
- **Pen** types (3 circular icons): **Ballpoint** (uniform width, slight pressure), **Fountain** (strong pressure + slight tilt variation, default), **Brush/Felt** (tapered ends). Gear ⚙ → Settings sub-page.
- **Size**: presets 0.35 / 0.8 / 1.2 / 1.9 / 3.8 mm (selected = blue outline) + continuous slider with value label (0.1–10 mm).
- **Settings sub-page** `[18]` (‹ back): Writing Pressure (1–10, default 6), Min Width (0–100%, default 50%), Width Smoothing (0–100%, default 30%), Motion Smoothing (0–10, default 2; a stabiliser using a moving average / one-euro filter), **Reset Settings**. Each has a "?" info button.

**Tab 2** `[19]`
- *Draw and hold*: **Auto Shape** toggle — pause at the end of a stroke for `Delay` → replace stroke with the recognised line / arrow / rectangle / circle / ellipse / triangle / polygon; the shape stays editable (drag endpoints). **Scribble to Erase** toggle — zig-zag scribble then hold → erase strokes under it. **Delay** slider (0.2–1.5 s, default 0.5 s). ▶ buttons show short demo videos.
- *Pen Pressure*: **Enhanced Sensitivity** (keep updating stroke width as late pressure data arrives — use `touchesEstimatedPropertiesUpdated`), **Draw on Hover** (allow drawing with no pressure, e.g. finger/trackpad).

**Colour**: toolbar shows favourites (unlimited in this clone; default 3: navy, ...). Tap a favourite = use; tap the active one = colour picker popover (swatch grid + spectrum + hex + opacity + "Add to favourites"). Long-press a favourite → Edit / Delete. Up to 9 **Pen favourites** (colour + size + type) mapped to keys 1–9.

### 7.2 Highlighter `[20]`
- Segmented header **Highlighter | Tape**.
- Size presets 1.9 / 2.7 / 4.6 / 5.4 / 6.2 mm + slider. **Draw Straight Lines** toggle (and auto-straighten when held).
- Rendered **semi-transparent, multiply blend, below pen ink** of the same layer; overlapping parts of one stroke don't darken.
- Separate colour favourites (default yellow, green, purple).

### 7.3 Tape (study tool) `[21]`
- **Tape Type**: straight line, rectangle, freehand.
- **Size**: 5 presets. **Pattern**: 4 striped pastel textures (peach, pink, lilac, blue).
- Tape is an object drawn over content and is **opaque**. **Tap with finger or Pencil → toggles revealed** (revealed = faint outline only). Tip banner: "Use Tape to aid memorization. Cover your notes with tape, then test yourself."
- Buttons **Cover All** / **Reveal All** (current page).

### 7.4 Eraser `[22]`
- Header: **Reset** | "Eraser Tool". 5 sizes.
- Options: **Erase Whole Line** (stroke eraser: touching a stroke removes it entirely) — default ON in clone; **Erase Highlight Only**; **Auto-select Previous Tool** (after lifting the pencil, return to previous tool); **Erase Shapes**; **Erase Tapes**.
- Pixel/partial eraser must split strokes cleanly.

### 7.5 Lasso `[23]–[26]`
- Header: Reset | "Lasso Tool". **Shape**: freeform | rectangle.
- **Object Selection** toggles: Handwriting, Highlights, Text Boxes, Images, Shapes, Tapes (all on by default).
- Selection shows dashed blue outline `[24]`; drag inside to move; corner handle to resize; rotate handle.
- Tap / long-press the selection → menu `[25]`: Cut · Copy · Duplicate · Delete · — · **Resize** · **Style** · **Take Screenshot** (image to clipboard/share) · **Convert to Text** (Vision OCR → text box) · — · **Save** (save as Sticker) · — · **Group** / Ungroup · — · **Arrange ›** (Bring to Front, Send to Back) · **Move to Layer… ›** (list of layers).
- **Style** popover `[26]` "Stroke Style": Color swatches, Stroke style (solid / dashed / dotted), Pen Size presets.

### 7.6 Hand
- Pan/scroll only; follows links; toggles tapes. Useful for finger users.

### 7.7 Presentation / laser `[27]`
- Pointer style colours: red, blue, green, purple. **Draw Lines** toggle (laser ink that fades after ~1.5 s).
- Display options (when an external display / AirPlay is connected): **Screen Mirroring** or **Hide Toolbar UI** (external screen shows only the page).

### 7.8 Audio recording
- Mic button starts/stops recording; a small floating recorder bar (time, pause, stop). Recordings list per notebook (play, rename, delete, share).
- **Improvement**: timestamp every stroke while recording so tapping ink during playback seeks audio (see §15).

### 7.9 Stickers `[28]`
- Popover "Recent Stickers" with ✕. Empty state: "No stickers — Use Lasso tool to select objects then choose 'Save' to create a reusable sticker." + **Install Samples** button (bundled pack incl. "?" quiz stickers, arrows, stars, bulbs).
- Tap a sticker → drops it in the centre of the visible page as a movable object.

---

## 8. Insert menu and objects `[29]–[35]`

Insert ⊕ popover `[29]` (🔧 = Customize Toolbar): **Tags** (tag current page) · **Page** (§9 Insert Page) · **Image** (Photos) · **Textbox** · **Shape** · **Sticker** · **Import File** (PDF/image) · — · **Take Photo** (camera).

### 8.1 Text boxes `[35]`
- Text tool (T) or Insert → Textbox: tap the page to place a box and start typing (software keyboard or Scribble handwriting-to-text). Box has 8 handles and a ⊕ move handle; width fixed, height grows.
- Formatting bar docked above the keyboard / bottom: Font (Helvetica default) · Size (16) · **B** *I* U S · **Aa** (styles: Title, Heading, Body, Caption) · text colour · box fill colour · align left/centre/right · line spacing · vertical align top/middle/bottom · bulleted list · numbered list · outdent / indent.

### 8.2 Shapes `[30]–[33]`
- Insert Shape grid: square, rounded square, pill, circle, diamond, triangle, hexagon, cloud, burst, star, arrow, curved arrow. Inserted in the centre with a default fill (yellow), with handles.
- Shape long-press menu `[32]`: Cut, Copy, Duplicate, Delete, Resize, Style, **Link**, Save, Arrange ›, Move to Layer ›.
- **Shape Style** `[31]`: Color (swatches + picker), Style (filled / tinted fill / outline / dashed / dotted), Border Size (none / thin / medium / thick), Corner Radius slider, Opacity slider (0–100).

### 8.3 Links `[33]`
- "Link Settings" popover: type picker **Web page | Notebook**. Web page → URL field with paste button. Notebook → pick notebook + page. **Remove Link** (red).
- Linked objects: tap (Hand tool or finger) to follow. Internal jumps push to a back stack shown as "↶ Page N" chip.
- Also: **Copy Page Link** (More menu) creates a deep link `myapp://notebook/<id>/page/<id>`.

### 8.4 Images
- Move, resize (aspect lock), rotate, **crop** (free in clone), opacity, Arrange, Move to Layer, Link.

---

## 9. Page management `[40], [41], [57]`

- **Page Manager** (▦ button): full-screen grid of page thumbnails with numbers. Top: ✕, undo/redo, title, **Select**. Action row: **Insert · Cut · Copy · Paste · Rotate · Tag · Delete · Extract · Share**. Drag thumbnails to reorder (multi-select drag supported). Pinch to resize thumbnails.
- **Insert Page** popover `[41]`: "Insert template: **After current page** ▾" (Before current / After current / At end), Cover | Paper tabs, template row (Current, Blank, Ruled, … `•••`). Other options: **Import File**, **Merge Notebook…** (append all pages of another notebook), **Choose Photo** (photo as a page).
- **Thumbnail ▾ menu** `[57]`: Insert Page, Cut, Copy, Paste, Rotate, Duplicate, Clear Page, Delete.
- **Extract**: selected pages → new notebook or PDF/share.
- **Expand & Resize Page**: add blank margin around a page (great for annotating PDFs) — drag edges or pick presets.

---

## 10. Notebook sidebar `[42]–[44], [53]–[55]`

Right-hand panel (~300 pt, can also be an **overlay** instead of pushing content). Header: `•••` (Show Recently Deleted · Show in: Overlay / Sidebar), title, **+**. Segmented tabs:

1. **Overview** `[55]`: 2-column page thumbnails, page number with ▾ menu (§9), tag chips under pages, red bookmark ribbon on bookmarked pages, current page outlined blue. Tap to jump. Drag to reorder.
2. **Bookmarks** `[54]`: red ribbon icon, editable title, page number, `•••` (Rename, Delete). **+** bookmarks current page.
3. **Outlines** `[53]`: tree of titled entries with page numbers; current section bold; nested with disclosure chevron; `•••` (Rename, Indent, Outdent, Delete). **+** adds entry for current page. On PDF import, import the PDF's own outline.
4. **Layers** `[43]`, `[52]`: per page. Each row: thumbnail, name, **active-layer pen icon**, **lock** 🔒, **visibility** 👁, `•••` (Rename, Duplicate, Merge Down, Delete). Drag to reorder. **+** adds layer. Unlimited in clone. New strokes go to the active layer; locked layers can't be edited; hidden layers aren't rendered (used for "answer" layers).

---

## 11. View panel, ruler, zoom window `[45]–[47]`

**View** popover `[45]`:
- **Page View**: Vertical (continuous scroll) | Horizontal (paged swipe) | Two Pages (side-by-side spreads, landscape).
- **Toolbar Position**: left | top | right | bottom.
- **Tab Bar Position**: top | bottom.
- **Keep iPad Awake** (`isIdleTimerDisabled`), **Night Mode** (invert paper to dark, keep ink colours readable; ⚙ for options), **Status Bar** (show/hide iPadOS status bar).
- **Quick Access**: Ruler, Zoom Window, **Timer** (floating study timer/stopwatch) toggles — controls which floating buttons are shown.

**Ruler** `[46]`: semi-transparent grey straight-edge with cm (or inch) ticks on both edges and a centre angle dial (0°). Move with one finger, rotate with two fingers (snaps at 0/15/30/45/90°). Pen/highlighter strokes that start near an edge snap to it.

**Zoom Window** `[47]`: bottom docked magnifier (~30% of screen height). A blue target rectangle on the page shows where ink will land; writing in the magnifier draws there at small scale. Controls: ⚙ settings (zoom level, auto-advance margin), ← → ↑ ↓ to move the target, ✕ close. A shaded right-margin region auto-advances the target to the next line when the pen enters it. Resize target by its corner handle.

---

## 12. More menu & settings `[36]–[39], [48]–[50]`

**More `•••`** popover `[36]`:
- Button row: **Paper** (change template / colour / size of current page or all pages), **Share** (Export: PDF flattened / PDF with editable notebook / images; Print), **Find** (search in notebook: typed text, PDF text, OCR handwriting; results list with page numbers), **1 – 1** (Go to page).
- *Page*: Pin Page N, Expand & Resize Page, Copy Page Link, Clear Page (confirm).
- *Settings*: Gesture Settings ›, Document Editing ›, Customize Toolbar.

**Gesture Settings** `[37]`: Undo & Redo = Two Finger Tap ("two finger tap to undo, three finger tap to redo") | Three Finger Swipe | None (✓). Zoom to Fit Width: Finger Double Tap toggle. *Add in clone*: Draw with finger toggle, Pencil double-tap/squeeze action.

**Document Editing** `[38]`: Pull-to-add Page toggle · Page Number Indicator: Display (Bottom left / Bottom right / Top / Hidden), Zoom Percentage toggle · Zoom & Ruler Buttons: Display (Show on right / left / Hidden) · Ruler Units (Centimeters / Inches) · Apple Pencil: Hover Effect toggle ("Only available on some Apple Pencil models").

**Customize Toolbar** `[39]`: list with toggle + drag handle for: Pen, Highlighter, Tape, Eraser, Lasso, Hand, Presentation, Audio Recordings, Text, Page, Image, Shape, Stickers, Take Photo, Insert. **Reset**, ✓ done. Defaults: Text/Page/Image/Shape/Take Photo/Tape OFF (reachable via Insert).

**Settings / About sheet** `[48]` (logo button): app banner · (Upgrade row — omit) · About: Rate on App Store, Send Feedback (mail composer) · **Notebook Templates** › (manage saved templates, Quick Create defaults) · Settings: **Themes** › `[49]`, **Cloud Sync** › `[50]` (Use iCloud toggle, description, "Last Sync: …", Diagnostics), **Document Editing** ›, **Gesture Settings** ›. *Add in clone*: **Backup** (auto-backup to iCloud Drive/Files daily, keep N copies, Restore), **Pencil** settings, **Default paper**.

---

## 13. Keyboard shortcuts (hardware keyboard on iPad) `[34]`

| Menu | Items |
|---|---|
| File | New Notebook ⌘N, New Window ⌥⌘N, Move Tab to New Window, Open Library, Close Notebook ⌘W, Close All |
| Edit | Undo ⌘Z, Redo ⇧⌘Z, Cut/Copy/Paste/Delete, Find… ⌘F, Insert Textbox ⇧⌘T, Insert Image ⇧⌘I, Select Previous Layer `[`, Select Next Layer `]` |
| Tools | Pen **F**, Highlighter **G**, Eraser **E**, Lasso **A**, Hand **H**, Text **T**, Pen Favorite 1–9 → **1–9** |
| View | Zoom to 100% ⌘0, Zoom In ⌘+, Zoom Out ⌘−, Zoom to Fit, Full Screen |
| Navigate | Page Up/Down, Scroll to Top/Bottom, Go to Page… ⌘G, Back ⌘[, Forward ⌘], Previous/Next Tab ⇧⌘[ / ⇧⌘], First/Last Tab |

Implement with SwiftUI `.commands` / `UIKeyCommand` so they show in the iPad ⌘-hold overlay.

---

## 14. Premium features — what to include

Noteful's Pro list `[51]`: Sync with iCloud, Unlimited Notebooks, >3 Colour Pickers, Premium Templates, Unlimited Layers, Bookmarks, Outlines, Auto-Backup, Premium Shapes, Image Cropping, Advanced Lasso, Advanced Eraser.

**Include (free):** iCloud sync, auto-backup, unlimited notebooks, unlimited colours, bookmarks, outlines, unlimited layers, stroke eraser + highlight-only eraser, lasso object filters, image cropping, Dot / Cornell / 2–3-column templates, custom template import.

**Skip for v1:** music / guitar TAB / storyboard / handwriting-practice templates, extra decorative cover packs (keep Classic solid colours + "use image as cover"), premium shapes beyond the basic set (keep triangle; drop hexagon/cloud/burst/star), tab-bar position option, "auto-select previous tool".

---

## 15. Improvements over Noteful (build after parity)

1. Tool options in one tap: long-press a tool = options; single tap always selects.
2. Typing directly: tap with finger on empty page in Text mode creates a box; Apple Pencil **Scribble** works in text boxes.
3. Handwriting search in library and notebooks (background Vision OCR index).
4. Audio ↔ ink sync (tap ink to seek recording).
5. Study mode: turn tapes, "?" stickers and hidden layers into flashcards with spaced repetition.
6. Convert handwriting to math (LaTeX) and diagrams.
7. AI helpers (summarise page, make flashcards, explain highlighted PDF text) — optional, via the Claude API.
8. Share link / collaboration (later, needs a backend).

---

## 16. Data model

```swift
@Model class Folder   { id: UUID; name: String; parent: Folder?; createdAt: Date }
@Model class Tag      { id: UUID; path: String /* "Sample/Drawing" */; colorHex: String? }
@Model class Notebook { id: UUID; title: String; folder: Folder?; tags: [Tag]; cover: CoverSpec?;
                        isPinned: Bool; lastOpenedPageIndex: Int; createdAt: Date; updatedAt: Date; trashedAt: Date?;
                        pages: [Page] /* ordered */; bookmarks: [Bookmark]; outline: [OutlineItem]; recordings: [AudioRecording] }
@Model class Page     { id: UUID; index: Int; background: PageBackground /* .template(TemplateSpec) | .pdf(fileID, pageNo) | .image(fileID) */;
                        size: CGSize /* points */; rotation: Int; extraMargins: EdgeInsets; tags: [Tag]; isPinned: Bool; layers: [Layer] }
@Model class Layer    { id: UUID; name: String; order: Int; isVisible: Bool; isLocked: Bool; inkBlob: Data /* strokes */; objects: [CanvasObject] }
struct Stroke         { id: UUID; tool: .pen|.highlighter; penType: .ballpoint|.fountain|.brush; colorRGBA: UInt32; baseWidth: Float;
                        dash: .solid|.dashed|.dotted; points: [InkPoint] /* x,y,force,altitude,azimuth,timeOffset */; createdAt: Date; audioTime: Double? }
@Model class CanvasObject { id: UUID; kind: .text|.shape|.image|.sticker|.tape; frame: CGRect; rotation: Double; z: Int;
                        style: Data /* JSON per kind */; content: Data; link: LinkTarget? /* .url(String) | .page(notebookID, pageID) */; isRevealed: Bool /* tape */ }
struct TemplateSpec   { kind: .blank|.ruled|.margin|.marginRight|.grid|.dot|.cornell|.columns(n); paperColor; lineColor; lineSpacing; lineThickness; orientation }
@Model class Bookmark { id; title; page: Page }
@Model class OutlineItem { id; title; page: Page; parent: OutlineItem?; order: Int }
@Model class Sticker  { id; name; payload: Data; thumbnail: Data; lastUsed: Date }
@Model class AudioRecording { id; fileID; duration; startedAt; title }
@Model class SavedTemplate { id; name; kind: .cover|.paper; source: .pdfPage(fileID, n) | .image(fileID) | .spec(TemplateSpec) }
```
Undo/redo: per-notebook command stack (`UndoManager`) covering strokes, objects, page ops, layer ops.

---

## 17. Build phases (do in order; each phase must run on an iPad simulator)

1. **Project & library**: SwiftUI app, SwiftData models, library sidebar (Notebooks/Untagged/Unfiled/Trashed/Pinned/Folders/Tags), grid & list, sort, search by name, create (Quick Create) / rename / duplicate / trash / restore, context menus.
2. **Editor shell**: tabs, vertical page scroll, pinch-zoom, procedural templates (blank/ruled/margin/grid/dot), status chips, pull-to-add page.
3. **Ink engine**: Pencil input (coalesced + predicted), fountain/ballpoint/brush rendering, pressure & smoothing settings, colour favourites, undo/redo, palm rejection, finger-scroll. **Latency and smoothness are the top priority.**
4. **Highlighter + eraser** (stroke + partial), scribble-to-erase, auto-shape.
5. **Lasso**: freeform/rect, filters, move/resize/rotate, menu actions, style, copy/paste between pages/notebooks.
6. **PDF**: import (Files, drag & drop, share sheet), render, annotate, export flattened PDF, Share.
7. **Page manager**: grid, reorder, insert (position + template), duplicate, rotate, delete, extract, merge notebook.
8. **Objects**: text boxes + format bar, shapes + style, images (+crop), links (web + internal with back chip), camera.
9. **New Notebook sheet + template picker** (covers + paper, filters, saved/imported templates).
10. **Sidebar**: Overview, Bookmarks, Outlines (incl. PDF outline import), Layers.
11. **Study tools**: tape (cover/reveal all), stickers (save from lasso, sample pack), hidden layers.
12. **View options**: page view modes, toolbar position, night mode, keep awake, ruler, zoom window, timer, presentation/laser + external display.
13. **Settings**: themes, gestures, document editing, customize toolbar, Pencil double-tap/squeeze, hover.
14. **Sync & backup**: CloudKit sync, auto-backup + restore.
15. **Search & OCR**: Vision OCR index, library + in-notebook find, Convert to Text.
16. **Audio** recording + ink sync.
17. Polish: multiple windows, keyboard shortcuts, accessibility (VoiceOver labels, Dynamic Type in chrome), empty states, onboarding sample notebook.

## 18. Acceptance checklist (test on iPad)
- [ ] Writing with Pencil feels instant; no gaps at fast speed; pressure changes width on fountain pen.
- [ ] Fingers scroll/zoom but never draw (unless "Draw with finger" is on); palm on screen leaves no marks.
- [ ] Two-finger tap undoes; three-finger tap redoes (when enabled).
- [ ] Highlighter sits under ink and doesn't darken where a stroke overlaps itself.
- [ ] Tape hides content; tap toggles; Cover All / Reveal All work.
- [ ] Lasso with "Handwriting only" selects ink but not the PDF/images under it.
- [ ] Import a 200-page PDF; scroll stays smooth; annotations export into the PDF.
- [ ] Layers: hide "answers" layer → its ink disappears; locked layer can't be erased.
- [ ] Internal link jumps to a page and the "↶ Page N" chip returns.
- [ ] Works in portrait, landscape, Split View and Stage Manager; two notebooks in two windows.
- [ ] Data survives app kill; iCloud sync brings a notebook to a second iPad.

---

## Screenshot index
01 library home · 02 sort menu · 03 list view · 04 notebook menu · 05 + menu · 06 new notebook · 07–08 cover templates · 09–11 paper templates · 12 saved templates · 13 folders · 14 search · 15 select mode · 16 editor · 17 pen · 18 pen settings · 19 pen gestures · 20 highlighter · 21 tape · 22 eraser · 23 lasso options · 24 lasso selection · 25 lasso menu · 26 stroke style · 27 presentation · 28 stickers · 29 insert · 30 shapes · 31 shape style · 32 shape menu · 33 links · 34 tool shortcuts · 35 text format bar · 36 more menu · 37 gestures · 38 document editing · 39 customize toolbar · 40 page manager · 41 insert page · 42 sidebar overview · 43 layers · 44 sidebar menu · 45 view panel · 46 ruler · 47 zoom window · 48 settings · 49 themes · 50 cloud sync · 51 pro list · 52 PDF markup + tabs + layers · 53 outlines · 54 bookmarks · 55 overview thumbnails · 56 page jump + quiz stickers · 57 thumbnail menu
