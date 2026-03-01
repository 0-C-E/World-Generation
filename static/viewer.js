const MAP_SIZE = {{ MAP_SIZE }};
const TILE_SIZE = {{ TILE_SIZE }};
const MAX_ZOOM = {{ MAX_ZOOM }};
// Zoom thresholds scale with MAX_ZOOM so they work for any map size.
const CITY_ZOOM_THRESHOLD = Math.max(2, Math.round(MAX_ZOOM * 0.875));
const MAX_ISLAND_DISPLAY = 100;
const factor = TILE_SIZE / MAP_SIZE;
const WORLD_FP = document.body.getAttribute('data-world-fingerprint') || '0';
// Round to 15-minute windows so tiles stay cached within that period.
const SESSION = Math.floor(Date.now() / 900000);

// Custom CRS: positive-Y-down matching image coordinates
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

// Default view at zoom 3, centered on map
let center = L.latLng(MAP_SIZE / 2, MAP_SIZE / 2);
map.setView(center, Math.max(1, Math.round(MAX_ZOOM * 0.375)));
map.setMaxBounds(bounds.pad(0.1));

// City marker icon
let cityIcon = L.icon({
    iconUrl: '/city-icon.svg',
    iconSize: [20, 20],
    iconAnchor: [10, 10],
    popupAnchor: [0, -12]
});

// -- Three-tier layer system --
let islandLayer = L.layerGroup();
let cityLayer = L.layerGroup();
let highlightLayer = L.layerGroup().addTo(map);
let allIslands = null;
let worldReady = false;

// ---------------------------------------------------------------------------
// Viewport city fetching
//
// Instead of loading all cities at once, we request only the cities within
// the current viewport (plus a small padding buffer) on every pan/zoom.
// An AbortController cancels any in-flight request when the viewport changes
// before the response arrives, so stale data never overwrites fresh data.
// ---------------------------------------------------------------------------

let activeCityFetch = null;   // current AbortController, or null
let cityDebounceTimer = null; // debounce handle for pan/zoom events

// How much to expand the fetch bbox beyond the visible viewport (in world
// tiles). Pre-fetching just outside the edges means nearby cities appear
// instantly when panning a short distance.
const FETCH_PADDING_FRAC = 0.25;

function scheduleCityFetch() {
    clearTimeout(cityDebounceTimer);
    cityDebounceTimer = setTimeout(fetchCitiesForViewport, 150);
}

function fetchCitiesForViewport() {
    // Abort any previous in-flight request.
    if (activeCityFetch) {
        activeCityFetch.abort();
        activeCityFetch = null;
    }

    let vb = map.getBounds();
    let padX = (vb.getEast() - vb.getWest()) * FETCH_PADDING_FRAC;
    let padY = (vb.getNorth() - vb.getSouth()) * FETCH_PADDING_FRAC;

    let x0 = Math.max(0, Math.floor(vb.getWest() - padX));
    let y0 = Math.max(0, Math.floor(vb.getSouth() - padY));
    let x1 = Math.min(MAP_SIZE - 1, Math.ceil(vb.getEast() + padX));
    let y1 = Math.min(MAP_SIZE - 1, Math.ceil(vb.getNorth() + padY));

    let ctrl = new AbortController();
    activeCityFetch = ctrl;

    setLoading('Loading cities...');

    fetch('/cities?x0=' + x0 + '&y0=' + y0 + '&x1=' + x1 + '&y1=' + y1,
        { signal: ctrl.signal })
        .then(function (resp) { return resp.json(); })
        .then(function (cities) {
            activeCityFetch = null;
            clearLoading();
            renderCities(cities, map.getBounds());
        })
        .catch(function (err) {
            if (err.name !== 'AbortError') {
                console.error('City fetch failed:', err);
                clearLoading();
            }
            // AbortError is expected and silent -- a newer fetch is already running.
        });
}

function renderCities(cities, vb) {
    cityLayer.clearLayers();
    for (let i = 0; i < cities.length; i++) {
        let cx = cities[i][0];
        let cy = cities[i][1];
        let rid = cities[i][2];
        let res = cities[i][3];
        // Only place markers that are actually inside the (unpadded) viewport.
        if (!vb.contains(L.latLng(cy, cx))) continue;
        let marker = L.marker(L.latLng(cy, cx), { icon: cityIcon });
        marker.bindPopup(buildCityPopup(cx, cy, rid, res),
            { className: 'city-popup', minWidth: 180 });
        cityLayer.addLayer(marker);
    }
    if (!map.hasLayer(cityLayer)) map.addLayer(cityLayer);
}

