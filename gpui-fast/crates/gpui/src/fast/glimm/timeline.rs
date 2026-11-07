//! Deterministic counterpart of GlimmScope's gather/ready/release frame loop.

use super::options::{GlimmEffect, GlimmOptions};

/// Shader state at a single instant. Logical time is milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlimmSample {
    /// Traversal progress.
    pub progress: f32,
    /// Band strength, before peak-alpha multiplication.
    pub strength: f32,
    /// Pull towards the point of action.
    pub gather: f32,
    /// Gather blur strength.
    pub haze: f32,
    /// Travelling content ripple strength.
    pub ring: f32,
    /// Whether the old/new split has begun.
    pub burst: bool,
    /// Whether another frame is needed.
    pub active: bool,
    /// Whether the midpoint/burst callback may fire.
    pub midpoint: bool,
}

/// Pure, seekable timeline. It allocates no renderer resources.
#[derive(Clone, Debug)]
pub struct GlimmTimeline {
    options: GlimmOptions,
    ready: Option<(f32, GlimmSample, f32)>,
    release: Option<(f32, GlimmSample)>,
    fast: Option<(f32, f32)>,
    waiting_for_work: bool,
}

impl GlimmTimeline {
    /// Begin an immediate transition, or gather while asynchronous work runs.
    pub fn new(options: GlimmOptions, waiting_for_work: bool) -> Self {
        let options = options.validated();
        let ready = (!waiting_for_work).then_some((0., GlimmSample::default(), 0.));
        Self {
            options,
            ready,
            release: None,
            fast: None,
            waiting_for_work,
        }
    }
    /// Gather fully before an anticipated change with no asynchronous work.
    /// Unlike a work promise, the reference's `anticipate: true` pulls to 1 rather than 0.32.
    pub fn anticipation(options: GlimmOptions) -> Self {
        let mut timeline = Self::new(options, true);
        timeline.waiting_for_work = false;
        timeline
    }
    /// Settings frozen for this run.
    pub fn options(&self) -> &GlimmOptions {
        &self.options
    }
    /// Commit successful work. Returns false after cancellation or an earlier commit.
    pub fn resolve(&mut self, elapsed_ms: f32) -> bool {
        if self.ready.is_some() || self.release.is_some() {
            return false;
        }
        let sample = self.sample(elapsed_ms);
        let snap_ms = if elapsed_ms < 120. || !self.options.page_effect || self.fast.is_some() {
            0.
        } else {
            440. * (1. - sample.gather)
        };
        self.ready = Some((elapsed_ms, sample, snap_ms));
        true
    }
    /// Return to rest without committing work or playing a successful sweep.
    pub fn cancel(&mut self, elapsed_ms: f32) {
        if self.release.is_none() {
            self.release = Some((elapsed_ms, self.sample(elapsed_ms)));
        }
    }
    /// Finish an in-flight traversal in 160ms, without a discontinuous jump.
    pub fn fast_forward(&mut self, elapsed_ms: f32) {
        if self.fast.is_none() {
            let k = self
                .ready
                .map(|(at, _, snap)| ratio(elapsed_ms - at - snap, self.options.sweep_ms))
                .unwrap_or(0.);
            self.fast = Some((elapsed_ms, k));
        }
    }
    /// Evaluate at an absolute elapsed time, with the reference's exact timing constants.
    pub fn sample(&self, elapsed_ms: f32) -> GlimmSample {
        let t = if elapsed_ms.is_finite() {
            elapsed_ms.max(0.)
        } else {
            0.
        };
        if let Some((at, from)) = self.release {
            let k = ratio(t - at, 320.);
            let gain = (1. - k).powi(2);
            return GlimmSample {
                gather: from.gather * gain,
                haze: from.haze.min(from.gather * gain),
                strength: from.strength * gain,
                progress: from.progress,
                ring: from.ring * gain,
                burst: from.burst,
                active: k < 1.,
                midpoint: false,
            };
        }
        if self.options.effect == GlimmEffect::Band {
            return self.band_sample(t);
        }
        let mut result = GlimmSample {
            active: true,
            ..GlimmSample::default()
        };
        if let Some((ready_at, from, snap_ms)) = self.ready {
            result.burst = true;
            let run_at = ready_at + snap_ms;
            if t >= run_at {
                let since = t - run_at;
                let mut k = ratio(since, self.options.sweep_ms);
                if let Some((ff_at, ff_from)) = self.fast {
                    let ff_at = ff_at.max(run_at);
                    if t >= ff_at {
                        k = ff_from + (1. - ff_from) * smooth(ratio(t - ff_at, 160.));
                    }
                }
                result.gather = (if snap_ms > 0. { 1. } else { from.gather })
                    * (1. - since / 220.).max(0.).powi(2);
                result.haze = result.gather;
                result.progress = 0.08 + 0.92 * (1. - (1. - k).powf(2.4));
                let fade = self
                    .options
                    .fade
                    .map_or(1., |[a, b]| 1. - smooth(ratio(result.progress - a, b - a)));
                let ramp_from = if snap_ms > 0. { 1. } else { from.strength };
                let ramp = ramp_from + (1. - ramp_from) * smooth(ratio(since, 140.));
                result.strength = (1. - k).powf(1.2) * fade * ramp;
                result.ring = result.strength * smooth(ratio(since, 120.));
                result.midpoint = since >= 90.;
                result.active = k < 1. && self.options.fade.is_none_or(|f| result.progress < f[1]);
                if !result.active {
                    result.strength = 0.;
                    result.gather = 0.;
                    result.haze = 0.;
                    result.ring = 0.;
                }
                return result;
            }
            let k = smooth(ratio(t - ready_at, snap_ms));
            result.gather = from.gather + (1. - from.gather) * k;
            result.haze = from.haze + (1. - from.haze) * k;
            result.strength = (0.55 * smooth(ratio(t, self.options.gather_ms))).max(k);
            result.progress = 0.08 * smooth(ratio(t, self.options.gather_ms));
            return result;
        }
        if !self.options.page_effect {
            return result;
        }
        let pull = ratio(t, self.options.gather_ms);
        let ease = smooth(pull);
        result.gather = (if self.waiting_for_work { 0.32 } else { 1. }) * ease * pull;
        result.haze = if self.waiting_for_work {
            0.
        } else {
            result.gather * (1. - smooth(ratio(t - self.options.gather_ms - 400., 700.)))
        };
        result.strength = (if self.waiting_for_work { 0.55 } else { 1. }) * ease;
        result.progress = 0.08 * ease;
        result
    }
    fn band_sample(&self, t: f32) -> GlimmSample {
        let Some((at, _, _)) = self.ready else {
            return GlimmSample {
                active: true,
                ..GlimmSample::default()
            };
        };
        let since = (t - at).max(0.);
        // playSweep enforces an 80ms minimum even for a zero-length traversal.
        let duration = self.options.sweep_ms.max(80.);
        let (k, finish) = if let Some((ff_at, from)) = self.fast {
            if t >= ff_at {
                (
                    from + (1. - from) * smooth(ratio(t - ff_at, 160.)),
                    ff_at + 160.,
                )
            } else {
                (ratio(since, duration), at + duration)
            }
        } else {
            (ratio(since, duration), at + duration)
        };
        let progress = self.options.easing.sample(k).clamp(0., 1.);
        let fade = if t >= finish {
            1. - super::options::GlimmEasing::EaseInOutCubic
                .sample(ratio(t - finish, self.options.outro_ms))
        } else {
            1.
        };
        GlimmSample {
            progress,
            strength: fade,
            active: t < finish + self.options.outro_ms,
            burst: true,
            midpoint: progress >= self.options.midpoint,
            ..GlimmSample::default()
        }
    }
}
pub(super) fn ratio(t: f32, duration: f32) -> f32 {
    if duration <= 0. {
        1.
    } else {
        (t / duration).clamp(0., 1.)
    }
}
pub(super) fn smooth(t: f32) -> f32 {
    t * t * (3. - 2. * t)
}
