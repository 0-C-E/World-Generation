# Template

## 🖼️ Enabling GUI Support for Matplotlib Inside the DevContainer

This project supports displaying `matplotlib` figures (e.g. `plt.show()`) from inside the DevContainer using X11 forwarding. Because containers are headless by default, some setup is needed on your **host system**.

> 🧪 Tested with `TkAgg` backend on Alpine-based containers

### ✅ Requirements

* DevContainer with X11 libraries (already configured in the `Dockerfile`)
* An X11 server running on your **host machine**
* `DISPLAY` environment variable set in `.devcontainer/devcontainer.json`

## 🧭 Setup Instructions by Host OS

### 🪟 Windows (with Docker Desktop)

1. **Install an X11 server**
   We recommend [VcXsrv](https://sourceforge.net/projects/vcxsrv/)

2. **Launch VcXsrv with these settings**:

   * `Multiple windows`
   * `Start no client`
   * ✅ Check **Disable access control**

3. **Ensure your `.devcontainer/devcontainer.json` includes**:

   ```json
   "containerEnv": {
     "DISPLAY": "host.docker.internal:0"
   }
   ```

4. **Rebuild the DevContainer**

5. **Test it**
   Inside the DevContainer:

   ```bash
   xclock  # should open a clock window on your host
   python3 show_chunks.py  # should display the stitched image
   ```

### 🍏 macOS (with Docker Desktop)

1. **Install XQuartz**:
   Download from [https://www.xquartz.org](https://www.xquartz.org)

2. **Start XQuartz**, then enable network access:

   ```bash
   defaults write org.xquartz.X11 enable_iglx -bool true
   xhost + 127.0.0.1
   ```

3. **Update `.devcontainer/devcontainer.json`**:

   ```json
   "containerEnv": {
     "DISPLAY": "host.docker.internal:0"
   }
   ```

4. **Rebuild the DevContainer**, then test as above.

> [!NOTE]
> macOS support for X11 is less reliable — consider using `plt.savefig()` as a fallback.

### 🐧 Linux

1. **Allow local X11 access**:

   ```bash
   xhost +local:
   ```

2. **Expose your X11 socket to the container**
   Add this to your Docker run config or override file:

   ```yml
   volumes:
     - /tmp/.X11-unix:/tmp/.X11-unix
   environment:
     DISPLAY: :0
   ```

3. **In `.devcontainer/devcontainer.json`**, set:

   ```json
   "containerEnv": {
     "DISPLAY": ":0"
   }
   ```

4. **Rebuild the container**, and test `xclock` and your Python script.

## 🧯 Headless Fallback

If GUI display is not possible, the script can be adapted to save instead:

```python
plt.savefig("stitched_output.png")
```

This allows contributors without GUI/X11 support to still see the result.

## 📎 Related Files

* [`show_chunks.py`](show_chunks.py) — loads chunked images and displays them in a GUI window
* [`Dockerfile`](Dockerfile) — includes X11 libraries and font packages
* [`.devcontainer/devcontainer.json`](.devcontainer/devcontainer.json) — sets the `DISPLAY` environment variable
