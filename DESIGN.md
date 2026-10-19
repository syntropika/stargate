---
name: Stargate
description: Warm editorial typography and ruled dark fields for embedded authentication.
colors:
  ground: "#13120e"
  surface: "#191813"
  raised: "#201f19"
  ink: "#f4f0e8"
  body: "#d9d4c9"
  muted: "#aaa79c"
  line: "#323127"
  accent: "#c4a882"
  input-border: "#686251"
  success: "#a6c7a4"
  active-field: "#1d241a"
  active-border: "#424c37"
  code-ground: "#10100c"
  code-ink: "#ded9cb"
  code-string: "#b8c6a5"
  primary-hover: "#d9d4c8"
typography:
  display:
    fontFamily: "Stargate Serif, Georgia, serif"
    fontSize: "clamp(3.25rem, 5.7vw, 5.125rem)"
    fontWeight: 400
    lineHeight: 0.99
    letterSpacing: "-0.035em"
  headline:
    fontFamily: "Stargate Serif, Georgia, serif"
    fontSize: "clamp(2.25rem, 3.5vw, 3.25rem)"
    fontWeight: 400
    lineHeight: 1.1
    letterSpacing: "-0.035em"
  account-title:
    fontFamily: "Stargate Serif, Georgia, serif"
    fontSize: "3.1875rem"
    fontWeight: 400
    lineHeight: 1.1
    letterSpacing: "-0.035em"
  account-section:
    fontFamily: "Stargate Serif, Georgia, serif"
    fontSize: "1.875rem"
    fontWeight: 400
    lineHeight: 1.2
  body:
    fontFamily: "Stargate Sans, system-ui, sans-serif"
    fontSize: "1rem"
    fontWeight: 400
    lineHeight: 1.65
  supporting:
    fontFamily: "Stargate Sans, system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: 1.65
  action:
    fontFamily: "Stargate Sans, system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 500
    lineHeight: 1.4
  code:
    fontFamily: "Stargate Mono, monospace"
    fontSize: "0.75rem"
    fontWeight: 400
    lineHeight: 1.9
rounded:
  control: "2px"
spacing:
  compact: "0.5rem"
  control-gap: "0.75rem"
  group: "1.25rem"
  section-gap: "2rem"
  account-gutter: "2.5rem"
  editorial-gutter: "3.5rem"
components:
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.ground}"
    rounded: "{rounded.control}"
    typography: "{typography.action}"
    padding: "0.6875rem 1.0625rem"
  button-primary-hover:
    backgroundColor: "{colors.primary-hover}"
  button-public-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "0.6875rem 1.0625rem"
  button-outline:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    typography: "{typography.action}"
    padding: "0.6875rem 1.0625rem"
  field:
    backgroundColor: "{colors.ground}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "0.625rem 0.75rem"
  status-active:
    backgroundColor: "{colors.active-field}"
    textColor: "{colors.success}"
    padding: "0.25rem 0.5rem"
  code-window:
    backgroundColor: "{colors.code-ground}"
    textColor: "{colors.code-ink}"
  secret-panel:
    rounded: "{rounded.control}"
    padding: "1.5rem"
---

# Design System: Stargate

## Overview

**Creative North Star: "The Ruled Developer Editorial"**

Stargate is a ruled developer editorial: warm charcoal fields, ivory text, muted bronze details, and expressive serif headings. The public website gives an embedded authentication library a clear, spacious introduction; the account panel uses the same materials for precise credential management.

The portal-and-star geometry is the binding visual asset. Typography and hairline divisions carry the interface, while animation remains an optional expression of the mark. The account panel is embeddable: host-provided names, logos, accent colors, route prefixes, and stylesheets remain supported rather than being replaced by fixed Stargate branding.

**Key Characteristics:**
- Warm dark fields with fine structural rules.
- Locally served variable serif, interface, and monospace families.
- Restrained rectangular controls and compact credential registers.
- Optional ASCII brand motion with accessible pause and a static fallback.
- Responsive account records that retain explicit labels.

## Colors

The palette is warm and subdued. The normative frontmatter records the shared default values; host theme overrides remain authoritative in the embedded panel.

### Primary

- **Muted Bronze:** links, focus, active navigation details, shell prompts, and the Stargate mark. The account implementation exposes this as the configurable `--accent` custom property.

### Neutral

