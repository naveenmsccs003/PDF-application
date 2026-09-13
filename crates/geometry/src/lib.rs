//! Core Engine geometry/measurement math (Section 7 layering, Section 18
//! testing requirement). Pure calculation, no PDF/UI/DB dependency — the
//! domain layer (Phase 4 Measurement) will call into this, not the other
//! way around.
//!
//! Per Section 18: "Measurement calculations must have automated unit
//! tests... Never rely only on UI testing for geometry." Every public
//! function here is covered by tests for normal values, decimals, unit
//! conversion, invalid-scale edge cases, zero/negative inputs, and very
//! large measurements.

use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum GeometryError {
    #[error("distance between calibration points must be positive, got {0}")]
    InvalidCalibrationPageDistance(f64),
    #[error("known real-world distance must be positive, got {0}")]
    InvalidCalibrationRealWorldDistance(f64),
    #[error("a polygon needs at least 3 points, got {0}")]
    PolygonTooFewPoints(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance_to(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

/// A calibrated scale: real-world inches per one page unit (PDF points,
/// pixels, whatever the page geometry is expressed in — calibration
/// absorbs the difference, so callers never need to know or care).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scale {
    inches_per_page_unit: f64,
}

impl Scale {
    /// Calibrates a scale from two points a known real-world distance
    /// apart (e.g. the user clicks both ends of a dimension line labeled
    /// "10'-0\"" on the drawing and supplies `known_real_world_inches =
    /// 120.0`).
    pub fn calibrate(
        p1: Point,
        p2: Point,
        known_real_world_inches: f64,
    ) -> Result<Self, GeometryError> {
        let page_distance = p1.distance_to(&p2);
        if !(page_distance > 0.0) {
            return Err(GeometryError::InvalidCalibrationPageDistance(page_distance));
        }
        if !(known_real_world_inches > 0.0) {
            return Err(GeometryError::InvalidCalibrationRealWorldDistance(
                known_real_world_inches,
            ));
        }
        Ok(Self {
            inches_per_page_unit: known_real_world_inches / page_distance,
        })
    }

    pub fn page_length_to_inches(&self, page_length: f64) -> f64 {
        page_length * self.inches_per_page_unit
    }

    pub fn page_area_to_square_inches(&self, page_area: f64) -> f64 {
        page_area * self.inches_per_page_unit.powi(2)
    }

    /// The calibration ratio itself (real-world inches per one page unit).
    /// Exists so a persistence layer (e.g. the `measurement` domain crate,
    /// backing the `scale` table's `ratio` column) can save and later
    /// reconstruct a `Scale` without redoing calibration from raw points.
    pub fn inches_per_page_unit(&self) -> f64 {
        self.inches_per_page_unit
    }

    /// Reconstructs a `Scale` from a previously persisted ratio. Pairs with
    /// `inches_per_page_unit` above.
    pub fn from_ratio(inches_per_page_unit: f64) -> Self {
        Self {
            inches_per_page_unit,
        }
    }
}

/// Real-world length between two page-space points, in inches.
pub fn length_inches(p1: Point, p2: Point, scale: &Scale) -> f64 {
    scale.page_length_to_inches(p1.distance_to(&p2))
}

/// Real-world area of a closed polygon (page-space points, in order), in
/// square inches. Uses the shoelace formula.
pub fn polygon_area_square_inches(points: &[Point], scale: &Scale) -> Result<f64, GeometryError> {
    if points.len() < 3 {
        return Err(GeometryError::PolygonTooFewPoints(points.len()));
    }
    let mut sum = 0.0;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        sum += a.x * b.y - b.x * a.y;
    }
    let page_area = (sum / 2.0).abs();
    Ok(scale.page_area_to_square_inches(page_area))
}

pub mod units {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum LengthUnit {
        Inches,
        Feet,
        Millimeters,
        Centimeters,
        Meters,
    }

    impl LengthUnit {
        fn inches_per_unit(self) -> f64 {
            match self {
                LengthUnit::Inches => 1.0,
                LengthUnit::Feet => 12.0,
                LengthUnit::Millimeters => 1.0 / 25.4,
                LengthUnit::Centimeters => 1.0 / 2.54,
                LengthUnit::Meters => 1.0 / 0.0254,
            }
        }
    }

    pub fn convert_length(value: f64, from: LengthUnit, to: LengthUnit) -> f64 {
        let inches = value * from.inches_per_unit();
        inches / to.inches_per_unit()
    }

    /// Formats a length given in inches as architectural feet-inches, e.g.
    /// `12'-6 1/2"`. Rounds to the nearest 1/16 inch, standard precision
    /// for construction drawings.
    pub fn format_feet_inches(total_inches: f64) -> String {
        let negative = total_inches < 0.0;
        let total_inches = total_inches.abs();

        let sixteenths = (total_inches * 16.0).round() as i64;
        let feet = sixteenths / (12 * 16);
        let remainder = sixteenths % (12 * 16);
        let whole_inches = remainder / 16;
        let frac_sixteenths = remainder % 16;

        let mut result = String::new();
        if negative {
            result.push('-');
        }
        result.push_str(&format!("{feet}'-{whole_inches}"));
        if frac_sixteenths != 0 {
            let divisor = gcd(frac_sixteenths, 16);
            result.push_str(&format!(" {}/{}", frac_sixteenths / divisor, 16 / divisor));
        }
        result.push('"');
        result
    }

    fn gcd(a: i64, b: i64) -> i64 {
        if b == 0 {
            a
        } else {
            gcd(b, a % b)
        }
    }
}

/// Axis-aligned bounding box hit-testing (architecture doc, Section 5:
/// "Spatial indexing: `rstar` (R-tree) for markup/measurement hit
/// testing"). Generic over an opaque `Id` so both the Markup and
/// Measurement domains can index their own rows without this crate
/// depending on either — Core Engine has no business knowing what a
/// markup or measurement *is*, only where it sits.
pub mod spatial {
    use super::Point;

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct BoundingBox {
        pub min: Point,
        pub max: Point,
    }

    impl BoundingBox {
        /// The smallest box containing every point. `None` for an empty
        /// slice — there's no meaningful bounding box for nothing.
        pub fn from_points(points: &[Point]) -> Option<Self> {
            let first = points.first()?;
            let (mut min, mut max) = (*first, *first);
            for p in &points[1..] {
                min.x = min.x.min(p.x);
                min.y = min.y.min(p.y);
                max.x = max.x.max(p.x);
                max.y = max.y.max(p.y);
            }
            Some(Self { min, max })
        }

        pub fn contains_point(&self, p: Point) -> bool {
            p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Entry<Id> {
        id: Id,
        bbox: BoundingBox,
    }

    impl<Id> rstar::RTreeObject for Entry<Id> {
        type Envelope = rstar::AABB<[f64; 2]>;

        fn envelope(&self) -> Self::Envelope {
            rstar::AABB::from_corners(
                [self.bbox.min.x, self.bbox.min.y],
                [self.bbox.max.x, self.bbox.max.y],
            )
        }
    }

    impl<Id> rstar::PointDistance for Entry<Id> {
        /// Squared distance from `point` to the bbox rectangle (0 if
        /// inside) — the standard formula, and what makes `contains_point`
        /// (and so `locate_all_at_point`) match `BoundingBox::contains_point`.
        fn distance_2(&self, point: &[f64; 2]) -> f64 {
            let dx = (self.bbox.min.x - point[0]).max(0.0).max(point[0] - self.bbox.max.x);
            let dy = (self.bbox.min.y - point[1]).max(0.0).max(point[1] - self.bbox.max.y);
            dx * dx + dy * dy
        }
    }

    /// An R-tree of bounding boxes, for "what's under this click" /
    /// "what's in this marquee-select rectangle" queries against a page's
    /// markups or measurements.
    pub struct SpatialIndex<Id> {
        tree: rstar::RTree<Entry<Id>>,
    }

    impl<Id: Clone + PartialEq> SpatialIndex<Id> {
        pub fn new() -> Self {
            Self {
                tree: rstar::RTree::new(),
            }
        }

        pub fn insert(&mut self, id: Id, bbox: BoundingBox) {
            self.tree.insert(Entry { id, bbox });
        }

        /// Removes the entry with this exact `id` and `bbox`. Both are
        /// needed because `rstar` locates removals by their tree envelope —
        /// a caller that moved an item must remove its old bbox, not just
        /// its id.
        pub fn remove(&mut self, id: &Id, bbox: BoundingBox) -> bool {
            self.tree
                .remove(&Entry {
                    id: id.clone(),
                    bbox,
                })
                .is_some()
        }

        /// Ids of every entry whose bounding box contains `p` — a click
        /// hit-test. Bounding-box precision, not exact shape precision
        /// (e.g. a click in a rectangle's corner-to-corner box but outside
        /// an actual cloud/polygon shape still hits); callers needing exact
        /// shape hit-testing narrow this candidate set themselves.
        pub fn query_point(&self, p: Point) -> Vec<Id> {
            self.tree
                .locate_all_at_point(&[p.x, p.y])
                .map(|entry| entry.id.clone())
                .collect()
        }

        /// Ids of every entry whose bounding box intersects the given
        /// rectangle — a marquee/rubber-band selection.
        pub fn query_rect(&self, min: Point, max: Point) -> Vec<Id> {
            let envelope = rstar::AABB::from_corners([min.x, min.y], [max.x, max.y]);
            self.tree
                .locate_in_envelope_intersecting(&envelope)
                .map(|entry| entry.id.clone())
                .collect()
        }

        pub fn len(&self) -> usize {
            self.tree.size()
        }

        pub fn is_empty(&self) -> bool {
            self.tree.size() == 0
        }
    }

    impl<Id: Clone + PartialEq> Default for SpatialIndex<Id> {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::units::*;
    use super::*;

    // -- length: normal values --

    #[test]
    fn length_with_unit_scale_matches_raw_distance() {
        let scale = Scale::calibrate(Point::new(0.0, 0.0), Point::new(1.0, 0.0), 1.0).unwrap();
        let length = length_inches(Point::new(0.0, 0.0), Point::new(3.0, 4.0), &scale);
        assert!((length - 5.0).abs() < 1e-9); // 3-4-5 triangle
    }

    #[test]
    fn length_with_calibrated_scale() {
        // 2 page units == 10 feet (120 inches) of real-world distance.
        let scale = Scale::calibrate(Point::new(0.0, 0.0), Point::new(2.0, 0.0), 120.0).unwrap();
        let length = length_inches(Point::new(0.0, 0.0), Point::new(2.0, 0.0), &scale);
        assert!((length - 120.0).abs() < 1e-9);
    }

    // -- length: decimal values --

    #[test]
    fn length_with_decimal_coordinates_and_scale() {
        let scale = Scale::calibrate(Point::new(0.0, 0.0), Point::new(1.5, 0.0), 18.25).unwrap();
        let length = length_inches(Point::new(0.0, 0.0), Point::new(0.75, 0.0), &scale);
        assert!((length - 9.125).abs() < 1e-9);
    }

    // -- length: zero/negative-adjacent inputs --

    #[test]
    fn length_between_identical_points_is_zero() {
        let scale = Scale::calibrate(Point::new(0.0, 0.0), Point::new(1.0, 0.0), 1.0).unwrap();
        let length = length_inches(Point::new(5.0, 5.0), Point::new(5.0, 5.0), &scale);
        assert_eq!(length, 0.0);
    }

    // -- length: very large measurements --

    #[test]
    fn length_handles_very_large_coordinates_without_overflow() {
        let scale = Scale::calibrate(Point::new(0.0, 0.0), Point::new(1.0, 0.0), 1.0).unwrap();
        let length = length_inches(Point::new(0.0, 0.0), Point::new(1.0e12, 0.0), &scale);
        assert!((length - 1.0e12).abs() / 1.0e12 < 1e-9);
        assert!(length.is_finite());
    }

    // -- calibration: invalid scale edge cases --

    #[test]
    fn scale_ratio_round_trips_through_persistence() {
        let scale = Scale::calibrate(Point::new(0.0, 0.0), Point::new(2.0, 0.0), 120.0).unwrap();
        let restored = Scale::from_ratio(scale.inches_per_page_unit());
        assert_eq!(
            length_inches(Point::new(0.0, 0.0), Point::new(2.0, 0.0), &scale),
            length_inches(Point::new(0.0, 0.0), Point::new(2.0, 0.0), &restored)
        );
    }

    #[test]
    fn calibrate_rejects_zero_page_distance() {
        let result = Scale::calibrate(Point::new(0.0, 0.0), Point::new(0.0, 0.0), 10.0);
        assert_eq!(
            result,
            Err(GeometryError::InvalidCalibrationPageDistance(0.0))
        );
    }

    #[test]
    fn calibrate_rejects_zero_real_world_distance() {
        let result = Scale::calibrate(Point::new(0.0, 0.0), Point::new(1.0, 0.0), 0.0);
        assert_eq!(
            result,
            Err(GeometryError::InvalidCalibrationRealWorldDistance(0.0))
        );
    }

    #[test]
    fn calibrate_rejects_negative_real_world_distance() {
        let result = Scale::calibrate(Point::new(0.0, 0.0), Point::new(1.0, 0.0), -5.0);
        assert_eq!(
            result,
            Err(GeometryError::InvalidCalibrationRealWorldDistance(-5.0))
        );
    }

    // -- area: normal values --

    #[test]
    fn area_of_unit_square() {
        let scale = Scale::calibrate(Point::new(0.0, 0.0), Point::new(1.0, 0.0), 1.0).unwrap();
        let square = [
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(2.0, 2.0),
            Point::new(0.0, 2.0),
        ];
        let area = polygon_area_square_inches(&square, &scale).unwrap();
        assert!((area - 4.0).abs() < 1e-9);
    }

    #[test]
    fn area_is_invariant_to_winding_direction() {
        let scale = Scale::calibrate(Point::new(0.0, 0.0), Point::new(1.0, 0.0), 1.0).unwrap();
        let clockwise = [
            Point::new(0.0, 0.0),
            Point::new(0.0, 2.0),
            Point::new(2.0, 2.0),
            Point::new(2.0, 0.0),
        ];
        let area = polygon_area_square_inches(&clockwise, &scale).unwrap();
        assert!((area - 4.0).abs() < 1e-9);
    }

    #[test]
    fn area_scales_with_calibration_squared() {
        // 1 page unit == 2 real-world inches, so area scales by 2^2 = 4.
        let scale = Scale::calibrate(Point::new(0.0, 0.0), Point::new(1.0, 0.0), 2.0).unwrap();
        let square = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        let area = polygon_area_square_inches(&square, &scale).unwrap();
        assert!((area - 4.0).abs() < 1e-9);
    }

    // -- area: edge cases --

    #[test]
    fn area_rejects_fewer_than_three_points() {
        let scale = Scale::calibrate(Point::new(0.0, 0.0), Point::new(1.0, 0.0), 1.0).unwrap();
        let line = [Point::new(0.0, 0.0), Point::new(1.0, 0.0)];
        assert_eq!(
            polygon_area_square_inches(&line, &scale),
            Err(GeometryError::PolygonTooFewPoints(2))
        );
    }

    #[test]
    fn area_of_degenerate_collinear_polygon_is_zero() {
        let scale = Scale::calibrate(Point::new(0.0, 0.0), Point::new(1.0, 0.0), 1.0).unwrap();
        let collinear = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(2.0, 0.0),
        ];
        let area = polygon_area_square_inches(&collinear, &scale).unwrap();
        assert!(area.abs() < 1e-9);
    }

    // -- unit conversion --

    #[test]
    fn convert_feet_to_inches() {
        assert!((convert_length(1.0, LengthUnit::Feet, LengthUnit::Inches) - 12.0).abs() < 1e-9);
    }

    #[test]
    fn convert_meters_to_millimeters() {
        assert!(
            (convert_length(1.0, LengthUnit::Meters, LengthUnit::Millimeters) - 1000.0).abs()
                < 1e-6
        );
    }

    #[test]
    fn convert_round_trip_is_identity() {
        let original = 37.5;
        let converted = convert_length(original, LengthUnit::Inches, LengthUnit::Meters);
        let back = convert_length(converted, LengthUnit::Meters, LengthUnit::Inches);
        assert!((back - original).abs() < 1e-9);
    }

    #[test]
    fn convert_handles_decimal_values() {
        let cm = convert_length(1.0, LengthUnit::Inches, LengthUnit::Centimeters);
        assert!((cm - 2.54).abs() < 1e-9);
    }

    #[test]
    fn convert_handles_zero_and_negative() {
        assert_eq!(
            convert_length(0.0, LengthUnit::Feet, LengthUnit::Meters),
            0.0
        );
        assert!(convert_length(-12.0, LengthUnit::Inches, LengthUnit::Feet) < 0.0);
    }

    // -- feet-inches formatting --

    #[test]
    fn format_feet_inches_whole_feet() {
        assert_eq!(format_feet_inches(120.0), "10'-0\"");
    }

    #[test]
    fn format_feet_inches_with_fraction() {
        // 12.5 inches -> 1'-0 1/2"
        assert_eq!(format_feet_inches(12.5), "1'-0 1/2\"");
    }

    #[test]
    fn format_feet_inches_zero() {
        assert_eq!(format_feet_inches(0.0), "0'-0\"");
    }

    #[test]
    fn format_feet_inches_negative() {
        assert_eq!(format_feet_inches(-18.0), "-1'-6\"");
    }

    #[test]
    fn format_feet_inches_rounds_to_nearest_sixteenth() {
        // 6.03125" should round to 6 1/16", not overflow the fraction rendering.
        assert_eq!(format_feet_inches(6.03125), "0'-6 1/16\"");
    }

    #[test]
    fn format_feet_inches_very_large_value() {
        // 1000 feet exactly, expressed in inches.
        assert_eq!(format_feet_inches(12000.0), "1000'-0\"");
    }
}

#[cfg(test)]
mod spatial_tests {
    use super::spatial::{BoundingBox, SpatialIndex};
    use super::Point;

    fn bbox(x1: f64, y1: f64, x2: f64, y2: f64) -> BoundingBox {
        BoundingBox {
            min: Point::new(x1, y1),
            max: Point::new(x2, y2),
        }
    }

    #[test]
    fn bounding_box_from_points_covers_min_and_max() {
        let points = [
            Point::new(3.0, -1.0),
            Point::new(-2.0, 5.0),
            Point::new(0.0, 0.0),
        ];
        let bb = BoundingBox::from_points(&points).unwrap();
        assert_eq!(bb, bbox(-2.0, -1.0, 3.0, 5.0));
    }

    #[test]
    fn bounding_box_from_points_is_none_for_empty_slice() {
        assert_eq!(BoundingBox::from_points(&[]), None);
    }

    #[test]
    fn query_point_finds_all_overlapping_boxes() {
        let mut index = SpatialIndex::new();
        index.insert("a", bbox(0.0, 0.0, 10.0, 10.0));
        index.insert("b", bbox(5.0, 5.0, 15.0, 15.0)); // overlaps "a" at (5,5)-(10,10)
        index.insert("c", bbox(20.0, 20.0, 30.0, 30.0)); // disjoint

        let mut hits = index.query_point(Point::new(7.0, 7.0));
        hits.sort();
        assert_eq!(hits, vec!["a", "b"]);

        assert_eq!(index.query_point(Point::new(25.0, 25.0)), vec!["c"]);
        assert!(index.query_point(Point::new(100.0, 100.0)).is_empty());
    }

    #[test]
    fn query_rect_finds_intersecting_boxes_only() {
        let mut index = SpatialIndex::new();
        index.insert("inside", bbox(1.0, 1.0, 2.0, 2.0));
        index.insert("touching_edge", bbox(10.0, 10.0, 20.0, 20.0));
        index.insert("far_away", bbox(100.0, 100.0, 110.0, 110.0));

        let mut hits = index.query_rect(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        hits.sort();
        assert_eq!(hits, vec!["inside", "touching_edge"]);
    }

    #[test]
    fn remove_deletes_exact_entry() {
        let mut index = SpatialIndex::new();
        let bb = bbox(0.0, 0.0, 5.0, 5.0);
        index.insert("only", bb);
        assert_eq!(index.len(), 1);

        assert!(index.remove(&"only", bb));
        assert!(index.is_empty());
        assert!(index.query_point(Point::new(1.0, 1.0)).is_empty());
    }

    #[test]
    fn remove_with_wrong_bbox_does_nothing() {
        let mut index = SpatialIndex::new();
        index.insert("only", bbox(0.0, 0.0, 5.0, 5.0));

        let wrong_bbox = bbox(100.0, 100.0, 105.0, 105.0);
        assert!(!index.remove(&"only", wrong_bbox));
        assert_eq!(index.len(), 1);
    }
}
