//! Object recognition against the real model and runtime, when present.
//!
//! Skipped with a note when they are not, the same way the OCR tests skip
//! without Tesseract: neither is something `cargo test` can fetch, and a
//! contributor without them still gets a useful run.
//!
//! - `HOMECLOUD_OBJECT_MODEL`: path to `yolox_s.onnx`.
//! - `HOMECLOUD_ONNXRUNTIME`: path to the ONNX Runtime shared library;
//!   optional when it is installed where the loader looks.
//! - `HOMECLOUD_TEST_DOG_PHOTO`: optional, a photograph with a dog in it.
//!   No such photograph is committed: there is no fixture here whose
//!   licence is clear enough to ship in an AGPL repository.

use std::path::PathBuf;

use homecloud_ai::objects::ObjectDetector;
use image::{Rgb, RgbImage};

fn detector() -> Option<ObjectDetector> {
    let Some(model) = std::env::var_os("HOMECLOUD_OBJECT_MODEL").map(PathBuf::from) else {
        eprintln!("skipping: set HOMECLOUD_OBJECT_MODEL to a YOLOX model to run this");
        return None;
    };
    let runtime = std::env::var_os("HOMECLOUD_ONNXRUNTIME").map(PathBuf::from);

    Some(ObjectDetector::load(runtime.as_deref(), &model).expect("the configured model loads"))
}

#[test]
fn a_blank_picture_contains_nothing() {
    let Some(detector) = detector() else { return };

    let grey = RgbImage::from_pixel(800, 600, Rgb([128, 128, 128]));
    assert_eq!(detector.recognise(&grey).expect("recognition runs"), []);
}

#[test]
fn the_model_is_stamped_by_its_contents() {
    let Some(detector) = detector() else { return };

    let (name, hash) = detector.model().split_once('#').expect("name#hash");
    assert!(name.ends_with(".onnx"), "{name}");
    assert_eq!(hash.len(), 12);
}

#[test]
fn a_dog_is_recognised_as_a_dog() {
    let Some(detector) = detector() else { return };
    let Some(photo) = std::env::var_os("HOMECLOUD_TEST_DOG_PHOTO") else {
        eprintln!("skipping: set HOMECLOUD_TEST_DOG_PHOTO to a photograph of a dog");
        return;
    };

    let image = image::open(photo).expect("the photograph opens").to_rgb8();
    let labels = detector.recognise(&image).expect("recognition runs");
    let names: Vec<_> = labels.iter().map(|label| label.name()).collect();

    assert!(names.contains(&"dog"), "saw {names:?}");
}
