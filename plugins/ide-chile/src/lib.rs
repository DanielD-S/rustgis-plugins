//! IDE Chile: public geospatial services of Chile as one-click layers.
//!
//! Each layer in the catalog (`catalog.rs`, generated with `plugin.json` from `generate.py`) is a
//! command that adds features: GeoJSON from an ArcGIS feature layer or a WFS feature type, added
//! through RustGIS's `layer.addUrl` (which needs the `network` permission). Small layers are read
//! whole; large ones in the map view, where an ArcGIS service's layers that reach the view (by
//! region, mostly) each become a layer, generalised to the view's pixel. Where a service also
//! draws a map image, `{"map": true}` adds that instead, as a web tile layer under the map's
//! layers; chosen again, it is taken away.
//!
//! The sources: the Geoportal de Chile (IDE Chile, GeoServer WFS), IDE MINAGRI (CIREN's ArcGIS
//! Server), IDE MINVU (ArcGIS Server) and the INE's Census 2017 (ArcGIS Online).

use rustgis_plugin::{Request, Value, host, json};

mod catalog;

pub use catalog::CATALOG;

/// `plugin.json`, for its translations.
const MANIFEST: &str = include_str!("../plugin.json");

/// A layer in the catalog.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    /// The command id, also kept on the map image's tile layer to recognise it later.
    pub id: &'static str,
    /// The layer's name on the map (the dataset's own name).
    pub name: &'static str,
    /// A tile template of the service's map image (`{bbox-epsg-3857}`), or empty.
    pub tiles: &'static str,
    /// The feature service's address.
    pub service: &'static str,
    /// The service's kind: `arcgis` or `wfs`.
    pub kind: &'static str,
    /// A WFS feature type name (an ArcGIS service's layers are [`Entry::subs`]).
    pub layer: &'static str,
    /// An ArcGIS service's queryable layers.
    pub subs: &'static [Sub],
    /// Whether the layer is small enough to read whole (one request); the others are read in
    /// the map view.
    pub whole: bool,
    /// The credit shown with the layer.
    pub credit: &'static str,
}

/// One queryable layer of an ArcGIS service.
#[derive(Clone, Copy, Debug)]
pub struct Sub {
    pub id: u32,
    pub name: &'static str,
    /// Its extent in Web Mercator `[west, south, east, north]`, when the service gave one the
    /// plugin could read; without it the layer is always considered.
    pub bbox: Option<[f64; 4]>,
}

/// The most features one read in the map view brings (ArcGIS servers stop at 2,000 anyway).
const IN_VIEW: u32 = 5000;

/// The most of a service's layers one command reads in the map view.
const MAX_SUBS: usize = 8;

/// The smallest map scale (the largest denominator) a layer is read in the map view at: farther
/// out, the box would ask a national layer for most of itself.
const MAX_VIEW_SCALE: f64 = 2_000_000.0;

fn handle(req: Request) -> Result<Value, String> {
    if let Some(e) = CATALOG.iter().find(|e| e.id == req.id) {
        return choose(&req, e, req.bool("map").unwrap_or(false));
    }
    match req.id.as_str() {
        "clear" => clear(&req),
        "about" => about(&req),
        other => Err(format!("IDE Chile has no command '{other}'")),
    }
}

rustgis_plugin::export!(handle);

/// The map's tile layers, top first: `[{"index", "name", "id", "overlay", …}]`.
fn tile_layers() -> Result<Vec<Value>, String> {
    host::run("map.tiles", json!({}))?.as_array().cloned().ok_or_else(|| "map.tiles returned an unexpected result".to_string())
}

/// The id a catalog entry's tile layer carries (RustGIS keeps letters, digits and hyphens of a
/// tile layer's id, so no slash).
pub fn tile_id(e: &Entry) -> String {
    format!("ide-chile-{}", e.id)
}

