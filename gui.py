import sys
import os
import json
import threading
import io
import contextlib
import queue
import urllib.request
import webbrowser
from pathlib import Path

import arabic_reshaper
from bidi.algorithm import get_display

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "subsourceCLI", "src"))

import dearpygui.dearpygui as dpg

from subsourceCLI.downloader import SubtitleDownloader, VIDEO_EXTENSIONS

CONFIG_DIR = Path(os.path.expanduser("~/.subsource-gui"))
CONFIG_FILE = CONFIG_DIR / "config.json"
FONT_PATH = Path(__file__).parent / "fonts" / "NotoSansArabic.ttf"

LANG = {
    "en": {
        "file_menu": "File",
        "settings_menu": "Settings",
        "help_menu": "Help",
        "select_directory": "Select Directory",
        "exit": "Exit",
        "api_key_menu": "API Key...",
        "proxy_menu": "Proxy...",
        "about_menu": "About",
        "top_n": "Top N:",
        "dry_run": "Dry Run",
        "recursive": "Recursive",
        "start_scan": "Start Scan",
        "scanning": "Scanning...",
        "no_dir": "No directory selected",
        "video_files": "Video Files",
        "log_output": "Log Output:",
        "full_progress": "Full Progress:",
        "tip": "Select a directory, configure your API key in Settings, then click Start Scan.",
        "about_title": "About",
        "about_name": "SubSource Farsi Subtitle Downloader",
        "about_ver": "Version 1.0.0",
        "about_desc": "Downloads Farsi subtitles for your video files.",
        "about_powered": "Powered by SubSource API and Dear PyGui.",
        "about_author": "github.com/saeedrss",
        "about_repo": "Open GitHub repository",
        "about_releases": "View Releases",
        "close": "Close",
        "save": "Save",
        "api_key_title": "API Key",
        "api_key_prompt": "Enter SubSource API Key:",
        "api_key_hint": "Paste your API key here",
        "api_key_empty": "API key is empty.",
        "api_key_updated": "API key updated.",
        "proxy_title": "Proxy Settings",
        "proxy_prompt": "Proxy URL (e.g. http://127.0.0.1:8080):",
        "proxy_hint": "Leave empty for no proxy",
        "proxy_updated": "Proxy updated.",
        "pick_language": "Choose Language / انتخاب زبان",
        "lang_en": "English",
        "lang_fa": "فارسی",
        "select_dir_error": "Please select a valid directory first.",
        "no_video_files": "No video files found in the selected directory.",
        "scan_error": "Scan failed: {0}",
        "filename_col": "Filename",
        "episode_col": "Episode",
        "status_col": "Status",
        "pending": "Pending",
        "done": "Done",
        "failed": "Failed",
        "error": "Error",
        "stats_fmt": "Scanned: {0} | Found: {1} | Downloaded: {2} | Errors: {3} | Skipped: {4}",
        "lang_toggle": "FA",
        "lang_toggle_tt": "Switch to Farsi",
    },
    "fa": {
        "file_menu": "فایل",
        "settings_menu": "تنظیمات",
        "help_menu": "راهنما",
        "select_directory": "انتخاب پوشه",
        "exit": "خروج",
        "api_key_menu": "کلید API...",
        "proxy_menu": "پراکسی...",
        "about_menu": "درباره",
        "top_n": "تعداد:",
        "dry_run": "آزمایشی",
        "recursive": "به‌همراه زیرپوشه‌ها",
        "start_scan": "شروع اسکن",
        "scanning": "درحال اسکن...",
        "no_dir": "پوشه‌ای انتخاب نشده",
        "video_files": "فایل‌های ویدیویی",
        "log_output": "خروجی گزارش:",
        "full_progress": "پیشرفت کامل:",
        "tip": "پوشه را انتخاب کنید، کلید API را در تنظیمات وارد کنید، سپس Start Scan را بزنید.",
        "about_title": "درباره",
        "about_name": "دانلود زیرنویس فارسی",
        "about_ver": "نسخه ۱.۰.۰",
        "about_desc": "دانلود زیرنویس فارسی برای فایل‌های ویدیویی شما.",
        "about_powered": "قدرت گرفته از SubSource API و Dear PyGui.",
        "about_author": "github.com/saeedrss",
        "about_repo": "مخزن GitHub",
        "about_releases": "مشاهده انتشارات",
        "close": "بستن",
        "save": "ذخیره",
        "api_key_title": "کلید API",
        "api_key_prompt": "کلید API ساب‌سورس را وارد کنید:",
        "api_key_hint": "کلید خود را اینجا بچسبانید",
        "api_key_empty": "کلید API خالی است.",
        "api_key_updated": "کلید API به‌روز شد.",
        "proxy_title": "تنظیمات پراکسی",
        "proxy_prompt": "آدرس پراکسی (مثال http://127.0.0.1:8080):",
        "proxy_hint": "برای عدم استفاده خالی بگذارید",
        "proxy_updated": "پراکسی به‌روز شد.",
        "pick_language": "انتخاب زبان / Choose Language",
        "lang_en": "English",
        "lang_fa": "فارسی",
        "select_dir_error": "لطفا ابتدا یک پوشه معتبر انتخاب کنید.",
        "no_video_files": "در پوشه انتخاب شده فایل ویدیویی یافت نشد.",
        "scan_error": "خطا در اسکن: {0}",
        "filename_col": "نام فایل",
        "episode_col": "قسمت",
        "status_col": "وضعیت",
        "pending": "در انتظار",
        "done": "انجام شد",
        "failed": "ناموفق",
        "error": "خطا",
        "stats_fmt": "اسکن: {0} | یافت: {1} | دانلود: {2} | خطا: {3} | رد شده: {4}",
        "lang_toggle": "EN",
        "lang_toggle_tt": "Switch to English",
    },
}

