# glimm

WebGL **sweep transitions** for the web. A color band sweeps across
the viewport, revealing the incoming view behind it while retaining the outgoing
view ahead.

- **Framework-agnostic core** — drive the shader directly from vanilla JS.
- **React adapter** — `<GlimmProvider>` + `useGlimm()`.
- **Next.js adapter** — `<TransitionLink>`, `useTransitionRouter()`, auto link interception.
- Zero runtime dependencies. WebGL decoration with browser snapshots and an opaque fallback; no image assets required.
- Respects `prefers-reduced-motion` and degrades gracefully when WebGL is unavailable.

## Install

```bash
npm install glimm
```

`react` and `next` are optional peer dependencies — install whichever adapter
you use. The core entry point needs neither.

## Quick start (Next.js, App Router)

```tsx
// app/layout.tsx
import { GlimmProvider, InterceptLinks } from 'glimm/next'

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <GlimmProvider palette="prism">
          <InterceptLinks />
          {children}
        </GlimmProvider>
      </body>
    </html>
  )
}
```

`<InterceptLinks />` runs every same-origin link click through a sweep. Opt a
link out with `data-glimm-skip`.

### Per-navigation control

```tsx
'use client'
import { TransitionLink, useTransitionRouter } from 'glimm/next'

// Declarative — a drop-in <Link> replacement:
<TransitionLink href="/about" sweep={{ palette: 'berry', direction: 'rtl' }}>
  About
</TransitionLink>

// Programmatic — a drop-in useRouter():
const router = useTransitionRouter()
await save(form)
router.push('/dashboard', { palette: 'ember' })
```

## React (without Next.js)

```tsx
import { GlimmProvider, useGlimm } from 'glimm/react'

function MyButton() {
  const { sweep } = useGlimm()
  return <button onClick={() => sweep(() => doNavigation())}>Go</button>
}
```

`sweep(navigate, options?)` plays the band, awaits your `navigate` callback at
the midpoint, and returns a `{ midpoint, done, cancel }` handle.

## Vanilla / framework-agnostic core

```ts
import { createShader, playSweep } from 'glimm'

const canvas = document.querySelector('canvas')!
const ctrl = createShader({ canvas, /* palette, bandTight, direction */ })

playSweep(ctrl, {
  palette: 'citrus',
  onMidpoint: () => swapPageContent(),
  onComplete: () => ctrl.destroy(),
})
```

Alternate looks: `createMeshShader` (vertex-displaced mesh) and
`createNamedropShader` (radial bulge reveal).

### Sweeping inside a container

The canvas doesn't have to cover the viewport. Size it to any element (a
card, a chart, a table) and the sweep plays inside that box. A band that
stops at the box's border looks cut off, so the shaders fade it out towards
the canvas edges:

```ts
const ctrl = createShader({ canvas, feather: 'auto' }) // the default
```

| `feather` | Effect |
| --------- | ------ |
| `'auto'`  | Default. Fades over 24 px (or 15% of the shorter side on small boxes); off when the canvas fills the viewport, where there is no edge to hide. |
| number    | Fade width in CSS px. |
| `0`       | No fade; the band runs to the edges. |

Change it later with `ctrl.setFeather(value)`.

## Moving the page with `GlimmScope`

`GlimmProvider` sweeps the viewport during navigation. `GlimmScope` wraps one box, which can be a whole page, a section or a card, and plays when you ask it to. Where the browser supports HTML-in-Canvas (Chrome 157 and later), the content breathes in towards the action and ripples as the band passes. Everywhere else it's a plain sweep over the same box, with the change still revealed behind the band, so there's nothing to check or branch on.

```tsx
'use client'
import { useRef } from 'react'
import { GlimmScope, type GlimmScopeHandle } from 'glimm/next' // or 'glimm/react'

const scope = useRef<GlimmScopeHandle>(null)

const upgrade = (event: React.MouseEvent) => {
  const r = event.currentTarget.getBoundingClientRect()
  setBusy(true)
  scope.current?.play({
    at: [r.left + r.width / 2, r.top + r.height / 2], // CSS px from the box's top-left
    work: () => requestUpgrade(),   // starts on press; the page leans in while it runs
    change: () => setPlan('pro'),   // runs as soon as the work is done, behind the band
    onBurst: () => setBusy(false),  // the band has broken away
    onFail: () => setBusy(false),   // the work rejected: no sweep, nothing changes
  })
}

<GlimmScope ref={scope} curve={0.12} direction="ttb" sweepMs={2700} style={{ position: 'absolute', inset: 0 }}>
  <Billing />
</GlimmScope>
```

