import * as react_jsx_runtime from 'react/jsx-runtime';
import * as react from 'react';
import { ReactNode, CSSProperties } from 'react';

type Palette = {
    a: [number, number, number];
    b: [number, number, number];
    c: [number, number, number];
    d: [number, number, number];
};
declare const PALETTES: {
    spectrum: {
        a: [number, number, number];
        b: [number, number, number];
        c: [number, number, number];
        d: [number, number, number];
    };
    prism: Palette;
    berry: Palette;
    lagoon: Palette;
    citrus: Palette;
    azure: Palette;
    ember: Palette;
};
type PaletteName = keyof typeof PALETTES;

type Direction = 'ltr' | 'rtl' | 'ttb' | 'btt';
type ShaderController = {
    canvas: HTMLCanvasElement;
    setProgress: (p: number) => void;
    setAlpha: (a: number) => void;
    setPalette: (p: Palette) => void;
    setBandTight: (b: number) => void;
    setDirection: (d: Direction) => void;
    setWaveAmount: (v: number) => void;
    setRippleAmount: (v: number) => void;
    setWaveSpeed: (v: number) => void;
    setBrightness: (v: number) => void;
    setSwellAmount: (v: number) => void;
    /** See the `feather` option. Optional so custom shader factories keep type-checking. */
    setFeather?: (v: number | 'auto') => void;
    setCenter?: (v: number) => void;
    getProgress: () => number;
    getAlpha: () => number;
    destroy: () => void;
};

declare const EASINGS: {
    linear: (p: number) => number;
    easeOutQuart: (p: number) => number;
    easeOutCubic: (p: number) => number;
    easeInCubic: (p: number) => number;
    easeInOutCubic: (p: number) => number;
    easeOutExpo: (p: number) => number;
    easeInOutQuint: (p: number) => number;
    snap: Easing;
    ease: Easing;
    back: Easing;
};
type EasingName = keyof typeof EASINGS;

type Easing = (p: number) => number;
type SweepOptions = {
    /** ms for the band to traverse. Default 1100. */
    sweepMs?: number;
    /** ms for the post-traversal fade-out. Default 220. */
    outroMs?: number;
    /** 0..1, when in the sweep to fire `onMidpoint` (i.e. swap pages). Default 0.56. */
    midpoint?: number;
    palette?: PaletteName | Palette;
    bandTight?: number;
    direction?: Direction;
    easing?: EasingName | Easing;
    /** Caps the band's peak alpha. 0..1.5, default 1. Useful for dimming. */
    peakAlpha?: number;
    /** 0..2, opt-in edge displacement. 0 = straight default, 1 = organic, 2 = strong. */
    waveAmount?: number;
    /** 0..2, vertical ripple texture intensity. 0 = smooth, 1 = default. */
    rippleAmount?: number;
    /** 0..3, multiplies all time-based shader motion. 1 = default. */
    waveSpeed?: number;
    /** 0..1.5, multiplies the band's RGB. Lower it on dark backgrounds so the
     *  iridescent colours don't read as harsh whites. 1 = default. */
    brightness?: number;
    /** 0..1, depth/iridescent swell on top of the flat band. 0 = legacy flat
     *  stripe, 1 = full crest highlight + Fresnel rim. Default left to the
     *  shader controller (0.55). */
    swellAmount?: number;
    /** Called once the band has reached `midpoint`. Use this to navigate / swap content. */
    onMidpoint?: () => void | Promise<void>;
    /** Called when the full animation (incl. outro) finishes. */
    onComplete?: () => void;
};
type SweepHandle = {
    /** Resolves when the band reaches the configured midpoint. */
    midpoint: Promise<void>;
    /** Resolves when the outro fade finishes. */
    done: Promise<void>;
    /**
     * Cancel mid-flight. Stops the animation loop but leaves the shader's
     * current alpha/progress untouched so a following sweep can continue
     * from where this one was.
     */
    cancel: () => void;
};

/** A factory matching `createShader`'s signature. Lets callers swap in a
 *  different shader controller implementation (e.g. `createMeshShader`)
 *  without rewriting the provider. */
type ShaderFactory = (opts: {
    canvas: HTMLCanvasElement;
    palette?: Palette;
    bandTight?: number;
    direction?: Direction;
}) => ShaderController | null;
type GlimmDefaults = Omit<SweepOptions, 'onMidpoint' | 'onComplete'> & {
    /** 'instant' skips the sweep, 'sweep' plays anyway. Default 'instant'. */
    reducedMotion?: 'instant' | 'sweep';
    /** z-index of the canvas overlay. Default 9999. */
    zIndex?: number;
    /** Optional override for the shader controller factory. Defaults to the
     *  fullscreen-quad `createShader`; pass `createMeshShader` (or any other
     *  ShaderFactory) to use a different sweep look in this subtree. */
    shaderFactory?: ShaderFactory;
    /** Fires once when the underlying shader controller is created. Lets
     *  callers grab a ref to the controller for live tuning (e.g. piping
     *  idea-playground flags into a MeshShaderController). */
    onController?: (ctrl: ShaderController) => void;
};
type SweepFn = (
/** A function that triggers your navigation (router.push, etc.). Awaited inside the sweep. */
navigate: () => void | Promise<void>, 
/** Per-call overrides (merged over provider defaults). */
options?: SweepOptions) => SweepHandle;
type Ctx = {
    sweep: SweepFn;
    defaults: GlimmDefaults;
};
declare function useGlimm(): Ctx;
declare function GlimmProvider({ children, ...defaults }: GlimmDefaults & {
    children: ReactNode;
}): react_jsx_runtime.JSX.Element;