current_lang = "en"
farsi_font_tag = None
labeled_items = []


def tr(key, *args):
    val = LANG.get(current_lang, LANG["en"]).get(key, key)
    if args:
        val = val.format(*args)
    val = get_display(arabic_reshaper.reshape(val))
    return val


def save_config():
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    with open(CONFIG_FILE, "w", encoding="utf-8") as f:
        json.dump({"language": current_lang}, f, ensure_ascii=False)


def load_config():
    global current_lang
    if CONFIG_FILE.exists():
        try:
            with open(CONFIG_FILE, "r", encoding="utf-8") as f:
                data = json.load(f)
                current_lang = data.get("language", "en")
                return True
        except Exception:
            pass
    return False


def refresh_ui_labels():
    for tag, key in labeled_items:
        try:
            dpg.configure_item(tag, label=tr(key))
        except Exception:
            pass


def toggle_language():
    global current_lang
    current_lang = "fa" if current_lang == "en" else "en"
    save_config()
    refresh_ui_labels()
    dpg.configure_item("lang_btn", label=tr("lang_toggle"))
    if current_lang == "fa":
        bind_farsi_font_to_all_items()
    if dpg.does_item_exist("welcome_window"):
        dpg.configure_item("welcome_window", show=False)


def bind_farsi_font_to_all_items():
    if farsi_font_tag is None:
        return
    def walk(items):
        for item in items:
            try:
                dpg.bind_item_font(item, farsi_font_tag)
            except Exception:
                pass
            children = dpg.get_item_children(item, 1) or []
            walk(children)
    walk(dpg.get_windows())


class LogCapture(io.StringIO):
    def __init__(self, log_queue, file_log_capture=None):
        super().__init__()
        self.log_queue = log_queue
        self.file_log = file_log_capture

    def write(self, s):
        if s.strip():
            self.log_queue.put(s)
            if self.file_log is not None:
                self.file_log.append(s)
        super().write(s)

    def flush(self):
        super().flush()


class AppState:
    def __init__(self):
        self.directory = ""
        self.top_n = 5
        self.dry_run = False
        self.recursive = True
        self.proxy = ""
        self.api_key = os.environ.get("SUBSOURCE_API_KEY", "")
        self.scanning = False
        self.log_queue = queue.Queue()
        self.file_results = []
        self.selected_file_idx = -1
        self.file_logs = {}
        self.scan_thread = None


dpg_updates = queue.Queue()


def dpg_set_threadsafe(tag, **kwargs):
    dpg_updates.put((tag, kwargs))