The app never waits for the animation. Without `work`, `change` runs right on the press. A later `play()` interrupts whatever is running, and `play()` returns `{ fastForward, cancel }`. For something nobody pressed, such as a payment arriving, pass `anticipate: true` so the page leans in fully before the change.

Give the box a size, because the scope clips to its box and fills it. Where the page effect runs, the content doesn't give the box its height, so a section or card needs a set height or it collapses to nothing.

Size it to what changed. These are good starting points:

| What changed | Props |
| ------------ | ----- |
| The whole page or view | `curve={0.12}` |
| Something notable in one place | `curve={0.22} span={0.64} fade={[0.16, 0.64]}` |
| One element, with the scope around just that element | `curve={0.06} bandTight={34}` |

`direction` can be `ttb`, `btt`, `ltr` or `rtl`, so the band can travel away from the action. Each scope uses its own WebGL contexts and a page can only keep 16 alive, so keep a handful on a page rather than one per row.

`usePageEffect()` returns `[on, setOn]`, a switch a reader can use to turn the page effect off. It's remembered on the device, and every scope follows it.

## Sweep options

| Option        | Default  | Notes |
| ------------- | -------- | ----- |
| `sweepMs`     | `1100`   | ms for the band to cross the viewport |
| `outroMs`     | `220`    | ms for the post-traversal fade-out |
| `midpoint`    | `0.56`   | 0–1 progress point where `onMidpoint` fires |
| `palette`     | `spectrum` | preset name or a custom `Palette` |
| `direction`   | `ltr`    | `ltr` \| `rtl` \| `ttb` \| `btt` |
| `easing`      | `ease` | preset name or an `(p) => number` fn |
| `bandTight`   | —        | band sharpness |
| `peakAlpha`   | `1`      | caps the band's peak opacity |
| `brightness`  | `1`      | 0–1.5, multiplies band RGB (dim it on dark backgrounds) |
| `waveAmount`  | `0`      | 0–2, opt-in edge displacement; 0 is straight |
| `rippleAmount`| `1`      | 0–2, vertical ripple texture |
| `waveSpeed`   | `1`      | 0–3, multiplies time-based motion |
| `swellAmount` | `0.55`   | 0–1, depth swell |
| `onMidpoint`  | —        | fires when the band reaches `midpoint` — swap pages here |
| `onComplete`  | —        | fires when the outro fade finishes |

**Built-in palettes:** `spectrum`, `prism`, `berry`, `lagoon`, `citrus`, `azure`,
`ember`. Build your own with `accentPair`,
`accentChain`, or `shuffleAccentPalette`.

**Built-in easings:** `linear`, `easeOutQuart`, `easeOutCubic`, `easeInCubic`,
`easeInOutCubic`, `easeOutExpo`, `easeInOutQuint`, `snap`, `ease`, `back`.

## Entry points

| Import          | Use for |
| --------------- | ------- |
| `glimm`         | Framework-agnostic core: shader factories, `playSweep`, palettes, easings, color math. |
| `glimm/react`   | `<GlimmProvider>` and `useGlimm()` for any React app. |
| `glimm/next`    | Everything in `glimm/react` plus `<TransitionLink>`, `useTransitionRouter()`, and `interceptLinks` / `<InterceptLinks>`. |

Import the provider and hooks from a **single** entry point per app so they
share one React context.

## License

[MIT](./LICENSE) © Noman Ijaz. Free to use, modify, and distribute.

## Lifecycle and cancellation

`midpoint` settles after the midpoint callback finishes. `done` waits for that
work and the outro. Callback failures are logged, and handles settle without
rejecting. Instant reduced-motion and no-WebGL navigation also run the per-call
lifecycle callbacks.

