//! Turning a picture into the model's input, and the model's output into
//! labels.
//!
//! Pure functions, so every rule here is tested without a model or a
//! runtime installed.

use image::imageops::{self, FilterType};
use image::RgbImage;

use super::classes::CLASSES;

/// Side of the square the model was exported for.
pub const INPUT_SIZE: u32 = 640;

/// The grey YOLOX pads with during training. Padding with anything else
/// is a small distribution shift the model never saw.
const PAD_VALUE: f32 = 114.0;

/// Values per output row: four box coordinates, objectness, then one
/// score per class.
const ROW_WIDTH: usize = 5 + CLASSES.len();

/// The least confidence a label needs to be kept.
///
/// High on purpose. A missed label costs a search that finds a little
/// less; a wrong one puts a stranger's photograph of a sofa into a search
/// for "dog", and that is what erodes trust in the whole feature.
pub const MIN_CONFIDENCE: f32 = 0.5;

/// Most labels kept for one picture. A busy street scene can contain
/// dozens of things; past this many the rest are noise for search.
pub const MAX_LABELS: usize = 12;

/// Something recognised in a picture.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    /// Index into [`CLASSES`].
    pub class: usize,
    /// The model's confidence, 0 to 1.
    pub confidence: f32,
}

impl Label {
    pub fn name(&self) -> &'static str {
        CLASSES[self.class].name
    }
}

/// Lays a picture out the way YOLOX was trained to see it.
///
/// Scaled to fit 640×640 without changing its shape, anchored top-left,
/// the rest filled with grey; then planar BGR as unnormalised floats —
/// the preprocessing the published YOLOX models expect, which is not the
/// ImageNet normalisation most vision models use.
pub fn letterbox(image: &RgbImage) -> Vec<f32> {
    let side = INPUT_SIZE as usize;
    let plane = side * side;
    let mut input = vec![PAD_VALUE; 3 * plane];

    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return input;
    }

    let scale = f64::from(INPUT_SIZE) / f64::from(width.max(height));
    // At least one pixel, so a 1×4000 strip does not vanish entirely.
    let scaled_width = ((f64::from(width) * scale).round() as u32).clamp(1, INPUT_SIZE);
    let scaled_height = ((f64::from(height) * scale).round() as u32).clamp(1, INPUT_SIZE);

    // Triangle is bilinear, matching the OpenCV resize the model was
    // trained behind. Lanczos would be sharper and slower for no gain.
    let resized = if (scaled_width, scaled_height) == (width, height) {
        image.clone()
    } else {
        imageops::resize(image, scaled_width, scaled_height, FilterType::Triangle)
    };

    for (x, y, pixel) in resized.enumerate_pixels() {
        let offset = y as usize * side + x as usize;
        let [red, green, blue] = pixel.0;
        input[offset] = f32::from(blue);
        input[plane + offset] = f32::from(green);
        input[2 * plane + offset] = f32::from(red);
    }

    input
}

/// Why model output could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the model returned output of an unexpected shape")]
pub struct UnexpectedOutput;

