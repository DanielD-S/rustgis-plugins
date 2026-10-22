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
    ("CUT", "CUT", ["tool:join-cut", "tool:assign-cut", "tool:complete-cut"]),
    ("Manage", "Gestionar", ["clear", "about"]),
]

PARAMS = '{"map"?: bool (the service\'s own map image as a web tile layer under the layers, instead of the features)}'

VERSION = "0.3.0"
DESCRIPTION = "Public geospatial services of Chile as one-click layers: the Geoportal de Chile (IDE Chile), IDE MINAGRI, IDE MINVU and the INE's Censuses 2024 and 2017. Boundaries, soils, land use, wetlands, hydrography, urban planning, census data, roads, schools and more, as map images or as features you can query. CUT tools join any table of Chile to its communes, provinces or regions by their código único territorial, assign communes to points and complete codes and names."
DESCRIPTION_ES = "Servicios geoespaciales públicos de Chile en capas de un clic: Geoportal de Chile (IDE Chile), IDE MINAGRI, IDE MINVU y los Censos 2024 y 2017 del INE. Límites, suelos, uso de la tierra, humedales, hidrografía, planificación urbana, datos censales, red vial, escuelas y más, como imágenes de mapa o como entidades consultables. Las herramientas CUT unen cualquier tabla de Chile a sus comunas, provincias o regiones por su código único territorial, asignan comunas a puntos y completan códigos y nombres."

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


# ---------------------------------------------------------------------------------------------
# The CUT tools: joining tables to communes, provinces and regions by their código único
# territorial, assigning communes to points, and completing codes and names. Their table of
# communes is the Census 2024 one (communes.json, from refresh.py); the spellings people use that
# the INE doesn't, and the codes of before 2007 and 2018, are listed here and checked against it.

COMMUNES = json.loads((root / "communes.json").read_text(encoding="utf-8"))

# Words that stay lower case inside a name (never first).
SMALL = {"de", "del", "la", "las", "los", "y", "e"}


def titlecase(s):
    """'SAN PEDRO DE LA PAZ' → 'San Pedro de la Paz', "O'HIGGINS" → "O'Higgins", 'LLAY-LLAY' → 'Llay-Llay'."""
    words = []
    for i, w in enumerate(s.split(" ")):
        lw = w.lower()
        if i and lw in SMALL:
            words.append(lw)
            continue
        for sep in ("-", "'"):
            lw = sep.join(p[:1].upper() + p[1:] for p in lw.split(sep))
        words.append(lw)
    return " ".join(words)


# The regions: code → (short name, Roman numeral). The full names come from the INE.
REGIONS = {
    1: ("Tarapacá", "I"), 2: ("Antofagasta", "II"), 3: ("Atacama", "III"), 4: ("Coquimbo", "IV"), 5: ("Valparaíso", "V"),
    6: ("O'Higgins", "VI"), 7: ("Maule", "VII"), 8: ("Biobío", "VIII"), 9: ("La Araucanía", "IX"), 10: ("Los Lagos", "X"),
    11: ("Aysén", "XI"), 12: ("Magallanes", "XII"), 13: ("Metropolitana de Santiago", "XIII"), 14: ("Los Ríos", "XIV"),
    15: ("Arica y Parinacota", "XV"), 16: ("Ñuble", "XVI"),
}

# Other names of regions: what people write → the region's code. Accents, case, spaces, hyphens
# and apostrophes don't matter (the module normalises both sides), nor does a leading "Región de".
REGION_ALIASES = {
    "RM": 13, "Región Metropolitana": 13, "Metropolitana": 13, "Santiago": 13,
    "Araucanía": 9,
    "Aisén": 11, "Aisén del General Carlos Ibáñez del Campo": 11,
    "Magallanes y Antártica Chilena": 12, "Magallanes y la Antártica Chilena": 12,
    "Libertador Bernardo O'Higgins": 6, "Libertador Gral. Bernardo O'Higgins": 6,
    "Arica-Parinacota": 15, "Arica Parinacota": 15,
}

