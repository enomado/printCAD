//! Screen-plane geometry shared by the handles and other screen-space markers.
use emath::Pos2;

/// Screen distance from `point` to the segment `a–b`.
pub fn segment_distance(point: Pos2, a: Pos2, b: Pos2) -> f32 {
    let edge = b - a;
    if edge.length_sq() < 1e-8 {
        return point.distance(a);
    }
    point.distance(a + edge * ((point - a).dot(edge) / edge.length_sq()).clamp(0.0, 1.0))
}

/// Convex hull (Andrew's monotone chain), counter-clockwise in screen order. The
/// projection of a cone or a cube is the hull of its projected vertices, so the
/// drawn shape and the hit area are one polygon.
pub fn convex_hull(mut points: Vec<Pos2>) -> Vec<Pos2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let cross = |o: Pos2, a: Pos2, b: Pos2| (a - o).x * (b - o).y - (a - o).y * (b - o).x;
    let mut hull: Vec<Pos2> = Vec::with_capacity(points.len() + 1);
    for pass in [
        &points[..],
        &points.iter().rev().copied().collect::<Vec<_>>()[..],
    ] {
        let floor = hull.len();
        for &p in pass {
            while hull.len() >= floor + 2
                && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0
            {
                hull.pop();
            }
            hull.push(p);
        }
        // The last point of each chain starts the other one.
        hull.pop();
    }
    hull
}
