"""Reads what each ArcGIS service in the table offers and writes services.json for generate.py.

    python plugins/ide-chile/refresh.py && python plugins/ide-chile/generate.py && cargo fmt -p ide-chile

For every ArcGIS entry: the service's queryable layers (group layers can't be queried), each with
its name, geometry type and extent in Web Mercator (EPSG:3857), from which the plugin picks the
layers that reach the map view. An extent in another system is left out (the layer is then always
considered).
"""

import concurrent.futures
import importlib.util
import json
import math
import pathlib
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


def main():
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