# Other names of communes: what people write → the INE's name. Spelling differences only; a
# difference of accents, case, spaces, hyphens or apostrophes needs no entry.
COMMUNE_ALIASES = {
    "Aisén": "Aysén", "Puerto Aysén": "Aysén", "Puerto Aisén": "Aysén",
    "Coihaique": "Coyhaique",
    "Paiguano": "Paihuano",
    "Marchigüe": "Marchihue",
    "Llay-Llay": "Llaillay", "Llay Llay": "Llaillay",
    "La Calera": "Calera",
    "Treguaco": "Trehuaco",
    "Puerto Natales": "Natales",
    "Puerto Saavedra": "Saavedra",
    "Puerto Cisnes": "Cisnes",
    "San Vicente de Tagua Tagua": "San Vicente",
    "Cabo de Hornos (Ex-Navarino)": "Cabo de Hornos", "Navarino": "Cabo de Hornos", "Puerto Williams": "Cabo de Hornos",
    # The commune of Antártica (12202) is drawn with Cabo de Hornos in the Census 2024 division.
    "Antártica": "Cabo de Hornos",
    "Santiago Centro": "Santiago",
    "Rapa Nui": "Isla de Pascua",
}

# Codes of before the regions of Arica y Parinacota and Los Ríos (2007) and Ñuble (2018), and
# Antártica's: the old CUT → the commune's name today, from which its current CUT is taken.
OLD_CUTS = {
    # Tarapacá's provinces of Arica (012) and Parinacota (013), now region 15.
    1201: "Arica", 1202: "Camarones", 1301: "Putre", 1302: "General Lagos",
    # Los Lagos's province of Valdivia (105), now region 14.
    10501: "Valdivia", 10502: "Corral", 10503: "Futrono", 10504: "La Unión", 10505: "Lago Ranco", 10506: "Lanco",
    10507: "Los Lagos", 10508: "Máfil", 10509: "Mariquina", 10510: "Paillaco", 10511: "Panguipulli", 10512: "Río Bueno",
    # Biobío's province of Ñuble (084), now region 16.
    8401: "Chillán", 8402: "Bulnes", 8403: "Cobquecura", 8404: "Coelemu", 8405: "Coihueco", 8406: "Chillán Viejo", 8407: "El Carmen",
    8408: "Ninhue", 8409: "Ñiquén", 8410: "Pemuco", 8411: "Pinto", 8412: "Portezuelo", 8413: "Quillón", 8414: "Quirihue",
    8415: "Ránquil", 8416: "San Carlos", 8417: "San Fabián", 8418: "San Ignacio", 8419: "San Nicolás", 8420: "Trehuaco", 8421: "Yungay",
    # Antártica, drawn with Cabo de Hornos in the Census 2024 division.
    12202: "Cabo de Hornos",
}

LEVELS = ["Auto", "Commune", "Province", "Region"]
LEVELS_ES = ["Automático", "Comuna", "Provincia", "Región"]
LEVEL_HELP = "Whether the codes and names are communes, provinces or regions. Auto tells from the layer."
LEVEL_HELP_ES = "Si los códigos y nombres son de comunas, provincias o regiones. Automático lo deduce de la capa."
KEY_HELP = "The field holding the CUT code (1101 or 01101) or the name. Blank: the tool finds it."
KEY_HELP_ES = "El campo con el código único territorial (1101 o 01101) o el nombre. En blanco: la herramienta lo encuentra."