// ---------------------------------------------------------------------------
// Convex hull (Jarvis march)
// ---------------------------------------------------------------------------

function cross2d(O, A, B) {
    return (A[0] - O[0]) * (B[1] - O[1]) - (A[1] - O[1]) * (B[0] - O[0]);
}

function convexHull(points) {
    let n = points.length;
    if (n < 3) return points.slice();
    let startIdx = 0;
    for (let i = 1; i < n; i++) {
        if (points[i][0] < points[startIdx][0] ||
            (points[i][0] === points[startIdx][0] && points[i][1] < points[startIdx][1])) {
            startIdx = i;
        }
    }
    let hull = [];
    let current = startIdx;
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

// ---------------------------------------------------------------------------
// Island highlight
//
// On click:
//  1. Immediately draw the bounding-box rectangle (instant, no fetch).
//  2. Fetch cities within the island's bbox from the server.
//  3. Upgrade to a convex-hull polygon once the response arrives.
// ---------------------------------------------------------------------------

let highlightStyle = { color: '#e74c3c', weight: 2, fillOpacity: 0.15, dashArray: '6' };

function drawBboxHighlight(island) {
    highlightLayer.clearLayers();
    let minX = island[4], minY = island[5];
    let maxX = island[6], maxY = island[7];
    highlightLayer.addLayer(
        L.rectangle([[minY, minX], [maxY, maxX]], highlightStyle)
    );
}

function drawIslandHighlight(island) {
    // Step 1: instant bbox fallback.
    drawBboxHighlight(island);

    let rid = island[0];
    let minX = island[4], minY = island[5];
    let maxX = island[6], maxY = island[7];

    // Step 2: fetch island's cities and upgrade to hull.
    fetch('/cities?x0=' + minX + '&y0=' + minY + '&x1=' + maxX + '&y1=' + maxY)
        .then(function (resp) { return resp.json(); })
        .then(function (cities) {
            // Keep only cities that belong to this island.
            let pts = [];
            for (let i = 0; i < cities.length; i++) {
                if (cities[i][2] === rid) pts.push([cities[i][0], cities[i][1]]);
            }
            if (pts.length < 3) return; // bbox is already fine for tiny islands

            let hull = convexHull(pts);
            highlightLayer.clearLayers();
            highlightLayer.addLayer(
                L.polygon(hull.map(function (p) { return L.latLng(p[1], p[0]); }),
                    highlightStyle)
            );
        })
        .catch(function () { /* leave bbox in place on error */ });
}

// ---------------------------------------------------------------------------
// Island layer
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
            iconSize: [64, 36],
            iconAnchor: [32, 18]
        });
    }
    let bg;
    if (t === null) {
        // Only one island on the map -- distinct flat teal.
        bg = 'rgb(22, 160, 180)';
    } else {
        // Gradient: cool blue (small) → warm amber (large).
        // Small islands: #f98d00 (249, 141, 0)
        // Large islands: #bd2a01 (189, 42, 1)
        let small = [249, 141, 0];
        let large = [189, 42, 1];
        // Apply a slight ease-in so mid-sized islands don't all look the same.
        let eased = t * t * (3 - 2 * t); // smoothstep
        let rgb = lerpColor(small, large, eased);
        bg = 'rgb(' + rgb[0] + ',' + rgb[1] + ',' + rgb[2] + ')';
    }

    let size = 24 + Math.round(20 * t);
    return L.divIcon({
        className: '',
        html: '<div class="island-icon" style="background:' + bg + ';width:' + size + 'px;height:' + size + 'px;line-height:' + size + 'px;">' + count + '</div>',
        iconSize: [size, size],
        iconAnchor: [size / 2, size / 2],
    });
}

