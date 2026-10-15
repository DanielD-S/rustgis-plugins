//! IDE Chile: public geospatial services of Chile as one-click layers.
//!
//! Each layer in the catalog (`catalog.rs`, generated with `plugin.json` from `generate.py`) is a
//! command. A layer may offer a map image (an ArcGIS MapServer `export` template, added through
//! RustGIS's `map.addTiles` as a web tile layer, which needs the `network` permission), features
//! (GeoJSON from an ArcGIS feature layer or a WFS feature type, added through `layer.addUrl`:
//! small layers whole, large ones in the map view), or both: then `{"data": true}` asks for the
//! features. A map image chosen again is taken away.
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
    /// A tile template of the map image (`{bbox-epsg-3857}`), or empty.
    pub tiles: &'static str,
    /// The feature service's address, or empty.
    pub service: &'static str,
    /// The service's type for `service.add`: `arcgis` or `wfs`.
    pub kind: &'static str,
    /// The layer in the service: an ArcGIS layer id, or a WFS feature type name.
    pub layer: &'static str,
    /// Whether the layer is small enough to read whole (one request); the others are read in
    /// the map view.
    pub whole: bool,
    /// The credit shown with the layer.
    pub credit: &'static str,
}

fn handle(req: Request) -> Result<Value, String> {
    if let Some(e) = CATALOG.iter().find(|e| e.id == req.id) {
        return choose(&req, e, req.bool("data").unwrap_or(false));
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

/// The id a catalog entry's tile layer carries.
pub fn tile_id(e: &Entry) -> String {
    format!("ide-chile/{}", e.id)
}

/// The parameters of `map.addTiles` for an entry's map image.
pub fn tiles_params(e: &Entry) -> Value {
    json!({
        "url": e.tiles, "kind": "raster", "name": e.name, "attribution": e.credit, "maxZoom": 19,
        "overlay": true, "id": tile_id(e),
    })
}

/// The most features one read in the map view brings (ArcGIS servers stop at 2,000 anyway).
const IN_VIEW: u32 = 5000;

/// The smallest map scale (the largest denominator) a layer is read in the map view at: farther
/// out, the box would ask a national layer for most of itself.
const MAX_VIEW_SCALE: f64 = 2_000_000.0;

/// The address of an entry's features as GeoJSON: the whole layer, or, given a Web Mercator box
/// `[west, south, east, north]`, the features in it.
pub fn features_url(e: &Entry, bbox: Option<[f64; 4]>) -> String {
    let b = bbox.map(|[w, s, e, n]| format!("{w:.0},{s:.0},{e:.0},{n:.0}"));
    match e.kind {
        "wfs" => {
            let mut u = format!(
                "{}?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetFeature&TYPENAMES={}&OUTPUTFORMAT=application%2Fjson&SRSNAME=EPSG:4326",
                e.service, e.layer
            );
            if let Some(b) = b {
                u.push_str(&format!("&BBOX={b},EPSG:3857&COUNT={IN_VIEW}"));
            }
            u
        }
        _ => {
            // Five decimals of a degree (about a metre) keep the answers small.
            let mut u = format!(
                "{}/{}/query?where=1%3D1&outFields=*&returnGeometry=true&outSR=4326&geometryPrecision=5&f=geojson",
                e.service, e.layer
            );
            match b {
                Some(b) => u.push_str(&format!(
                    "&geometry={b}&geometryType=esriGeometryEnvelope&inSR=3857&spatialRel=esriSpatialRelIntersects&resultRecordCount={IN_VIEW}"
                )),
                // A whole layer is generalised to about 10 m, which makes national boundaries a
                // few megabytes instead of tens; the map view has the full detail.
                None => u.push_str("&maxAllowableOffset=0.0001"),
            }
            u
        }
    }
}

/// The map view as a Web Mercator box, from `host.view`'s centre and scale: the box a screen of
/// about 1600 × 1000 pixels at 96 dpi shows at that scale, which covers the view with a margin.
pub fn view_bbox(view: &Value) -> Option<[f64; 4]> {
    let c = view.get("center")?.as_array()?;
    let (lon, lat) = (c.first()?.as_f64()?, c.get(1)?.as_f64()?);
    let scale = view.get("scale")?.as_f64()?;
    if !(lon.is_finite() && lat.is_finite() && scale.is_finite()) || scale <= 0.0 {
        return None;
    }
    let (x, y) = mercator(lon, lat.clamp(-85.05, 85.05));
    let m_per_px = scale * 0.0254 / 96.0;
    let (half_w, half_h) = (800.0 * m_per_px, 500.0 * m_per_px);
    Some([x - half_w, y - half_h, x + half_w, y + half_h])
}

/// Longitude/latitude in degrees to Web Mercator metres.
fn mercator(lon: f64, lat: f64) -> (f64, f64) {
    const R: f64 = 6_378_137.0;
    let x = lon.to_radians() * R;
    let y = ((std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0).tan()).ln() * R;
    (x, y)
}

fn choose(req: &Request, e: &Entry, data: bool) -> Result<Value, String> {
    let name = &[("name", e.name)];
    if data || e.tiles.is_empty() {
        if e.service.is_empty() {
            return Err(req.tr(MANIFEST, "{name} offers no features: it is a map image.", name));
        }
        if data && e.tiles.is_empty() {
            host::log(&req.tr(MANIFEST, "{name} has no map image: it is read as features.", name));
        }
        let bbox = if e.whole {
            None
        } else {
            let view = host::view()?;
            if view.get("scale").and_then(|s| s.as_f64()).is_none_or(|s| s > MAX_VIEW_SCALE) {
                return Err(req.tr(MANIFEST, "{name} is read in the map view: zoom in to 1:2,000,000 or closer first.", name));
            }
            Some(view_bbox(&view).ok_or("the map has no view to read the features in")?)
        };
        let out = host::run("layer.addUrl", json!({"url": features_url(e, bbox), "name": e.name, "zoom": e.whole}))?;
        host::notify(&req.tr(MANIFEST, if e.whole { "Added {name}." } else { "Added {name} in the map view." }, name));
        return Ok(out);
    }
    // A map image already on the map is taken away again.
    let id = tile_id(e);
    if tile_layers()?.iter().any(|t| t["id"] == id) {
        host::run("map.removeTiles", json!({"id": id}))?;
        host::notify(&req.tr(MANIFEST, "Removed {name}.", name));
        return Ok(json!({"removed": e.name}));
    }
    let out = host::run("map.addTiles", tiles_params(e))?;
    host::notify(&req.tr(MANIFEST, "Added {name} (map image).", name));
    Ok(out)
}

/// Removes the plugin's map images, bottom first so the indexes stay valid.
fn clear(req: &Request) -> Result<Value, String> {
    let mut removed = Vec::new();
    for t in tile_layers()?.iter().rev() {
        if t["id"].as_str().is_some_and(|id| id.starts_with("ide-chile/")) {
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
        .map(|e| json!({"id": e.id, "name": e.name, "map": !e.tiles.is_empty(), "features": !e.service.is_empty(), "credit": e.credit}))
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
            assert!(!e.tiles.is_empty() || !e.service.is_empty(), "{} offers nothing", e.id);
            assert!(!e.name.is_empty() && !e.credit.is_empty(), "{}", e.id);
            if !e.tiles.is_empty() {
                assert!(e.tiles.starts_with("https://") && e.tiles.contains("{bbox-epsg-3857}"), "{}: {}", e.id, e.tiles);
            }
            if !e.service.is_empty() {
                assert!(e.service.starts_with("https://") && !e.layer.is_empty(), "{}: {}", e.id, e.service);
                assert!(matches!(e.kind, "arcgis" | "wfs"), "{}: {}", e.id, e.kind);
            }
            assert!(!e.whole || !e.service.is_empty(), "{}: whole without a service", e.id);
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

    #[test]
    fn builds_calls() {
        let regions = CATALOG.iter().find(|e| e.id == "regions").unwrap();
        let p = tiles_params(regions);
        assert_eq!(p["id"], "ide-chile/regions");
        assert_eq!(p["overlay"], true);
        assert!(!regions.whole, "44,000 boundary polygons: read in the view");
        let stations = CATALOG.iter().find(|e| e.id == "weather-stations").unwrap();
        assert!(stations.whole);
        let whole = features_url(stations, None);
        assert!(whole.contains("/MapServer/7/query?") && whole.contains("f=geojson") && !whole.contains("geometry="), "{whole}");
        assert!(whole.contains("&maxAllowableOffset=0.0001"), "{whole}");
        let wetlands = CATALOG.iter().find(|e| e.id == "wetlands").unwrap();
        assert!(wetlands.tiles.is_empty() && !wetlands.whole);
        let in_view = features_url(wetlands, Some([-7900000.0, -3970000.0, -7820000.0, -3890000.0]));
        assert!(
            in_view.contains("TYPENAMES=Humedales:humedales")
                && in_view.contains("&BBOX=-7900000,-3970000,-7820000,-3890000,EPSG:3857&COUNT=5000"),
            "{in_view}"
        );
        let blocks = CATALOG.iter().find(|e| e.id == "census-blocks").unwrap();
        let in_view = features_url(blocks, Some([1.0, 2.0, 3.0, 4.0]));
        assert!(in_view.contains("&geometry=1,2,3,4&geometryType=esriGeometryEnvelope&inSR=3857"), "{in_view}");
    }

    #[test]
    fn view_box_from_center_and_scale() {
        // Santiago at 1:150,000: about 63 km wide and 40 km high, centred on the city.
        let b = view_bbox(&json!({"center": [-70.65, -33.45], "scale": 150000.0, "projection": "EPSG:3857"})).unwrap();
        assert!((b[0] - -7_864_568.0).abs() < 32_000.0 && (b[2] - -7_864_568.0).abs() < 32_000.0, "{b:?}");
        assert!((b[2] - b[0] - 63_500.0).abs() < 1.0 && (b[3] - b[1] - 39_687.5).abs() < 1.0, "{b:?}");
        assert!(b[1] < -3_957_000.0 && b[3] > -3_957_000.0, "{b:?}");
        assert!(view_bbox(&json!({"center": [0.0, 0.0]})).is_none());
        assert!(view_bbox(&json!({"center": [0.0, 0.0], "scale": -1.0})).is_none());
    }
}