def select_directory(sender, app_data):
    if app_data and "file_path_name" in app_data:
        state.directory = app_data["file_path_name"]
        dpg_set_threadsafe("directory_path", value=state.directory)


def _add_file_row(idx, fname, icon, file_info):
    se = f"S{file_info['season']}E{file_info['episode']}" if file_info['is_episode'] else ""
    with dpg.table_row(parent="file_table", tag=f"filerow_{idx}"):
        dpg.add_text(icon, tag=f"fileicon_{idx}")
        dpg.add_selectable(label=fname, tag=f"filename_{idx}", span_columns=True,
                           callback=on_file_select,
                           user_data={"idx": idx, "name": fname, "info": file_info})
        dpg.add_text(se, tag=f"fileep_{idx}")
        dpg.add_text(tr("pending"), tag=f"filestatus_{idx}")


def _update_file_row(idx, icon, status_text="Done"):
    try:
        dpg.set_value(f"fileicon_{idx}", icon)
        dpg.set_value(f"filestatus_{idx}", status_text)
    except Exception:
        pass


def on_file_select(sender, app_data, user_data):
    data = user_data
    if data is None:
        return
    idx = data["idx"]
    name = data["name"]
    info = data["info"]
    state.selected_file_idx = idx
    se = f"S{info['season']}E{info['episode']}" if info.get('is_episode') else ""
    header = f"{name}  ({se})" if se else name
    dpg.set_value("detail_header", header)
    log = state.file_logs.get(idx, "")
    dpg.set_value("detail_log", log)


def on_scan():
    if state.scanning:
        return
    if not state.directory or not Path(state.directory).exists():
        dpg_set_threadsafe("global_log", value="\n[ERROR] " + tr("select_dir_error") + "\n")
        return

    state.scanning = True
    dpg_set_threadsafe("scan_btn", enabled=False)
    dpg_set_threadsafe("global_log", value="\n")
    dpg_set_threadsafe("stats_text", value="")
    dpg_updates.put(("__dpg_cmd__", {"cmd": "delete_children", "tag": "file_table"}))
    state.file_results.clear()
    state.file_logs.clear()
    state.selected_file_idx = -1
    dpg_set_threadsafe("detail_header", value="")
    dpg_set_threadsafe("detail_log", value="")

    def scan_worker():
        api_key = state.api_key
        try:
            downloader = SubtitleDownloader(
                api_key=api_key,
                top_n=state.top_n,
                proxy=state.proxy if state.proxy else None,
            )

            video_files = []
            pattern = "**/*" if state.recursive else "*"
            directory = Path(state.directory)

            for ext in VIDEO_EXTENSIONS:
                for f in directory.glob(f"{pattern}{ext}"):
                    if "sample" not in f.name.lower():
                        video_files.append(f)

            video_files.sort()

            global_capture = LogCapture(state.log_queue)
            with contextlib.redirect_stdout(global_capture):
                if not video_files:
                    print("[INFO] " + tr("no_video_files"))
                    dpg_set_threadsafe("scan_btn", enabled=True)
                    state.scanning = False
                    return

                scanned = 0
                found = 0
                downloaded = 0
                errors = 0
                skipped = 0

                for idx, fpath in enumerate(video_files):
                    file_info = downloader.parse_filename(fpath.name)
                    dpg_updates.put(("__dpg_cmd__", {
                        "cmd": "add_file_row", "idx": idx, "fname": fpath.name,
                        "icon": "⏳", "file_info": file_info,
                    }))

                    per_file_log = []
                    file_capture = LogCapture(state.log_queue, per_file_log)
                    with contextlib.redirect_stdout(file_capture):
                        try:
                            success = downloader.process_video_file(fpath, state.dry_run)
                            scanned += 1
                            if success:
                                icon = "✅"
                                status_txt = tr("done")
                                found += 1
                                downloaded += 1
                            else:
                                icon = "❌"
                                status_txt = tr("failed")
                                errors += 1
                        except Exception as e:
                            print(f"  [EXCEPTION] {e}")
                            icon = "❌"
                            status_txt = tr("error")
                            errors += 1

                    state.file_logs[idx] = "".join(per_file_log)
                    state.file_results.append({
                        "name": fpath.name,
                        "status": icon,
                        "info": file_info,
                    })

                    dpg_updates.put(("__dpg_cmd__", {
                        "cmd": "update_file_row", "idx": idx,
                        "icon": icon, "status_text": status_txt,
                    }))

                stats = tr("stats_fmt", scanned, found, downloaded, errors, skipped)
                dpg_set_threadsafe("stats_text", value=stats)
                dpg_set_threadsafe("scan_btn", enabled=True)
                state.scanning = False
        except Exception as e:
            print(tr("scan_error", str(e)))
            import traceback
            traceback.print_exc()
            dpg_set_threadsafe("scan_btn", enabled=True)
            state.scanning = False

    state.scan_thread = threading.Thread(target=scan_worker, daemon=True)
    state.scan_thread.start()