function updateIslandView() {
    islandLayer.clearLayers();
    if (!allIslands) return;

    let vb = map.getBounds();

    // Collect all islands visible in the current viewport.
    let visible = [];
    for (let i = 0; i < allIslands.length; i++) {
        let isl = allIslands[i];
        if (vb.contains(L.latLng(isl[2], isl[1]))) visible.push(isl);
    }

    // Keep only the top MAX_ISLAND_DISPLAY largest (by city count).
    // The spawn island is always included regardless of rank.
    visible.sort(function (a, b) { return b[3] - a[3]; });
    let spawn = null;
    let nonSpawn = [];
    for (let i = 0; i < visible.length; i++) {
        if (visible[i][8] === 1) spawn = visible[i];
        else nonSpawn.push(visible[i]);
    }
    let topN = nonSpawn.slice(0, MAX_ISLAND_DISPLAY);

    // Gradient bounds from the rendered non-spawn set.
    let minCount = topN.length > 0 ? topN[topN.length - 1][3] : 0;
    let maxCount = topN.length > 0 ? topN[0][3] : 1;
    let singleIsland = topN.length === 1;

    // Render spawn island first (always on top visually).
    let toRender = spawn ? [spawn].concat(topN) : topN;

    for (let i = 0; i < toRender.length; i++) {
        let island = toRender[i];
        let isSpawn = island[8] === 1;
        let cityCount = island[3];
        let spawnOrder = island[9];

        let t = null;
        if (!isSpawn) {
            t = singleIsland ? null
                : maxCount > minCount ? (cityCount - minCount) / (maxCount - minCount)
                    : 0.5;
        }

        let latlng = L.latLng(island[2], island[1]);
        let marker = L.marker(latlng, { icon: makeIslandIcon(cityCount, isSpawn, t) });
        let popupLabel = isSpawn
            ? '★ World Spawn &mdash; ' + cityCount + ' cities'
            : 'Island #' + island[0] + ' &mdash; ' + cityCount + ' cities'
            + ' &middot; spawn order #' + spawnOrder;
        marker.bindPopup(popupLabel);
        marker.on('click', function () { drawIslandHighlight(island); });
        islandLayer.addLayer(marker);
    }

    if (!map.hasLayer(islandLayer)) map.addLayer(islandLayer);
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

function waitForReady() {
    setLoading('Loading world data...');
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
        .catch(function () { setTimeout(waitForReady, 1000); });
}

function loadIslands() {
    setLoading('Loading islands...');
    fetch('/islands.json')
        .then(function (resp) { return resp.json(); })
        .then(function (islands) {
            allIslands = islands;
            clearLoading();
            if (map.getZoom() < CITY_ZOOM_THRESHOLD) {
                updateIslandView();
            }
        })
        .catch(function () { setLoading('Failed to load islands'); });
}

waitForReady();

// ---------------------------------------------------------------------------
// Popup helpers
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
        ['\u{2728} Favor', res.favor]
    ];
    let html = '<div class="city-popup-inner">';
    html += '<div class="city-popup-title">City (' + cx + ', ' + cy + ')</div>';
    html += '<div class="city-popup-sub">Island #' + rid + ' &middot; ' + res.biome + '</div>';
    html += '<table class="city-res">';
    for (let i = 0; i < rows.length; i++) {
        html += '<tr><td>' + rows[i][0] + '</td><td>' + fmtMod(rows[i][1]) + '</td></tr>';
    }
    if (res.gold_nodes > 0) {
        html += '<tr><td>\u{1FA99} Gold nodes</td><td><span class="res-gold">' + res.gold_nodes + '</span></td></tr>';
    }
    html += '</table></div>';
    return html;
}

// ---------------------------------------------------------------------------
// Zoom / pan event handlers
// ---------------------------------------------------------------------------

map.on('zoomend', function () {
    highlightLayer.clearLayers();
    let z = map.getZoom();
    if (z >= CITY_ZOOM_THRESHOLD) {
        if (map.hasLayer(islandLayer)) map.removeLayer(islandLayer);
        scheduleCityFetch();
    } else {
        // Zoomed out of city view -- cancel any pending fetch and show islands.
        if (activeCityFetch) { activeCityFetch.abort(); activeCityFetch = null; clearLoading(); }
        clearTimeout(cityDebounceTimer);
        if (map.hasLayer(cityLayer)) map.removeLayer(cityLayer);
        cityLayer.clearLayers();
        updateIslandView();
    }
});

map.on('moveend', function () {
    let z = map.getZoom();
    if (z >= CITY_ZOOM_THRESHOLD) {
        scheduleCityFetch();
    } else if (allIslands) {
        updateIslandView();
    }
});

// ---------------------------------------------------------------------------
// Loading indicator helpers
// ---------------------------------------------------------------------------

function setLoading(msg) {
    let el = document.getElementById('loading');
    el.textContent = msg;
    el.style.display = '';
}

function clearLoading() {
    document.getElementById('loading').style.display = 'none';
}
