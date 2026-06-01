# DearPyGui UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a native Windows desktop GUI for the Farsi subtitle downloader using Dear PyGui.

**Architecture:** Single-file `gui.py` that imports `SubSourceAPI`/`SubtitleDownloader` from the existing `subsourceCLI` package. Uses stdout redirect in a worker thread to capture `print()` output from the downloader and pipe it into the GUI's log display. Master-detail split layout with file list on left, detail/log panel on right.

**Tech Stack:** Python 3, Dear PyGui (`dearpygui`), `threading`, `io`, `contextlib`, `queue`

---

### Task 1: Create gui.py — Dear PyGui window shell + settings bar

**Files:**
- Create: `X:\AI Agant\sub\New folder\gui.py`

- [ ] **Step 1: Write the gui.py scaffold with Dear PyGui window, menu bar, settings bar, and master-detail layout**

```python
import io
import os
import sys
import re
import threading
import contextlib
from queue import Queue
from pathlib import Path
from typing import Optional

import dearpygui.dearpygui as dpg

from subsourceCLI.api import SubSourceAPI
from subsourceCLI.downloader import SubtitleDownloader, VIDEO_EXTENSIONS


# ── Stdout capture ──────────────────────────────────────────────────

class LogCapture(io.StringIO):
    def __init__(self, queue: Queue):
        super().__init__()
        self.queue = queue

    def write(self, s: str):
        if s and s != '\n':
            self.queue.put(s)
        super().write(s)

    def flush(self):
        pass


# ── GUI state ───────────────────────────────────────────────────────

class AppState:
    directory: Optional[str] = None
    top_n: int = 5
    dry_run: bool = False
    recursive: bool = True
    proxy: Optional[str] = None
    api_key: str = ""
    scanning: bool = False
    log_queue: Queue = Queue()
    file_results: list = []
    selected_file_idx: Optional[int] = None
    file_logs: dict = {}
    scan_thread: Optional[threading.Thread] = None


state = AppState()


# ── Callbacks ────────────────────────────────────────────────────────

def select_directory(sender, app_data):
    state.directory = app_data["file_path_name"]
    dpg.set_value("dir_label", state.directory)


def on_scan():
    if state.scanning:
        return
    if not state.directory:
        dpg.set_value("global_log", "Please select a directory first.\n")
        return
    api_key = state.api_key or os.environ.get("SUBSOURCE_API_KEY")
    if not api_key:
        dpg.set_value("global_log", "API key required. Set via input or SUBSOURCE_API_KEY env var.\n")
        return

    state.scanning = True
    state.file_results = []
    state.file_logs = {}
    state.selected_file_idx = None
    dpg.configure_item("scan_btn", label="Scanning...", enabled=False)
    dpg.delete_item("file_table", children_only=True)
    dpg.set_value("global_log", "")
    dpg.set_value("detail_log", "")
    dpg.set_value("detail_header", "")
    dpg.set_value("stats_text", "Scanning...")

    def scan_worker():
        downloader = SubtitleDownloader(api_key, top_n=state.top_n, proxy=state.proxy)
        log_q = state.log_queue
        capture = LogCapture(log_q)

        with contextlib.redirect_stdout(capture):
            directory = Path(state.directory).expanduser().resolve()
            pattern = "**/*" if state.recursive else "*"
            videos = []
            for ext in VIDEO_EXTENSIONS:
                videos.extend(directory.glob(f"{pattern}{ext}"))
            videos = sorted([v for v in videos if "sample" not in v.name.lower()])

            for fname in [v.name for v in videos]:
                dpg_set_threadsafe("stats_text", f"Scanning: {fname}")

            log_q.put(f"Found {len(videos)} video file(s)\n")

            for idx, vpath in enumerate(videos):
                fname = vpath.name
                log_q.put(f"\n{'='*60}\n[{idx+1}/{len(videos)}] {fname}\n{'='*60}\n")

                file_info = downloader.parse_filename(vpath.name)

                dpg_set_threadsafe("stats_text", f"[{idx+1}/{len(videos)}] {fname}")
                dpg_set_threadsafe_threadsafe("stats_text", f"[{idx+1}/{len(videos)}] {fname}")
                _add_file_row(idx, fname, "⏳", file_info)

                try:
                    downloader.process_video_file(vpath, state.dry_run)
                    result_icon = "✅"
                except Exception as e:
                    log_q.put(f"  [EXCEPTION] {e}\n")
                    result_icon = "❌"
                    downloader.stats["errors"] += 1

                state.file_results.append({
                    "name": fname,
                    "status": result_icon,
                    "info": file_info,
                })
                _update_file_row(idx, result_icon)

            stats = downloader.stats
            log_q.put(
                f"\n{'='*60}\n"
                f"Scanned: {stats['scanned']}  "
                f"Found: {stats['found']}  "
                f"Downloaded: {stats['downloaded']}  "
                f"Skipped: {stats['skipped']}  "
                f"Errors: {stats['errors']}\n"
                f"{'='*60}\n"
            )

        dpg_set_threadsafe("stats_text",
            f"Done — {stats['found']} found, {stats['downloaded']} downloaded, "
            f"{stats['errors']} errors, {stats['skipped']} skipped")
        dpg_set_threadsafe_threadsafe("scan_btn", label="Start Scan", enabled=True)
        state.scanning = False

    state.scan_thread = threading.Thread(target=scan_worker, daemon=True)
    state.scan_thread.start()


def dpg_set_threadsafe(tag, **kwargs):
    dpg.set_value(tag, kwargs["value"]) if "value" in kwargs else None  # simplified
    # We use direct queued approach below instead


# Thread-safe DPG updates via queue processed in render loop
dpg_updates = Queue()


def dpg_set_threadsafe(tag, **kwargs):
    dpg_updates.put((tag, kwargs))


def _add_file_row(idx, fname, icon, file_info):
    se = f"S{file_info['season']}E{file_info['episode']}" if file_info['is_episode'] else ""
    dpg_set_threadsafe("file_table", value=None)  # will use add_row approach


def _update_file_row(idx, icon):
    pass  # handled via file_results rebuild in render loop


# ── Render loop ─────────────────────────────────────────────────────

def render():
    # Process DPG updates
    while not dpg_updates.empty():
        tag, kwargs = dpg_updates.get()
        try:
            if "value" in kwargs:
                dpg.set_value(tag, kwargs["value"])
            if "label" in kwargs:
                dpg.configure_item(tag, label=kwargs["label"])
            if "enabled" in kwargs:
                dpg.configure_item(tag, enabled=kwargs["enabled"])
        except Exception:
            pass

    # Process log queue → global_log
    log_lines = []
    while not state.log_queue.empty():
        line = state.log_queue.get()
        log_lines.append(line)
    if log_lines:
        current = dpg.get_value("global_log") or ""
        dpg.set_value("global_log", current + "".join(log_lines))
        dpg.set_y_scroll("global_log", 1e6)  # auto-scroll


# ── Setup Dear PyGui ────────────────────────────────────────────────

def setup_ui():
    dpg.create_context()

    with dpg.viewport_menu_bar():
        with dpg.menu(label="File"):
            dpg.add_menu_item(label="Select Directory", callback=lambda: dpg.show_item("dir_dialog"))
            dpg.add_separator()
            dpg.add_menu_item(label="Exit", callback=lambda: dpg.stop_dearpygui())
        with dpg.menu(label="Settings"):
            dpg.add_menu_item(label="API Key...", callback=lambda: dpg.show_item("api_key_window"))
            dpg.add_menu_item(label="Proxy...", callback=lambda: dpg.show_item("proxy_window"))
        with dpg.menu(label="Help"):
            dpg.add_menu_item(label="About", callback=lambda: dpg.show_item("about_window"))

    with dpg.window(tag="main_window", label="SubSource Farsi Subtitle Downloader",
                    width=1200, height=800, no_close=True, no_collapse=True):

        # ── Settings bar (top) ──
        with dpg.group(horizontal=True):
            dpg.add_button(label="Select Directory", callback=lambda: dpg.show_item("dir_dialog"))
            dpg.add_text(tag="dir_label", default_value="No directory selected")
        dpg.add_separator()

        with dpg.group(horizontal=True):
            dpg.add_text("Top N:")
            dpg.add_input_int(tag="top_n", default_value=5, min_value=1, max_value=20, width=60,
                              callback=lambda s, a: setattr(state, 'top_n', a))
            dpg.add_checkbox(tag="dry_run", label="Dry Run",
                             callback=lambda s, a: setattr(state, 'dry_run', a))
            dpg.add_checkbox(tag="recursive", label="Recursive", default_value=True,
                             callback=lambda s, a: setattr(state, 'recursive', a))
            dpg.add_button(tag="scan_btn", label="Start Scan", callback=on_scan)

        dpg.add_separator()

        # ── Master-detail split ──
        with dpg.group(horizontal=True):
            # Left: file list
            with dpg.child_window(width=400, height=500):
                dpg.add_text("Video Files")
                dpg.add_separator()
                dpg.add_text(tag="stats_text", default_value="No files yet")
                dpg.add_separator()
                dpg.add_text(tag="file_table", default_value="(scan to populate)", wrap=380)

            # Right: detail + log
            with dpg.child_window(width=700, height=500):
                dpg.add_text(tag="detail_header", default_value="Select a file to view details")
                dpg.add_separator()
                dpg.add_text("Log Output:")
                dpg.add_input_text(tag="global_log", multiline=True, readonly=True,
                                   width=680, height=430, default_value="")

        dpg.add_separator()
        dpg.add_text("Tip: Select directory → Start Scan → Click files in left panel for details",
                     color=[150, 150, 150])

    # ── Directory dialog ──
    with dpg.file_dialog(directory_selector=True, tag="dir_dialog", show=False,
                         callback=select_directory, width=600, height=400):
        dpg.add_file_extension(".*")

    # ── API Key dialog ──
    with dpg.window(tag="api_key_window", label="API Key", width=400, height=150, show=False, no_resize=True):
        dpg.add_text("Enter your SubSource API key:")
        dpg.add_input_text(tag="api_key_input", password=True, width=380,
                           callback=lambda s, a: setattr(state, 'api_key', a))
        dpg.add_button(label="Close", callback=lambda: dpg.hide_item("api_key_window"))

    # ── Proxy dialog ──
    with dpg.window(tag="proxy_window", label="Proxy", width=400, height=150, show=False, no_resize=True):
        dpg.add_text("Proxy URL (e.g. http://127.0.0.1:8080):")
        dpg.add_input_text(tag="proxy_input", width=380,
                           callback=lambda s, a: setattr(state, 'proxy', a or None))
        dpg.add_button(label="Close", callback=lambda: dpg.hide_item("proxy_window"))

    # ── About dialog ──
    with dpg.window(tag="about_window", label="About", width=350, height=180, show=False, no_resize=True):
        dpg.add_text("SubSource Farsi Subtitle Downloader")
        dpg.add_text("Version 1.0")
        dpg.add_text("Uses SubSource API to download Farsi subtitles")
        dpg.add_separator()
        dpg.add_button(label="Close", callback=lambda: dpg.hide_item("about_window"))

    dpg.create_viewport(title="SubSource Farsi Subtitle Downloader", width=1220, height=860)
    dpg.setup_dearpygui()
    dpg.show_viewport()
    dpg.maximize_viewport()

    # Render loop
    while dpg.is_dearpygui_running():
        render()
        dpg.render_dearpygui_frame()

    dpg.destroy_context()


if __name__ == "__main__":
    setup_ui()
```