def render():
    while not dpg_updates.empty():
        tag, kwargs = dpg_updates.get_nowait()
        try:
            if tag == "__dpg_cmd__":
                cmd = kwargs.get("cmd")
                if cmd == "delete_children":
                    dpg.delete_item(kwargs["tag"], children_only=True)
                elif cmd == "add_file_row":
                    _add_file_row(kwargs["idx"], kwargs["fname"], kwargs["icon"], kwargs["file_info"])
                elif cmd == "update_file_row":
                    _update_file_row(kwargs["idx"], kwargs["icon"], kwargs["status_text"])
                continue
            if "value" in kwargs:
                dpg.set_value(tag, kwargs["value"])
            if "label" in kwargs:
                dpg.configure_item(tag, label=kwargs["label"])
            if "enabled" in kwargs:
                dpg.configure_item(tag, enabled=kwargs["enabled"])
        except (SystemError, Exception):
            pass

    while not state.log_queue.empty():
        try:
            line = state.log_queue.get_nowait()
            current = dpg.get_value("global_log") or ""
            dpg.set_value("global_log", current + line)
            dpg.set_y_scroll("global_log", dpg.get_y_scroll_max("global_log"))
        except (SystemError, Exception):
            pass

    if state.selected_file_idx >= 0:
        try:
            log = state.file_logs.get(state.selected_file_idx, "")
            current = dpg.get_value("detail_log")
            if current != log:
                dpg.set_value("detail_log", log)
            dpg.set_y_scroll("detail_log", dpg.get_y_scroll_max("detail_log"))
        except (SystemError, Exception):
            pass