# The tools, with their Spanish: (manifest entry, {english: spanish}).
TOOLS = [
    ({
        "id": "join-cut", "name": "Join Table by CUT", "toolbox": "IDE Chile",
        "summary": "Joins a CSV table to communes, provinces or regions by their código único territorial (CUT) or their name: codes with or without the leading zero, names however they are spelt, and the codes of before Ñuble (2018) and Arica y Parinacota and Los Ríos (2007). The table may be semicolon-separated and in Windows-1252, with decimal commas.",
        "params": [
            {"name": "layer", "label": "Layer of communes, provinces or regions", "kind": "vectorLayer", "required": True, "help": "A layer with a CUT or name field: Census 2024 › Communes, Provinces or Regions, or any other."},
            {"name": "table", "label": "Table (CSV)", "kind": "file", "required": True, "extensions": ["csv", "txt", "tsv"], "help": "A CSV file with a column of CUT codes or names, as the INE, SINIM, CASEN, Servel or DEIS publish them."},
            {"name": "level", "label": "Level", "kind": "choice", "choices": LEVELS, "default": "Auto", "help": LEVEL_HELP},
            {"name": "layer_key", "label": "Key field in the layer", "kind": "text", "default": "", "help": KEY_HELP},
            {"name": "table_key", "label": "Key column in the table", "kind": "text", "default": "", "help": "The column holding the CUT code or the name. Blank: the tool finds it."},
            {"name": "keep_unmatched", "label": "Keep features without a row", "kind": "bool", "default": True, "help": "Off: the result has only the features the table has a row for."},
        ],
    }, {
        "Join Table by CUT": "Unir tabla por CUT",
        "Joins a CSV table to communes, provinces or regions by their código único territorial (CUT) or their name: codes with or without the leading zero, names however they are spelt, and the codes of before Ñuble (2018) and Arica y Parinacota and Los Ríos (2007). The table may be semicolon-separated and in Windows-1252, with decimal commas.": "Une una tabla CSV a comunas, provincias o regiones por su código único territorial (CUT) o por su nombre: códigos con o sin el cero inicial, nombres como vengan escritos, y los códigos de antes de Ñuble (2018) y de Arica y Parinacota y Los Ríos (2007). La tabla puede venir separada por punto y coma, en Windows-1252 y con decimales con coma.",
        "Layer of communes, provinces or regions": "Capa de comunas, provincias o regiones",
        "A layer with a CUT or name field: Census 2024 › Communes, Provinces or Regions, or any other.": "Una capa con un campo de CUT o de nombre: Censo 2024 › Comunas, Provincias o Regiones, o cualquier otra.",
        "Table (CSV)": "Tabla (CSV)",
        "A CSV file with a column of CUT codes or names, as the INE, SINIM, CASEN, Servel or DEIS publish them.": "Un archivo CSV con una columna de códigos CUT o de nombres, como los publican el INE, SINIM, CASEN, Servel o DEIS.",
        "Key field in the layer": "Campo clave en la capa",
        "Key column in the table": "Columna clave en la tabla",
        "The column holding the CUT code or the name. Blank: the tool finds it.": "La columna con el código único territorial o el nombre. En blanco: la herramienta la encuentra.",
        "Keep features without a row": "Conservar entidades sin fila",
        "Off: the result has only the features the table has a row for.": "Desactivado: el resultado tiene solo las entidades con una fila en la tabla.",
    }),
    ({
        "id": "assign-cut", "name": "Assign Commune to Points", "toolbox": "IDE Chile",
        "summary": "Gives each point the commune, province and region it falls in, with their CUT codes and names, from a layer of communes (Census 2024 › Communes, or provinces or regions).",
        "params": [
            {"name": "points", "label": "Points", "kind": "pointLayer", "required": True, "help": "Addresses, stations, premises… in the same coordinate system as the communes."},
            {"name": "communes", "label": "Communes, provinces or regions", "kind": "polygonLayer", "required": True, "help": "Census 2024 › Communes gives all three levels; a layer of provinces or regions gives its own and above."},
            {"name": "level", "label": "Level", "kind": "choice", "choices": LEVELS, "default": "Auto", "help": LEVEL_HELP},
            {"name": "key", "label": "Key field in the polygons", "kind": "text", "default": "", "help": KEY_HELP},
        ],
    }, {
        "Assign Commune to Points": "Asignar comuna a puntos",
        "Gives each point the commune, province and region it falls in, with their CUT codes and names, from a layer of communes (Census 2024 › Communes, or provinces or regions).": "Da a cada punto la comuna, provincia y región en que cae, con sus códigos CUT y nombres, a partir de una capa de comunas (Censo 2024 › Comunas, o de provincias o regiones).",
        "Points": "Puntos",
        "Addresses, stations, premises… in the same coordinate system as the communes.": "Direcciones, estaciones, locales… en el mismo sistema de coordenadas que las comunas.",
        "Communes, provinces or regions": "Comunas, provincias o regiones",
        "Census 2024 › Communes gives all three levels; a layer of provinces or regions gives its own and above.": "Censo 2024 › Comunas da los tres niveles; una capa de provincias o regiones da el suyo y los superiores.",
        "Key field in the polygons": "Campo clave en los polígonos",
    }),
    ({
        "id": "complete-cut", "name": "Complete CUT", "toolbox": "IDE Chile",
        "summary": "Adds the current CUT code and the official names of the commune, province and region to a layer that has a code or name field, however it is written: 4 or 5 digits, old codes, other spellings.",
        "params": [
            {"name": "layer", "label": "Layer", "kind": "vectorLayer", "required": True, "help": "Any layer with a field of CUT codes or of commune, province or region names."},
            {"name": "key", "label": "Key field", "kind": "text", "default": "", "help": KEY_HELP},
            {"name": "level", "label": "Level", "kind": "choice", "choices": LEVELS, "default": "Auto", "help": LEVEL_HELP},
        ],
    }, {
        "Complete CUT": "Completar CUT",
        "Adds the current CUT code and the official names of the commune, province and region to a layer that has a code or name field, however it is written: 4 or 5 digits, old codes, other spellings.": "Añade el código único territorial vigente y los nombres oficiales de comuna, provincia y región a una capa que tenga un campo de código o de nombre, como venga escrito: 4 o 5 dígitos, códigos antiguos, otras grafías.",
        "Layer": "Capa",
        "Any layer with a field of CUT codes or of commune, province or region names.": "Cualquier capa con un campo de códigos CUT o de nombres de comuna, provincia o región.",
        "Key field": "Campo clave",
    }),
]

