//! Vector Toolkit: hexagon and square grids, and random points in polygons.
//!
//! Coordinates are longitude/latitude, as RustGIS hands layers to tools. Grids are laid out in
//! Web Mercator, so cells look regular on the map; they are sized in kilometres at the
//! extent's middle latitude (and so cover less ground towards the poles, like the map does).

use rustgis_plugin::{Kind, Request, ToolOutput, Value, geojson, json};

/// The most cells or points a run may make (each is a GeoJSON feature in the reply, and the
/// sandbox has 512 MB).
const MAX_ITEMS: usize = 50_000;
/// The most cells a run may consider when it keeps only those with features.
const MAX_CONSIDERED: f64 = 2_000_000.0;

fn handle(req: Request) -> Result<Value, String> {
    match (req.kind, req.id.as_str()) {
        (Kind::Tool, "hexagon-grid") => grid(&req, true),
        (Kind::Tool, "square-grid") => grid(&req, false),
        (Kind::Tool, "random-points") => random_points(&req),
        (_, other) => Err(format!("Vector Toolkit has no '{other}'")),
    }
}

rustgis_plugin::export!(handle);

const R: f64 = 6_378_137.0;
const MAX_LAT: f64 = 85.051_128_78;

/// Longitude/latitude to Web Mercator metres.
fn merc(p: [f64; 2]) -> [f64; 2] {
    let lat = p[1].clamp(-MAX_LAT, MAX_LAT).to_radians();
    [R * p[0].to_radians(), R * (std::f64::consts::FRAC_PI_4 + lat / 2.0).tan().ln()]
}

/// Web Mercator metres to longitude/latitude.
fn lonlat(m: [f64; 2]) -> [f64; 2] {
    [(m[0] / R).to_degrees(), (2.0 * (m[1] / R).exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees()]
}

fn grid(req: &Request, hex: bool) -> Result<Value, String> {
    let name = req.str("input").unwrap_or("Input").to_string();
    let features = req.features("input")?;
    let vertices: Vec<[f64; 2]> = features.iter().flat_map(|f| geojson::positions(&f["geometry"])).collect();
    let ll = geojson::bbox(&vertices).ok_or("the layer has no features")?;
    let size = req.f64("size").filter(|s| *s > 0.0).ok_or("the cell size must be more than 0")?;
    let (lo, hi) = (merc([ll[0], ll[1]]), merc([ll[2], ll[3]]));
    let b = [lo[0], lo[1], hi[0], hi[1]];
    // Mercator metres for `size` km at the middle latitude.
    let mid = ((ll[1] + ll[3]) / 2.0).clamp(-MAX_LAT, MAX_LAT).to_radians().cos().max(0.01);
    let w = size * 1000.0 / mid;
    let touching = req.bool("touching") == Some(true);
    let mvertices: Vec<[f64; 2]> = if touching { vertices.iter().map(|p| merc(*p)).collect() } else { Vec::new() };
    let mut cells = Vec::new();
    let mut push = |ring: Vec<[f64; 2]>, row: i64, col: i64| -> Result<(), String> {
        if touching {
            let Some(cb) = geojson::bbox(&ring) else { return Ok(()) };
            let rings = [ring.clone()];
            if !mvertices.iter().any(|p| p[0] >= cb[0] && p[0] <= cb[2] && p[1] >= cb[1] && p[1] <= cb[3] && inside(p, &rings)) {
                return Ok(());
            }
        }
        if cells.len() >= MAX_ITEMS {
            return Err(format!("more than {MAX_ITEMS} cells: use a larger cell size"));
        }
        let id = cells.len() + 1;
        let ring: Vec<[f64; 2]> = ring.into_iter().map(lonlat).collect();
        cells.push(geojson::feature(geojson::polygon(&ring), json!({"id": id, "row": row, "col": col})));
        Ok(())
    };
    let too_many = |rows: f64, cols: f64| {
        let n = rows.max(1.0) * cols.max(1.0);
        if touching { n > MAX_CONSIDERED } else { n > MAX_ITEMS as f64 }
    };
    if hex {
        // Pointy-top hexagons `w` across the flat sides; rows are 3/4 of a hexagon's height apart.
        let r = w / 3f64.sqrt();
        let rh = r * 1.5;
        let (rows, cols) = (((b[3] - b[1]) / rh).ceil() + 1.0, ((b[2] - b[0]) / w).ceil() + 1.0);
        if too_many(rows, cols) {
            return Err(format!("that would be about {:.0} cells, more than {MAX_ITEMS}: use a larger cell size", rows * cols));
        }
        for row in 0..rows as i64 {
            let cy = b[1] + row as f64 * rh;
            let shift = if row % 2 == 1 { w / 2.0 } else { 0.0 };
            for col in 0..cols as i64 {
                let cx = b[0] + col as f64 * w + shift;
                let ring: Vec<[f64; 2]> = (0..6)
                    .map(|i| {
                        let a = (60.0 * i as f64 + 30.0).to_radians();
                        [cx + r * a.cos(), cy + r * a.sin()]
                    })
                    .collect();
                push(ring, row, col)?;
            }
        }
    } else {
        let (rows, cols) = (((b[3] - b[1]) / w).ceil().max(1.0), ((b[2] - b[0]) / w).ceil().max(1.0));
        if too_many(rows, cols) {
            return Err(format!("that would be about {:.0} cells, more than {MAX_ITEMS}: use a larger cell size", rows * cols));
        }
        for row in 0..rows as i64 {
            for col in 0..cols as i64 {
                let (x, y) = (b[0] + col as f64 * w, b[1] + row as f64 * w);
                push(vec![[x, y], [x + w, y], [x + w, y + w], [x, y + w]], row, col)?;
            }
        }
    }
    let n = cells.len();
    let kind = if hex { "hexagons" } else { "squares" };
    Ok(ToolOutput::new().layer(&format!("{name} {kind}"), geojson::collection(cells)).message(format!("{n} {kind} of {size} km")).into())
}

/// Even-odd point-in-polygon over all rings (holes included).
fn inside(p: &[f64; 2], rings: &[Vec<[f64; 2]>]) -> bool {
    let mut c = false;
    for ring in rings {
        let n = ring.len();
        for i in 0..n {
            let (a, b) = (ring[i], ring[(i + n - 1) % n]);
            if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
                c = !c;
            }
        }
    }
    c
}