Cancelling settles the handle and prevents any midpoint callback that has not
started. It cannot undo navigation or asynchronous work that has already begun.
The core `cancel()` preserves the current band position and alpha for a replacement
sweep; when stopping without a replacement, call `ctrl.setAlpha(0)` or destroy
the controller. The default renderer stops drawing while alpha is zero.

Next.js router calls do not return a readiness promise. The provider coordinates
them through React transitions; this does not guarantee that every image, effect,
or independently streamed boundary has finished loading.

## Local checks

From `packages/glimm`, run `npm test`, `npm run typecheck`, and `npm run build`.
The regression tests cover cancellation, callback failures, slow midpoint work,
invalid timing values, instant fallback behavior, link handling, unmount cleanup,
and idle rendering.

## Masked React and Next.js transitions

The provider now retains the outgoing rendered view with the browser View
Transitions API. The sweep is a real boundary: the incoming view is revealed
behind the band, while outgoing pixels remain ahead of it. The browser's default
crossfade is disabled. The iridescent canvas is a separate live transition layer.

Navigation/state updates run during snapshot preparation. The provider waits for
React transition commits (including suspending updates) and a returned navigation
promise before revealing the new tree. Supply work through the navigation callback;
updates scheduled later by unrelated timers/effects are outside this handoff.
An explicitly committed loading boundary can still be the incoming view.

The page is never held up by the sweep. The browser stops painting while it waits
for the new view, so the provider gives a route 200 ms to commit; a prefetched
route lands well inside that. A slower route lets the snapshot go: the outgoing
page stays live and interactive while it loads, and the band sweeps once the new
view is on screen. During a sweep the page takes input as usual. A press, scroll
or key ends the snapshot early, and the press is handed on to whatever was under
the pointer.

When browser snapshots are unavailable or fail, an opaque, theme-aware curtain
covers the view before updating it, then reveals the committed content. Its opacity
is independent of shader brightness and alpha. The same 200 ms limit applies: if the
route hasn't committed by then, the curtain lets go so the outgoing page stays in
view, and the band passes over the new view once it arrives. Reduced motion and unavailable
WebGL continue to use immediate navigation. An 8-second timeout stops waiting on a route
that never commits; it cannot cancel application requests already in progress.

The low-level `playSweep` API remains a decorative canvas sweep; automatic content
coordination belongs to `GlimmProvider`. Core cancellation preserves shader state;
provider cancellation now also removes its masks and clears the overlay.

Run `npm run test:browser` for browser pixel checks. It uses an installed Chrome
(default macOS path); set `CHROME_PATH` for another installation. The test verifies
both sides of the boundary with shader alpha set to zero, all four directions,
slow Suspense commits, and opaque fallback behavior.

To inspect the local form demo at exactly 0.1× animation speed, run
`npm run test:slow` while the site is running at `http://127.0.0.1:3041`.
Set `GLIMM_URL` to use another form-demo URL. This slows the animation clock,
including shader motion, and checks the expected ~13.2-second duration, monotonic
mask movement, cleanup, and browser errors. Captured frames and a JSON report are
written to a temporary directory printed by the command.


Use `BROWSER=webkit npm run test:browser` or
`BROWSER=firefox npm run test:browser` for additional engines. Install their test
binaries with `npx playwright-core install webkit firefox` first. Firefox's
screenshot implementation omits active View Transitions ([Mozilla bug 2008417](https://bugzilla.mozilla.org/show_bug.cgi?id=2008417)),
so its suite checks live mask styles and lifecycle behavior; native-window visual
inspection is required for its composited boundary. Chrome and WebKit assert
outgoing/incoming pixel colors in all four directions.

For native Safari or an iOS simulator, run `npm run test:native` and open
`http://127.0.0.1:3043/?client=safari` (use `client=ios` on the simulator).
The page runs manually released Suspense content, replacement transitions,
cancellation, and a slow visual sweep, then saves its reports in the printed
temporary directory. The visual case reports completion/cleanup; inspect the
actual browser window for rendering. Native Safari automation need not be enabled.

For a production integration check against the npm tarball in an isolated Next.js
app, follow the repository’s [packaged fixture instructions](tests/next-app/README.md). This
checks the installed public entry points and actual server-route navigation,
including slow responses, rapid navigation, history, anchors, and cancellation.