- [ ] **Step 2: Verify file written correctly**

Run: `python -c "import ast; ast.parse(open('gui.py').read()); print('Syntax OK')"`
Expected: `Syntax OK`

- [ ] **Step 3: Test that it launches**

Run: `python gui.py`
Expected: Dear PyGui window appears with settings bar, empty master-detail layout, and menu bar. Close the window.

---

### Task 2: Implement file table (left panel) with live status updates

**Files:**
- Modify: `gui.py`

- [ ] **Step 1: Replace the static `file_table` text with a proper `dpg.add_table()` in the left panel**

Replace the left child_window content in `setup_ui()`:

```python
with dpg.child_window(tag="left_panel", width=400, height=500):
    dpg.add_text("Video Files")
    dpg.add_separator()
    dpg.add_text(tag="stats_text", default_value="0 files")
    dpg.add_separator()
    with dpg.table(tag="file_table", header_row=True, borders_innerH=True, borders_outerH=True,
                   borders_innerV=True, borders_outerV=True, scrollY=True, height=400):
        dpg.add_table_column(label="", width_fixed=True, width=30)
        dpg.add_table_column(label="Filename", width_stretch=True)
        dpg.add_table_column(label="Episode", width_fixed=True, width=80)
        dpg.add_table_column(label="Status", width_fixed=True, width=100)
```

