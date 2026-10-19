"""Generates the IDE Chile plugin's plugin.json and src/catalog.rs from one table.

    python plugins/ide-chile/generate.py && cargo fmt -p ide-chile

Edit the table (LAYERS), the groups and the translations here, never the two generated files.

Every address below was checked against the live service when the table was written (the date
is in the README). A layer offers a map image (an ArcGIS MapServer `export` tile template, drawn
by RustGIS as a web tile layer), features (a feature service RustGIS reads in the map view), or
both; features come as GeoJSON from one address, the whole layer or the part in the map view.
"""

import json
import pathlib

root = pathlib.Path(__file__).resolve().parent

# What each ArcGIS service offers, as read from the services (refresh.py fetches it again): its
# queryable layers, with their names and Web Mercator extents.
SERVICES = json.loads((root / "services.json").read_text(encoding="utf-8"))

CIREN = "https://esri.ciren.cl/server/rest/services/IDEMINAGRI/{}/MapServer"
MINVU = "https://geoide.minvu.cl/server/rest/services/{}/MapServer"
INE = "https://services5.arcgis.com/hUyD8u3TeZLKPe4T/arcgis/rest/services/{}/FeatureServer"
GEOPORTAL = "https://geoportal.cl/geoserver/{}/wfs"

MINAGRI_CREDIT = "IDE MINAGRI / CIREN (esri.ciren.cl)"
MINVU_CREDIT = "IDE MINVU (geoide.minvu.cl)"
INE_CREDIT = "INE, Censo 2017 (geoine-ine-chile.opendata.arcgis.com)"
INE_2024_CREDIT = "INE, Censo 2024 (geoine-ine-chile.opendata.arcgis.com)"
GEOPORTAL_CREDIT = "Geoportal de Chile, IDE Chile ({})"


def export(base, show=None):
    """An ArcGIS MapServer export tile template, optionally of some layers only."""
    t = base + "/export?bbox={bbox-epsg-3857}&bboxSR=3857&imageSR=3857&size=512,512&format=png32&transparent=true&f=image"
    if show:
        t += "&layers=show:" + ",".join(str(i) for i in show)
    return t


def subs_of(id, show=None):
    """An ArcGIS service's queryable layers (the ones in `show`, or all): (id, name, bbox)."""
    layers = SERVICES[id]["layers"]
    chosen = [l for l in layers if not show or l["id"] in show]
    assert chosen, id
    assert not show or len(chosen) == len(show), (id, show)
    return [(l["id"], l["name"], l["bbox"] if isinstance(l.get("bbox"), list) else None) for l in chosen]


def arcgis(id, en, es, help_en, help_es, base, credit, show=None, whole=False, icon="polygon_layer", name=None, tiles=True):
    subs = subs_of(id, show)
    assert not whole or len(subs) == 1, (id, "a layer read whole is one layer")
    return dict(
        id=id, en=en, es=es, help_en=help_en, help_es=help_es, name=name or es, icon=icon,
        tiles=export(base, show) if tiles else "", service=base, kind="arcgis", layer="", subs=subs,
        whole=whole, credit=credit,
    )


def minagri(id, en, es, help_en, help_es, service, show=None, whole=False, icon="polygon_layer", name=None):
    return arcgis(id, en, es, help_en, help_es, CIREN.format(service), MINAGRI_CREDIT, show, whole, icon, name)


def minvu(id, en, es, help_en, help_es, service, show=None, whole=False, icon="polygon_layer", name=None):
    return arcgis(id, en, es, help_en, help_es, MINVU.format(service), MINVU_CREDIT, show, whole, icon, name)


def ine(id, en, es, help_en, help_es, service, show=None, whole=False, icon="polygon_layer", name=None, credit=INE_CREDIT):
    # A hosted feature service draws no map image.
    return arcgis(id, en, es, help_en, help_es, INE.format(service), credit, show, whole, icon, name, tiles=False)


def ine_2024(id, en, es, help_en, help_es, service, show=None, whole=False, icon="polygon_layer", name=None):
    return ine(id, en, es, help_en, help_es, service, show, whole, icon, name, credit=INE_2024_CREDIT)


