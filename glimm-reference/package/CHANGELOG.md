# Changelog

## 0.4.1

- Fix the page freezing during `GlimmProvider` and `TransitionLink` sweeps, which came in with 0.3.1. While a route loaded, the browser stopped painting the page for as long as the route took, up to 8 seconds, and every click or scroll during the sweep was swallowed by the snapshot overlay. A route now has 200 ms to commit inside the snapshot. A slower one lets it go, so the outgoing page stays live and interactive while it loads, and the band sweeps once the new view is on screen. During a sweep, a press, scroll or key ends the snapshot early and the press reaches what was under the pointer.
- Apply the same limit to the opaque curtain used where View Transitions aren't available. It covered the page until the route committed, again up to 8 seconds; now it lets go after 200 ms, so a slower route keeps the outgoing page in view while it loads, and the band passes over the new view once it arrives.
- Run the outgoing view's clip as an animation on its own pseudo-element instead of a custom property on `<html>`, which restyled the whole page every frame. On a large page this cuts style work during a sweep by about 85%.

## 0.4.0

- Add the `spectrum` palette and make it the default: one full hue cycle across the band. Every existing palette is unchanged, and a provider that sets `palette` looks exactly as before. Only a provider with no `palette` changes, and only after upgrading to 0.4. To keep the old look, set `palette="prism"`.
- Fade the band out towards the canvas edges when a shader runs inside a container, so contained sweeps don't end on a hard border. New `feather` option on `createShader` and `createMeshShader` (`'auto'` by default: 24 px, or 15% of the shorter side; off for full-viewport canvases), and `setFeather()` on the controller.
- Add `GlimmScope` to `glimm/react` and `glimm/next`: a sweep over one box that also moves the content under the band, on HTML-in-Canvas. The page breathes in towards the action and ripples as the band passes. It takes an app's real work through `play({ work, change })`, so the change never waits for the animation and a new action can interrupt. Where HTML-in-Canvas isn't available it's a plain glimm sweep, with the change still revealed by the band. Also exports `supportsHtmlInCanvas` and `usePageEffect`, a switch for readers to turn the effect off.
- Add `curve`, `span` and `center` options to `createShader`, and `setCenter()` on its controller, for a band that bends and covers part of the box.
- Let the band fully leave a contained canvas. It used to stop at 20% past the far edge, where a tight band is still about a third as strong, so it seemed to hang on the edge while it faded out. Contained canvases now travel far enough for the band to drop under 1% at the edge; full-viewport sweeps are unchanged.

## 0.3.1

- Shorten the default outro from 700 ms to 220 ms, localize the wake behind the band, and soften the halo to reduce lingering color on revealed content.

- Settle sweep handles and clear the overlay when completion callbacks or custom easing functions throw.
- Release cancelled animation phases, skip queued midpoint callbacks after cancellation, and stop waiting internally for cancelled midpoint work.
- Normalize invalid durations and non-finite easing output to prevent infinite animation loops and invalid alpha values.
- Cancel an active sweep before instant reduced-motion or no-WebGL navigation. Run fallback lifecycle callbacks consistently and handle navigation errors without unhandled rejections.
- Stop the default WebGL renderer while transparent and resume when the next sweep starts.
- Preserve serialized object URL queries, fragments, and `scroll` options in `TransitionLink`; leave downloads to the browser.
- Intercept query changes with fragments correctly and refresh custom interception predicates when they change.
- Remove controller canvases during provider cleanup.

### Content masking

- Use browser-rendered snapshots for React/Next transitions. Clip the outgoing
  view at the band boundary instead of swapping visible content under an overlay.
- Keep the shader as a separate live layer and disable the browser crossfade.
- Wait for React transition commits and returned navigation promises before reveal.
- Use a truly opaque cover/update/reveal fallback where snapshots are unavailable.
- Release masks on cancellation, failure, timeout, unmount, or browser snapshot skip.
- Align vertical shader travel with top-to-bottom/bottom-to-top masks and remove
  the viewport-edge fade that exposed the boundary.

No new public props. Provider navigation now runs during snapshot preparation;
the low-level core sweep retains its existing midpoint-driven behavior.