Also add a `click_callback` on rows to select files.

- [ ] **Step 2: Add callback for file selection**

When a user clicks a file in the table, show its parsed info + log in the right panel.

```python
def on_file_click(sender, app_data):
    row = dpg.get_item_user_data(sender)
    if row is None:
        return
    state.selected_file_idx = row["idx"]
    info = row["info"]
    header = f"{row['name']}  |  {info.get('title', '')}"
    if info.get('is_episode'):
        header += f"  S{info['season']}E{info['episode']}"
    dpg.set_value("detail_header", header)
    log = state.file_logs.get(row["idx"], "")
    dpg.set_value("detail_log", log)
```

- [ ] **Step 3: Update `_add_file_row` and `_update_file_row` to use table rows**

```python
def _add_file_row(idx, fname, icon, file_info):
    se = f"S{file_info['season']}E{file_info['episode']}" if file_info['is_episode'] else ""
    status = "Pending"
    with dpg.table_row(parent="file_table", tag=f"filerow_{idx}"):
        t = dpg.add_text(icon, tag=f"fileicon_{idx}")
        n = dpg.add_text(fname, tag=f"filename_{idx}")
        e = dpg.add_text(se, tag=f"fileep_{idx}")
        s = dpg.add_text(status, tag=f"filestatus_{idx}")
    # Make clickable
    for tag in [f"filerow_{idx}", f"fileicon_{idx}", f"filename_{idx}", f"fileep_{idx}", f"filestatus_{idx}"]:
        dpg.set_item_user_data(tag, {"idx": idx, "name": fname, "info": file_info})


def _update_file_row(idx, icon, status_text="Done"):
    try:
        dpg.set_value(f"fileicon_{idx}", icon)
        dpg.set_value(f"filestatus_{idx}", status_text)
    except Exception:
        pass
```

