---
name: episodic
description: Compact local episode production workbench
colors:
  workbench-teal: "#09695d"
  control-teal: "#087f5b"
  surface: "#ffffff"
  canvas: "#f7f8f8"
  text: "#000000"
  root-text: "#1f2933"
  muted-text: "#52616b"
  border: "#cbd2d5"
  divider: "#e0e5e5"
  control-border: "#ced4da"
  control-hover: "#f8f9fa"
typography:
  headline:
    fontFamily: "system-ui, sans-serif"
    fontSize: "1.625rem"
    fontWeight: 700
    lineHeight: 1.35
  title:
    fontFamily: "system-ui, sans-serif"
    fontSize: "1.375rem"
    fontWeight: 700
    lineHeight: 1.4
  body:
    fontFamily: "system-ui, sans-serif"
    fontSize: "1rem"
    lineHeight: 1.55
  label:
    fontFamily: "system-ui, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 500
    lineHeight: 1.55
  path:
    fontFamily: "monospace"
rounded:
  sm: "4px"
spacing:
  xs: "10px"
  sm: "12px"
  md: "16px"
  lg: "20px"
  page: "24px"
components:
  button-default:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.sm}"
    height: "36px"
    padding: "0 18px"
  button-default-hover:
    backgroundColor: "{colors.control-hover}"
  button-subtle:
    backgroundColor: "transparent"
    textColor: "{colors.control-teal}"
    rounded: "{rounded.sm}"
    height: "36px"
  episode-card:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.sm}"
    padding: "16px"
    width: "100%"
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.sm}"
    height: "36px"
---

<!-- SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd -->
<!-- SPDX-License-Identifier: CC-BY-SA-4.0 -->

# Design System: episodic

## Overview

**Creative North Star: "Production workbench"**

The existing browser workbench is compact and utilitarian. System typography, bordered episode buttons and familiar Mantine controls keep production records readable without decorative imagery. Teal identifies the product and some interactions; episode information carries the hierarchy.

This captures the current implementation, not a replacement identity. The descriptive direction was confirmed during documentation. Depth is recorded as observed, not imposed as a new doctrine. Sources are `gui/src/theme.ts`, `gui/src/style.css`, `gui/src/main.tsx` and the installed Mantine 9.8.2 defaults. Synthetic desktop, tablet, phone and reflow captures were regenerated with `python gui/e2e/smoke.py`; desktop, phone and episode captures were visually inspected. No UI code changed in this pass.

**Key Characteristics:**
- Compact controls and small corners.
- Plain text evidence, readable paths and restrained teal accents.
- A responsive lifecycle board with a separate inspection view.

## Colors

Functional names describe the implemented palette; the frontmatter owns exact values.

### Primary

- Workbench teal identifies the brand, episode hover borders and the global keyboard outline.
- Control teal comes from Mantine's teal palette at shade 9 for the subtle return control. It is not the same color as workbench teal; preserve this distinction when documenting current output.

### Neutral