/// The parameters of `map.addTiles` for an entry's map image.
pub fn tiles_params(e: &Entry) -> Value {
    // 512-pixel tiles halve the requests a server renders on demand answers, and nothing is
    // asked below zoom 4, where Chile is a sliver and a continent's worth of tiles would be.
    json!({
        "url": e.tiles, "kind": "raster", "name": e.name, "attribution": e.credit, "minZoom": 4, "maxZoom": 19,
        "tileSize": 512, "overlay": true, "id": tile_id(e),
    })
}

/// The map view: its Web Mercator box `[west, south, east, north]` and metres per pixel, from
/// `host.view`'s centre and scale: the box a screen of about 1600 × 1000 pixels at 96 dpi shows
/// at that scale, which covers the view with a margin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub bbox: [f64; 4],
    pub m_per_px: f64,
}

pub fn view_of(view: &Value) -> Option<View> {
    let c = view.get("center")?.as_array()?;
    let (lon, lat) = (c.first()?.as_f64()?, c.get(1)?.as_f64()?);
    let scale = view.get("scale")?.as_f64()?;
    if !(lon.is_finite() && lat.is_finite() && scale.is_finite()) || scale <= 0.0 {
        return None;
    }
    let (x, y) = mercator(lon, lat.clamp(-85.05, 85.05));
    let m_per_px = scale * 0.0254 / 96.0;
    let (half_w, half_h) = (800.0 * m_per_px, 500.0 * m_per_px);
    Some(View { bbox: [x - half_w, y - half_h, x + half_w, y + half_h], m_per_px })
}

/// Longitude/latitude in degrees to Web Mercator metres.
fn mercator(lon: f64, lat: f64) -> (f64, f64) {
    const R: f64 = 6_378_137.0;
    let x = lon.to_radians() * R;
    let y = ((std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0).tan()).ln() * R;
    (x, y)
}

