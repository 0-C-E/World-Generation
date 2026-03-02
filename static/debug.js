const MAP_SIZE = {{ MAP_SIZE }};
const TILE_SIZE = {{ TILE_SIZE }};
const MAX_ZOOM = {{ MAX_ZOOM }};
const factor = TILE_SIZE / MAP_SIZE;
const WORLD_FP = document.body.getAttribute('data-world-fingerprint') || '0';
const SESSION = Math.floor(Date.now() / 900000);

// Custom CRS: same as the normal viewer
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

// Use debug tiles (/dtile/) which have borders and labels baked in
let tileLayer = L.tileLayer('/dtile/{z}/{x}/{y}.png?v=' + WORLD_FP + '.' + SESSION, {
    minZoom: 0,
    maxZoom: MAX_ZOOM,
    tileSize: TILE_SIZE,
    noWrap: true,
    bounds: bounds
}).addTo(map);

let center = L.latLng(MAP_SIZE / 2, MAP_SIZE / 2);
map.setView(center, 3);
map.setMaxBounds(bounds.pad(0.1));

// ---------------------------------------------------------------------------
// Spawn-order island overlay
//
// Every island is labelled with its spawn order number. The world spawn island
// gets a distinct gold "★ SPAWN" badge. A connecting polyline traces the
// population path from the spawn island outward in order, giving an at-a-
// glance picture of the expansion wave.
// ---------------------------------------------------------------------------

let spawnLayer = L.layerGroup().addTo(map);
let spawnLineLayer = L.layerGroup().addTo(map);
let allIslands = null;

// How many connections to draw on the spawn-order polyline. Set to Infinity
// to draw all of them (can be noisy on large maps with many islands).
const MAX_SPAWN_LINE_SEGMENTS = 60;

function makeSpawnOrderIcon(island) {
    let isSpawn = island[8] === 1;
    let spawnOrder = island[9];
    let cityCount = island[3];

    if (isSpawn) {
        return L.divIcon({
            className: '',
            html: '<div class="dbg-island-icon dbg-spawn">\u2605<br>SPAWN<br>' + cityCount + '</div>',
            iconSize: [56, 56],
            iconAnchor: [28, 28]
        });
    }

    // Color gradient: early orders (low number) are bright green, later ones
    // fade toward red, giving an intuitive warm/cool heat-map feel.
    let maxOrder = allIslands ? allIslands.length : 1;
    let t = Math.min(1, spawnOrder / (maxOrder * 0.6)); // saturate at 60% of islands
    let r = Math.round(50 + t * 200);
    let g = Math.round(200 - t * 160);
    let b = 50;
    let bg = 'rgb(' + r + ',' + g + ',' + b + ')';

    return L.divIcon({
        className: '',
        html: '<div class="dbg-island-icon" style="background:' + bg + ';">'
            + '#' + spawnOrder + '<br><small>' + cityCount + '</small></div>',
        iconSize: [40, 40],
        iconAnchor: [20, 20]
    });
}

function renderSpawnOverlay() {
    spawnLayer.clearLayers();
    spawnLineLayer.clearLayers();
    if (!allIslands || allIslands.length === 0) return;

    // Sort a copy by spawn_order so we can draw the connection line in order.
    let sorted = allIslands.slice().sort(function (a, b) { return a[9] - b[9]; });

    // Draw connecting polyline (spawn → #1 → #2 → …)
    let linePoints = sorted
        .slice(0, MAX_SPAWN_LINE_SEGMENTS + 1)
        .map(function (isl) { return L.latLng(isl[2], isl[1]); });

    if (linePoints.length > 1) {
        spawnLineLayer.addLayer(L.polyline(linePoints, {
            color: '#ffe066',
            weight: 1.5,
            opacity: 0.55,
            dashArray: '4 6'
        }));
    }

    // Draw island markers for all islands in the current viewport.
    let vb = map.getBounds();
    for (let i = 0; i < allIslands.length; i++) {
        let isl = allIslands[i];
        let latlng = L.latLng(isl[2], isl[1]);
        if (!vb.contains(latlng)) continue;

        let marker = L.marker(latlng, { icon: makeSpawnOrderIcon(isl) });

        (function (island) {
            marker.bindPopup(buildSpawnPopup(island));
        })(isl);

        spawnLayer.addLayer(marker);
    }
}