def geoportal(id, en, es, help_en, help_es, workspace, typename, publisher, whole=False, icon="point_layer", name=None):
    return dict(
        id=id, en=en, es=es, help_en=help_en, help_es=help_es, name=name or es, icon=icon,
        tiles="", service=GEOPORTAL.format(workspace), kind="wfs", layer=f"{workspace}:{typename}", subs=[],
        whole=whole, credit=GEOPORTAL_CREDIT.format(publisher),
    )


# The layers, in the order of the ribbon. Every layer is read as features. `whole` marks the ones
# small enough to read whole (one request, at most 2,000 ArcGIS features); the others are read in
# the map view, an ArcGIS service's layers that reach the view each as a layer (see lib.rs).
# `show` picks an ArcGIS service's layers; without it, every queryable layer is offered.
LAYERS = [
    # Boundaries (IDE MINAGRI)
    minagri("regions", "Regions", "Regiones", "Regional and international boundaries of Chile (IDE MINAGRI, CIREN). As features: the boundary polygons in the map view, in full detail (Census 2024 › Regions has the 16 regions).", "Límites regionales e internacionales de Chile (IDE MINAGRI, CIREN). Como entidades: los polígonos en la vista del mapa, con todo su detalle (Censo 2024 › Regiones trae las 16 regiones).", "LIMITES_ADMINISTRATIVOS_MINAGRI", show=[1, 2, 3, 4], name="Regiones (IDE MINAGRI)"),
    minagri("communes", "Communes", "Comunas", "Communal boundaries of Chile (IDE MINAGRI, CIREN). As features: the communes in the map view, in full detail (Census 2024 › Communes has them all).", "Límites comunales de Chile (IDE MINAGRI, CIREN). Como entidades: las comunas en la vista del mapa, con todo su detalle (Censo 2024 › Comunas las trae todas).", "LIMITES_ADMINISTRATIVOS_MINAGRI", show=[6, 7, 8, 9], name="Comunas (IDE MINAGRI)"),
    # Agriculture and soils (IDE MINAGRI)
    minagri("soils", "Agrological Soils", "Suelos agrológicos", "Land use capability classes of agricultural soils, by region (CIREN agrological studies).", "Clases de capacidad de uso de los suelos agrícolas, por región (estudios agrológicos de CIREN).", "SUELOS_AGROLOGICOS", name="Suelos agrológicos (CIREN)"),
    minagri("land-use", "Land Use (CONAF)", "Uso de la tierra (CONAF)", "CONAF's vegetation and land use cadastre, by region.", "Catastro de uso de la tierra y vegetación de CONAF, por región.", "USO_DE_LA_TIERRA_CUT_CONAF", name="Uso de la tierra (CONAF)"),
    minagri("rural-properties", "Rural Properties", "Propiedades rurales", "Rural property boundaries from SII data, by region (CIREN).", "Límites de propiedades rurales según datos del SII, por región (CIREN).", "PROPIEDADES_RURALES", name="Propiedades rurales (CIREN)"),
    minagri("irrigation", "Irrigation", "Infraestructura de riego", "Irrigation canals, intakes, infrastructure and the reservoir plan.", "Canales de riego, bocatomas, infraestructura y plan de embalses.", "INFRAESTRUCTURA_RIEGO", icon="line_layer", name="Infraestructura de riego (CIREN)"),
    minagri("erosion", "Erosion", "Erosión actual", "Current erosion of soils, by region (CIREN).", "Erosión actual de los suelos, por región (CIREN).", "EROSION", name="Erosión actual (CIREN)"),
    minagri("fruit-cadastre", "Fruit Cadastre", "Catastro frutícola", "Fruit producers, agroindustries, packing plants and cold storage, by region (CIREN).", "Productores frutícolas, agroindustrias, packings y frigoríficos, por región (CIREN).", "CATASTRO_FRUTICOLA", icon="point_layer", name="Catastro frutícola (CIREN)"),
    minagri("weather-stations", "Weather Stations", "Estaciones meteorológicas", "Agrometeorological stations (2011) and agroclimatic districts and zones.", "Estaciones meteorológicas (2011), distritos y zonas agroclimáticas.", "AGROCLIMA_IDEMINAGRI", show=[7], whole=True, icon="point_layer", name="Estaciones meteorológicas (IDE MINAGRI)"),
    minagri("summer-pastures", "Summer Pastures", "Veranadas", "Summer pastures (veranadas) of the Andes.", "Veranadas de la cordillera.", "VERANADAS", whole=True, name="Veranadas (IDE MINAGRI)"),
    minagri("hunting-ban", "Hunting Ban Areas", "Áreas prohibidas de caza", "Areas where hunting is prohibited (SAG, 2013).", "Áreas prohibidas de caza (SAG, 2013).", "AREAS_PROHIBIDAS_CAZA", whole=True, name="Áreas prohibidas de caza (SAG)"),
    # Environment and hazards
    minagri("wildfires", "Wildfires 2024–2025", "Incendios 2024–2025", "Forest fires of the 2024–2025 season (CONAF).", "Incendios forestales de la temporada 2024–2025 (CONAF).", "INCENDIOS", name="Incendios forestales 2024–2025 (CONAF)"),
    minagri("ramsar", "Ramsar Wetlands", "Humedales Ramsar", "Ramsar wetlands of Chile (2013).", "Humedales Ramsar de Chile (2013).", "HUMEDALES", whole=True, name="Humedales Ramsar"),
    geoportal("wetlands", "Wetlands (MMA)", "Humedales (MMA)", "The Ministry of the Environment's wetlands inventory. Read in the map view.", "Inventario de humedales del Ministerio del Medio Ambiente. Se lee en la vista del mapa.", "Humedales", "humedales", "MMA", icon="polygon_layer", name="Humedales (MMA)"),
    geoportal("hydrography", "Hydrography", "Hidrografía", "Rivers and water bodies from Arica to O'Higgins (SNIT). Read in the map view.", "Ríos y cuerpos de agua de Arica a O'Higgins (SNIT). Se lee en la vista del mapa.", "Hidrografia", "hidrografa", "SNIT", icon="line_layer", name="Hidrografía (SNIT)"),
    geoportal("basins", "Basins", "Cuencas", "Basins of the National Water Bank (MOP, DGA).", "Cuencas del Banco Nacional de Aguas (MOP, DGA).", "Cuencas_Banco_Nacional_Aguas", "cuencas_banco_nacional_de_aguas", "MOP", whole=True, icon="polygon_layer", name="Cuencas (Banco Nacional de Aguas)"),
    geoportal("aquifers", "Aquifer Restrictions", "Restricción de acuíferos", "Aquifer restriction and prohibition zones (MOP, DGA).", "Zonas de restricción y prohibición de acuíferos (MOP, DGA).", "Restriccion_Prohibicion_Acuiferos", "restriccin_prohibicin_de_acuferos", "MOP", whole=True, icon="polygon_layer", name="Restricción y prohibición de acuíferos (DGA)"),
    geoportal("climate-zones", "Climate Zones", "Zonas climáticas", "Köppen-Geiger climate zones of Chile at 1:1,500,000 (Universidad de Chile): a 26 MB download.", "Zonas climáticas de Chile según Köppen-Geiger a 1:1.500.000 (Universidad de Chile): una descarga de 26 MB.", "Zonas_climaticas_Chile_Koppen_Geiger_escala_1_1500000", "zonas_climticas_de_chile_segn_kppengeiger_escala_11500000", "U. de Chile", whole=True, icon="polygon_layer", name="Zonas climáticas Köppen-Geiger"),
    # Cities and housing (IDE MINVU)
    minvu("urban-limits", "Urban Limits", "Límites urbanos", "Urban limits of the communal, intercommunal and metropolitan plans (PRC, PRI, PRMS, PREMVAL, PRMC). As features: the PRC limits.", "Límites urbanos de los planes reguladores comunales, intercomunales y metropolitanos (PRC, PRI, PRMS, PREMVAL, PRMC). Como entidades: los límites de PRC.", "IPT/Limites_Urbanos", name="Límites urbanos (MINVU)"),
    minvu("risk-areas", "Risk Areas (IPT)", "Áreas de riesgo (IPT)", "Risk areas of the communal and intercommunal plans.", "Áreas de riesgo de los planes reguladores comunales e intercomunales.", "IPT/IPT_AREA_RIESGO", name="Áreas de riesgo IPT (MINVU)"),
    minvu("heritage", "Heritage", "Patrimonio", "Heritage conservation buildings (ICH) and zones (ZCH).", "Inmuebles (ICH) y zonas (ZCH) de conservación histórica.", "IPT/Patrimonio", name="Patrimonio ICH y ZCH (MINVU)"),
    minvu("urban-wetlands", "Urban Wetlands", "Humedales urbanos", "The 2020 urban wetlands inventory.", "Inventario de humedales urbanos 2020.", "Inventario_de_Humedales", name="Humedales urbanos 2020 (MINVU)"),
    minvu("settlements", "Informal Settlements", "Campamentos", "The 2026 cadastre of informal settlements (campamentos).", "Catastro de campamentos 2026.", "Catastros/Catastros_campamentos", show=[0], whole=True, name="Campamentos 2026 (MINVU)"),
    minvu("urban-parks", "Urban Parks", "Parques urbanos", "The cadastre of urban parks and their accesses.", "Catastro de parques urbanos y sus accesos.", "Catastros/Catastro_de_parques_urbanos", show=[1], whole=True, name="Parques urbanos (MINVU)"),
    minvu("housing-deficit", "Housing Deficit 2024", "Déficit habitacional 2024", "Quantitative housing deficit by commune (Census 2024).", "Déficit habitacional cuantitativo por comuna (Censo 2024).", "Otros/D%C3%A9ficit_habitacional_cuantitativo_2024", show=[1], name="Déficit habitacional 2024 (MINVU)"),
    minvu("bike-lanes", "Bike Lanes", "Ciclovías", "Bike lanes built by MINVU, their counters and nearby facilities.", "Ciclovías construidas por el MINVU, sus contadores y equipamientos cercanos.", "Planes_Programas/Ciclov%C3%ADas_Minvu", icon="line_layer", name="Ciclovías (MINVU)"),
    # Census 2024 (INE): the political-administrative division (DPA) the census was taken on, its
    # indicators by commune and region, and the neighbourhood units with their figures.
    ine_2024("census-2024-regions", "Regions", "Regiones", "The 16 regions of the Census 2024 political-administrative division (INE): a download of about 10 MB.", "Las 16 regiones de la división político-administrativa del Censo 2024 (INE): una descarga de unos 10 MB.", "L%C3%ADmites_DPA___Censo_2024", show=[2], whole=True, name="Censo 2024: regiones"),
    ine_2024("census-2024-provinces", "Provinces", "Provincias", "The 56 provinces of the Census 2024 political-administrative division (INE): a download of about 27 MB.", "Las 56 provincias de la división político-administrativa del Censo 2024 (INE): una descarga de unos 27 MB.", "L%C3%ADmites_DPA___Censo_2024", show=[3], whole=True, name="Censo 2024: provincias"),
    ine_2024("census-2024-communes", "Communes", "Comunas", "The communes of the Census 2024 political-administrative division, with their CUT codes (INE): a download of about 25 MB.", "Las comunas de la división político-administrativa del Censo 2024, con su código único territorial (INE): una descarga de unos 25 MB.", "L%C3%ADmites_DPA___Censo_2024", show=[4], whole=True, name="Censo 2024: comunas"),
    ine_2024("census-2024-commune-indicators", "Commune Indicators", "Indicadores por comuna", "Census 2024 indicators by commune (INE): dwellings, households, population by sex and age group, ageing index.", "Indicadores del Censo 2024 por comuna (INE): viviendas, hogares, población por sexo y grupo de edad, índice de envejecimiento.", "CENSO2024_V2_gdb", show=[0], whole=True, icon="stats", name="Censo 2024: indicadores por comuna"),
    ine_2024("census-2024-region-indicators", "Region Indicators", "Indicadores por región", "Census 2024 indicators by region (INE): dwellings, households, population by sex and age group, ageing index.", "Indicadores del Censo 2024 por región (INE): viviendas, hogares, población por sexo y grupo de edad, índice de envejecimiento.", "CENSO2024_V2_gdb", show=[1], whole=True, icon="stats", name="Censo 2024: indicadores por región"),
    ine_2024("census-2024-neighbourhoods", "Neighbourhood Units", "Unidades vecinales", "Census 2024 figures by neighbourhood unit (unidad vecinal, INE): population by age, migration, indigenous peoples, commuting, schooling and housing. Read in the map view, up to 2,000 at a time.", "Cifras del Censo 2024 por unidad vecinal (INE): población por edad, migración, pueblos originarios, transporte, escolaridad y vivienda. Se lee en la vista del mapa, hasta 2.000 por vez.", "UV_Nacional", name="Censo 2024: unidades vecinales"),
    # Census 2017 (INE)
    ine("census-regions", "Regions", "Regiones", "Census 2017 figures by region.", "Cifras del Censo 2017 por región.", "Cifras_Regiones_2017", whole=True, name="Censo 2017: regiones"),
    ine("census-provinces", "Provinces", "Provincias", "Census 2017 figures by province.", "Cifras del Censo 2017 por provincia.", "Cifras_Provincias_2017", whole=True, name="Censo 2017: provincias"),
    ine("census-communes", "Communes", "Comunas", "Census 2017 figures by commune.", "Cifras del Censo 2017 por comuna.", "Cifras_Comunas_2017", whole=True, name="Censo 2017: comunas"),
    ine("census-blocks", "Blocks", "Manzanas", "Census 2017 microdata by block (manzana). Read in the map view, up to 2,000 blocks at a time.", "Microdatos del Censo 2017 por manzana. Se lee en la vista del mapa, hasta 2.000 manzanas por vez.", "Manzana_2017_2", name="Censo 2017: manzanas"),
    ine("census-localities", "Localities", "Entidades", "Census 2017 microdata by rural locality (entidad). Read in the map view, up to 2,000 at a time.", "Microdatos del Censo 2017 por entidad rural. Se lee en la vista del mapa, hasta 2.000 por vez.", "Entidad_2017_2", icon="point_layer", name="Censo 2017: entidades"),
    ine("census-cities", "Cities", "Ciudades", "Cities of the Census 2017.", "Ciudades del Censo 2017.", "Ciudades_2017", whole=True, name="Censo 2017: ciudades"),
    ine("census-towns", "Towns", "Pueblos", "Towns of the Census 2017.", "Pueblos del Censo 2017.", "Pueblos_2017", whole=True, name="Censo 2017: pueblos"),
    ine("census-villages", "Villages", "Aldeas", "Villages (aldeas) of the Census 2017.", "Aldeas del Censo 2017.", "Aldeas_2017", whole=True, name="Censo 2017: aldeas"),
    ine("census-hamlets", "Hamlets", "Caseríos", "Hamlets (caseríos) of the Census 2017. Read in the map view, up to 2,000 at a time.", "Caseríos del Censo 2017. Se lee en la vista del mapa, hasta 2.000 por vez.", "Caserios_2017", icon="point_layer", name="Censo 2017: caseríos"),
    # Transport and services (Geoportal de Chile)
    geoportal("roads", "Road Network", "Red vial", "The road network (Instituto de la Propiedad). Read in the map view.", "Red vial (Instituto de la Propiedad). Se lee en la vista del mapa.", "Redvial", "red_vial", "Instituto de la Propiedad", icon="line_layer", name="Red vial"),
    geoportal("railways", "Railways", "Red ferroviaria", "The railway network (Biblioteca del Congreso Nacional).", "Red ferroviaria (Biblioteca del Congreso Nacional).", "Red_Ferroviaria", "red_ferroviaria", "BCN", whole=True, icon="line_layer", name="Red ferroviaria (BCN)"),
    geoportal("airports", "Airports", "Aeródromos", "Airports and aerodromes (MOP).", "Aeropuertos y aeródromos (MOP).", "Infraestructura_Aerea", "infraestructura_area", "MOP", whole=True, name="Infraestructura aérea (MOP)"),
    geoportal("schools", "Schools", "Establecimientos escolares", "School establishments (Ministry of Education). Read in the map view.", "Establecimientos de educación escolar (Mineduc). Se lee en la vista del mapa.", "Establecimientos_Educacion_Escolar", "establecimientos_educacin_escolar", "Mineduc", name="Establecimientos educación escolar (Mineduc)"),
    geoportal("preschools", "Preschools", "Educación parvularia", "Preschool establishments (Ministry of Education). Read in the map view.", "Establecimientos de educación parvularia (Mineduc). Se lee en la vista del mapa.", "Establecimientos_Educacion_Parvularia", "establecimientos_educacin_parvularia", "Mineduc", name="Establecimientos educación parvularia (Mineduc)"),
    geoportal("higher-education", "Higher Education", "Educación superior", "Higher education institutions (Ministry of Education).", "Establecimientos de educación superior (Mineduc).", "Establecimientos_Educacion_Superior", "establecimientos_de_educacin_superior", "Mineduc", whole=True, name="Establecimientos educación superior (Mineduc)"),
    geoportal("health-services", "Health Services", "Servicios de Salud", "Offices of the Health Services (Ministry of Health).", "Direcciones de los Servicios de Salud (Minsal).", "Direcciones_Servicios_Salud", "direcciones_servicios_de_salud", "Minsal", whole=True, name="Direcciones Servicios de Salud (Minsal)"),
    geoportal("police", "Police Stations", "Cuarteles de Carabineros", "Carabineros de Chile stations.", "Cuarteles de Carabineros de Chile.", "Cuarteles_Carabineros", "cuarteles_de_carabineros", "Carabineros", whole=True, name="Cuarteles de Carabineros"),
    geoportal("municipalities", "Municipalities", "Municipios", "Municipal offices (SUBDERE).", "Municipios (SUBDERE).", "Municipios", "municipios", "SUBDERE", whole=True, name="Municipios (SUBDERE)"),
    geoportal("capitals", "Capitals", "Capitales", "Regional and provincial capitals (SNIT).", "Capitales regionales y provinciales (SNIT).", "Capitales", "capitales", "SNIT", whole=True, name="Capitales (SNIT)"),
    geoportal("tourist-attractions", "Tourist Attractions", "Atractivos turísticos", "National tourist attractions 2020 (SERNATUR).", "Atractivos turísticos nacionales 2020 (Sernatur).", "Atractivos_Turisticos_Nacionales", "atractivos_tursticos_2020", "Sernatur", whole=True, name="Atractivos turísticos 2020 (Sernatur)"),
    geoportal("street-markets", "Street Markets", "Ferias libres", "Street markets (ferias libres, SNIT).", "Ferias libres (SNIT).", "Ferias_Libres", "ferias_libres", "SNIT", whole=True, name="Ferias libres"),
    geoportal("sports", "Sports Facilities", "Infraestructura deportiva", "Sports facilities (Ministry of Sport). Read in the map view.", "Infraestructura deportiva (Ministerio del Deporte). Se lee en la vista del mapa.", "Infraestructura_Deportiva", "infraestructura_deportiva", "Mindep", name="Infraestructura deportiva (Mindep)"),
    geoportal("sendero", "Sendero de Chile", "Sendero de Chile", "The Sendero de Chile trail (MMA).", "Sendero de Chile (MMA).", "Sendero_Chile", "sendero_de_chile", "MMA", whole=True, icon="line_layer", name="Sendero de Chile (MMA)"),
]