/// The things in a picture, most confident first.
///
/// For each class, the best score over every candidate the model
/// proposed. Where things are is never kept — search only needs what is
/// there — so there is no box decoding and no overlap suppression: two
/// overlapping boxes around one dog and two dogs both answer "dog".
///
/// `output` is the model's single output, flattened: rows of
/// `[x, y, w, h, objectness, class scores…]`, with objectness and class
/// scores already passed through a sigmoid by the exported model.
pub fn labels(output: &[f32]) -> Result<Vec<Label>, UnexpectedOutput> {
    if output.is_empty() || !output.len().is_multiple_of(ROW_WIDTH) {
        return Err(UnexpectedOutput);
    }

    let mut best = [0.0_f32; CLASSES.len()];

    for row in output.chunks_exact(ROW_WIDTH) {
        let objectness = row[4];
        // Most rows are background. Skipping them early is most of the
        // work this function does not do.
        if objectness.is_nan() || objectness < MIN_CONFIDENCE {
            continue;
        }

        for (class, score) in row[5..].iter().enumerate() {
            let confidence = objectness * score;
            if confidence > best[class] {
                best[class] = confidence;
            }
        }
    }

    let mut found: Vec<Label> = best
        .iter()
        .enumerate()
        .filter(|(_, confidence)| **confidence >= MIN_CONFIDENCE)
        .map(|(class, confidence)| Label {
            class,
            confidence: confidence.min(1.0),
        })
        .collect();

    found.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    found.truncate(MAX_LABELS);

    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    fn row(objectness: f32, class: usize, score: f32) -> Vec<f32> {
        let mut row = vec![0.0; ROW_WIDTH];
        row[4] = objectness;
        row[5 + class] = score;
        row
    }

    fn output(rows: &[Vec<f32>]) -> Vec<f32> {
        rows.concat()
    }

    const DOG: usize = 16;
    const PERSON: usize = 0;

    #[test]
    fn a_confident_detection_becomes_a_label() {
        let found = labels(&output(&[row(0.9, DOG, 0.95)])).unwrap();

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name(), "dog");
        assert!((found[0].confidence - 0.855).abs() < 1e-4);
    }

    #[test]
    fn a_doubtful_detection_is_not_a_label() {
        // Confident something is there, unsure what: 0.9 × 0.5 = 0.45.
        assert!(labels(&output(&[row(0.9, DOG, 0.5)])).unwrap().is_empty());
        // Sure what it would be, unsure anything is there.
        assert!(labels(&output(&[row(0.3, DOG, 1.0)])).unwrap().is_empty());
    }

    #[test]
    fn many_boxes_around_one_thing_are_one_label_with_the_best_score() {
        let found = labels(&output(&[
            row(0.8, DOG, 0.8),
            row(0.95, DOG, 0.9),
            row(0.7, DOG, 0.9),
        ]))
        .unwrap();

        assert_eq!(found.len(), 1);
        assert!((found[0].confidence - 0.855).abs() < 1e-4);
    }

    #[test]
    fn labels_come_most_confident_first() {
        let found = labels(&output(&[row(0.7, DOG, 0.8), row(0.99, PERSON, 0.99)])).unwrap();

        let names: Vec<_> = found.iter().map(Label::name).collect();
        assert_eq!(names, ["person", "dog"]);
    }

    #[test]
    fn a_crowded_scene_is_bounded() {
        let rows: Vec<_> = (0..CLASSES.len())
            .map(|class| row(1.0, class, 0.9))
            .collect();
        assert_eq!(labels(&output(&rows)).unwrap().len(), MAX_LABELS);
    }

    #[test]
    fn an_empty_scene_is_an_answer_not_an_error() {
        let background = output(&[row(0.01, DOG, 0.1), row(0.0, PERSON, 0.0)]);
        assert_eq!(labels(&background), Ok(Vec::new()));
    }

    #[test]
    fn output_of_the_wrong_shape_is_refused_rather_than_misread() {
        // A different model file — say one trained on other classes —
        // must not be read as if its columns meant these.
        assert_eq!(labels(&[]), Err(UnexpectedOutput));
        assert_eq!(labels(&vec![0.5; ROW_WIDTH + 1]), Err(UnexpectedOutput));
    }

    #[test]
    fn not_a_number_is_ignored_rather_than_trusted() {
        let mut broken = row(f32::NAN, DOG, f32::NAN);
        broken.extend(row(0.9, PERSON, f32::NAN));
        assert_eq!(labels(&broken), Ok(Vec::new()));
    }

    #[test]
    fn the_input_is_three_square_planes() {
        let input = letterbox(&RgbImage::new(32, 32));
        assert_eq!(input.len(), 3 * 640 * 640);
    }

    #[test]
    fn channels_are_blue_green_red_and_unnormalised() {
        let input = letterbox(&RgbImage::from_pixel(8, 8, Rgb([10, 20, 30])));
        let plane = 640 * 640;

        assert_eq!(input[0], 30.0, "blue first");
        assert_eq!(input[plane], 20.0, "then green");
        assert_eq!(input[2 * plane], 10.0, "then red");
    }

    #[test]
    fn a_wide_picture_is_padded_below_not_stretched() {
        // 2:1, so it fills the width and half the height.
        let input = letterbox(&RgbImage::from_pixel(200, 100, Rgb([0, 0, 0])));

        let at = |x: usize, y: usize| input[y * 640 + x];
        assert_eq!(at(639, 0), 0.0, "the picture reaches the right edge");
        assert_eq!(at(0, 319), 0.0, "and half way down");
        assert_eq!(at(0, 321), 114.0, "below it is padding");
        assert_eq!(at(639, 639), 114.0);
    }

    #[test]
    fn a_tall_picture_is_padded_to_the_right() {
        let input = letterbox(&RgbImage::from_pixel(100, 200, Rgb([0, 0, 0])));

        let at = |x: usize, y: usize| input[y * 640 + x];
        assert_eq!(at(0, 639), 0.0);
        assert_eq!(at(321, 0), 114.0);
    }

    #[test]
    fn a_sliver_survives_scaling() {
        let input = letterbox(&RgbImage::from_pixel(1, 4000, Rgb([0, 0, 0])));
        assert_eq!(input[0], 0.0, "at least one column of picture remains");
    }

    #[test]
    fn an_empty_picture_is_all_padding() {
        let input = letterbox(&RgbImage::new(0, 0));
        assert!(input.iter().all(|value| *value == 114.0));
    }
}