- Surface and text are the effective Mantine light-mode body colors.
- Canvas and root text are declarations on the document root. Mantine overrides them on the body; canvas is visible beyond the body's content height. Do not treat root text as the observed body color.
- Muted text identifies dates, counts, artefact summaries and footer guidance.
- Border divides the shell and outlines episode cards; divider separates each episode's next action. Control border and control hover belong to Mantine's default controls.
- Status colours are authored tokens in `gui/src/style.css`: info (blue), warning (amber), danger (red) and accent (teal), each with a soft background. Stage headings, status labels and milestone marks use them, always alongside text or shape. Alerts retain Mantine's red and yellow variants. The read-only badge uses Mantine's outlined gray variant.
- YouTube red (#ff0000) appears only on the YouTube and YouTube Studio marks, which identify links to those services.

## Typography

Body and headings use the same system sans-serif stack. Filesystem paths use monospace and may break anywhere. No external font is loaded.

The headline role describes the Episodes and inspection headings. The channel heading is semantically h1 but visually uses the title role. Stage headings are smaller (15px); muted metadata and next-action text use 14px. Episode titles are bold within body-sized content. Mantine controls use their native small size rather than a bespoke type system.

## Layout

The shell has a wrapping header, padded main content and a ruled footer. Header padding is 20px vertically and 24px horizontally; main padding is 24px. Main has a minimum height of 65vh, not a sticky-footer layout. Header and toolbar controls wrap with 16px gaps. The search field has a 230px minimum width.

The lifecycle board uses four equal columns with a 20px gap, two columns at viewport widths of 1000px or less, and one at 560px or less. At the narrow breakpoint, main and header padding becomes 16px. Episode cards have 16px padding and 12px bottom spacing. Inspection content has a maximum width of 1200px. Tables sit inside Mantine scroll areas.

These are observations of the current board, not required compositions for every future surface.

## Elevation & Depth

Header, episode cards and inspection content have no authored shadows. White surfaces, fine borders and whitespace separate content. The routing modal uses Mantine's default overlay, transition and modal shadow. These library defaults remain library-owned; no custom elevation vocabulary has been established.

Reduced-motion preferences disable animations and transitions through the global stylesheet. No custom motion durations or easing tokens exist.

## Shapes

Episode cards and default Mantine controls have small corners, using the shared small radius. Cards use a single-pixel border; stage headings use a two-pixel bottom rule. The outlined read-only badge retains Mantine's pill shape. Episode thumbnails are the only imagery: real channel artwork shown at 16:9 on cards, in the episode view and in the frame-candidate grid. There is no illustration or decorative shape system.

## Components

### Buttons

Default bordered Mantine buttons provide Channel, Refresh and recorded external links. The return action uses the subtle variant. Controls retain native Mantine hover, disabled and loading treatments; Refresh shows its loading state. Lucide icons are 16px and accompany text. There is no filled primary button in the current UI.

### Episode cards

A card is an article whose title is the native button that opens the episode; the button's hit area is stretched over the card, so the whole card is clickable while the YouTube links remain separate controls (interactive elements are never nested). Content reads thumbnail (when one exists), date, bold title, optional guest, milestone strip, separated next action, milestone count and YouTube icon links. Long text wraps rather than truncating. Hover changes the border to workbench teal; keyboard focus on the title outlines the whole card. There is no selected-card or drag-and-drop state.

### Milestone marks

Ten-pixel marks in production order. State is carried by shape and fill as well as colour: filled square present, hollow square not started, diagonal half-fill possibly stale, red circle failing, dashed square not applicable. The strip has a text summary as its accessible name; the episode view lists every milestone with a text status.

### Brand icons

The YouTube and YouTube Studio marks come from Simple Icons and are used only as icon-only links with an accessible name and a tooltip, on cards, the episode heading and the channel header. Other link icons are Lucide.

### Search field

A labelled Mantine TextInput uses a search icon and the placeholder "Title, guest or date". Its label remains visible. Focus and disabled styling remain Mantine defaults; do not invent custom field error styling absent from the source.

### Navigation and focus

The header contains channel identity and workspace controls, not a sidebar or tab system. Inspection has an Episodes return button. A skip link appears on focus; opening an episode focuses its heading. The authored global focus-visible outline is 3px workbench teal with a 3px offset. Mantine may apply its own component-specific focus selectors as well.

### Badge, tables and modal

The gray outlined badge states "Read-only preview"; it is not a filter or status selector. Artefact and model-routing tables use plain Mantine rows, headers and dividers. The routing modal uses size xl and ordinary dismissal. Error alerts and empty-state prose report the available evidence without adding illustrations or new actions.

## Do's and Don'ts

### Do:

- Do retain the compact, utilitarian component character.
- Do use functional color names and distinguish authored tokens from Mantine defaults.
- Do preserve visible labels, native buttons and wrapping paths.
- Do keep stage, presence and verification meaning in text rather than color alone.

### Don't:

- Don't describe root declarations as effective body styles when Mantine overrides them.
- Don't invent a filled primary button, card shadow scale or custom animation system.
- Don't turn the current board layout or observed flatness into a global design doctrine.
- Don't present these synthetic browser checks as full accessibility conformance.