def setup_ui():
    dpg.create_context()

    # ── Font ──
    global farsi_font_tag
    if FONT_PATH.exists():
        with dpg.font_registry():
            farsi_font_tag = dpg.add_font(str(FONT_PATH), 26)

    # ── Config + language ──
    is_first_run = not load_config()

    with dpg.file_dialog(
        directory_selector=True,
        show=False,
        callback=select_directory,
        tag="dir_dialog",
        width=600,
        height=400,
    ):
        pass

    # ── Welcome / language picker (first run) ──
    with dpg.window(
        tag="welcome_window",
        label=tr("pick_language"),
        modal=True,
        no_close=True,
        show=is_first_run,
        width=400,
        height=200,
        pos=(400, 300),
    ):
        dpg.add_text(tr("pick_language"), tag="welcome_title")
        dpg.add_spacer(height=20)

        def set_lang_en():
            global current_lang
            current_lang = "en"
            save_config()
            dpg.configure_item("welcome_window", show=False)
            dpg.configure_item("main_window", label=tr("about_name"))
            refresh_ui_labels()

        def set_lang_fa():
            global current_lang
            current_lang = "fa"
            save_config()
            dpg.configure_item("welcome_window", show=False)
            dpg.configure_item("main_window", label=tr("about_name"))
            refresh_ui_labels()
            if farsi_font_tag is not None:
                bind_farsi_font_to_all_items()

        with dpg.group(horizontal=True):
            dpg.add_button(label=tr("lang_en"), width=150, callback=set_lang_en)
            dpg.add_button(label=tr("lang_fa"), width=150, callback=set_lang_fa)

    with dpg.window(
        tag="api_key_window",
        label=tr("api_key_title"),
        modal=True,
        show=False,
        no_close=True,
        width=400,
    ):
        dpg.add_text(tr("api_key_prompt"))
        api_key_input = dpg.add_input_text(
            tag="api_key_input",
            password=True,
            width=380,
            hint=tr("api_key_hint"),
        )
        dpg.add_spacer(height=8)
        labeled_items.append(("api_key_window", "api_key_title"))

        def save_api_key():
            key = dpg.get_value("api_key_input")
            if key:
                state.api_key = key
                dpg.configure_item("api_key_window", show=False)
                dpg_set_threadsafe("global_log", value="\n[INFO] " + tr("api_key_updated") + "\n")
            else:
                dpg_set_threadsafe("global_log", value="\n[WARNING] " + tr("api_key_empty") + "\n")

        dpg.add_button(label=tr("save"), callback=lambda: save_api_key())

    with dpg.window(
        tag="proxy_window",
        label=tr("proxy_title"),
        modal=True,
        show=False,
        no_close=True,
        width=400,
    ):
        dpg.add_text(tr("proxy_prompt"))
        dpg.add_input_text(
            tag="proxy_input",
            width=380,
            hint=tr("proxy_hint"),
        )
        dpg.add_spacer(height=8)
        labeled_items.append(("proxy_window", "proxy_title"))

        def save_proxy():
            proxy = dpg.get_value("proxy_input")
            state.proxy = proxy
            dpg.configure_item("proxy_window", show=False)
            dpg_set_threadsafe("global_log", value="\n[INFO] " + tr("proxy_updated") + "\n")

        dpg.add_button(label=tr("save"), callback=lambda: save_proxy())

    with dpg.window(
        tag="about_window",
        label=tr("about_title"),
        modal=True,
        show=False,
        no_close=True,
        width=400,
        height=280,
    ):
        dpg.add_text(tr("about_name"))
        dpg.add_text(tr("about_ver"))
        dpg.add_spacer(height=8)
        dpg.add_text(tr("about_desc"))
        dpg.add_spacer(height=8)
        dpg.add_text(tr("about_powered"))
        dpg.add_text(tr("about_author"))
        dpg.add_spacer(height=4)
        dpg.add_button(
            label=tr("about_repo"),
            callback=lambda: webbrowser.open("https://github.com/saeedrss/subsourceCLI"),
        )
        dpg.add_button(
            label=tr("about_releases"),
            callback=lambda: webbrowser.open("https://github.com/saeedrss/subsourceCLI/releases"),
        )

        def close_about():
            dpg.configure_item("about_window", show=False)

        dpg.add_button(label=tr("close"), callback=lambda: close_about())

    with dpg.window(
        tag="main_window",
        label=tr("about_name"),
        width=1200,
        height=800,
        no_close=True,
        no_collapse=True,
    ):
        # ── Menu bar ──
        with dpg.menu_bar():
            with dpg.menu(label=tr("file_menu")) as fm:
                labeled_items.append((fm, "file_menu"))
                dpg.add_menu_item(
                    label=tr("select_directory"),
                    tag="menu_select_dir",
                    callback=lambda: dpg.show_item("dir_dialog"),
                )
                labeled_items.append(("menu_select_dir", "select_directory"))
                dpg.add_menu_item(label=tr("exit"), tag="menu_exit",
                                  callback=lambda: dpg.stop_dearpygui())
                labeled_items.append(("menu_exit", "exit"))

            with dpg.menu(label=tr("settings_menu")) as sm:
                labeled_items.append((sm, "settings_menu"))
                dpg.add_menu_item(
                    label=tr("api_key_menu"),
                    tag="menu_api_key",
                    callback=lambda: dpg.configure_item("api_key_window", show=True),
                )
                labeled_items.append(("menu_api_key", "api_key_menu"))
                dpg.add_menu_item(
                    label=tr("proxy_menu"),
                    tag="menu_proxy",
                    callback=lambda: dpg.configure_item("proxy_window", show=True),
                )
                labeled_items.append(("menu_proxy", "proxy_menu"))

            with dpg.menu(label=tr("help_menu")) as hm:
                labeled_items.append((hm, "help_menu"))
                dpg.add_menu_item(
                    label=tr("about_menu"),
                    tag="menu_about",
                    callback=lambda: dpg.configure_item("about_window", show=True),
                )
                labeled_items.append(("menu_about", "about_menu"))

        # ── Settings bar ──
        with dpg.group(horizontal=True):
            dpg.add_button(
                label=tr("select_directory"),
                tag="btn_select_dir",
                callback=lambda: dpg.show_item("dir_dialog"),
            )
            labeled_items.append(("btn_select_dir", "select_directory"))
            dpg.add_text(tr("no_dir"), tag="directory_path")
            dpg.add_separator()
            dpg.add_text(tr("top_n"), tag="lbl_top_n")
            labeled_items.append(("lbl_top_n", "top_n"))
            dpg.add_input_int(
                tag="top_n",
                default_value=5,
                min_value=1,
                max_value=50,
                width=60,
                callback=lambda s, a: setattr(state, 'top_n', a),
            )
            dpg.add_checkbox(label=tr("dry_run"), tag="dry_run", default_value=False,
                             callback=lambda s, a: setattr(state, 'dry_run', a))
            labeled_items.append(("dry_run", "dry_run"))
            dpg.add_checkbox(label=tr("recursive"), tag="recursive", default_value=True,
                             callback=lambda s, a: setattr(state, 'recursive', a))
            labeled_items.append(("recursive", "recursive"))
            dpg.add_button(label=tr("start_scan"), tag="scan_btn", callback=on_scan)
            labeled_items.append(("scan_btn", "start_scan"))
            dpg.add_button(label=tr("lang_toggle"), tag="lang_btn",
                           callback=toggle_language)
            dpg.add_separator()

        # ── Master-detail split ──
        with dpg.group(horizontal=True):
            with dpg.child_window(width=400, height=500):
                dpg.add_text(tr("video_files"), tag="lbl_video_files")
                labeled_items.append(("lbl_video_files", "video_files"))
                dpg.add_separator()
                dpg.add_text("", tag="stats_text")
                dpg.add_separator()
                with dpg.table(tag="file_table", header_row=True, borders_innerH=True,
                               borders_outerH=True, borders_innerV=True, borders_outerV=True,
                               scrollY=True, height=400,
                               policy=dpg.mvTable_SizingStretchProp):
                    dpg.add_table_column(label="", width_fixed=True, width=30)
                    dpg.add_table_column(label=tr("filename_col"), width_stretch=True)
                    dpg.add_table_column(label=tr("episode_col"))
                    dpg.add_table_column(label=tr("status_col"))
                # Register column headers for translation
                col_tags = dpg.get_item_children("file_table", 1)
                if len(col_tags) >= 4:
                    labeled_items.append((col_tags[1], "filename_col"))
                    labeled_items.append((col_tags[2], "episode_col"))
                    labeled_items.append((col_tags[3], "status_col"))

            with dpg.child_window(tag="right_panel", width=700, height=500):
                dpg.add_text("", tag="detail_header")
                dpg.add_separator()
                dpg.add_text(tr("log_output"), tag="lbl_log_output")
                labeled_items.append(("lbl_log_output", "log_output"))
                dpg.add_input_text(tag="detail_log", multiline=True, readonly=True,
                                   width=680, height=200, default_value="")
                dpg.add_separator()
                dpg.add_text(tr("full_progress"), tag="lbl_full_progress")
                labeled_items.append(("lbl_full_progress", "full_progress"))
                dpg.add_input_text(tag="global_log", multiline=True, readonly=True,
                                   width=680, height=200, default_value="")

        dpg.add_text(tr("tip"), tag="lbl_tip")
        labeled_items.append(("lbl_tip", "tip"))

    dpg.create_viewport(
        title=LANG["en"]["about_name"],
        width=1200,
        height=800,
    )

    dpg.setup_dearpygui()
    dpg.show_viewport()
    dpg.set_primary_window("main_window", True)

    if farsi_font_tag is not None:
        bind_farsi_font_to_all_items()

    while dpg.is_dearpygui_running():
        render()
        dpg.render_dearpygui_frame()

    dpg.destroy_context()


if __name__ == "__main__":
    state = AppState()
    setup_ui()