/// Whether two Web Mercator boxes overlap.
fn overlaps(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

/// The address of an ArcGIS layer's features as GeoJSON: the whole layer, or, given the view,
/// the features in it, generalised to about a pixel.
pub fn arcgis_url(e: &Entry, sub: &Sub, view: Option<View>) -> String {
    let mut u =
        format!("{}/{}/query?where=1%3D1&outFields=*&returnGeometry=true&outSR=4326&geometryPrecision=5&f=geojson", e.service, sub.id);
    match view {
        Some(v) => {
            let [w, s, e, n] = v.bbox;
            // The offset is in the answer's degrees: a pixel's worth at the equator's scale.
            let offset = v.m_per_px / 111_320.0;
            u.push_str(&format!(
                "&geometry={w:.0},{s:.0},{e:.0},{n:.0}&geometryType=esriGeometryEnvelope&inSR=3857&spatialRel=esriSpatialRelIntersects&resultRecordCount={IN_VIEW}&maxAllowableOffset={offset:.7}"
            ));
        }
        // A whole layer is generalised to about 10 m, which makes national boundaries a few
        // megabytes instead of tens; the map view has the full detail.
        None => u.push_str("&maxAllowableOffset=0.0001"),
    }
    u
}

/// The address of a WFS feature type's features as GeoJSON: the whole layer, or, given the
/// view, the features in it.
pub fn wfs_url(e: &Entry, view: Option<View>) -> String {
    let mut u = format!(
        "{}?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetFeature&TYPENAMES={}&OUTPUTFORMAT=application%2Fjson&SRSNAME=EPSG:4326",
        e.service, e.layer
    );
    if let Some(v) = view {
        let [w, s, e, n] = v.bbox;
        u.push_str(&format!("&BBOX={w:.0},{s:.0},{e:.0},{n:.0},EPSG:3857&COUNT={IN_VIEW}"));
    }
    u
}

/// The layers a command reads, each as (name on the map, address): a WFS feature type, an
/// ArcGIS layer read whole, or the ArcGIS layers that reach the view.
pub fn reads(e: &Entry, view: Option<View>) -> Vec<(String, String)> {
    if e.kind == "wfs" {
        return vec![(e.name.to_string(), wfs_url(e, view))];
    }
    let reaching: Vec<&Sub> = match view {
        Some(v) => e.subs.iter().filter(|s| s.bbox.is_none_or(|b| overlaps(b, v.bbox))).take(MAX_SUBS).collect(),
        None => e.subs.iter().take(1).collect(),
    };
    let one = e.subs.len() == 1;
    reaching
        .into_iter()
        .map(|s| (if one { e.name.to_string() } else { format!("{} · {}", e.name, s.name) }, arcgis_url(e, s, view)))
        .collect()
}

fn choose(req: &Request, e: &Entry, map: bool) -> Result<Value, String> {
    let name = &[("name", e.name)];
    if map {
        if e.tiles.is_empty() {
            return Err(req.tr(MANIFEST, "{name} has no map image.", name));
        }
        return toggle_tiles(req, e);
    }
    let view = if e.whole {
        None
    } else {
        let view = host::view()?;
        if view.get("scale").and_then(|s| s.as_f64()).is_none_or(|s| s > MAX_VIEW_SCALE) {
            return Err(req.tr(MANIFEST, "{name} is read in the map view: zoom in to 1:2,000,000 or closer first.", name));
        }
        Some(view_of(&view).ok_or("the map has no view to read the features in")?)
    };
    let reads = reads(e, view);
    if reads.is_empty() {
        return Err(req.tr(MANIFEST, "Nothing of {name} reaches the map view.", name));
    }
    let mut added = Vec::new();
    let mut pending = false;
    for (layer, url) in &reads {
        match host::run("layer.addUrl", json!({"url": url, "name": layer, "zoom": e.whole})) {
            Ok(out) => {
                pending |= out.get("pending").and_then(Value::as_bool) == Some(true);
                added.push(out);
            }
            // A RustGIS that can't add data by address from a command: 0.2.0, which has no
            // layer.addUrl, or a web app that downloads only from its own Add Data dialog. The
            // map image instead, where there is one; else the address, for Add Data › From URL.
            Err(err) if (err.contains("web app") || err.contains("unknown command")) && added.is_empty() => {
                let old = err.contains("unknown command");
                if e.tiles.is_empty() {
                    return Err(req.tr(
                        MANIFEST,
                        if old {
                            "This RustGIS can't add data by address from a plugin: update it (0.2.1 or later) for {name}, or add this address with Home › Add Data › From URL: {url}"
                        } else {
                            "In the web app, add {name} with Home › Add Data › From URL and this address: {url}"
                        },
                        &[("name", e.name), ("url", url)],
                    ));
                }
                let out = host::run("map.addTiles", tiles_params(e))?;
                host::notify(&req.tr(
                    MANIFEST,
                    if old {
                        "This RustGIS can't add data by address from a plugin: {name} is shown as a map image. Update RustGIS (0.2.1 or later) for its features."
                    } else {
                        "In the web app, {name} is shown as a map image; its features need the desktop app."
                    },
                    name,
                ));
                return Ok(out);
            }
            Err(err) => return Err(err),
        }
    }
    let n = added.len().to_string();
    host::notify(&req.tr(
        MANIFEST,
        if pending {
            "Downloading {name}; it is added when it lands."
        } else if e.whole {
            "Added {name}."
        } else if added.len() == 1 {
            "Added {name} in the map view."
        } else {
            "Added {n} layers of {name} in the map view."
        },
        &[("name", e.name), ("n", &n)],
    ));
    Ok(json!({"layers": added, "pending": pending}))
}

/// Adds an entry's map image (a web tile layer, drawn under the map's own layers), or takes it
/// away when it is already there.
fn toggle_tiles(req: &Request, e: &Entry) -> Result<Value, String> {
    let name = &[("name", e.name)];
    let id = tile_id(e);
    if tile_layers()?.iter().any(|t| t["id"] == id) {
        host::run("map.removeTiles", json!({"id": id}))?;
        host::notify(&req.tr(MANIFEST, "Removed {name}.", name));
        return Ok(json!({"removed": e.name}));
    }
    let out = host::run("map.addTiles", tiles_params(e))?;
    host::notify(&req.tr(MANIFEST, "Added {name} (map image, under the layers; Map › Basemap lists it).", name));
    Ok(out)
}

/// Removes the plugin's map images, bottom first so the indexes stay valid.
fn clear(req: &Request) -> Result<Value, String> {
    let mut removed = Vec::new();
    for t in tile_layers()?.iter().rev() {
        if t["id"].as_str().is_some_and(|id| id.starts_with("ide-chile-")) {
            host::run("map.removeTiles", json!({"index": t["index"]}))?;
            removed.push(t["name"].clone());
        }
    }
    host::notify(&if removed.is_empty() {
        req.tr(MANIFEST, "No IDE Chile map layers to remove.", &[])
    } else {
        req.tr(MANIFEST, "Removed {n} IDE Chile layers.", &[("n", &removed.len().to_string())])
    });
    Ok(json!({"removed": removed}))
}

fn about(req: &Request) -> Result<Value, String> {
    let n = CATALOG.len().to_string();
    host::notify(&req.tr(
        MANIFEST,
        "IDE Chile: {n} layers from Geoportal de Chile, IDE MINAGRI, IDE MINVU and INE. Each layer's credit names its source.",
        &[("n", &n)],
    ));
    let layers: Vec<Value> = CATALOG
        .iter()
        .map(|e| json!({"id": e.id, "name": e.name, "whole": e.whole, "layers": e.subs.len().max(1), "map": !e.tiles.is_empty(), "credit": e.credit}))
        .collect();
    Ok(json!({"layers": layers}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustgis_plugin::serde_json;

    #[test]
    fn catalog_and_manifest_agree() {
        let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
        let commands: Vec<&str> = manifest["commands"].as_array().unwrap().iter().filter_map(|c| c["id"].as_str()).collect();
        for e in CATALOG {
            assert!(commands.contains(&e.id), "{} has no command", e.id);
            assert!(e.service.starts_with("https://"), "{}: {}", e.id, e.service);
            assert!(!e.name.is_empty() && !e.credit.is_empty(), "{}", e.id);
            match e.kind {
                "wfs" => assert!(e.layer.contains(':') && e.subs.is_empty(), "{}", e.id),
                "arcgis" => assert!(e.layer.is_empty() && !e.subs.is_empty(), "{}", e.id),
                other => panic!("{}: {other}", e.id),
            }
            assert!(!e.whole || e.kind == "wfs" || e.subs.len() == 1, "{}: a layer read whole is one layer", e.id);
            if !e.tiles.is_empty() {
                assert!(e.tiles.starts_with("https://") && e.tiles.contains("{bbox-epsg-3857}"), "{}: {}", e.id, e.tiles);
            }
            for s in e.subs {
                assert!(!s.name.is_empty(), "{}: layer {}", e.id, s.id);
                if let Some(b) = s.bbox {
                    // A layer of one point has a degenerate extent.
                    assert!(b[0] <= b[2] && b[1] <= b[3] && b.iter().all(|v| v.abs() < 40_000_000.0), "{}: {:?}", e.id, b);
                }
            }
        }
        for c in commands {
            assert!(CATALOG.iter().any(|e| e.id == c) || ["clear", "about"].contains(&c), "{c} isn't handled");
        }
        assert_eq!(manifest["permissions"], json!(["edit", "network"]));
        // Every item of the tab is a command.
        let ids: Vec<&str> = manifest["commands"].as_array().unwrap().iter().filter_map(|c| c["id"].as_str()).collect();
        for g in manifest["tab"]["groups"].as_array().unwrap() {
            for it in g["items"].as_array().unwrap() {
                match it {
                    Value::String(id) => assert!(ids.contains(&id.as_str()), "{id}"),
                    menu => {
                        for id in menu["items"].as_array().unwrap() {
                            assert!(ids.contains(&id.as_str().unwrap()), "{id}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn spanish_covers_the_interface() {
        let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
        let es = manifest["i18n"]["es"].as_object().unwrap();
        for c in manifest["commands"].as_array().unwrap() {
            for k in ["label", "help"] {
                let s = c[k].as_str().unwrap();
                assert!(es.contains_key(s), "no Spanish for {s:?}");
            }
        }
        for g in manifest["tab"]["groups"].as_array().unwrap() {
            assert!(es.contains_key(g["label"].as_str().unwrap()), "{}", g["label"]);
        }
    }

    /// Santiago at 1:150,000, as `host.view` reports it.
    fn santiago() -> View {
        view_of(&json!({"center": [-70.65, -33.45], "scale": 150000.0, "projection": "EPSG:3857"})).unwrap()
    }

    #[test]
    fn view_box_from_center_and_scale() {
        // About 63 km wide and 40 km high, centred on the city.
        let v = santiago();
        let b = v.bbox;
        assert!((b[0] - -7_864_568.0).abs() < 32_000.0 && (b[2] - -7_864_568.0).abs() < 32_000.0, "{b:?}");
        assert!((b[2] - b[0] - 63_500.0).abs() < 1.0 && (b[3] - b[1] - 39_687.5).abs() < 1.0, "{b:?}");
        assert!(b[1] < -3_957_000.0 && b[3] > -3_957_000.0, "{b:?}");
        assert!((v.m_per_px - 39.6875).abs() < 1e-9);
        assert!(view_of(&json!({"center": [0.0, 0.0]})).is_none());
        assert!(view_of(&json!({"center": [0.0, 0.0], "scale": -1.0})).is_none());
    }

    #[test]
    fn reads_the_layers_that_reach_the_view() {
        // Agrological soils: the regional layers whose extents reach the view (Santiago's touches
        // the Metropolitan region's box and Valparaíso's, which overlap), not the dozen others.
        let soils = CATALOG.iter().find(|e| e.id == "soils").unwrap();
        assert!(!soils.whole && soils.subs.len() > 10);
        let r = reads(soils, Some(santiago()));
        assert!((1..=2).contains(&r.len()), "{r:?}");
        let rm = r.iter().find(|(n, _)| n.starts_with("Suelos agrológicos (CIREN) · REGIÓN METROPOLITANA")).unwrap();
        assert!(
            rm.1.contains("/MapServer/3/query?") && rm.1.contains("&geometry=") && rm.1.contains("&maxAllowableOffset=0.0003565"),
            "{}",
            rm.1
        );
        // Nothing reaches a view in the ocean.
        let sea = view_of(&json!({"center": [-100.0, -33.45], "scale": 150000.0})).unwrap();
        assert!(reads(soils, Some(sea)).is_empty());
        // A layer read whole: one request, generalised to about 10 m, no box.
        let stations = CATALOG.iter().find(|e| e.id == "weather-stations").unwrap();
        assert!(stations.whole);
        let r = reads(stations, None);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, stations.name);
        assert!(
            r[0].1.contains("/MapServer/7/query?") && !r[0].1.contains("geometry=") && r[0].1.contains("&maxAllowableOffset=0.0001"),
            "{}",
            r[0].1
        );
        // A WFS feature type in the view.
        let wetlands = CATALOG.iter().find(|e| e.id == "wetlands").unwrap();
        let r = reads(wetlands, Some(santiago()));
        assert_eq!(r.len(), 1);
        assert!(r[0].1.contains("TYPENAMES=Humedales:humedales") && r[0].1.contains(",EPSG:3857&COUNT=5000"), "{}", r[0].1);
        // Communes: the continental layer reaches Santiago, the islands' don't.
        let communes = CATALOG.iter().find(|e| e.id == "communes").unwrap();
        let r = reads(communes, Some(santiago()));
        assert_eq!(r.len(), 1, "{r:?}");
        assert!(r[0].1.contains("/MapServer/9/query?"), "{}", r[0].1);
    }

    #[test]
    fn map_images_are_an_option() {
        let regions = CATALOG.iter().find(|e| e.id == "regions").unwrap();
        let p = tiles_params(regions);
        assert_eq!(p["id"], "ide-chile-regions");
        assert_eq!(p["overlay"], true);
        assert!(CATALOG.iter().find(|e| e.id == "census-communes").unwrap().tiles.is_empty());
    }
}