# The ribbon tab: group label (en, es) and its items (ids, or menus).
TAB = [
    ("Boundaries", "Límites", ["regions", "communes"]),
    ("Agriculture", "Agricultura", [("Soils & Agriculture", "Suelos y agricultura", "polygon_layer", "Soils, land use, properties, irrigation and crops (IDE MINAGRI).", "Suelos, uso de la tierra, propiedades, riego y cultivos (IDE MINAGRI).", ["soils", "land-use", "rural-properties", "irrigation", "erosion", "fruit-cadastre", "weather-stations", "summer-pastures", "hunting-ban"])]),
    ("Environment", "Medio ambiente", [("Environment & Hazards", "Medio ambiente y riesgos", "relief", "Wildfires, wetlands, water and climate.", "Incendios, humedales, aguas y clima.", ["wildfires", "ramsar", "wetlands", "hydrography", "basins", "aquifers", "climate-zones"])]),
    ("Cities", "Ciudades", [("Cities & Housing", "Ciudades y vivienda", "polygon_layer", "Urban planning and housing layers (IDE MINVU).", "Capas de planificación urbana y vivienda (IDE MINVU).", ["urban-limits", "risk-areas", "heritage", "urban-wetlands", "settlements", "urban-parks", "housing-deficit", "bike-lanes"])]),
    ("Census", "Censo", [("Census 2024", "Censo 2024", "stats", "Census 2024 boundaries, indicators and neighbourhood units (INE).", "Límites, indicadores y unidades vecinales del Censo 2024 (INE).", ["census-2024-regions", "census-2024-provinces", "census-2024-communes", "census-2024-commune-indicators", "census-2024-region-indicators", "census-2024-neighbourhoods"]), ("Census 2017", "Censo 2017", "stats", "Census 2017 figures and microdata (INE).", "Cifras y microdatos del Censo 2017 (INE).", ["census-regions", "census-provinces", "census-communes", "census-blocks", "census-localities", "census-cities", "census-towns", "census-villages", "census-hamlets"])]),
    ("Services", "Servicios", [("Transport & Services", "Transporte y servicios", "point_layer", "Roads, railways, airports, schools, health, police and tourism (Geoportal de Chile).", "Red vial, ferrocarriles, aeródromos, escuelas, salud, Carabineros y turismo (Geoportal de Chile).", ["roads", "railways", "airports", "schools", "preschools", "higher-education", "health-services", "police", "municipalities", "capitals", "tourist-attractions", "street-markets", "sports", "sendero"])]),
    ("Manage", "Gestionar", ["clear", "about"]),
]

