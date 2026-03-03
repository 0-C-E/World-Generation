const MAP_SIZE = {{ MAP_SIZE }};
const TILE_SIZE = {{ TILE_SIZE }};
const MAX_ZOOM = {{ MAX_ZOOM }};
const CITY_ZOOM_THRESHOLD = Math.max(2, Math.round(MAX_ZOOM * 0.875));
// Villages appear one zoom level before cities — they are larger features.
const VILLAGE_ZOOM_THRESHOLD = Math.max(1, CITY_ZOOM_THRESHOLD - 1);
const MAX_ISLAND_DISPLAY = 100;
const factor = TILE_SIZE / MAP_SIZE;
const WORLD_FP = document.body.getAttribute('data-world-fingerprint') || '0';
const SESSION = Math.floor(Date.now() / 900000);

// Resource display helpers
const RES_ICON = { Wood: '🌲', Stone: '⛰️', Food: '🌾', Metal: '⚙️', Favor: '✨' };
const RES_COLOR = {
    Wood: '#27ae60',
    Stone: '#7f8c8d',
    Food: '#e67e22',
    Metal: '#2980b9',
    Favor: '#8e44ad',
};

// Custom CRS: positive-Y-down
let CustomCRS = L.extend({}, L.CRS.Simple, {
    transformation: new L.Transformation(factor, 0, factor, 0)
});

let map = L.map('map', {
    crs: CustomCRS,
    minZoom: 0,
    maxZoom: MAX_ZOOM,
    zoomSnap: 1,
    zoomDelta: 1
});

let bounds = L.latLngBounds(L.latLng(0, 0), L.latLng(MAP_SIZE, MAP_SIZE));

L.tileLayer('/tile/{z}/{x}/{y}.png?v=' + WORLD_FP + '.' + SESSION, {
    minZoom: 0,
    maxZoom: MAX_ZOOM,
    tileSize: TILE_SIZE,
    noWrap: true,
    bounds: bounds
}).addTo(map);

let center = L.latLng(MAP_SIZE / 2, MAP_SIZE / 2);
map.setView(center, Math.max(1, Math.round(MAX_ZOOM * 0.375)));
map.setMaxBounds(bounds.pad(0.1));

// Icons
let cityIcon = L.icon({
    iconUrl: '/city-icon.svg',
    iconSize: [20, 20],
    iconAnchor: [10, 10],
    popupAnchor: [0, -12]
});

let villageIcon = L.icon({
    iconUrl: '/village-icon.svg',
    iconSize: [18, 18],
    iconAnchor: [9, 9],
    popupAnchor: [0, -11]
});

// Layer groups
let islandLayer = L.layerGroup();
let cityLayer = L.layerGroup();
let villageLayer = L.layerGroup();
let highlightLayer = L.layerGroup().addTo(map);
let allIslands = null;
let worldReady = false;

// ---------------------------------------------------------------------------
// Viewport fetching — shared debounce + abort pattern
// ---------------------------------------------------------------------------

const FETCH_PADDING_FRAC = 0.25;

let activeCityFetch = null;
let activeVillageFetch = null;
let cityDebounceTimer = null;
let villageDebounceTimer = null;

function scheduleCityFetch() {
    clearTimeout(cityDebounceTimer);
    cityDebounceTimer = setTimeout(fetchCitiesForViewport, 150);
}

function scheduleVillageFetch() {
    clearTimeout(villageDebounceTimer);
    villageDebounceTimer = setTimeout(fetchVillagesForViewport, 150);
}

function viewportBbox() {
    let vb = map.getBounds();
    let padX = (vb.getEast() - vb.getWest()) * FETCH_PADDING_FRAC;
    let padY = (vb.getNorth() - vb.getSouth()) * FETCH_PADDING_FRAC;
    return {
        x0: Math.max(0, Math.floor(vb.getWest() - padX)),
        y0: Math.max(0, Math.floor(vb.getSouth() - padY)),
        x1: Math.min(MAP_SIZE - 1, Math.ceil(vb.getEast() + padX)),
        y1: Math.min(MAP_SIZE - 1, Math.ceil(vb.getNorth() + padY)),
    };
}

// -- Cities -----------------------------------------------------------------

