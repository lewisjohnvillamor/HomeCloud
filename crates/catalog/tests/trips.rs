//! Finding trips in a library of photographs.
//!
//! Pure arithmetic over dates and coordinates, so these run without a
//! database — which is the point of keeping the engine deterministic.

use homecloud_catalog::trips::{self, Trip};
use homecloud_catalog::{Item, ItemKind};
use homecloud_domain::identity::{ItemId, LibraryId};
use homecloud_storage::LibraryPath;
use time::macros::datetime;
use time::{Duration, OffsetDateTime};

/// A photograph taken at a place and a time.
fn photo(name: &str, taken: OffsetDateTime, latitude: f64, longitude: f64) -> Item {
    Item {
        id: ItemId::from_uuid(uuid::Uuid::new_v4()),
        library: LibraryId::from_uuid(uuid::Uuid::nil()),
        parent: None,
        path: LibraryPath::parse(name).expect("a path"),
        name: name.to_owned(),
        kind: ItemKind::File,
        size_bytes: 1000,
        content_type: Some("image/jpeg".to_owned()),
        modified_at: Some(taken),
        taken_at: Some(taken),
        camera: None,
        latitude: Some(latitude),
        longitude: Some(longitude),
        trashed_at: None,
        missing_since: None,
    }
}

/// A run of photographs at one place over consecutive hours.
fn run(prefix: &str, start: OffsetDateTime, count: i64, place: (f64, f64)) -> Vec<Item> {
    (0..count)
        .map(|index| {
            photo(
                &format!("{prefix}-{index}.jpg"),
                start + Duration::hours(index * 3),
                place.0,
                place.1,
            )
        })
        .collect()
}

fn keys(found: &[Trip]) -> Vec<String> {
    found.iter().map(|trip| trip.key.clone()).collect()
}

// London, and places at various distances from it.
const HOME: (f64, f64) = (51.5, -0.12);
const WALES: (f64, f64) = (51.48, -3.18);
const SYDNEY: (f64, f64) = (-33.87, 151.21);

#[test]
fn a_week_away_is_a_trip() {
    let mut items = run("home", datetime!(2026-01-05 09:00 UTC), 40, HOME);
    items.extend(run("wales", datetime!(2026-06-01 10:00 UTC), 8, WALES));

    let found = trips::find(&items);

    assert_eq!(found.len(), 1, "expected one trip, got {:?}", keys(&found));
    assert_eq!(found[0].items.len(), 8);
    assert!(
        found[0].key.starts_with("trip-2026-06-01"),
        "{}",
        found[0].key
    );
}

#[test]
fn photographs_taken_at_home_are_not_a_trip() {
    // The whole library is where the person lives. A memories engine
    // that calls this "a trip" is describing somebody's kitchen.
    let items = run("home", datetime!(2026-01-05 09:00 UTC), 60, HOME);

    assert!(trips::find(&items).is_empty());
}

#[test]
fn two_journeys_months_apart_are_two_trips() {
    let mut items = run("home", datetime!(2026-01-05 09:00 UTC), 60, HOME);
    items.extend(run("wales", datetime!(2026-06-01 10:00 UTC), 6, WALES));
    items.extend(run("sydney", datetime!(2026-09-10 10:00 UTC), 6, SYDNEY));

    let found = trips::find(&items);

    assert_eq!(found.len(), 2, "{:?}", keys(&found));
    // Most recent first: last month matters more than six months ago.
    assert!(found[0].started > found[1].started);
}

#[test]
fn a_couple_of_snaps_is_not_a_trip() {
    let mut items = run("home", datetime!(2026-01-05 09:00 UTC), 40, HOME);
    // Two photographs at a service station on the way somewhere.
    items.extend(run("services", datetime!(2026-06-01 10:00 UTC), 2, WALES));

    assert!(trips::find(&items).is_empty());
}

#[test]
fn a_long_gap_in_the_same_place_splits_the_trip() {
    let mut items = run("home", datetime!(2026-01-05 09:00 UTC), 60, HOME);
    items.extend(run("spring", datetime!(2026-04-01 10:00 UTC), 6, WALES));
    items.extend(run("autumn", datetime!(2026-10-01 10:00 UTC), 6, WALES));

    let found = trips::find(&items);

    // Same cottage, two holidays. One trip spanning six months would be
    // a memory of nothing.
    assert_eq!(found.len(), 2, "{:?}", keys(&found));
}

#[test]
fn a_library_with_no_home_still_finds_its_trips() {
    // Someone who only photographs while travelling. Inventing a home
    // here would suppress the very trips the library is made of.
    let mut items = run("wales", datetime!(2026-06-01 10:00 UTC), 6, WALES);
    items.extend(run("sydney", datetime!(2026-09-10 10:00 UTC), 6, SYDNEY));

    assert_eq!(trips::find(&items).len(), 2);
}

#[test]
fn photographs_with_no_place_are_ignored_rather_than_guessed_at() {
    let mut items = run("home", datetime!(2026-01-05 09:00 UTC), 40, HOME);
    let mut unplaced = photo("mystery.jpg", datetime!(2026-06-01 10:00 UTC), 0.0, 0.0);
    unplaced.latitude = None;
    unplaced.longitude = None;
    items.push(unplaced);

    // No coordinate means no opinion about where it was taken.
    assert!(trips::find(&items).is_empty());
}

#[test]
fn an_empty_library_has_no_trips() {
    assert!(trips::find(&[]).is_empty());
}

#[test]
fn the_same_library_produces_the_same_trips_whatever_order_it_arrives_in() {
    let mut items = run("home", datetime!(2026-01-05 09:00 UTC), 40, HOME);
    items.extend(run("wales", datetime!(2026-06-01 10:00 UTC), 8, WALES));

    let first = trips::find(&items);
    items.reverse();
    let second = trips::find(&items);

    // A memory hidden yesterday has to stay hidden today, which means
    // the key cannot depend on the order rows came back in.
    assert_eq!(keys(&first), keys(&second));
    assert!(!first.is_empty());
}