PARAMS = '{"map"?: bool (the service\'s own map image as a web tile layer under the layers, instead of the features)}'

# Messages the module builds while it runs (see src/lib.rs), with their Spanish.
MESSAGES = {
    "Added {name} (map image, under the layers; Map › Basemap lists it).": "Añadida {name} (imagen del mapa, bajo las capas; Mapa › Mapa base la lista).",
    "In the web app, add {name} with Home › Add Data › From URL and this address: {url}": "En la app web, añada {name} con Inicio › Añadir datos › Desde URL y esta dirección: {url}",
    "In the web app, {name} is shown as a map image; its features need the desktop app.": "En la app web, {name} se muestra como imagen del mapa; sus entidades requieren la app de escritorio.",
    "This RustGIS can't add data by address from a plugin: update it (0.2.1 or later) for {name}, or add this address with Home › Add Data › From URL: {url}": "Este RustGIS no puede añadir datos por dirección desde un complemento: actualícelo (0.2.1 o posterior) para {name}, o añada esta dirección con Inicio › Añadir datos › Desde URL: {url}",
    "This RustGIS can't add data by address from a plugin: {name} is shown as a map image. Update RustGIS (0.2.1 or later) for its features.": "Este RustGIS no puede añadir datos por dirección desde un complemento: {name} se muestra como imagen del mapa. Actualice RustGIS (0.2.1 o posterior) para sus entidades.",
    "Added {name}.": "Añadida {name}.",
    "Added {name} in the map view.": "Añadida {name} en la vista del mapa.",
    "Downloading {name}; it is added when it lands.": "Descargando {name}; se añade cuando termine.",
    "{name} is read in the map view: zoom in to 1:2,000,000 or closer first.": "{name} se lee en la vista del mapa: acerque primero a 1:2.000.000 o más cerca.",
    "Removed {name}.": "Quitada {name}.",
    "{name} has no map image.": "{name} no tiene imagen de mapa.",
    "Added {n} layers of {name} in the map view.": "Añadidas {n} capas de {name} en la vista del mapa.",
    "Nothing of {name} reaches the map view.": "Nada de {name} llega a la vista del mapa.",
    "Removed {n} IDE Chile layers.": "Quitadas {n} capas de IDE Chile.",
    "No IDE Chile map layers to remove.": "No hay capas de mapa de IDE Chile que quitar.",
    "IDE Chile: {n} layers from Geoportal de Chile, IDE MINAGRI, IDE MINVU and INE. Each layer's credit names its source.": "IDE Chile: {n} capas del Geoportal de Chile, IDE MINAGRI, IDE MINVU e INE. El crédito de cada capa nombra su fuente.",
}