/** Whether this browser can draw the page into a canvas (Chrome 157 and later,
 *  or an earlier Chrome with the flag on). Without it, GlimmScope is plain glimm. */
declare function supportsHtmlInCanvas(): boolean;
type ScopePlayOptions<T = unknown> = {
    /** The real work behind the action (a request, a generation), started
     *  on press. The page leans in to a low hold and stays there, deepens to
     *  a full gather once it resolves, and the band breaks away on the
     *  result: one breath, however long the work takes. If it rejects there's no sweep: the gather lets go and
     *  onFail runs. If it resolves almost at once there's no gather. */
    work?: () => Promise<T>;
    /** The app's change, with work's result. It runs the moment it's ready,
     *  never held for the animation: without work, right on the press. On
     *  screen it appears behind the band. May return a promise (a route
     *  change): the page holds its gather until that settles and has painted,
     *  then the band breaks away. */
    change?: (result: T) => void | Promise<unknown>;
    /** Nobody is waiting on this (an event arriving, not a press), so there's
     *  time to lean in for a full gather before the change. Don't use it for
     *  anything a person just asked for. */
    anticipate?: boolean;
    /** Gather towards this point instead (CSS px in the box), e.g. the link
     *  or button that was pressed. The curve and span centre on it too. */
    at?: [number, number];
    /** The change has already happened (browser back/forward): freeze what
     *  was on screen now and gather that, instead of the live page. */
    fromOld?: boolean;
    /** The band has just broken away: flip whatever the action changes at
     *  its origin (a busy label, the button itself). Don't wait for onLeave. */
    onBurst?: () => void;
    /** work rejected: there's no sweep and the page stays as it was. */
    onFail?: (error: unknown) => void;
    /** Everything has settled (also after a fail or cancel). */
    onLeave?: () => void;
};
type ScopePlayHandle = {
    /** Run what's left of the sweep in a moment. */
    fastForward: () => void;
    /** Before the change has committed: let go without it. */
    cancel: () => void;
};
type GlimmScopeHandle = {
    /** Latest wins, and the new change never waits. Mid-sweep, the band runs
     *  out in about 160 ms and the next follows; before the band has left, the
     *  one playing gives way and the new one carries on from its gather. */
    play: <T>(opts?: ScopePlayOptions<T>) => ScopePlayHandle;
};
type GlimmScopeProps = {
    children: ReactNode;
    style?: CSSProperties;
    /** Where the gather pulls towards, as fractions of the box, plus a CSS px
     *  offset. Also where the band's curve and span centre. */
    origin?: [number, number];
    originOffset?: [number, number];
    /** Down the box, up it (from an action at the bottom), or across it left
     *  to right (from a sidebar). */
    direction?: 'ttb' | 'btt' | 'ltr' | 'rtl';
    gatherMs?: number;
    /** The band's run across the box. */
    sweepMs?: number;
    /** Band progress over which band and ripple dissolve, e.g. [0.16, 0.64]
     *  to be gone by mid-page. */
    fade?: [number, number];
    /** Multiplies how hard the content moves (already scaled by box size). */
    intensity?: number;
    /** The band, as for a glimm sweep. */
    palette?: PaletteName | Palette;
    bandTight?: number;
    peakAlpha?: number;
    brightness?: number;
    swellAmount?: number;
    /** How far the band's ends trail its middle, as a fraction of the height. */
    curve?: number;
    /** 0..1, share of the cross axis the band covers, centred on the origin. */
    span?: number;
    /** false: never move the page, only sweep the band over it, as without
     *  HTML-in-Canvas. The reader can also turn it off (usePageEffect). */
    pageEffect?: boolean;
};
declare const GlimmScope: react.ForwardRefExoticComponent<GlimmScopeProps & react.RefAttributes<GlimmScopeHandle>>;

declare function getPageEffect(): boolean;
declare function setPageEffect(on: boolean): void;
/** Whether the reader has the page effect on, and a way to change it. */
declare function usePageEffect(): [boolean, (on: boolean) => void];

export { type Direction, EASINGS, type EasingName, type GlimmDefaults, GlimmProvider, GlimmScope, type GlimmScopeHandle, type GlimmScopeProps, PALETTES, type Palette, type PaletteName, type ScopePlayHandle, type ScopePlayOptions, type SweepFn, type SweepHandle, type SweepOptions, getPageEffect, setPageEffect, supportsHtmlInCanvas, useGlimm, usePageEffect };
