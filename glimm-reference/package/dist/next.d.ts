import * as react from 'react';
import { ComponentProps } from 'react';
import Link from 'next/link';
import { SweepOptions, SweepHandle, SweepFn } from './react.js';
export { Direction, EASINGS, EasingName, GlimmDefaults, GlimmProvider, GlimmScope, GlimmScopeHandle, GlimmScopeProps, PALETTES, Palette, PaletteName, ScopePlayHandle, ScopePlayOptions, getPageEffect, setPageEffect, supportsHtmlInCanvas, useGlimm, usePageEffect } from './react.js';
import 'react/jsx-runtime';

type LinkProps = ComponentProps<typeof Link>;
type TransitionLinkProps = LinkProps & {
    /** Per-link sweep overrides (palette, direction, duration…). */
    sweep?: SweepOptions;
    /** If true, skip the sweep and just navigate. Default false. */
    noTransition?: boolean;
};
/**
 * Drop-in replacement for next/link that plays a glimm sweep around the navigation.
 * Falls back to a normal Link click for modifier clicks, target=_blank, external URLs,
 * and same-page anchors.
 */
declare const TransitionLink: react.ForwardRefExoticComponent<Omit<TransitionLinkProps, "ref"> & react.RefAttributes<HTMLAnchorElement>>;

type Href = string;
type TransitionRouter = {
    /** Push a new route, animated by a sweep. Returns the sweep handle. */
    push: (href: Href, options?: SweepOptions) => SweepHandle;
    /** Replace the current route, animated by a sweep. */
    replace: (href: Href, options?: SweepOptions) => SweepHandle;
    /** Navigate back, animated by a sweep. */
    back: (options?: SweepOptions) => SweepHandle;
    /** Refresh the current route, animated by a sweep. */
    refresh: (options?: SweepOptions) => SweepHandle;
};
/**
 * Drop-in wrapper around Next.js's `useRouter()` that plays a glimm sweep
 * around each navigation. Use this for programmatic navigation (form submits,
 * conditional redirects, post-action redirects). For declarative tab links,
 * prefer `<TransitionLink>`.
 *
 * @example
 *   const router = useTransitionRouter()
 *   const onSubmit = async (data) => {
 *     await save(data)
 *     router.push('/dashboard', { palette: 'berry' })
 *   }
 */
declare function useTransitionRouter(): TransitionRouter;

type InterceptLinksOptions = {
    /**
     * Per-link override. Default selects every internal `<a href>` that's not
     * marked with `data-glimm-skip`, doesn't `target="_blank"`, isn't an
     * anchor or download, and isn't a modifier-click. You usually don't need
     * to customize this.
     */
    shouldIntercept?: (e: MouseEvent, anchor: HTMLAnchorElement) => boolean;
    /** Optional sweep overrides applied to every intercepted nav. */
    sweep?: SweepOptions;
};
/**
 * Vanilla, framework-agnostic: installs a delegated click listener on the
 * document that runs every same-origin link click through a sweep.
 * Returns a cleanup function that removes the listener.
 *
 * Pair with a navigate callback that performs the actual route change.
 */
declare function interceptLinks(sweep: SweepFn, navigate: (href: string) => void, opts?: InterceptLinksOptions): () => void;
/**
 * React hook variant. Drop it once at the root of a Next.js app (e.g. in
 * the layout that wraps <GlimmProvider>) and every internal link click
 * gets a sweep automatically. Opt individual links out with
 * `data-glimm-skip`.
 *
 * @example
 *   export default function Layout({ children }) {
 *     return (
 *       <GlimmProvider>
 *         <InterceptLinks />
 *         {children}
 *       </GlimmProvider>
 *     )
 *   }
 */
declare function InterceptLinks(props: InterceptLinksOptions): null;

export { InterceptLinks, type InterceptLinksOptions, SweepFn, SweepHandle, SweepOptions, TransitionLink, type TransitionLinkProps, type TransitionRouter, interceptLinks, useTransitionRouter };
