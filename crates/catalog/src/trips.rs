//! Trips: a run of photographs taken away from home, close together in
//! time and place.
//!
//! Deterministic on purpose. The memories engine has to work with AI
//! switched off, so this is arithmetic over capture dates and
//! coordinates — no model, no scoring, and the same library produces the
//! same trips tomorrow.
//!
//! "Away from home" is the part that makes a trip a trip. Most photos in
//! a personal library are taken where the person lives, and a memory
//! called "September in the kitchen" is not a memory. Home is inferred
//! as the place the library photographs most, and anything near it is
//! not a trip.

use time::{Duration, OffsetDateTime};

use crate::Item;

/// How far apart two places must be before they are different places.
/// About 25 km: far enough that a neighbouring town is not "home", close
/// enough that a day out stays one trip.
const AWAY_KILOMETRES: f64 = 25.0;

/// The longest gap inside one trip. Two days covers a night in a hotel
/// with no photographs; longer than that is a different journey.
const SAME_TRIP_GAP: Duration = Duration::days(2);

/// Fewest photographs worth calling a trip. Below this it is a couple of
/// snaps at a service station.
const MIN_PHOTOS: usize = 4;

/// Size of the grid cell home is counted in, in degrees. Roughly 11 km
/// at the equator — a town rather than a street.
const HOME_CELL: f64 = 0.1;

/// Separate days a place needs before it counts as home. Below this it
/// is somewhere the person went, not somewhere they live.
const HOME_DAYS: usize = 3;

/// A run of photographs taken away from home.
#[derive(Debug)]
pub struct Trip {
    /// Deterministic, so hiding one keeps it hidden tomorrow.
    pub key: String,
    pub started: OffsetDateTime,
    pub ended: OffsetDateTime,
    pub latitude: f64,
    pub longitude: f64,
    pub items: Vec<Item>,
}

/// Groups located photographs into trips.
///
/// Takes items rather than querying, so the caller decides how much of a
/// library to consider and this stays testable without a database.
pub fn find(items: &[Item]) -> Vec<Trip> {
    let mut located: Vec<&Item> = items
        .iter()
        .filter(|item| item.latitude.is_some() && item.longitude.is_some())
        .filter(|item| taken(item).is_some())
        .collect();

    if located.is_empty() {
        return Vec::new();
    }

    located.sort_by_key(|item| taken(item));

    let home = home_of(&located);

    // Walk in time order, breaking when the gap is too long or the place
    // has moved. A single pass, which is why this stays cheap on a large
    // library.
    let mut trips = Vec::new();
    let mut current: Vec<&Item> = Vec::new();

    for item in located {
        let start_new = match current.last() {
            None => true,
            Some(previous) => {
                let gap = taken(item).zip(taken(previous)).map(|(a, b)| a - b);
                let moved = distance(item, previous) > AWAY_KILOMETRES;

                gap.is_none_or(|gap| gap > SAME_TRIP_GAP) || moved
            }
        };

        if start_new && !current.is_empty() {
            if let Some(trip) = build(&current, home) {
                trips.push(trip);
            }
            current.clear();
        }

        current.push(item);
    }

    if let Some(trip) = build(&current, home) {
        trips.push(trip);
    }

    // Most recent first: a trip last month is more interesting than one
    // from six years ago.
    trips.sort_by(|a, b| b.started.cmp(&a.started));
    trips
}

/// Turns a run of photographs into a trip, if it is one.
fn build(run: &[&Item], home: Option<(f64, f64)>) -> Option<Trip> {
    if run.len() < MIN_PHOTOS {
        return None;
    }

    let latitude = average(run.iter().map(|item| item.latitude.unwrap_or_default()));
    let longitude = average(run.iter().map(|item| item.longitude.unwrap_or_default()));

    // Photographs taken where the person lives are not a trip.
    if let Some((home_latitude, home_longitude)) = home {
        if kilometres(latitude, longitude, home_latitude, home_longitude) <= AWAY_KILOMETRES {
            return None;
        }
    }

    let started = taken(run.first()?)?;
    let ended = taken(run.last()?)?;

    Some(Trip {
        // Keyed by when and roughly where, so the same trip has the same
        // key on every request.
        key: format!("trip-{}-{:.1}-{:.1}", started.date(), latitude, longitude),
        started,
        ended,
        latitude,
        longitude,
        items: run.iter().map(|item| (*item).clone()).collect(),
    })
}

/// Where this library photographs on the most separate days.
///
/// Counted in days rather than photographs, because a fortnight in Rome
/// produces more pictures than a month at home while home is the place
/// you keep coming back to. Counting photographs made the busiest
/// holiday look like home and suppressed the very trip it was.
///
/// Returns nothing when nowhere qualifies. A library belonging to
/// someone who only photographs while travelling has no home in it, and
/// inventing one would swallow every trip.
fn home_of(items: &[&Item]) -> Option<(f64, f64)> {
    use std::collections::{HashMap, HashSet};

    let mut days: HashMap<(i64, i64), HashSet<time::Date>> = HashMap::new();

    for item in items {
        let (Some(latitude), Some(longitude), Some(taken)) =
            (item.latitude, item.longitude, taken(item))
        else {
            continue;
        };

        let cell = (
            (latitude / HOME_CELL).floor() as i64,
            (longitude / HOME_CELL).floor() as i64,
        );
        days.entry(cell).or_default().insert(taken.date());
    }

    // Ties broken by the cell itself so the answer never depends on the
    // order rows arrived in — a memory hidden yesterday must stay hidden.
    let (cell, count) = days
        .into_iter()
        .map(|(cell, dates)| (cell, dates.len()))
        .max_by_key(|(cell, count)| (*count, *cell))?;

    if count < HOME_DAYS {
        return None;
    }

    Some((
        (cell.0 as f64 + 0.5) * HOME_CELL,
        (cell.1 as f64 + 0.5) * HOME_CELL,
    ))
}

fn average(values: impl Iterator<Item = f64>) -> f64 {
    let mut total = 0.0;
    let mut count = 0.0;

    for value in values {
        total += value;
        count += 1.0;
    }

    if count == 0.0 {
        0.0
    } else {
        total / count
    }
}

/// When the picture was taken, preferring the camera's own answer.
fn taken(item: &Item) -> Option<OffsetDateTime> {
    item.taken_at.or(item.modified_at)
}

fn distance(a: &Item, b: &Item) -> f64 {
    match (a.latitude, a.longitude, b.latitude, b.longitude) {
        (Some(a_lat), Some(a_lon), Some(b_lat), Some(b_lon)) => {
            kilometres(a_lat, a_lon, b_lat, b_lon)
        }
        _ => f64::MAX,
    }
}

/// Great-circle distance in kilometres.
///
/// The haversine formula: accurate to a fraction of a percent, which is
/// far more than deciding "same place or not" needs.
fn kilometres(a_lat: f64, a_lon: f64, b_lat: f64, b_lon: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;

    let latitude_difference = (b_lat - a_lat).to_radians();
    let longitude_difference = (b_lon - a_lon).to_radians();

    let a = (latitude_difference / 2.0).sin().powi(2)
        + a_lat.to_radians().cos()
            * b_lat.to_radians().cos()
            * (longitude_difference / 2.0).sin().powi(2);

    EARTH_RADIUS_KM * 2.0 * a.sqrt().asin()
}