- [ ] **Step 4: Wire file selection**

After table creation, add:

```python
with dpg.handler_registry():
    dpg.add_click_handler(callback=on_file_click)
```

Actually, use row-click via `dpg.add_table_next_column` approach. Instead, set user_data on each cell and use a click handler.

---

### Task 3: Wire detail panel and per-file log capture

**Files:**
- Modify: `gui.py`

- [ ] **Step 1: Replace right panel to show detail header + log per selected file**

```python
with dpg.child_window(tag="right_panel", width=700, height=500):
    dpg.add_text(tag="detail_header", default_value="Select a file to view details")
    dpg.add_separator()
    dpg.add_text("Progress Log:")
    dpg.add_input_text(tag="detail_log", multiline=True, readonly=True,
                       width=680, height=430, default_value="")
```

- [ ] **Step 2: In the scan worker, capture per-file logs**

In the scan loop, create a per-file log buffer and write to both the global log queue and the detail log when that file is selected:

```python
per_file_log = []
file_log_q = Queue()  # per-file log lines

# When starting a file:
per_file_log = []
file_log_q = Queue()
log_q.put(f"\n{'='*60}\n[{idx+1}/{len(videos)}] {fname}\n{'='*60}\n")

# After processing:
state.file_logs[idx] = "".join(per_file_log)
```

Replace `LogCapture.write` to also capture to a thread-local buffer:

```python
class LogCapture(io.StringIO):
    def __init__(self, queue: Queue, file_log_capture: list = None):
        super().__init__()
        self.queue = queue
        self.file_log = file_log_capture

    def write(self, s: str):
        if s and s != '\n':
            self.queue.put(s)
            if self.file_log is not None:
                self.file_log.append(s)
        super().write(s)
```

- [ ] **Step 3: Update worker to pass file_log_capture**

```python
capture = LogCapture(log_q, per_file_log)
```

---

### Task 4: Final wiring + testing

**Files:**
- Modify: `gui.py`

- [ ] **Step 1: Ensure all pieces connect**

Checklist:
- File dialog sets `state.directory`
- Scan button validates inputs and starts worker thread
- Worker populates file table with rows as it discovers videos
- Each file shows icon: ⏳ → ✅ or ❌
- Per-file logs captured and displayed when file is clicked
- Global log shows all output concatenated
- Stats bar updates live
- Scan button re-enables when done

- [ ] **Step 2: Run the app**

Run: `python gui.py`
Expected: Full UI works end-to-end.

- [ ] **Step 3: Edge case — run without API key**

Verify error message appears when trying to scan without setting API key.

- [ ] **Step 4: Edge case — run with empty directory**

Verify graceful message about 0 files found.
