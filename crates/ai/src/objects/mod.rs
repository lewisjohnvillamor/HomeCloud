//! Recognising what is in a photograph.
//!
//! Backed by YOLOX-s through ONNX Runtime. Both are found at runtime, the
//! same contract Tesseract and FFmpeg sign: the runtime is a shared
//! library the operator installs, the model is a file the operator points
//! at, and a deployment with neither builds, starts, and says the
//! capability is absent. ADR 0005 records why this model and this runtime.
//!
//! Only *which* things are in a picture comes out of here, never where.
//! That is all search needs, and it is less to keep about someone's
//! photographs.
//!
//! Everything in this module that touches the model is blocking and
//! CPU-bound. Callers run it on a blocking pool, never on an async
//! executor.

mod classes;
mod decode;

use std::path::Path;
use std::sync::Mutex;

use image::RgbImage;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;
use sha2::{Digest, Sha256};

use crate::AiError;

pub use classes::{Class, CLASSES};
pub use decode::{
    labels, letterbox, Label, UnexpectedOutput, INPUT_SIZE, MAX_LABELS, MIN_CONFIDENCE,
};

/// What the platform's loader is asked for when no path is configured,
/// so a runtime installed system-wide is found the way any shared library
/// is.
#[cfg(target_os = "windows")]
const DEFAULT_RUNTIME: &str = "onnxruntime.dll";
#[cfg(target_os = "macos")]
const DEFAULT_RUNTIME: &str = "libonnxruntime.dylib";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const DEFAULT_RUNTIME: &str = "libonnxruntime.so";

/// A loaded model, ready to look at pictures.
pub struct ObjectDetector {
    /// Running a session needs exclusive access. One picture at a time is
    /// also the right amount of work: the session already spreads each
    /// one across several cores.
    session: Mutex<Session>,
    model: String,
}

impl std::fmt::Debug for ObjectDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectDetector")
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

impl ObjectDetector {
    /// Loads the runtime and the model, and proves they work together.
    ///
    /// `runtime` is the ONNX Runtime shared library; `None` asks the
    /// platform's loader for it by its usual name. Blocking: it reads a
    /// file of tens of megabytes and runs the model once.
    pub fn load(runtime: Option<&Path>, model: &Path) -> Result<Self, AiError> {
        if !model.is_file() {
            return Err(AiError::Unavailable("object recognition"));
        }

        let runtime = runtime.unwrap_or(Path::new(DEFAULT_RUNTIME));
        ort::init_from(runtime)
            .map_err(|error| {
                tracing::debug!(error = %error, "ONNX Runtime could not be loaded");
                AiError::Unavailable("object recognition")
            })?
            .with_name("homecloud")
            .commit();

        let stamp = stamp(model)?;

        let session = open_session(model).map_err(|error| {
            tracing::warn!(error = %error, "the object model could not be loaded");
            AiError::Failed("the model could not be loaded".to_owned())
        })?;

        let detector = Self {
            session: Mutex::new(session),
            model: stamp,
        };

        // A model file with other classes, or other outputs, would load
        // happily and then label every photograph wrongly. One run on a
        // blank picture catches that before anything is written.
        detector.recognise(&RgbImage::new(INPUT_SIZE, INPUT_SIZE))?;

        Ok(detector)
    }

    /// Which model produced a label: the file name and the start of its
    /// SHA-256. Stored beside every label, so a different model file is
    /// recognised as different and its labels are rebuilt rather than
    /// mixed with the old ones.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The things in a picture, most confident first. An empty answer is
    /// an ordinary one: most photographs of a sky contain nothing.
    pub fn recognise(&self, image: &RgbImage) -> Result<Vec<Label>, AiError> {
        let side = i64::from(INPUT_SIZE);
        let input = Tensor::from_array(([1_i64, 3, side, side], letterbox(image)))
            .map_err(|error| AiError::Failed(error.to_string()))?;

        let mut session = self
            .session
            .lock()
            .map_err(|_| AiError::Failed("the model is unusable after a failure".to_owned()))?;

        let outputs = session.run(ort::inputs![input]).map_err(|error| {
            tracing::debug!(error = %error, "object recognition did not succeed");
            AiError::Failed("the model could not run".to_owned())
        })?;

        let (_, scores) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|_| AiError::Failed(UnexpectedOutput.to_string()))?;

        labels(scores).map_err(|error| AiError::Failed(error.to_string()))
    }
}

fn open_session(model: &Path) -> ort::Result<Session> {
    Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(intra_threads())?
        .with_inter_threads(1)?
        .commit_from_file(model)
}

/// Half the processor. Recognition runs in the background, and an upload
/// or a preview must still feel instant while it does.
fn intra_threads() -> usize {
    std::thread::available_parallelism()
        .map(|cores| (cores.get() / 2).max(1))
        .unwrap_or(1)
}

fn stamp(model: &Path) -> Result<String, AiError> {
    let bytes = std::fs::read(model)
        .map_err(|_| AiError::Unreadable("the model file could not be read".to_owned()))?;
    let digest = Sha256::digest(&bytes);
    let hex: String = digest[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let name = model
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "model".to_owned());

    Ok(format!("{name}#{hex}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_model_is_unavailable_not_a_failure() {
        let error = ObjectDetector::load(None, Path::new("/nonexistent/yolox_s.onnx")).unwrap_err();
        assert!(matches!(error, AiError::Unavailable(_)), "{error:?}");
    }

    #[test]
    fn a_missing_runtime_is_unavailable_not_a_failure() {
        let model = tempfile::NamedTempFile::new().unwrap();
        let error = ObjectDetector::load(
            Some(Path::new("/nonexistent/libonnxruntime.so")),
            model.path(),
        )
        .unwrap_err();
        assert!(matches!(error, AiError::Unavailable(_)), "{error:?}");
    }

    #[test]
    fn the_stamp_names_the_file_and_its_contents() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("yolox_s.onnx");

        std::fs::write(&path, b"one model").unwrap();
        let first = stamp(&path).unwrap();
        std::fs::write(&path, b"another model").unwrap();
        let second = stamp(&path).unwrap();

        assert!(first.starts_with("yolox_s.onnx#"), "{first}");
        assert_eq!(first.len(), "yolox_s.onnx#".len() + 12);
        assert_ne!(first, second, "a different model is a different stamp");
    }

    #[test]
    fn recognition_leaves_half_the_processor_free() {
        let cores = std::thread::available_parallelism().map_or(1, |cores| cores.get());
        assert!(intra_threads() >= 1);
        assert!(intra_threads() <= cores.div_ceil(2));
    }
}