function buildSpawnPopup(isl) {
    let isSpawn = isl[8] === 1;
    let spawnOrder = isl[9];
    let html = '<b>' + (isSpawn ? '\u2605 World Spawn Island' : 'Island #' + isl[0]) + '</b><br>';
    html += 'Cities: ' + isl[3] + '<br>';
    if (!isSpawn) html += 'Spawn order: <b>#' + spawnOrder + '</b><br>';
    html += 'Centroid: (' + isl[1] + ', ' + isl[2] + ')';
    return html;
}

function loadIslands() {
    fetch('/islands.json')
        .then(function (r) { return r.json(); })
        .then(function (islands) {
            allIslands = islands;
            renderSpawnOverlay();
        })
        .catch(function (e) { console.error('Failed to load islands.json', e); });
}

loadIslands();

// Re-render markers when panning (the polyline covers the full map already).
map.on('moveend', function () {
    if (allIslands) renderSpawnOverlay();
});

// ---------------------------------------------------------------------------
// Debug info panel - updates on every zoom/pan
// ---------------------------------------------------------------------------

let debugInfo = document.getElementById('debug-info');

function updateDebugPanel() {
    let z = map.getZoom();
    let c = map.getCenter();
    let b = map.getBounds();
    let size = map.getSize();
    let tilesPerAxis = Math.pow(2, z);
    let regionW = MAP_SIZE / tilesPerAxis;
    let regionH = MAP_SIZE / tilesPerAxis;

    // Pixel bounds from Leaflet's perspective
    let pixelOrigin = map.getPixelOrigin();
    let pixelBounds = map.getPixelBounds();

    // Which tile indices are visible
    let nw = b.getNorthWest();
    let se = b.getSouthEast();
    let tileXmin = Math.floor(Math.max(0, nw.lng) / regionW);
    let tileYmin = Math.floor(Math.max(0, nw.lat) / regionH);
    let tileXmax = Math.floor(Math.min(MAP_SIZE - 1, se.lng) / regionW);
    let tileYmax = Math.floor(Math.min(MAP_SIZE - 1, se.lat) / regionH);

    let spawnInfo = '';
    if (allIslands) {
        let spawn = allIslands.filter(function (i) { return i[8] === 1; })[0];
        if (spawn) {
            spawnInfo = '<div style="margin-top:6px;border-top:1px solid #555;padding-top:4px;color:#ffe066;">'
                + '\u2605 Spawn island #' + spawn[0] + ' at (' + spawn[1] + ', ' + spawn[2] + ')'
                + ' &mdash; ' + spawn[3] + ' cities</div>'
                + '<div><span class="debug-label">Total islands:</span> ' + allIslands.length + '</div>';
        }
    }

    let html = '';
    html += '<div><span class="debug-label">Zoom:</span> ' + z + '</div>';
    html += '<div><span class="debug-label">Tiles/axis:</span> ' + tilesPerAxis + '</div>';
    html += '<div><span class="debug-label">Region size:</span> ' + regionW.toFixed(2) + ' x ' + regionH.toFixed(2) + ' world px</div>';
    html += '<div><span class="debug-label">Center (lat,lng):</span> ' + c.lat.toFixed(1) + ', ' + c.lng.toFixed(1) + '</div>';
    html += '<div><span class="debug-label">Viewport:</span> ' + size.x + ' x ' + size.y + ' CSS px</div>';
    html += '<div><span class="debug-label">Pixel origin:</span> ' + pixelOrigin.x + ', ' + pixelOrigin.y + '</div>';
    html += '<div><span class="debug-label">Pixel bounds:</span> ' + pixelBounds.min.x + ',' + pixelBounds.min.y + ' to ' + pixelBounds.max.x + ',' + pixelBounds.max.y + '</div>';
    html += '<div><span class="debug-label">Visible tiles X:</span> ' + tileXmin + ' - ' + tileXmax + '</div>';
    html += '<div><span class="debug-label">Visible tiles Y:</span> ' + tileYmin + ' - ' + tileYmax + '</div>';
    html += '<div style="margin-top:6px;border-top:1px solid #555;padding-top:4px;">';
    html += '<span class="debug-label">CRS transform:</span> factor=' + factor.toFixed(6) + '</div>';
    html += '<div><span class="debug-label">tileSize:</span> ' + TILE_SIZE + '</div>';
    html += '<div><span class="debug-label">MAP_SIZE:</span> ' + MAP_SIZE + '</div>';
    html += spawnInfo;

    // Check Leaflet's internal tile positioning
    html += '<div style="margin-top:6px;border-top:1px solid #555;padding-top:4px;color:#ff0;">Tile CSS check (first 4):</div>';
    let tiles = document.querySelectorAll('.leaflet-tile');
    let count = 0;
    for (let i = 0; i < tiles.length && count < 4; i++) {
        let tile = tiles[i];
        if (tile.src && tile.src.indexOf('/dtile/') !== -1) {
            let parts = tile.src.match(/\/dtile\/(\d+)\/(\d+)\/(\d+)/);
            if (parts) {
                let tz = parts[1], tx = parts[2], ty = parts[3];
                let style = tile.style;
                html += '<div>  t(' + tz + '/' + tx + '/' + ty + ') pos=' + style.left + ',' + style.top + ' w=' + style.width + ' h=' + style.height + '</div>';
                count++;
            }
        }
    }

    debugInfo.innerHTML = html;
}

