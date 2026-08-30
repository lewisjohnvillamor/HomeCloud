---
name: taste
description: HomeCloud's visual standard — design tokens, component conventions, icon rules, and the checks a screen must pass before it ships. Use when writing or changing any UI in apps/web, when adding a component or a page, or when reviewing how something looks.
---

# HomeCloud's visual standard

`AGENTS.md` asks for "Taste Skill principles, especially Photos, Memories,
shares, onboarding, and public pages. Do not force landing-page aesthetics into
dense file-management UI." This file is that standard, written down so it is
enforceable rather than a matter of whoever is typing.

## The one-line brief

**A quiet tool for looking after your own things.** Not a product demo. The
interface should feel closer to a well-made file manager than to a marketing
site: content is the subject, chrome recedes, and nothing decorative competes
with a person's own photos and files.

## Three rules that decide most arguments

1. **The content is the subject.** A photo wall is photos, not cards with
   photos in them. A file list is names, not a grid of controls with names
   squeezed between. When space is short, the user's data keeps it and our
   chrome gives way.
2. **Say the true thing plainly.** Empty states, limits, and failures are
   written out in words a person would use. If the product cannot do
   something — keep a version of a change it did not make, show what is in a
   folder you only have an upload link for — the interface says so where
   someone would otherwise assume.
3. **Density is a feature, not a failure.** File management is dense work.
   Do not add breathing room that pushes rows off the screen, and do not
   import landing-page scale (huge type, deep padding, hero blocks) into
   Files, Search, or More.

## Tokens

Defined in `apps/web/app/globals.css`. **Use the token, never the literal.**
A new hardcoded `0.6rem` is how a design system dies.

### Spacing — 4px grid, two deliberate half-steps

| Token | Value | Use |
|---|---|---|
| `--space-1` | 0.25rem | hairline gaps, icon-to-label |
| `--space-2` | 0.375rem | tight gaps inside a control or row |
| `--space-3` | 0.5rem | default gap between related things |
| `--space-4` | 0.75rem | gap between controls |
| `--space-5` | 1rem | padding inside a panel |
| `--space-6` | 1.5rem | gap between groups |
| `--space-7` | 2rem | gap between sections |
| `--space-8` | 3rem | page-level separation |
| `--space-9` | 4rem | rare; full-page empty states |

`--space-2` (6px) and `--space-4`'s neighbours exist because the codebase
already used them heavily and they read better than the pure grid for tight
row work. They are the only off-grid values allowed.

### Radius

| Token | Value | Use |
|---|---|---|
| `--radius-sm` | 0.375rem | inline chips, small marks |
| `--radius-md` | 0.5rem | buttons, inputs, list rows |
| `--radius-lg` | 0.75rem | panels, dialogs, photo tiles |
| `--radius-round` | 50% | avatars, circular checks |

### Type

| Token | Value | Use |
|---|---|---|
| `--text-xs` | 0.75rem | captions over photos, badges |
| `--text-sm` | 0.8125rem | secondary metadata, compact controls |
| `--text-md` | 0.875rem | the workhorse: rows, buttons, most UI |
| `--text-body` | 1rem | prose a person reads in sentences |
| `--text-lg` | 1.0625rem | section leads |
| `--text-xl` | 1.25rem | dialog titles |

Page titles use `clamp()` so they scale with the viewport rather than
stepping at a breakpoint. Never set a font size in `px`.

### Colour

The existing colour tokens are the whole palette: `--background`,
`--surface`, `--surface-hover`, `--surface-selected`, `--border`, `--text`,
`--text-muted`, `--focus`, `--danger`. **Do not add a colour.** If something
needs to stand out, it needs `--focus` or weight, not a new hue. Every token
has a dark-mode value; anything you add must too.

Colour is never the only signal. A destructive action says "Delete", a
selected tile carries a check, an error carries text.

### Controls

| Token | Value | Use |
|---|---|---|
| `--control-height` | 2.75rem | any control a finger might hit |
| `--control-height-compact` | 2.25rem | pointer-only secondary actions |

**A compact control must grow to `--control-height` at `max-width: 40rem`.**
44px is the floor for a touch target, and pointer type is not reliably
reported by browsers — key off width, not `pointer: coarse`.

## Layout rules

- **Two widths matter: 390px and 320px.** Every screen is checked at both.
- Wide content (tables, code, diagrams) scrolls inside its own
  `overflow-x: auto` container. **The page body never scrolls sideways.**
- A flex child that contains a text input needs `min-inline-size: 0`, or the
  input's intrinsic width pushes its neighbours off the screen.
- When a table has more than three row actions, give the columns explicit
  widths at phone width. The automatic algorithm hands the name column
  whatever is left, which is nothing.
- Bottom navigation is fixed; leave room for `env(safe-area-inset-bottom)`.

## Icons

Icons are labels' companions, never their replacement.

- **One family, one weight.** Inline SVG, `1.5` stroke, `currentColor`,
  24×24 viewBox, sized to `1em` so they scale with their text.
- **Never icon-only for a destructive or ambiguous action.** "Delete" keeps
  its word. An icon may carry an action alone only when it is universally
  understood *and* it has an accessible name (the star on a photo tile).
- Decorative icons take `aria-hidden="true"`; the adjacent text is the name.
- Icons sit at `--space-1` from their label, vertically centred.
- No emoji as interface iconography. Emoji render differently on every
  platform and cannot inherit colour or weight.

## Motion

Motion is decoration and is never the only signal that something changed.
Everything honours `prefers-reduced-motion`, which `globals.css` already
enforces globally. Transitions are 150–200ms and only on `opacity` and
`transform`.

## Before a screen ships

Both of these, in this order. Neither alone is enough.

1. **Measure it.** At 390px and 320px: no horizontal overflow on the page,
   no interactive target under 44px, focus visible on every control, and the
   accessible name of every control says what it does and to what.
2. **Look at it.** Take the screenshot and actually look. A layout can pass
   every measurement and still be wrong — five verbs stacked vertically in a
   file row passed the audit and turned a two-file folder into a full
   screen. **The numbers say "not broken"; only your eyes say "good".**

If the two disagree, the screenshot wins.