- **Warm Ground:** page background and recessed account fields.
- **Dark Surface / Raised Field:** nested forms, table headers, active filters, and icon tiles.
- **Ivory Ink:** display text, primary text, and filled primary actions.
- **Body Stone / Muted Stone:** supporting prose, secondary metadata, and field hints.
- **Olive Hairline / Input Stroke:** subtle structural rules and a stronger stroke around editable controls.
- **Code Ground / Code Ink:** recessed source examples and their readable foreground.

Success green and its dark field and border identify active credentials. Status also has a literal label. Muted green distinguishes code strings; bronze marks source keywords. The public and account stylesheets use separate custom-property names for the same shared palette roles, allowing independent embedding without separate visual identities.

**The Quiet Accent Rule.** Use the accent for focus, active navigation details, links, and brand marks; keep primary action fills and major text ivory. Respect a host-provided accent in the embedded panel.

## Typography

**Display Font:** locally served variable EB Garamond (400–800), exposed as `Stargate Serif`, with Georgia and serif fallbacks.
**Body Font:** locally served variable Geist (100–900), exposed as `Stargate Sans`, with system UI and sans-serif fallbacks.
**Label/Mono Font:** locally served variable JetBrains Mono (100–800), exposed as `Stargate Mono`, with a monospace fallback.

The serif supplies warmth and a clear task hierarchy. Geist is the neutral operational voice; medium action labels use an actual variable weight. Monospace gives source and credential values a separate, literal texture. Font files ship with the application rather than loading from a remote font service.

### Hierarchy

- **Display:** the public offer, using the responsive display role and tight negative tracking.
- **Headline:** public section statements; document headings also use the serif with a more generous line height.
- **Account Title / Account Section:** task headings and subordinate sections. Account tertiary headings use the same serif at a smaller size (1.375rem).
- **Body:** the public and guide reading baseline. Public hero support is slightly smaller (0.9375rem); account support uses the supporting role. Guide prose uses a roomier line height (1.8) and a reading measure (75ch).
- **Action:** compact medium-weight labels in primary and secondary account actions.
- **Code:** source examples and credential values. Scope chips and credential prefixes use smaller local sizes; these do not establish a display or heading role.

**The Three Roles Rule.** Use EB Garamond for editorial and task headings, Geist for interface text, and JetBrains Mono for code and credential values. Do not use the code face as the display voice.

## Layout

The public frame is centered with a maximum width (82.5rem) and fine side rules. Generous editorial gutters give headings and source examples room. The document reserves scrollbar space so the centered frame stays in place when navigating between short and long pages. The landing hero splits into text and brand artwork; installation controls and factual runtime information form a ruled rail. Below it, text sections, capability columns, and a divided code example retain the same alignment. These are page compositions, not requirements for every future surface.

The authenticated panel uses a persistent desktop sidebar (13.5rem / 216px), a breadcrumb bar, and a bounded workspace (88rem). It narrows the sidebar to 11.25rem at the medium viewport breakpoint. At the small viewport breakpoint, navigation becomes a compact horizontal strip and content becomes a single column. Anonymous sign-in uses a centered narrow shell rather than an empty authenticated sidebar.

Component behavior follows the account content container: forms expand to three columns at 48rem, then to a wider row with adjacent actions at 70rem. Below 48rem, the credential and user registers become stacked records with visible field labels while retaining semantic table headings. This preserves scope, expiration, status, and actions without requiring a wide desktop table.

Documentation shares the same public header and centered frame. A separate horizontal guide navigation sits below the header, followed by a centered article (75ch maximum) and previous/next guide links. Serif section headings and fine top rules organize the reading flow. Code blocks and tables scroll within their own bounds and are keyboard focusable.

The public landing stacks at 47.5rem and reduces gutters. Its intermediate adjustment is at 68.75rem. Shared header and guide navigation respond at 60rem and 45rem. On small screens, documentation uses a separate guide-navigation disclosure below the shared header; the global header retains the same structure and responsive behavior as the landing page. Spacing scales from compact inline gaps to larger page gutters instead of adding floating card margins everywhere.

## Elevation & Depth

The interface has no drop-shadow vocabulary. Subtle tonal changes, one-pixel rules, and stronger input borders establish structure. Forms use a flat dark surface, code uses a recessed field, and one-time secrets use an accent perimeter. Active navigation and filters use tonal fill rather than lift.

**The Ruled Field Rule.** Create structure with hairlines and tonal fields. Containers stay flat; drop shadows are not part of the current system.

## Shapes

Controls use nearly square corners with the shared small radius. Registers, scope tags, code windows, and structural divisions remain square. Circles belong to avatars, status dots, and small source-window indicators. Icons use consistent thin strokes in inline SVG, with the account set sharing an 18-unit view box. The brand retains its existing portal-and-star silhouette.