map.on('zoomend', updateDebugPanel);
map.on('moveend', updateDebugPanel);
map.on('load', updateDebugPanel);

// Initial update
setTimeout(updateDebugPanel, 500);

// Also log to console when tiles load
tileLayer.on('tileload', function (e) {
    let coords = e.coords;
    let tile = e.tile;
    let rect = tile.getBoundingClientRect();
    console.log(
        'Tile loaded z=' + coords.z + ' x=' + coords.x + ' y=' + coords.y +
        ' | CSS: left=' + tile.style.left + ' top=' + tile.style.top +
        ' w=' + tile.style.width + ' h=' + tile.style.height +
        ' | BBox: ' + rect.left.toFixed(1) + ',' + rect.top.toFixed(1) +
        ' ' + rect.width.toFixed(1) + 'x' + rect.height.toFixed(1)
    );
});

tileLayer.on('tileerror', function (e) {
    console.error('Tile error z=' + e.coords.z + ' x=' + e.coords.x + ' y=' + e.coords.y);
});

// Inject debug island icon styles into the page at runtime.
(function () {
    let style = document.createElement('style');
    style.textContent = [
        '.dbg-island-icon {',
        '  display: flex; flex-direction: column;',
        '  align-items: center; justify-content: center;',
        '  width: 40px; height: 40px; border-radius: 50%;',
        '  background: rgba(60,180,80,0.85);',
        '  border: 2px solid rgba(255,255,255,0.85);',
        '  color: #fff; font: bold 11px/1.1 monospace;',
        '  text-align: center; box-shadow: 0 1px 4px rgba(0,0,0,0.5);',
        '}',
        '.dbg-island-icon small { font-size: 9px; opacity: 0.85; }',
        '.dbg-spawn {',
        '  width: 56px; height: 56px;',
        '  background: rgba(210,160,0,0.95) !important;',
        '  border: 3px solid #fff !important;',
        '  font-size: 13px !important;',
        '  box-shadow: 0 0 12px 4px rgba(255,200,0,0.6) !important;',
        '}',
    ].join('\n');
    document.head.appendChild(style);
})();
