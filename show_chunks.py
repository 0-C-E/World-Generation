import os
import json
import argparse

import matplotlib
import matplotlib.pyplot as plt
import matplotlib.patches as patches
from PIL import Image

matplotlib.use("TkAgg")

CHUNK_FOLDER = "chunks"
CHUNK_SIZE = 250
WORLD_SAVE_FILE = "world_save.json"

def load_chunks(folder):
    chunk_files = sorted([f for f in os.listdir(folder) if f.startswith("chunk_") and f.endswith(".png")])
    # Determine grid size from filenames
    coords = [tuple(map(int, f.replace("chunk_", "").replace(".png", "").split("_"))) for f in chunk_files]
    if not coords:
        print("No chunk files found.")
        return None, 0, 0
    max_x = max(c[0] for c in coords) + 1
    max_y = max(c[1] for c in coords) + 1

    # Create empty grid
    grid = [[None for _ in range(max_x)] for _ in range(max_y)]
    for fname, (x, y) in zip(chunk_files, coords):
        img = Image.open(os.path.join(folder, fname))
        grid[y][x] = img
    return grid, max_x, max_y

def load_world_data(filename):
    """Load city and island data from JSON file"""
    try:
        with open(filename, 'r') as f:
            return json.load(f)
    except FileNotFoundError:
        print(f"Warning: {filename} not found. City data will not be available.")
        return None

def display_chunks(grid, max_x, max_y, world_data=None, show_cities=True, show_islands=False):
    # Stitch images together
    print("Stitching image chunks...")
    stitched = Image.new("RGB", (max_x * CHUNK_SIZE, max_y * CHUNK_SIZE))
    for y in range(max_y):
        for x in range(max_x):
            if grid[y][x]:
                stitched.paste(grid[y][x], (x * CHUNK_SIZE, y * CHUNK_SIZE))

    print("Creating matplotlib figure...")
    fig, ax = plt.subplots(figsize=(12, 12))
    fig.subplots_adjust(left=0, right=1, top=1, bottom=0, wspace=0, hspace=0)
    ax.imshow(stitched)
    ax.axis("off")

    # Debug: Print world data info
    if world_data and world_data.get('islands'):
        total_cities = sum(len(island['city_slots']) for island in world_data['islands'])
        print(f"Found {len(world_data['islands'])} islands with {total_cities} total cities")

        if show_cities and total_cities > 0:
            city_coords = []
            for island in world_data['islands']:
                for city in island['city_slots']:
                    city_coords.append((city['x'], city['y']))

            if city_coords:
                xs, ys = zip(*city_coords)
                ax.scatter(xs, ys, c='red', s=10, alpha=0.75, linewidths=0.1, label='Cities').set_sizes([40] * len(city_coords))
                print(f"Displayed {len(city_coords)} cities")

        # Show island summary
        if show_islands:
            print("Island summary:")
            sorted_islands = sorted(world_data['islands'], key=lambda x: len(x['city_slots']), reverse=True)

            # Show top 5 islands by city count
            for i, island in enumerate(sorted_islands[:5]):
                print(f"  Island {island['region_id']}: {len(island['city_slots'])} cities")
    else:
        print("No world data found or no islands in data")

    print("Displaying plot...")
    plt.show()

def main():
    parser = argparse.ArgumentParser(description='Display generated world chunks')
    parser.add_argument('--no-cities', action='store_true', help='Hide city markers')
    parser.add_argument('--show-islands', action='store_true', help='Show island information')
    parser.add_argument('--save', type=str, help='Save image to file instead of displaying')

    args = parser.parse_args()

    # Load world chunks
    grid, max_x, max_y = load_chunks(CHUNK_FOLDER)
    if not grid:
        return

    # Load world data
    world_data = load_world_data(WORLD_SAVE_FILE)

    # Display options
    show_cities = not args.no_cities and world_data is not None
    show_islands = args.show_islands

    if args.save:
        # Save mode
        stitched = Image.new("RGB", (max_x * CHUNK_SIZE, max_y * CHUNK_SIZE))
        for y in range(max_y):
            for x in range(max_x):
                if grid[y][x]:
                    stitched.paste(grid[y][x], (x * CHUNK_SIZE, y * CHUNK_SIZE))

        # If showing cities, we'd need to draw them on the PIL image
        # For now, just save the base terrain
        stitched.save(args.save)
        print(f"World map saved as {args.save}")
    else:
        # Interactive display
        display_chunks(grid, max_x, max_y, world_data, show_cities, show_islands)

if __name__ == "__main__":
    main()
