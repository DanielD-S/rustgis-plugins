# IDE Chile

Public geospatial services of Chile as one-click layers for RustGIS: the
[Geoportal de Chile](https://www.geoportal.cl) (the national node of IDE Chile),
[IDE MINAGRI](https://ide.minagri.gob.cl) (the Ministry of Agriculture, served by CIREN),
[IDE MINVU](https://ide.minvu.cl) (the Ministry of Housing and Urbanism) and the
[INE](https://geoine-ine-chile.opendata.arcgis.com)'s Census 2017.

Boundaries, soils, land use, wetlands, hydrography, urban planning, census data, roads, schools,
health and tourism: 49 layers in a ribbon tab of their own, each a command that scripts, the
command line and the AI Assistant can run too.

## Its tab

**IDE Chile**, at the end of the ribbon's tab strip:

- **Boundaries**: Regions, Communes (IDE MINAGRI)
- **Agriculture**: **Soils & Agriculture ▾** (9): agrological soils, land use (CONAF), rural
  properties, irrigation, erosion, fruit cadastre, weather stations, summer pastures, hunting ban
  areas
- **Environment**: **Environment & Hazards ▾** (7): wildfires 2024–2025, Ramsar wetlands, the
  MMA wetlands inventory, hydrography, basins, aquifer restrictions, Köppen-Geiger climate zones
- **Cities**: **Cities & Housing ▾** (8): urban limits, IPT risk areas, heritage, urban
  wetlands, informal settlements, urban parks, housing deficit 2024, bike lanes (IDE MINVU)
- **Census**: **Census 2017 ▾** (9): regions, provinces, communes, blocks, localities, cities,
  towns, villages, hamlets (INE)
- **Services**: **Transport & Services ▾** (14): roads, railways, airports, schools, preschools,
  higher education, health services, police stations, municipalities, capitals, tourist
  attractions, street markets, sports facilities, Sendero de Chile (Geoportal de Chile)
- **Manage**: Clear IDE Chile, About IDE Chile

The interface is in English with a full Spanish translation; layer names on the map are the
datasets' own names.

## Features, read whole or in the map view

Every layer is added as features: an attribute table, selection, symbology and analysis. They
come as GeoJSON from the service, on the desktop, on the command line and in the web app (a
RustGIS from after October 2026, whose `layer.addUrl` downloads off the frame there; older web
builds fall back to the map image where there is one, or give the address to paste into
**Home › Add Data › From URL**).

- Small layers (census communes, capitals, airports, Ramsar wetlands…) are read whole and zoomed
  to; ArcGIS layers read whole are generalised to about 10 m, so national data stays a few
  megabytes.
- Large layers (communes, soils, land use, the wetlands inventory, hydrography, the road
  network, census blocks, schools…) are read in the current map view, up to 5,000 features at a
  time (2,000 from ArcGIS servers), generalised to the view's pixel: zoom to your area first
  (1:2,000,000 or closer), and choose the layer again after moving the map to read the new view.
- An ArcGIS service published by region or by theme (agrological soils, land use, rural
  properties, erosion, the fruit cadastre, urban limits…) adds one layer per published layer that
  reaches the view, named after it, up to eight at a time. A layer whose extent reaches the view
  but holds nothing in it comes in empty.
- Where the service also draws its own map image, `{"map": true}` adds that instead, as a web
  tile layer (an ArcGIS MapServer `export` request per 512-pixel tile, from zoom 4 up): like every
  web tile layer in RustGIS it is drawn under the map's own layers and listed under
  **Map › Basemap**, not in the Layers pane; chosen again, it is taken away, and **Clear Map
  Images** removes them all.

```sh
rustgis-cli run --plugin ide-chile-0.1.0.rgplugin   --cmd 'map.goToXY={"x":-70.65,"y":-33.45}' --cmd 'map.setScale={"scale":300000}'   --cmd 'plugin:ide-chile/communes'   --cmd 'plugin:ide-chile/soils'   --cmd 'plugin:ide-chile/census-communes'   --cmd 'plugin:ide-chile/regions={"map":true}'   --cmd 'share.exportMap={"path":"santiago.png"}'
```

## Sources and terms

Every address was checked against the live service on 2026-10-15. Each layer shows its source as
its credit, and the data stays under its publisher's terms:

| Source | Service | Layers |
|---|---|---|
| [IDE MINAGRI](https://ide.minagri.gob.cl/geoweb/directorio-de-servicios/) | CIREN's ArcGIS Server (`esri.ciren.cl`) | boundaries, soils, land use, properties, irrigation, erosion, fruit cadastre, agroclimate, wildfires, Ramsar wetlands, hunting ban areas |
| [IDE MINVU](https://ide.minvu.cl) | ArcGIS Server (`geoide.minvu.cl`) | urban limits, risk areas, heritage, urban wetlands, settlements, parks, housing deficit, bike lanes |
| [INE](https://geoine-ine-chile.opendata.arcgis.com) | ArcGIS Online feature services | Census 2017 |
| [Geoportal de Chile](https://www.geoportal.cl) | GeoServer WFS (`geoportal.cl/geoserver`) | wetlands (MMA), hydrography, basins, aquifers, climate zones, roads, railways, airports, schools, health, police, municipalities, capitals, tourism, street markets, sports, Sendero de Chile |

The Geoportal de Chile's WMS layers are published in EPSG:4326 only, which RustGIS's tile layers
don't draw, so those layers are offered as features. Its catalogue (a CSW at
`https://geoportal.cl/csw`, 996 records) can be searched from RustGIS once RustGIS sends the
catalogue a filter it accepts.

## Permissions

edit and network (adding web tile layers and feature layers, which RustGIS loads from the
services' servers)

## Build

```sh
cargo build --release --target wasm32-unknown-unknown -p ide-chile
```

`plugin.json`'s commands, tab and translations and `src/catalog.rs` are generated by
[`generate.py`](generate.py) from one table and from `services.json`, what each ArcGIS service
publishes (its queryable layers and their extents), which [`refresh.py`](refresh.py) reads from
the services (`python plugins/ide-chile/refresh.py && python plugins/ide-chile/generate.py &&
cargo fmt -p ide-chile`); tests check they agree and that every string has its Spanish.
