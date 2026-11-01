# Freespace landing page design

Applies to `site/`. The app in `ui/` has its own working UI, and shares the brand colour and logo.

## World

Clean, modern and flat, with a technical voice: rounded shapes, drawn SVG illustrations, monospace paths and commands.
No screenshots, shadows or gradient fills.

## Colour

Indigo is the one brand colour across the app, logo, installer and this page. Green, amber and red are never brand
colours: they only ever label safety.

| Token | Value | Meaning |
| --- | --- | --- |
| `--brand` | `#4f46e5` | Freespace: actions, main drawing fill. Same value as the app's `--acc` |
| `--brand-dark` | `#3730a3` | Hover, text on tints |
| `--brand-300` to `--brand-50` | `#a5b4fc` to `#f1f3ff` | Drawing tints and tile grounds |
| `--ink` | `#15163a` | Headings, dark tiles |
| `--text` | `#474a6e` | Body text |
| `--safe`, `--review`, `--danger` | Same family as the app | Safety labels only |

## Type

Self-hosted from `site/fonts/`, under the SIL Open Font License.

- Headings and body: Geist, 400 to 600, tight tracking on headings.
- Code, paths, sizes and spec keys only: Geist Mono.

## Shape

- Radii: 32px panels, 24px tiles, 16px rows, full pills for buttons and labels.
- Flat: depth comes from tints, never from shadows.
- Drawings are inline SVG in the indigo tints, with round caps and joins. Their sizes are examples, and the hero and project bars say so.

## Components

- **Project anatomy (hero):** a folder with rounded tree elbows to tickable rows, above a disk gauge with a dimension
  line. Ticking updates the gauge. Works as a static drawing without JavaScript.
- **Flow:** five rounded icon nodes joined by dashed connectors; vertical on narrow screens.
- **Bento:** tiles of different widths, each a drawing above a short heading.
- **Spec list:** monospace keys over hairlines.

## Motion

Only the gauge moves, when a row is ticked. Reduced motion turns it off.

## Rules

- No kickers above headings, no section numbers, no rows of equal cards.
- No invented customers, benchmarks or download counts.
- Windows only. Do not imply other platforms.
