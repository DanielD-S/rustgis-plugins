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

## Map images and features

A layer comes as a **map image**, as **features**, or both:

- A map image is the service's own rendering (an ArcGIS MapServer `export` request per 512-pixel
  tile, from zoom 4 up), added as a web tile layer. Like every web tile layer in RustGIS it is
  drawn under the map's own layers and listed under **Map › Basemap**, not in the Layers pane;
  choosing it again takes it away. These work on the desktop, on the command line and in the web
  app.
- Features are data: an attribute table, selection, symbology and analysis. They come as GeoJSON
  from the service, on the desktop and on the command line. The web app downloads only from its
  own **Home › Add Data › From URL** dialog, so there a layer that offers both shows its map image
  and a features-only layer gives the address to paste into that dialog. Small layers (regions,
  census communes, capitals, airports…) are read whole and zoomed to; ArcGIS layers read whole are
  generalised to about 10 m, so national boundaries stay a few megabytes. Large layers (communes, the
  wetlands inventory, hydrography, the road network, census blocks, schools…) are read in the
  current map view, up to 5,000 features at a time (2,000 from ArcGIS servers): zoom to your area
  first (1:2,000,000 or closer), and choose the layer again after moving the map to read the new
  view.
- A layer that offers both adds its map image; `{"data": true}` asks for the features instead:

```sh
rustgis-cli run --plugin ide-chile-0.1.0.rgplugin \
  --cmd 'plugin:ide-chile/communes' \
  --cmd 'plugin:ide-chile/communes={"data":true}' \
  --cmd 'plugin:ide-chile/census-communes' \
  --cmd 'share.exportMap={"path":"chile.png"}'
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
[`generate.py`](generate.py) from one table (`python plugins/ide-chile/generate.py && cargo fmt -p
ide-chile`); tests check they agree and that every string has its Spanish.