function fetchCitiesForViewport() {
    if (activeCityFetch) { activeCityFetch.abort(); activeCityFetch = null; }
    let { x0, y0, x1, y1 } = viewportBbox();
    let ctrl = new AbortController();
    activeCityFetch = ctrl;
    setLoading('Loading cities...');
    fetch('/cities?x0=' + x0 + '&y0=' + y0 + '&x1=' + x1 + '&y1=' + y1,
        { signal: ctrl.signal })
        .then(r => r.json())
        .then(cities => {
            activeCityFetch = null;
            clearLoading();
            renderCities(cities, map.getBounds());
        })
        .catch(err => {
            if (err.name !== 'AbortError') { console.error('City fetch:', err); clearLoading(); }
        });
}

function renderCities(cities, vb) {
    cityLayer.clearLayers();
    for (let i = 0; i < cities.length; i++) {
        let cx = cities[i][0], cy = cities[i][1], rid = cities[i][2], res = cities[i][3];
        if (!vb.contains(L.latLng(cy, cx))) continue;
        let marker = L.marker(L.latLng(cy, cx), { icon: cityIcon });
        marker.bindPopup(buildCityPopup(cx, cy, rid, res),
            { className: 'city-popup', minWidth: 190 });
        cityLayer.addLayer(marker);
    }
    if (!map.hasLayer(cityLayer)) map.addLayer(cityLayer);
}

// -- Villages ---------------------------------------------------------------

function fetchVillagesForViewport() {
    if (activeVillageFetch) { activeVillageFetch.abort(); activeVillageFetch = null; }
    let { x0, y0, x1, y1 } = viewportBbox();
    let ctrl = new AbortController();
    activeVillageFetch = ctrl;
    fetch('/villages?x0=' + x0 + '&y0=' + y0 + '&x1=' + x1 + '&y1=' + y1,
        { signal: ctrl.signal })
        .then(r => r.json())
        .then(villages => {
            activeVillageFetch = null;
            renderVillages(villages, map.getBounds());
        })
        .catch(err => {
            if (err.name !== 'AbortError') console.error('Village fetch:', err);
        });
}

function renderVillages(villages, vb) {
    villageLayer.clearLayers();
    for (let i = 0; i < villages.length; i++) {
        //  [x, y, region_id, base_rate, offers, demands, biome]
        let vx = villages[i][0];
        let vy = villages[i][1];
        let rid = villages[i][2];
        let rate = villages[i][3];
        let offers = villages[i][4];
        let demands = villages[i][5];
        let biome = villages[i][6];
        if (!vb.contains(L.latLng(vy, vx))) continue;
        let marker = L.marker(L.latLng(vy, vx), { icon: villageIcon });
        marker.bindPopup(
            buildVillagePopup(vx, vy, rid, rate, offers, demands, biome),
            { className: 'village-popup', minWidth: 190 }
        );
        villageLayer.addLayer(marker);
    }
    if (!map.hasLayer(villageLayer)) map.addLayer(villageLayer);
}

// ---------------------------------------------------------------------------
// Popup builders
// ---------------------------------------------------------------------------

function fmtMod(v) {
    if (v > 0) return '<span class="res-pos">+' + v + '%</span>';
    if (v < 0) return '<span class="res-neg">' + v + '%</span>';
    return '<span class="res-zero">0%</span>';
}

function buildCityPopup(cx, cy, rid, res) {
    let rows = [
        ['\u{1F332} Wood', res.wood],
        ['\u{26F0}\uFE0F Stone', res.stone],
        ['\u{1F33E} Food', res.food],
        ['\u{2699}\uFE0F Metal', res.metal],
        ['\u{2728} Favor', res.favor],
    ];
    let html = '<div class="city-popup-inner">';
    html += '<div class="city-popup-title">City (' + cx + ', ' + cy + ')</div>';
    html += '<div class="city-popup-sub">Island #' + rid + ' &middot; ' + res.biome + '</div>';
    html += '<table class="city-res">';
    for (let i = 0; i < rows.length; i++) {
        html += '<tr><td>' + rows[i][0] + '</td><td>' + fmtMod(rows[i][1]) + '</td></tr>';
    }
    if (res.gold_nodes > 0) {
        html += '<tr><td>\u{1FA99} Gold nodes</td><td><span class="res-gold">'
            + res.gold_nodes + '</span></td></tr>';
    }
    html += '</table></div>';
    return html;
}