TOOL_STRINGS_ES = {
    "Level": "Nivel",
    LEVEL_HELP: LEVEL_HELP_ES,
    KEY_HELP: KEY_HELP_ES,
    "IDE Chile": "IDE Chile",
    **dict(zip(LEVELS, LEVELS_ES)),
}

# Messages the CUT tools build while they run (see src/cut.rs), with their Spanish.
TOOL_MESSAGES = {
    "The table is empty.": "La tabla está vacía.",
    "The table has no column of CUT codes or names: name it in 'Key column in the table'.": "La tabla no tiene una columna de códigos CUT ni de nombres: indíquela en 'Columna clave en la tabla'.",
    "The table has no column '{name}'.": "La tabla no tiene una columna '{name}'.",
    "The layer has no field of CUT codes or names: name it in the key field.": "La capa no tiene un campo de códigos CUT ni de nombres: indíquelo en el campo clave.",
    "The layer has no field '{name}'.": "La capa no tiene un campo '{name}'.",
    "{name} has no features.": "{name} no tiene entidades.",
    "Level: {level}. Key field in the layer: {field}. Key column in the table: {column}.": "Nivel: {level}. Campo clave en la capa: {field}. Columna clave en la tabla: {column}.",
    "Level: {level}. Key field: {field}.": "Nivel: {level}. Campo clave: {field}.",
    "{n} of {total} rows joined; {features} of {all} features got a row.": "{n} de {total} filas unidas; {features} de {all} entidades recibieron una fila.",
    "{n} rows without a feature (see the table 'Rows without a feature').": "{n} filas sin entidad (vea la tabla 'Filas sin entidad').",
    "{n} features without a row.": "{n} entidades sin fila.",
    "{n} features without a row were left out.": "{n} entidades sin fila se dejaron fuera.",
    "{n} of {total} points fall in a polygon; {outside} fall in none.": "{n} de {total} puntos caen en un polígono; {outside} no caen en ninguno.",
    "{n} of {total} features recognised; {unmatched} not (see the field CUT_NOTA).": "{n} de {total} entidades reconocidas; {unmatched} no (vea el campo CUT_NOTA).",
    "{n} old codes translated to today's.": "{n} códigos antiguos traducidos a los vigentes.",
    "{n} names matched by another spelling.": "{n} nombres reconocidos por otra grafía.",
    "The points and the polygons don't overlap: are they in the same coordinate system?": "Los puntos y los polígonos no se superponen: ¿están en el mismo sistema de coordenadas?",
    "Rows without a feature": "Filas sin entidad",
    "Row": "Fila",
    "Reason": "Motivo",
    "Joined": "Unida",
    "With commune": "Con comuna",
    "Completed": "Completada",
    "Commune": "Comuna",
    "Province": "Provincia",
    "Region": "Región",
    "not a known code": "no es un código conocido",
    "not a known name": "no es un nombre conocido",
    "empty": "vacío",
    "duplicate of row {n}": "duplicado de la fila {n}",
    "no feature with this code": "ninguna entidad con este código",
    "old code {old}": "código antiguo {old}",
    "spelt {name}": "escrito {name}",
    "Division: INE, Census 2024 ({n} communes); table checked on {date}.": "División: INE, Censo 2024 ({n} comunas); tabla verificada el {date}.",
}

# The day communes.json was last read from the INE (keep with refresh.py's runs).
COMMUNES_DATE = "2026-10-22"