## Components

### Buttons

Primary actions are ivory rectangles with dark text. Public secondary actions use a dark surface; account secondary actions are transparent with the stronger input stroke. Actions have compact padding and a minimum height (2.75rem). Public hover changes fill; account hover dims brightness. Active presses translate down (1px). Disabled account actions dim and lose the pointer cursor.

Keyboard focus uses the configurable accent outline (2px) with a clear offset (4px). Copy controls and icon buttons retain a usable minimum target rather than shrinking to their SVG dimensions.

### Chips

Active status tags pair green text and a small dot with a subtle green fill and border. Other states remain literal text labels in the neutral treatment. Scope chips use a fine square border and compact monospace text, and wrap when necessary. They represent values rather than decorative badges above headings.

### Cards / Containers

Content is organized through ruled registers and flat fields. Code windows have a recessed background, divided header and footer, and readable wrapped source. The public integration example has Rust, Node, Python, and Go tabs synchronized with the installation selector. Filename, source, and guide link follow the selected language. Overlapping grid panels reserve the largest example height at the current width; inactive panels stay invisible and unavailable to keyboard and assistive technology. Both tab groups support arrow keys, Home, and End. The account creation section uses a dark surface and a thin border. The one-time secret panel uses an accent perimeter and a recessed code block, with explicit copy and dismiss actions.

### Inputs / Fields

Inputs and selects use the warm ground, stronger olive stroke, small radius, and the shared minimum control height. Labels sit above fields; muted hints sit below. Search is a flat outlined rectangle with an inline icon and transparent input. The accent supplies caret and keyboard focus. Keep explicit required labels, busy state, native validation, and readable error output when extending forms.

### Navigation

The public header combines the portal lockup, Product, Integrations, Documentation, and GitHub. Landing and documentation pages use this same header; links from guides return to the relevant landing-page sections. Guide navigation forms a separate horizontal strip with a bronze underline for the current page. On small screens, its own Documentation disclosure opens a vertical link list with a tonal current item; Escape closes it and returns focus to the disclosure. The selected guide supplies location context; previous/next links continue the article navigation. Internal page links use Astro client navigation so the document and loaded fonts stay in place. Only the guide article crossfades: a 100ms exit and 160ms entrance with cubic-bezier(0.23, 1, 0.32, 1); the shared shell stays visually stationary. The transition overlay does not intercept pointer input. Reduced-motion preferences suppress the transition, and browsers without view-transition support swap the content directly. Page controls initialize once per body and dispose listeners and animation observers before a swap. The selected integration language stays shared in memory when returning to the landing page. Account navigation combines labeled icons with a tonal selected row and accent icon. At small widths, account navigation scrolls horizontally and sign-out remains available. The panel's app name, optional logo, and route destinations come from host configuration.

### Credential Register

Desktop rows align name and prefix, scopes, expiration, status, and action. Above the register, data-derived counts, pressed-state filters, and search support scanning. Narrow containers turn each row into a labeled record. Loading uses static tonal lines and a text status; failures expose recovery text; empty states explain the next useful action. A newly created secret is shown once with explicit copy and dismissal.

### ASCII Portal

The public mark is rendered as a glyph field sampled from the original SVG, with bronze characters and occasional ivory highlights. Glyph changes are capped at 24 painted frames per second. Manual pause preserves the current presentation; reduced motion draws a still frame and hides the animation toggle. Work suspends offscreen and while the document is hidden. The original SVG remains visible when canvas or font initialization is unavailable. Keep this optional brand treatment separate from the interface icon vocabulary.

## Do's and Don'ts

### Do:
- **Do** preserve the portal-and-star silhouette when rendering the mark.
- **Do** use the shared type roles, warm palette, and fine structural rules across public pages and account screens.
- **Do** keep account accent, name, logo, route prefix, and stylesheet integration configurable.
- **Do** retain visible keyboard focus, text status labels, and labeled records at narrow widths.
- **Do** keep brand animation optional, pausable, and still under reduced-motion preferences.
- **Do** retain loading, failure, empty, disabled, and one-time-secret states.

### Don't:
- **Don't** introduce a shadow-based card hierarchy into this flat system.
- **Don't** turn small figure captions or factual labels into decorative heading eyebrows.
- **Don't** encode status solely through color or replace explicit credential labels with icon-only shorthand.
- **Don't** require animation or canvas support to recognize the brand mark.
- **Don't** hardcode embedded branding or route assumptions into new account surfaces.
