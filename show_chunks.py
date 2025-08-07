import os

import matplotlib
import matplotlib.pyplot as plt
from PIL import Image

matplotlib.use("TkAgg")

CHUNK_FOLDER = "chunks"
CHUNK_SIZE = 250


def load_chunks(folder):
    chunk_files = sorted([f for f in os.listdir(folder) if f.startswith("chunk_") and f.endswith(".png")])
    # Determine grid size from filenames
    coords = [tuple(map(int, f.replace("chunk_", "").replace(".png", "").split("_"))) for f in chunk_files]
    if not coords:
        print("No chunk files found.")
        return None
    max_x = max(c[0] for c in coords) + 1
    max_y = max(c[1] for c in coords) + 1

    # Create empty grid
    grid = [[None for _ in range(max_x)] for _ in range(max_y)]
    for fname, (x, y) in zip(chunk_files, coords):
        img = Image.open(os.path.join(folder, fname))
        grid[y][x] = img
    return grid, max_x, max_y


def display_chunks(grid, max_x, max_y):
    # Stitch images together
    stitched = Image.new("RGB", (max_x * CHUNK_SIZE, max_y * CHUNK_SIZE))
    for y in range(max_y):
        for x in range(max_x):
            if grid[y][x]:
                stitched.paste(grid[y][x], (x * CHUNK_SIZE, y * CHUNK_SIZE))
    plt.figure(figsize=(12, 12))
    plt.subplots_adjust(left=0, right=1, top=1, bottom=0, wspace=0, hspace=0)
    plt.imshow(stitched)
    plt.axis("off")
    plt.show()


if __name__ == "__main__":
    grid, max_x, max_y = load_chunks(CHUNK_FOLDER)
    if grid:
        display_chunks(grid, max_x, max_y)