function buildVillagePopup(vx, vy, rid, rate, offers, demands, biome) {
    let offerIcon = RES_ICON[offers] || '?';
    let demandIcon = RES_ICON[demands] || '?';
    let offerColor = RES_COLOR[offers] || '#333';
    let demandColor = RES_COLOR[demands] || '#333';

    let html = '<div class="village-popup-inner">';
    // Title row with village name derived from what it offers
    html += '<div class="village-popup-title">'
        + offerIcon + ' ' + offers + ' Village</div>';
    html += '<div class="village-popup-sub">'
        + 'Island #' + rid + ' &middot; ' + biome + '</div>';

    // Trade banner — the main message
    html += '<div class="village-trade">';
    html += '<div class="village-trade-row">'
        + '<span class="trade-label">Offers</span>'
        + '<span class="trade-res" style="color:' + offerColor + '">'
        + offerIcon + ' ' + offers
        + '</span></div>';
    html += '<div class="village-trade-arrow">&#8597;</div>';
    html += '<div class="village-trade-row">'
        + '<span class="trade-label">Demands</span>'
        + '<span class="trade-res" style="color:' + demandColor + '">'
        + demandIcon + ' ' + demands
        + '</span></div>';
    html += '</div>';

    // Base rate
    html += '<div class="village-rate">'
        + 'Base rate: <strong>' + rate + ' u/h</strong>'
        + '</div>';

    html += '</div>';
    return html;
}

// ---------------------------------------------------------------------------
// Island layer (unchanged logic, preserved in full)
// ---------------------------------------------------------------------------

function lerpColor(a, b, t) {
    return [
        Math.round(a[0] + (b[0] - a[0]) * t),
        Math.round(a[1] + (b[1] - a[1]) * t),
        Math.round(a[2] + (b[2] - a[2]) * t),
    ];
}

function makeIslandIcon(count, isSpawn, t) {
    if (isSpawn) {
        return L.divIcon({
            className: '',
            html: '<div class="island-icon island-icon-spawn">★ Spawn</div>',
            iconSize: [64, 36], iconAnchor: [32, 18]
        });
    }
    let bg;
    if (t === null) {
        bg = 'rgb(22, 160, 180)';
    } else {
        let small = [249, 141, 0], large = [189, 42, 1];
        let eased = t * t * (3 - 2 * t);
        let rgb = lerpColor(small, large, eased);
        bg = 'rgb(' + rgb[0] + ',' + rgb[1] + ',' + rgb[2] + ')';
    }
    let size = 24 + Math.round(20 * t);
    return L.divIcon({
        className: '',
        html: '<div class="island-icon" style="background:' + bg
            + ';width:' + size + 'px;height:' + size + 'px;line-height:' + size + 'px;">'
            + count + '</div>',
        iconSize: [size, size], iconAnchor: [size / 2, size / 2],
    });
}

function updateIslandView() {
    islandLayer.clearLayers();
    if (!allIslands) return;
    let vb = map.getBounds();
    let visible = allIslands.filter(isl => vb.contains(L.latLng(isl[2], isl[1])));
    visible.sort((a, b) => b[3] - a[3]);
    let spawn = visible.find(isl => isl[8] === 1) || null;
    let nonSpawn = visible.filter(isl => isl[8] !== 1).slice(0, MAX_ISLAND_DISPLAY);
    let minCount = nonSpawn.length > 0 ? nonSpawn[nonSpawn.length - 1][3] : 0;
    let maxCount = nonSpawn.length > 0 ? nonSpawn[0][3] : 1;
    let single = nonSpawn.length === 1;
    let toRender = spawn ? [spawn].concat(nonSpawn) : nonSpawn;

    for (let i = 0; i < toRender.length; i++) {
        let island = toRender[i];
        let isSpawn = island[8] === 1;
        let count = island[3];
        let t = isSpawn ? null : (single ? null
            : maxCount > minCount ? (count - minCount) / (maxCount - minCount) : 0.5);
        let latlng = L.latLng(island[2], island[1]);
        let marker = L.marker(latlng, { icon: makeIslandIcon(count, isSpawn, t) });
        let label = isSpawn
            ? '★ World Spawn &mdash; ' + count + ' cities'
            : 'Island #' + island[0] + ' &mdash; ' + count + ' cities'
            + ' &middot; spawn order #' + island[9];
        marker.bindPopup(label);
        marker.on('click', () => drawIslandHighlight(island));
        islandLayer.addLayer(marker);
    }
    if (!map.hasLayer(islandLayer)) map.addLayer(islandLayer);
}

// ---------------------------------------------------------------------------
// Island highlight
// ---------------------------------------------------------------------------

let highlightStyle = { color: '#e74c3c', weight: 2, fillOpacity: 0.15, dashArray: '6' };

