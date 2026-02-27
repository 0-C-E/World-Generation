var MAP_SIZE = {{ MAP_SIZE }};
var TILE_SIZE = {{ TILE_SIZE }};
var MAX_ZOOM = {{ MAX_ZOOM }};
// Zoom thresholds scale with MAX_ZOOM so they work for any map size.
var ISLAND_ZOOM_MIN = Math.max(1, Math.round(MAX_ZOOM * 0.375));
var CITY_ZOOM_THRESHOLD = Math.max(2, Math.round(MAX_ZOOM * 0.875));
var MAX_ENTITIES = 500;
var factor = TILE_SIZE / MAP_SIZE;
var WORLD_FP = document.body.getAttribute('data-world-fingerprint') || '0';
// Round to 15-minute windows so tiles stay cached within that period.
var SESSION = Math.floor(Date.now() / 900000);

// Custom CRS: positive-Y-down matching image coordinates
var CustomCRS = L.extend({}, L.CRS.Simple, {
    transformation: new L.Transformation(factor, 0, factor, 0)
});

var map = L.map('map', {
    crs: CustomCRS,
    minZoom: 0,
    maxZoom: MAX_ZOOM,
    zoomSnap: 1,
    zoomDelta: 1
});

var bounds = L.latLngBounds(L.latLng(0, 0), L.latLng(MAP_SIZE, MAP_SIZE));

L.tileLayer('/tile/{z}/{x}/{y}.png?v=' + WORLD_FP + '.' + SESSION, {
    minZoom: 0,
    maxZoom: MAX_ZOOM,
    tileSize: TILE_SIZE,
    noWrap: true,
    bounds: bounds
}).addTo(map);

// Default view at zoom 3, centered on map
var center = L.latLng(MAP_SIZE / 2, MAP_SIZE / 2);
map.setView(center, ISLAND_ZOOM_MIN);
map.setMaxBounds(bounds.pad(0.1));

// City marker icon
var cityIcon = L.icon({
    iconUrl: '/city-icon.svg',
    iconSize: [20, 20],
    iconAnchor: [10, 10],
    popupAnchor: [0, -12]
});

// -- Three-tier layer system --
var islandLayer = L.layerGroup();
var cityLayer = L.layerGroup();
var highlightLayer = L.layerGroup().addTo(map);
var allIslands = null;  // loaded eagerly at startup
var allCities = null;   // lazy-loaded on first zoom to 7
var outlineCache = {};  // rid -> [[x,y], ...] polygon points
var worldReady = false; // tracks whether the server has loaded the world

function makeIslandIcon(count) {
    var large = count >= 100;
    var size = large ? 44 : 36;
    return L.divIcon({
        className: '',
        html: '<div class="island-icon' + (large ? ' island-icon-large' : '') + '">' + count + '</div>',
        iconSize: [size, size],
        iconAnchor: [size / 2, size / 2]
    });
}

// Poll server until world data is ready, then load islands
function waitForReady() {
    document.getElementById('loading').textContent = 'Loading world data...';
    fetch('/status')
        .then(function (resp) { return resp.json(); })
        .then(function (data) {
            if (data.ready) {
                worldReady = true;
                loadIslands();
            } else {
                setTimeout(waitForReady, 500);
            }
        })
        .catch(function () {
            setTimeout(waitForReady, 1000);
        });
}

// Load island summaries (called once world is ready)
function loadIslands() {
    document.getElementById('loading').textContent = 'Loading islands...';
    fetch('/islands.json')
        .then(function (resp) { return resp.json(); })
        .then(function (islands) {
            allIslands = islands;
            document.getElementById('loading').style.display = 'none';
            if (map.getZoom() >= ISLAND_ZOOM_MIN && map.getZoom() < CITY_ZOOM_THRESHOLD) {
                updateIslandView();
            }
        })
        .catch(function () {
            document.getElementById('loading').textContent = 'Failed to load islands';
        });
}

waitForReady();

