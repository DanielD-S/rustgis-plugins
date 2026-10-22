"""Reads what each ArcGIS service in the table offers and writes services.json for generate.py,
and the communes of the Census 2024 for communes.json.

    python plugins/ide-chile/refresh.py && python plugins/ide-chile/generate.py && cargo fmt -p ide-chile
    python plugins/ide-chile/refresh.py communes    # communes.json only

For every ArcGIS entry: the service's queryable layers (group layers can't be queried), each with
its name, geometry type and extent in Web Mercator (EPSG:3857), from which the plugin picks the
layers that reach the map view. An extent in another system is left out (the layer is then always
considered).

communes.json is the political-administrative division the INE took the Census 2024 on, from its
communes layer: each commune's CUT (código único territorial) with its name and its province's
and region's codes and names, as the INE spells them. The CUT tools (`src/cut.rs`) match codes
and names against it.
"""

import concurrent.futures
import importlib.util
import json
import math
import pathlib
import sys
import urllib.request

root = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("gen", root / "generate.py")
gen = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gen)

R = 6378137.0


def get(url):
    with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "RustGIS IDE Chile plugin"}), timeout=120) as r:
        return json.load(r)


def mercator(lon, lat):
    lat = max(min(lat, 85.05), -85.05)
    return math.radians(lon) * R, math.log(math.tan(math.pi / 4 + math.radians(lat) / 2)) * R


def fetch(entry):
    base = entry["service"]
    try:
        meta = get(base + "?f=pjson")
    except Exception as ex:
        return entry["id"], {"service": base, "error": str(ex)}
    layers = []
    for layer in meta.get("layers", []):
        if layer.get("subLayerIds"):
            continue
        try:
            d = get(f"{base}/{layer['id']}?f=pjson")
            ext = d.get("extent") or {}
            sr = ext.get("spatialReference") or {}
            wkid = sr.get("latestWkid") or sr.get("wkid")
            bbox = None
            if ext.get("xmin") is not None:
                if wkid in (3857, 102100):
                    bbox = [ext["xmin"], ext["ymin"], ext["xmax"], ext["ymax"]]
                elif wkid == 4326:
                    x0, y0 = mercator(ext["xmin"], ext["ymin"])
                    x1, y1 = mercator(ext["xmax"], ext["ymax"])
                    bbox = [x0, y0, x1, y1]
            if bbox:
                bbox = [round(v) for v in bbox]
            layers.append({"id": layer["id"], "name": d.get("name") or layer["name"], "geometry": d.get("geometryType"), "bbox": bbox, "max": d.get("maxRecordCount")})
        except Exception as ex:
            layers.append({"id": layer["id"], "name": layer["name"], "error": str(ex)})
    return entry["id"], {"service": base, "layers": layers}


def communes():
    """Writes communes.json from the Census 2024 communes layer (the whole layer, no geometry)."""
    entry = next(e for e in gen.LAYERS if e["id"] == "census-2024-communes")
    (layer,) = entry["subs"]
    url = f"{entry['service']}/{layer[0]}/query?where=1%3D1&outFields=CUT,COD_REGION,REGION,COD_PROVINCIA,PROVINCIA,COMUNA&returnGeometry=false&f=json"
    d = get(url)
    assert not d.get("exceededTransferLimit"), "the communes didn't come whole"
    rows = []
    for f in d["features"]:
        a = f["attributes"]
        rows.append({k.lower(): a[k] for k in ("CUT", "COD_REGION", "REGION", "COD_PROVINCIA", "PROVINCIA", "COMUNA")})
    rows.sort(key=lambda r: r["cut"])
    assert len(rows) == len({r["cut"] for r in rows}), "duplicate CUT"
    assert all(1101 <= r["cut"] <= 16399 and r["cut"] // 1000 == r["cod_region"] and r["cut"] // 100 == r["cod_provincia"] for r in rows)
    out = {"source": entry["service"], "layer": layer[0], "credit": entry["credit"], "communes": rows}
    (root / "communes.json").write_text(json.dumps(out, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"communes.json: {len(rows)} communes, {len({r['cod_provincia'] for r in rows})} provinces, {len({r['cod_region'] for r in rows})} regions")


def main():
    if sys.argv[1:] == ["communes"]:
        communes()
        return
    communes()
    entries = [e for e in gen.LAYERS if e["kind"] == "arcgis"]
    with concurrent.futures.ThreadPoolExecutor(8) as pool:
        services = dict(pool.map(fetch, entries))
    (root / "services.json").write_text(json.dumps(services, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    for k, v in services.items():
        if "error" in v:
            print(f"{k}: {v['error']}")
        else:
            print(f"{k}: {len(v['layers'])} layers")


if __name__ == "__main__":
    main()
