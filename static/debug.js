var MAP_SIZE = 10000;
var TILE_SIZE = 256;
var MAX_ZOOM = 8;
var factor = TILE_SIZE / MAP_SIZE;
var WORLD_FP = document.body.getAttribute('data-world-fingerprint') || '0';
var SESSION = Math.floor(Date.now() / 900000);

// Custom CRS: same as the normal viewer
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

// Use debug tiles (/dtile/) which have borders and labels baked in
var tileLayer = L.tileLayer('/dtile/{z}/{x}/{y}.png?v=' + WORLD_FP + '.' + SESSION, {
    minZoom: 0,
    maxZoom: MAX_ZOOM,
    tileSize: TILE_SIZE,
    noWrap: true,
    bounds: bounds
}).addTo(map);

var center = L.latLng(MAP_SIZE / 2, MAP_SIZE / 2);
map.setView(center, 3);
map.setMaxBounds(bounds.pad(0.1));

// ---------------------------------------------------------------------------
// Debug info panel - updates on every zoom/pan
// ---------------------------------------------------------------------------

var debugInfo = document.getElementById('debug-info');

function updateDebugPanel() {
    var z = map.getZoom();
    var c = map.getCenter();
    var b = map.getBounds();
    var size = map.getSize();
    var tilesPerAxis = Math.pow(2, z);
    var regionW = MAP_SIZE / tilesPerAxis;
    var regionH = MAP_SIZE / tilesPerAxis;

    // Pixel bounds from Leaflet's perspective
    var pixelOrigin = map.getPixelOrigin();
    var pixelBounds = map.getPixelBounds();

    // Which tile indices are visible
    var nw = b.getNorthWest();
    var se = b.getSouthEast();
    var tileXmin = Math.floor(Math.max(0, nw.lng) / regionW);
    var tileYmin = Math.floor(Math.max(0, nw.lat) / regionH);
    var tileXmax = Math.floor(Math.min(MAP_SIZE - 1, se.lng) / regionW);
    var tileYmax = Math.floor(Math.min(MAP_SIZE - 1, se.lat) / regionH);

    var html = '';
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

    // Check Leaflet's internal tile positioning
    html += '<div style="margin-top:6px;border-top:1px solid #555;padding-top:4px;color:#ff0;">Tile CSS check (first 4):</div>';
    var tiles = document.querySelectorAll('.leaflet-tile');
    var count = 0;
    for (var i = 0; i < tiles.length && count < 4; i++) {
        var tile = tiles[i];
        if (tile.src && tile.src.indexOf('/dtile/') !== -1) {
            var parts = tile.src.match(/\/dtile\/(\d+)\/(\d+)\/(\d+)/);
            if (parts) {
                var tz = parts[1], tx = parts[2], ty = parts[3];
                var style = tile.style;
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
    var coords = e.coords;
    var tile = e.tile;
    var rect = tile.getBoundingClientRect();
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