def communes_rs():
    """src/communes.rs: the Census 2024 division and the aliases, checked against it."""
    rows = COMMUNES["communes"]
    by_name = {}
    for r in rows:
        r["name"] = titlecase(r["comuna"])
        assert r["name"] not in by_name, r["name"]
        by_name[r["name"]] = r
    for alias, name in COMMUNE_ALIASES.items():
        assert name in by_name, f"alias {alias!r} → {name!r}: no such commune"
        assert alias not in by_name, f"alias {alias!r} is a commune's own name"
    for old, name in OLD_CUTS.items():
        assert name in by_name, f"old CUT {old} → {name!r}: no such commune"
        assert old not in {r["cut"] for r in rows}, f"old CUT {old} is current"
    provinces = {}
    regions = {}
    for r in rows:
        provinces.setdefault(r["cod_provincia"], (titlecase(r["provincia"]), r["cod_region"]))
        regions.setdefault(r["cod_region"], titlecase(r["region"]))
        assert provinces[r["cod_provincia"]] == (titlecase(r["provincia"]), r["cod_region"]), r
        assert regions[r["cod_region"]] == titlecase(r["region"]), r
    assert set(regions) == set(REGIONS), set(regions) ^ set(REGIONS)
    for alias, code in REGION_ALIASES.items():
        assert code in regions, alias

    def s(x):
        return json.dumps(x, ensure_ascii=False)

    out = [
        "//! The Census 2024 division and the other names and codes the CUT tools recognise.",
        "//! Generated by `generate.py` from `communes.json` (refresh.py reads it from the INE); edit the",
        "//! aliases and old codes there, not this file.",
        "",
        "use crate::cut::{Commune, Province, Region};",
        "",
        f"/// The day the division was read from the INE's Census 2024 communes layer.",
        f"pub const DATE: &str = {s(COMMUNES_DATE)};",
        "",
        "/// The communes, by CUT.",
        "pub const COMMUNES: &[Commune] = &[",
    ]
    for r in rows:
        out.append(f"    Commune {{ cut: {r['cut']}, name: {s(r['name'])}, province: {r['cod_provincia']}, region: {r['cod_region']} }},")
    out += ["];", "", "/// The provinces, by code.", "pub const PROVINCES: &[Province] = &["]
    for code, (name, region) in sorted(provinces.items()):
        out.append(f"    Province {{ code: {code}, name: {s(name)}, region: {region} }},")
    out += ["];", "", "/// The regions, by code, with their short names and Roman numerals.", "pub const REGIONS: &[Region] = &["]
    for code, name in sorted(regions.items()):
        short, roman = REGIONS[code]
        out.append(f"    Region {{ code: {code}, name: {s(name)}, short: {s(short)}, roman: {s(roman)} }},")
    out += ["];", "", "/// Other spellings of communes' names → the CUT.", "pub const COMMUNE_ALIASES: &[(&str, u32)] = &["]
    for alias, name in COMMUNE_ALIASES.items():
        out.append(f"    ({s(alias)}, {by_name[name]['cut']}),")
    out += ["];", "", "/// Other names of regions → the code.", "pub const REGION_ALIASES: &[(&str, u32)] = &["]
    for alias, code in REGION_ALIASES.items():
        out.append(f"    ({s(alias)}, {code}),")
    out += ["];", "", "/// Codes of before 2007 and 2018 (and Antártica's) → the current CUT.", "pub const OLD_CUTS: &[(u32, u32)] = &["]
    for old, name in sorted(OLD_CUTS.items()):
        out.append(f"    ({old}, {by_name[name]['cut']}),")
    out += ["];", ""]
    (root / "src" / "communes.rs").write_text("\n".join(out), encoding="utf-8")
    return len(rows), len(provinces), len(regions)


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
        DESCRIPTION: DESCRIPTION_ES,
    })
    es.update(MESSAGES)
    tools = []
    for t, t_es in TOOLS:
        tools.append(t)
        es.update(t_es)
    es.update(TOOL_STRINGS_ES)
    es.update(TOOL_MESSAGES)
    tool_ids = {"tool:" + t["id"] for t in tools}
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
                assert it in ids or it in ("clear", "about") or it in tool_ids, it
                out.append(it)
        groups.append({"label": en, "items": out})
    placed = {i for g in groups for it in g["items"] for i in (it["items"] if isinstance(it, dict) else [it])}
    assert placed >= ids | tool_ids, (ids | tool_ids) - placed
    manifest = {
        "id": "ide-chile",
        "name": "IDE Chile",
        "version": VERSION,
        "description": DESCRIPTION,
        "author": "Daniel Díaz",
        "homepage": "https://github.com/opengeos/rustgis-plugins/tree/main/plugins/ide-chile",
        "license": "MIT",
        "entry": "plugin.wasm",
        "abi": 1,
        # 0.3.0 brought layer.addUrl to plugins and files to their tools.
        "minRustgisVersion": "0.3.0",
        "permissions": ["edit", "files", "network"],
        "commands": commands,
        "tools": tools,
        "tab": {"label": "IDE Chile", "groups": groups},
        "i18n": {"es": es},
    }
    (root / "plugin.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    n_communes, n_provinces, n_regions = communes_rs()

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
    print(f"{len(LAYERS)} layers, {len(commands)} commands, {len(tools)} tools, {len(es)} Spanish strings; {n_communes} communes, {n_provinces} provinces, {n_regions} regions")


if __name__ == "__main__":
    main()
