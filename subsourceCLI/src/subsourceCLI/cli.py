import os
import sys
import argparse
from pathlib import Path

from subsourceCLI.downloader import SubtitleDownloader


def main():
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    parser = argparse.ArgumentParser(
        description="Download Farsi subtitles - extract best, keep top 5 as ZIP",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  subsourceCLI "G:\\1080\\MyShow"
  subsourceCLI "G:\\1080\\MyShow" --top 3
  subsourceCLI "G:\\1080\\MyShow" --dry-run
        """,
    )
    parser.add_argument("--directory", default=".", help="Directory to use (default: current)")
    parser.add_argument(
        "--api-key",
        help="API key (required if SUBSOURCE_API_KEY env var not set)",
    )
    parser.add_argument("--no-recursive", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument(
        "--top", type=int, default=5, help="Keep top N as backup (default: 5)"
    )
    parser.add_argument(
        "--proxy",
        default=None,
        help="Proxy URL (e.g. http://127.0.0.1:8080)",
    )

    args = parser.parse_args()

    api_key = args.api_key or os.environ.get("SUBSOURCE_API_KEY")
    if not api_key:
        print(
            "[ERROR] API key is required. Provide via --api-key or SUBSOURCE_API_KEY env var."
        )
        return 1

    directory = Path(args.directory).expanduser().resolve()
    if not directory.exists():
        print(f"[ERROR] Directory not found: {directory}")
        return 1

    downloader = SubtitleDownloader(api_key, top_n=args.top, proxy=args.proxy)
    downloader.scan_directory(directory, not args.no_recursive, args.dry_run)
    return 0


if __name__ == "__main__":
    exit(main())
