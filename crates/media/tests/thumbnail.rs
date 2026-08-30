//! Thumbnail generation, tested with the kinds of files a real library
//! contains — including the ones an attacker would put there.

use std::io::Cursor;

use homecloud_media::thumbnail::{is_thumbnailable, ThumbnailSize};
use homecloud_media::{generate_thumbnail, MediaError};
use image::{DynamicImage, ImageFormat, RgbImage};

/// Encodes a solid-colour image of the given size in the given format.
fn image_bytes(width: u32, height: u32, format: ImageFormat) -> Vec<u8> {
    let mut buffer = RgbImage::new(width, height);
    for (x, y, pixel) in buffer.enumerate_pixels_mut() {
        // A gradient rather than a flat colour, so resizing has
        // something to actually resample.
        *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]);
    }

    let mut output = Vec::new();
    DynamicImage::ImageRgb8(buffer)
        .write_to(&mut Cursor::new(&mut output), format)
        .expect("encode test image");

    output
}

fn dimensions(bytes: &[u8]) -> (u32, u32) {
    image::load_from_memory(bytes)
        .map(|image| (image.width(), image.height()))
        .expect("the thumbnail is a readable image")
}

#[test]
fn a_large_photo_is_reduced_to_the_requested_edge() {
    let source = image_bytes(1600, 1200, ImageFormat::Jpeg);

    let thumbnail = generate_thumbnail(&source, ThumbnailSize::Small).expect("thumbnail");

    let (width, height) = dimensions(&thumbnail);
    assert_eq!(width, 320);
    // Aspect ratio is preserved rather than cropped.
    assert_eq!(height, 240);
    assert!(
        thumbnail.len() < source.len(),
        "the thumbnail should be smaller than the original"
    );
}

#[test]
fn each_size_has_its_own_bound() {
    let source = image_bytes(2000, 2000, ImageFormat::Jpeg);

    for size in [
        ThumbnailSize::Small,
        ThumbnailSize::Medium,
        ThumbnailSize::Large,
    ] {
        let thumbnail = generate_thumbnail(&source, size).expect("thumbnail");

        let (width, height) = dimensions(&thumbnail);
        assert_eq!(width.max(height), size.max_edge(), "{}", size.as_str());
    }
}

#[test]
fn an_image_smaller_than_the_target_is_not_enlarged() {
    let source = image_bytes(100, 80, ImageFormat::Png);

    let thumbnail = generate_thumbnail(&source, ThumbnailSize::Large).expect("thumbnail");

    assert_eq!(dimensions(&thumbnail), (100, 80));
}

#[test]
fn every_supported_format_produces_a_thumbnail() {
    for format in [
        ImageFormat::Jpeg,
        ImageFormat::Png,
        ImageFormat::Gif,
        ImageFormat::Bmp,
        ImageFormat::Tiff,
    ] {
        let source = image_bytes(600, 400, format);

        let thumbnail = generate_thumbnail(&source, ThumbnailSize::Small)
            .unwrap_or_else(|error| panic!("{format:?} failed: {error}"));

        assert_eq!(dimensions(&thumbnail).0, 320, "{format:?}");
    }
}

#[test]
fn an_image_with_transparency_is_flattened_rather_than_refused() {
    let mut buffer = image::RgbaImage::new(200, 200);
    for pixel in buffer.pixels_mut() {
        *pixel = image::Rgba([10, 20, 30, 128]);
    }
    let mut source = Vec::new();
    DynamicImage::ImageRgba8(buffer)
        .write_to(&mut Cursor::new(&mut source), ImageFormat::Png)
        .expect("encode");

    let thumbnail = generate_thumbnail(&source, ThumbnailSize::Small).expect("thumbnail");

    assert_eq!(dimensions(&thumbnail), (200, 200));
}

// --- Hostile input ---

#[test]
fn the_declared_extension_does_not_choose_the_decoder() {
    // A PNG whose name would say JPEG: the bytes decide, so this works.
    let source = image_bytes(400, 300, ImageFormat::Png);

    assert!(generate_thumbnail(&source, ThumbnailSize::Small).is_ok());
}

#[test]
fn a_non_image_is_refused_without_panicking() {
    let source = b"#!/bin/sh\nrm -rf /\n";

    let error = generate_thumbnail(source, ThumbnailSize::Small).expect_err("not an image");

    assert!(
        matches!(error, MediaError::UnsupportedFormat | MediaError::Damaged),
        "{error:?}"
    );
}

#[test]
fn a_truncated_image_is_reported_as_damaged() {
    let source = image_bytes(800, 600, ImageFormat::Png);
    let truncated = &source[..source.len() / 3];

    let error = generate_thumbnail(truncated, ThumbnailSize::Small).expect_err("truncated");

    assert!(matches!(error, MediaError::Damaged), "{error:?}");
}

#[test]
fn an_empty_file_is_refused() {
    assert!(generate_thumbnail(&[], ThumbnailSize::Small).is_err());
}

#[test]
fn a_decompression_bomb_is_refused_before_any_pixels_are_allocated() {
    // A PNG header describing a 30,000 × 30,000 image: 900 megapixels,
    // which would need gigabytes of memory once decoded. The file itself
    // is a few hundred bytes because the pixel data is all zeroes.
    let mut buffer = image::GrayImage::new(1, 1);
    buffer.put_pixel(0, 0, image::Luma([0]));
    let mut bomb = Vec::new();
    DynamicImage::ImageLuma8(buffer)
        .write_to(&mut Cursor::new(&mut bomb), ImageFormat::Png)
        .expect("encode");

    // Rewrite the IHDR width and height in place, then fix the checksum.
    let ihdr_start = 12;
    bomb[ihdr_start + 4..ihdr_start + 8].copy_from_slice(&30_000u32.to_be_bytes());
    bomb[ihdr_start + 8..ihdr_start + 12].copy_from_slice(&30_000u32.to_be_bytes());
    let crc = crc32(&bomb[ihdr_start..ihdr_start + 17]);
    bomb[ihdr_start + 17..ihdr_start + 21].copy_from_slice(&crc.to_be_bytes());

    let started = std::time::Instant::now();
    let error = generate_thumbnail(&bomb, ThumbnailSize::Small).expect_err("bomb");

    assert_eq!(error, MediaError::TooLarge);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "the bomb was not refused quickly"
    );
}

#[test]
fn only_image_content_types_are_offered_a_thumbnail() {
    assert!(is_thumbnailable(Some("image/jpeg")));
    assert!(is_thumbnailable(Some("image/webp")));
    assert!(!is_thumbnailable(Some("image/svg+xml")));
    assert!(!is_thumbnailable(Some("video/mp4")));
    assert!(!is_thumbnailable(Some("application/pdf")));
    assert!(!is_thumbnailable(None));
}

/// Minimal CRC-32 so the bomb test can build a valid PNG header.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;

    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }

    !crc
}
