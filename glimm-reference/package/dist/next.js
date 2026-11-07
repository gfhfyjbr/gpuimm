'use client';
import { useGlimm } from './chunk-U6WLNRFO.js';
export { EASINGS, GlimmProvider, GlimmScope, PALETTES, getPageEffect, setPageEffect, supportsHtmlInCanvas, useGlimm, usePageEffect } from './chunk-U6WLNRFO.js';
import Link from 'next/link';
import { useRouter } from 'next/navigation';
import { forwardRef, useEffect } from 'react';
import { jsx } from 'react/jsx-runtime';

var TransitionLink = forwardRef(
  function TransitionLink2({ onClick, sweep: sweepOpts, noTransition, ...linkProps }, ref) {
    const router = useRouter();
    const { sweep } = useGlimm();
    const handleClick = (e) => {
      onClick?.(e);
      if (e.defaultPrevented) return;
      if (noTransition) return;
      if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button !== 0) return;
      if (linkProps.target && linkProps.target !== "_self") return;
      const anchor = e.currentTarget;
      if (anchor.hasAttribute("download")) return;
      const url = new URL(anchor.href, window.location.href);
      if (url.origin !== window.location.origin) return;
      if (url.pathname === window.location.pathname && url.search === window.location.search && url.hash) return;
      const href = url.pathname + url.search + url.hash;
      e.preventDefault();
      sweep(() => {
        if (linkProps.replace) router.replace(href, { scroll: linkProps.scroll });
        else router.push(href, { scroll: linkProps.scroll });
      }, sweepOpts);
    };
    return /* @__PURE__ */ jsx(Link, { ref, ...linkProps, onClick: handleClick });
  }
);
function useTransitionRouter() {
  const router = useRouter();
  const { sweep } = useGlimm();
  return {
    push: (href, options) => sweep(() => router.push(href), options),
    replace: (href, options) => sweep(() => router.replace(href), options),
    back: (options) => sweep(() => router.back(), options),
    refresh: (options) => sweep(() => router.refresh(), options)
  };
}
var defaultShouldIntercept = (e, a) => {
  if (e.defaultPrevented) return false;
  if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return false;
  if (e.button !== 0) return false;
  if (a.dataset.glimmSkip !== void 0) return false;
  if (a.target && a.target !== "_self") return false;
  if (a.hasAttribute("download")) return false;
  if (!a.href) return false;
  const url = new URL(a.href, window.location.href);
  if (url.origin !== window.location.origin) return false;
  if (url.pathname === window.location.pathname && url.search === window.location.search && url.hash) return false;
  return true;
};
function interceptLinks(sweep, navigate, opts = {}) {
  const shouldIntercept = opts.shouldIntercept ?? defaultShouldIntercept;
  const onClick = (e) => {
    const target = e.target;
    const anchor = target?.closest?.("a");
    if (!anchor) return;
    if (!shouldIntercept(e, anchor)) return;
    const href = new URL(anchor.href, window.location.href);
    e.preventDefault();
    sweep(() => navigate(href.pathname + href.search + href.hash), opts.sweep);
  };
  document.addEventListener("click", onClick);
  return () => document.removeEventListener("click", onClick);
}
function InterceptLinks(props) {
  const router = useRouter();
  const { sweep } = useGlimm();
  useEffect(() => {
    return interceptLinks(sweep, (href) => router.push(href), props);
  }, [router, sweep, props.sweep, props.shouldIntercept]);
  return null;
}

export { InterceptLinks, TransitionLink, interceptLinks, useTransitionRouter };