/// The polygons of a geometry, each as its rings.
fn polygons(g: &Value) -> Vec<Vec<Vec<[f64; 2]>>> {
    let rings = |v: &Value| -> Vec<Vec<[f64; 2]>> {
        v.as_array()
            .map(|rs| {
                rs.iter()
                    .map(|r| {
                        r.as_array()
                            .map(|ps| ps.iter().filter_map(|p| Some([p[0].as_f64()?, p[1].as_f64()?])).collect())
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    match g["type"].as_str() {
        Some("Polygon") => vec![rings(&g["coordinates"])],
        Some("MultiPolygon") => g["coordinates"].as_array().map(|ps| ps.iter().map(rings).collect()).unwrap_or_default(),
        Some("GeometryCollection") => g["geometries"].as_array().map(|gs| gs.iter().flat_map(polygons).collect()).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// A small, seedable random number generator (SplitMix64): the same seed gives the same points.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn random_points(req: &Request) -> Result<Value, String> {
    let name = req.str("input").unwrap_or("Input").to_string();
    let features = req.features("input")?;
    let count = req.f64("count").filter(|c| *c >= 1.0).ok_or("the number of points must be at least 1")?.floor() as usize;
    if count.saturating_mul(features.len()) > MAX_ITEMS {
        return Err(format!("more than {MAX_ITEMS} points: ask for fewer per polygon"));
    }
    let mut rng = Rng(req.f64("seed").unwrap_or(1.0).to_bits());
    let mut points = Vec::new();
    let mut short = 0;
    for (row, f) in features.iter().enumerate() {
        let polys = polygons(&f["geometry"]);
        let all: Vec<[f64; 2]> = polys.iter().flatten().flatten().copied().collect();
        let Some(b) = geojson::bbox(&all) else { continue };
        let mut made = 0;
        // Rejection sampling in the bounding box, with a bound for slivers.
        for _ in 0..count.saturating_mul(1000) {
            if made == count {
                break;
            }
            let p = [b[0] + rng.next() * (b[2] - b[0]), b[1] + rng.next() * (b[3] - b[1])];
            if polys.iter().any(|rings| inside(&p, rings)) {
                made += 1;
                points.push(geojson::feature(geojson::point(p[0], p[1]), json!({"polygon": row, "n": made})));
            }
        }
        if made < count {
            short += 1;
        }
    }
    let n = points.len();
    let mut out = ToolOutput::new().layer(&format!("{name} random points"), geojson::collection(points)).message(format!("{n} points"));
    if short > 0 {
        out = out.message(format!("{short} polygons were too thin to fit every point"));
    }
    Ok(out.into())
}