// Render up to MAX_ENTITIES islands visible in the current viewport
function updateIslandView() {
    islandLayer.clearLayers();
    if (!allIslands) return;

    var vb = map.getBounds();
    var z = map.getZoom();
    var count = 0;
    // At low zoom levels, hide small islands to reduce clutter.
    // The city-count threshold scales with map area (100 is the baseline for
    // a 10,000 x 10,000 world) and fades out as you zoom closer to CITY_ZOOM_THRESHOLD.
    var areaRatio = (MAP_SIZE * MAP_SIZE) / (10000 * 10000);
    var baseCityFilter = Math.round(100 * areaRatio);
    // How far through the island zoom band are we? 0 = just entered, 1 = about to switch to cities.
    var zoomProgress = (z - ISLAND_ZOOM_MIN) / Math.max(1, CITY_ZOOM_THRESHOLD - ISLAND_ZOOM_MIN - 1);
    // At the start of the band, filter aggressively; near the city threshold, show everything.
    var cityFilter = Math.round(baseCityFilter * (1 - Math.min(1, zoomProgress)));

    for (var i = 0; i < allIslands.length && count < MAX_ENTITIES; i++) {
        var rid = allIslands[i][0];
        var cx = allIslands[i][1];
        var cy = allIslands[i][2];
        var cityCount = allIslands[i][3];
        if (cityCount <= cityFilter) continue;
        var latlng = L.latLng(cy, cx);
        if (vb.contains(latlng)) {
            var marker = L.marker(latlng, { icon: makeIslandIcon(cityCount) });
            marker.bindPopup('Island #' + rid + ' - ' + cityCount + ' cities');
            // Bounding box highlight on click
            (function (island) {
                marker.on('click', function () {
                    highlightLayer.clearLayers();
                    var minX = island[4], minY = island[5];
                    var maxX = island[6], maxY = island[7];
                    var rect = L.rectangle(
                        [[minY, minX], [maxY, maxX]],
                        { color: '#e74c3c', weight: 2, fillOpacity: 0.15, dashArray: '6' }
                    );
                    highlightLayer.addLayer(rect);
                });
            })(allIslands[i]);
            islandLayer.addLayer(marker);
            count++;
        }
    }
    if (!map.hasLayer(islandLayer)) {
        map.addLayer(islandLayer);
    }
}

// Lazy-load individual cities on first zoom to max level
function loadCities() {
    if (allCities) return;
    allCities = []; // mark as loading to prevent double-fetch
    document.getElementById('loading').style.display = '';
    document.getElementById('loading').textContent = 'Loading cities...';

    fetch('/cities.json')
        .then(function (resp) { return resp.json(); })
        .then(function (cities) {
            allCities = cities;
            document.getElementById('loading').style.display = 'none';
            if (map.getZoom() >= CITY_ZOOM_THRESHOLD) {
                updateCityView();
            }
        })
        .catch(function () {
            document.getElementById('loading').textContent = 'Failed to load cities';
            allCities = null;
        });
}

// Show all cities from islands that have at least one city in the viewport
function updateCityView() {
    cityLayer.clearLayers();
    if (!allCities || allCities.length === 0) return;

    // Pass 1: find which islands have at least one city in the viewport
    var vb = map.getBounds();
    var visibleIslands = {};
    for (var i = 0; i < allCities.length; i++) {
        var latlng = L.latLng(allCities[i][1], allCities[i][0]);
        if (vb.contains(latlng)) {
            visibleIslands[allCities[i][2]] = true;
        }
    }

    // Pass 2: add all cities belonging to visible islands
    for (var i = 0; i < allCities.length; i++) {
        if (visibleIslands[allCities[i][2]]) {
            var cx = allCities[i][0];
            var cy = allCities[i][1];
            var rid = allCities[i][2];
            var marker = L.marker(L.latLng(cy, cx), { icon: cityIcon });
            marker.bindPopup('City at (' + cx + ', ' + cy + ') - Island #' + rid);
            cityLayer.addLayer(marker);
        }
    }
    if (!map.hasLayer(cityLayer)) {
        map.addLayer(cityLayer);
    }
}

// Toggle layers based on zoom
map.on('zoomend', function () {
    highlightLayer.clearLayers();
    var z = map.getZoom();
    if (z >= CITY_ZOOM_THRESHOLD) {
        // Zoom 7: individual cities
        if (map.hasLayer(islandLayer)) map.removeLayer(islandLayer);
        if (!allCities || allCities.length === 0) loadCities();
        else updateCityView();
    } else if (z >= ISLAND_ZOOM_MIN) {
        // Zoom 5-6: island summaries
        if (map.hasLayer(cityLayer)) map.removeLayer(cityLayer);
        updateIslandView();
    } else {
        // Zoom 0-4: clean map
        if (map.hasLayer(cityLayer)) map.removeLayer(cityLayer);
        if (map.hasLayer(islandLayer)) map.removeLayer(islandLayer);
    }
});

// Update visible markers on pan
map.on('moveend', function () {
    var z = map.getZoom();
    if (z >= CITY_ZOOM_THRESHOLD && allCities && allCities.length > 0) {
        updateCityView();
    } else if (z >= ISLAND_ZOOM_MIN && z < CITY_ZOOM_THRESHOLD && allIslands) {
        updateIslandView();
    }
});