def command(e):
    return {"id": e["id"], "label": e["en"], "help": e["help_en"], "icon": e["icon"], "params": PARAMS}


def main():
    ids = {e["id"] for e in LAYERS}
    assert len(ids) == len(LAYERS), "duplicate ids"
    commands = [command(e) for e in LAYERS] + [
        {"id": "clear", "label": "Clear Map Images", "help": "Removes the IDE Chile map images (web tile layers) from the map.", "icon": "delete", "params": "{}"},
        {"id": "about", "label": "About IDE Chile", "help": "The sources behind the layers.", "icon": "help", "params": "{}"},
    ]
    es = {}
    for e in LAYERS:
        es[e["en"]] = e["es"]
        es[e["help_en"]] = e["help_es"]
    es.update({
        "Clear Map Images": "Quitar imágenes de mapa",
        "Removes the IDE Chile map images (web tile layers) from the map.": "Quita del mapa las imágenes de mapa (capas de teselas) de IDE Chile.",
        "About IDE Chile": "Acerca de IDE Chile",
        "The sources behind the layers.": "Las fuentes de las capas.",
        "IDE Chile": "IDE Chile",
        "Public geospatial services of Chile as one-click layers: the Geoportal de Chile (IDE Chile), IDE MINAGRI, IDE MINVU and the INE's Censuses 2024 and 2017. Boundaries, soils, land use, wetlands, hydrography, urban planning, census data, roads, schools and more, as map images or as features you can query.": "Servicios geoespaciales públicos de Chile en capas de un clic: Geoportal de Chile (IDE Chile), IDE MINAGRI, IDE MINVU y los Censos 2024 y 2017 del INE. Límites, suelos, uso de la tierra, humedales, hidrografía, planificación urbana, datos censales, red vial, escuelas y más, como imágenes de mapa o como entidades consultables.",
    })
    es.update(MESSAGES)
    groups = []
    for en, es_label, items in TAB:
        es[en] = es_label
        out = []
        for it in items:
            if isinstance(it, tuple):
                menu_en, menu_es, icon, help_en, help_es, entries = it
                es[menu_en] = menu_es
                es[help_en] = help_es
                for i in entries:
                    assert i in ids, i
                out.append({"menu": menu_en, "icon": icon, "help": help_en, "items": entries})
            else:
                assert it in ids or it in ("clear", "about"), it
                out.append(it)
        groups.append({"label": en, "items": out})
    placed = {i for g in groups for it in g["items"] for i in (it["items"] if isinstance(it, dict) else [it])}
    assert placed >= ids, ids - placed
    manifest = {
        "id": "ide-chile",
        "name": "IDE Chile",
        "version": "0.2.0",
        "description": "Public geospatial services of Chile as one-click layers: the Geoportal de Chile (IDE Chile), IDE MINAGRI, IDE MINVU and the INE's Censuses 2024 and 2017. Boundaries, soils, land use, wetlands, hydrography, urban planning, census data, roads, schools and more, as map images or as features you can query.",
        "author": "Daniel Díaz",
        "homepage": "https://github.com/opengeos/rustgis-plugins/tree/main/plugins/ide-chile",
        "license": "MIT",
        "entry": "plugin.wasm",
        "abi": 1,
        "minRustgisVersion": "0.2.0",
        "permissions": ["edit", "network"],
        "commands": commands,
        "tools": [],
        "tab": {"label": "IDE Chile", "groups": groups},
        "i18n": {"es": es},
    }
    (root / "plugin.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

    def s(x):
        return json.dumps(x, ensure_ascii=False)

    def sub(l):
        bbox = "Some([" + ", ".join(f"{v:.1f}" for v in l[2]) + "])" if l[2] else "None"
        return f"Sub {{ id: {l[0]}, name: {s(l[1])}, bbox: {bbox} }}"

    rows = []
    for e in LAYERS:
        subs = "&[" + ", ".join(sub(l) for l in e["subs"]) + "]"
        rows.append(
            f"    Entry {{ id: {s(e['id'])}, name: {s(e['name'])}, tiles: {s(e['tiles'])}, service: {s(e['service'])}, kind: {s(e['kind'])}, layer: {s(e['layer'])}, subs: {subs}, whole: {'true' if e['whole'] else 'false'}, credit: {s(e['credit'])} }},"
        )
    rs = "//! The layer catalog. Generated by `generate.py`: edit the table there, not this file.\n\nuse crate::{Entry, Sub};\n\n/// Every layer the plugin offers, in ribbon order.\npub const CATALOG: &[Entry] = &[\n" + "\n".join(rows) + "\n];\n"
    (root / "src" / "catalog.rs").write_text(rs, encoding="utf-8")
    print(f"{len(LAYERS)} layers, {len(commands)} commands, {len(es)} Spanish strings")


if __name__ == "__main__":
    main()
