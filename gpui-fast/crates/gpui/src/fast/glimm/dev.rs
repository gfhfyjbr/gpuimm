//! Development aids: shader hot reload, the latest shader build status and false-colour views.

use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    fs, mem,
    path::PathBuf,
    sync::{Mutex, PoisonError},
    time::SystemTime,
};

/// Names a directory holding `glimm.metal`, `glimm.wgsl` or `glimm.hlsl`. When it is set, the
/// platform renderer compiles that file instead of its built-in source, and compiles it again
/// each time the file changes. A file that fails to compile leaves the previous shader in use.
pub const GLIMM_SHADER_DIR_ENV: &str = "GPUI_GLIMM_SHADER_DIR";

const SHADER_FILES: [&str; 3] = ["glimm.metal", "glimm.wgsl", "glimm.hlsl"];
static SHADER_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// What the content shader draws in place of the transition, to inspect how an effect is built.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GlimmDebugView {
    /// The transition itself.
    #[default]
    Off,
    /// Which frame each pixel shows: old content black, new content white.
    Mix,
    /// Where each pixel samples from: red and green are the horizontal and vertical offset,
    /// mid-grey is no displacement.
    Displacement,
    /// Blur radius, black for sharp.
    Blur,
    /// The travelling front: crest in red, trough in green, cross-axis span mask in blue.
    Front,
}
impl GlimmDebugView {
    /// Every view, in the order a control cycles through them.
    pub const ALL: [Self; 5] = [
        Self::Off,
        Self::Mix,
        Self::Displacement,
        Self::Blur,
        Self::Front,
    ];
    /// A short label for a control.
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Mix => "Old / new",
            Self::Displacement => "Displacement",
            Self::Blur => "Blur",
            Self::Front => "Front",
        }
    }
    /// The value the shaders compare against; see the `debug` branch of each content function.
    pub(super) fn uniform(self) -> f32 {
        match self {
            Self::Off => 0.,
            Self::Mix => 1.,
            Self::Displacement => 2.,
            Self::Blur => 3.,
            Self::Front => 4.,
        }
    }
}

/// A renderer's shader text: built in, or followed on disk when [`GLIMM_SHADER_DIR_ENV`] is set.
#[doc(hidden)]
pub struct GlimmShaderSource {
    embedded: &'static str,
    path: Option<PathBuf>,
    seen: Option<SystemTime>,
    taken: bool,
}
impl GlimmShaderSource {
    /// `file_name` is looked up in the followed directory; `embedded` is the built-in text.
    pub fn new(file_name: &str, embedded: &'static str) -> Self {
        Self::following(shader_dir().map(|dir| dir.join(file_name)), embedded)
    }
    /// Follow `path` whatever the environment says; `None` uses the built-in text alone.
    pub fn following(path: Option<PathBuf>, embedded: &'static str) -> Self {
        Self {
            embedded,
            path,
            seen: None,
            taken: false,
        }
    }
    /// The text to compile now: on the first call, and again whenever the followed file changes.
    /// A followed file that cannot be read is reported and yields nothing; the caller then keeps
    /// its pipeline, or builds one from [`Self::embedded`] if it has none.
    pub fn take_changed(&mut self) -> Option<Cow<'static, str>> {
        let Some(path) = &self.path else {
            return (!mem::replace(&mut self.taken, true)).then_some(Cow::Borrowed(self.embedded));
        };
        let modified = fs::metadata(path).and_then(|data| data.modified()).ok();
        if self.taken && modified == self.seen {
            return None;
        }
        self.taken = true;
        self.seen = modified;
        match fs::read_to_string(path) {
            Ok(text) => Some(Cow::Owned(text)),
            Err(error) => {
                glimm_report_shader(Err(format!("read {}: {error}", path.display())));
                None
            }
        }
    }
    /// The built-in text, which always compiles.
    pub fn embedded(&self) -> &'static str {
        self.embedded
    }
}

/// Record the outcome of the latest shader build. Called by the platform renderers.
#[doc(hidden)]
pub fn glimm_report_shader(result: Result<(), String>) {
    if let Err(error) = &result {
        log::error!("glimm shader: {error}");
    }
    *SHADER_ERROR.lock().unwrap_or_else(PoisonError::into_inner) = result.err();
}
/// Why the followed shader is not the one in use, if its latest build failed.
pub fn glimm_shader_error() -> Option<String> {
    SHADER_ERROR
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}
/// When the followed shader files last changed. A view that polls this and refreshes its window
/// on a change sees an edited shader at once, even while its effect is paused.
pub fn glimm_shader_stamp() -> Option<SystemTime> {
    let dir = shader_dir()?;
    SHADER_FILES
        .iter()
        .filter_map(|name| {
            fs::metadata(dir.join(name))
                .and_then(|data| data.modified())
                .ok()
        })
        .max()
}
fn shader_dir() -> Option<PathBuf> {
    std::env::var_os(GLIMM_SHADER_DIR_ENV)
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}