function cross2d(O, A, B) {
    return (A[0] - O[0]) * (B[1] - O[1]) - (A[1] - O[1]) * (B[0] - O[0]);
}

function convexHull(points) {
    let n = points.length;
    if (n < 3) return points.slice();
    let startIdx = 0;
    for (let i = 1; i < n; i++) {
        if (points[i][0] < points[startIdx][0] ||
            (points[i][0] === points[startIdx][0] && points[i][1] < points[startIdx][1]))
            startIdx = i;
    }
    let hull = [], current = startIdx;
    do {
        hull.push(points[current]);
        let next = (current + 1) % n;
        for (let j = 0; j < n; j++) {
            if (cross2d(points[current], points[next], points[j]) < 0) next = j;
        }
        current = next;
    } while (current !== startIdx && hull.length <= n);
    return hull;
}

function drawBboxHighlight(island) {
    highlightLayer.clearLayers();
    highlightLayer.addLayer(
        L.rectangle([[island[5], island[4]], [island[7], island[6]]], highlightStyle)
    );
}

function drawIslandHighlight(island) {
    drawBboxHighlight(island);
    let rid = island[0];
    fetch('/cities?x0=' + island[4] + '&y0=' + island[5]
        + '&x1=' + island[6] + '&y1=' + island[7])
        .then(r => r.json())
        .then(cities => {
            let pts = cities.filter(c => c[2] === rid).map(c => [c[0], c[1]]);
            if (pts.length < 3) return;
            let hull = convexHull(pts);
            highlightLayer.clearLayers();
            highlightLayer.addLayer(
                L.polygon(hull.map(p => L.latLng(p[1], p[0])), highlightStyle)
            );
        })
        .catch(() => { });
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

function waitForReady() {
    setLoading('Loading world data...');
    fetch('/status')
        .then(r => r.json())
        .then(data => {
            if (data.ready) { worldReady = true; loadIslands(); }
            else setTimeout(waitForReady, 500);
        })
        .catch(() => setTimeout(waitForReady, 1000));
}

function loadIslands() {
    setLoading('Loading islands...');
    fetch('/islands.json')
        .then(r => r.json())
        .then(islands => {
            allIslands = islands;
            clearLoading();
            let z = map.getZoom();
            if (z < VILLAGE_ZOOM_THRESHOLD) {
                updateIslandView();
            } else if (z < CITY_ZOOM_THRESHOLD) {
                updateIslandView();
                scheduleVillageFetch();
            } else {
                scheduleCityFetch();
                scheduleVillageFetch();
            }
        })
        .catch(() => setLoading('Failed to load islands'));
}

waitForReady();

// ---------------------------------------------------------------------------
// Zoom / pan handlers
// ---------------------------------------------------------------------------

map.on('zoomend', function () {
    highlightLayer.clearLayers();
    let z = map.getZoom();

    // -- Island layer
    if (z < VILLAGE_ZOOM_THRESHOLD) {
        updateIslandView();
    } else {
        if (map.hasLayer(islandLayer)) map.removeLayer(islandLayer);
    }

    // -- Village layer
    if (z >= VILLAGE_ZOOM_THRESHOLD) {
        scheduleVillageFetch();
    } else {
        if (activeVillageFetch) { activeVillageFetch.abort(); activeVillageFetch = null; }
        clearTimeout(villageDebounceTimer);
        if (map.hasLayer(villageLayer)) map.removeLayer(villageLayer);
        villageLayer.clearLayers();
    }

    // -- City layer
    if (z >= CITY_ZOOM_THRESHOLD) {
        scheduleCityFetch();
    } else {
        if (activeCityFetch) { activeCityFetch.abort(); activeCityFetch = null; clearLoading(); }
        clearTimeout(cityDebounceTimer);
        if (map.hasLayer(cityLayer)) map.removeLayer(cityLayer);
        cityLayer.clearLayers();
    }
});

map.on('moveend', function () {
    let z = map.getZoom();
    if (z < VILLAGE_ZOOM_THRESHOLD) {
        if (allIslands) updateIslandView();
    } else {
        scheduleVillageFetch();
        if (z >= CITY_ZOOM_THRESHOLD) scheduleCityFetch();
    }
});

// ---------------------------------------------------------------------------
// Loading indicator
// ---------------------------------------------------------------------------

function setLoading(msg) {
    let el = document.getElementById('loading');
    el.textContent = msg;
    el.style.display = '';
}
function clearLoading() {
    document.getElementById('loading').style.display = 'none';
}
