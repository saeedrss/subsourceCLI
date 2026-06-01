
import os
import re
import json
import time
import zipfile
import hashlib
import argparse
from pathlib import Path
from typing import List, Dict, Optional, Tuple, Set
import requests
import sys
from difflib import SequenceMatcher

API_BASE_URL = "https://api.subsource.net/api/v1"
API_KEY = "sk_f4963b83a6e4f6d5966c2d0fd31aa2b4b6e646e940142db2e04321e56e3881a6"

VIDEO_EXTENSIONS = {'.mp4', '.mkv', '.avi', '.mov', '.wmv', '.flv', '.m4v', '.webm'}
SUBTITLE_EXTENSIONS = {'.srt', '.ass', '.ssa', '.sub', '.vtt', '.txt'}
REQUEST_DELAY = 1.0


class SubSourceAPI:
    def __init__(self, api_key: str, proxy: str = None):
        self.api_key = api_key
        self.proxy = proxy
        self.headers = {
            "X-API-Key": api_key,
            "Accept": "application/json",
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
        }
        self.last_request_time = 0
    
    def _proxies(self) -> Optional[Dict[str, str]]:
        if self.proxy:
            return {"http": self.proxy, "https": self.proxy}
        return None

    def _rate_limit(self):
        elapsed = time.time() - self.last_request_time
        if elapsed < REQUEST_DELAY:
            time.sleep(REQUEST_DELAY - elapsed)
        self.last_request_time = time.time()

    def search_movie(self, query: str, year: str = None) -> Optional[List[Dict]]:
        self._rate_limit()
        try:
            params = {"searchType": "text", "q": query, "type": "all"}
            if year:
                params["year"] = year
            
            response = requests.get(
                f"{API_BASE_URL}/movies/search",
                headers=self.headers,
                params=params,
                timeout=30,
                proxies=self._proxies()
            )
            response.raise_for_status()
            data = response.json()
            
            if isinstance(data, dict) and "data" in data:
                return data["data"]
            return data if isinstance(data, list) else []
        except Exception as e:
            print(f"  Error searching: {e}")
            return None

    def get_subtitles(self, movie_id: str, lang: str) -> Optional[List[Dict]]:
        self._rate_limit()
        try:
            params = {
                "movieId": movie_id,
                "language": lang,
                "sort": "rating",
                "limit": 100
            }
            
            response = requests.get(
                f"{API_BASE_URL}/subtitles",
                headers=self.headers,
                params=params,
                timeout=30,
                proxies=self._proxies()
            )
            response.raise_for_status()
            data = response.json()
            
            if isinstance(data, dict) and "data" in data:
                return data["data"]
            return data if isinstance(data, list) else []
        except Exception as e:
            print(f"  Error fetching subtitles: {e}")
            return None

    def download_subtitle_zip(self, subtitle_id: str, output_path: str) -> bool:
        self._rate_limit()
        try:
            response = requests.get(
                f"{API_BASE_URL}/subtitles/{subtitle_id}/download",
                headers=self.headers,
                timeout=30,
                proxies=self._proxies()
            )
            response.raise_for_status()
            
            with open(output_path, 'wb') as f:
                f.write(response.content)
            return True
        except Exception as e:
            print(f"  Error downloading: {e}")
            return False


class SubtitleDownloader:
    def __init__(self, api_key: str, top_n: int = 5, proxy: str = None):
        self.api = SubSourceAPI(api_key, proxy)
        self.stats = {"scanned": 0, "found": 0, "downloaded": 0, "errors": 0, "skipped": 0}
        self.top_n = top_n
    
    def parse_filename(self, filename: str) -> Dict[str, str]:
        name = os.path.splitext(filename)[0]
        info = {"title": "", "year": "", "season": "", "episode": "", "is_episode": False, "search_query": ""}
        
        name_no_brackets = re.sub(r'[\[\(].*?[\]\)]', ' ', name)
        
        episode_match = re.search(r'[Ss](\d{1,2})[Ee](\d{1,2})', name)
        if episode_match:
            info["season"] = episode_match.group(1).zfill(2)
            info["episode"] = episode_match.group(2).zfill(2)
            info["is_episode"] = True
        
        year_match = re.search(r'(?<!\d)(19\d{2}|20[0-3]\d)(?!\d)', name_no_brackets)
        if year_match:
            info["year"] = year_match.group(1)
        
        if info["is_episode"]:
            match = re.search(r'[Ss]\d{1,2}[Ee]\d{1,2}', name)
            if match:
                name = name[:match.end()]
        
        clean_name = re.sub(r'\[.*?\]|\(.*?\)|\{.*?\}', ' ', name)
        
        tech_patterns = [
            r'\d{3,4}p', r'BluRay|BRRip|HDRip|WEB-DL|WEBRip|DVDRip|HDTV|AMZN|NF',
            r'x264|x265|HEVC|H\.264|AVC|h264|h265',
            r'DDP\d\.\d|AC3|AAC\d\.\d|DTS|Atmos|DDPA\d\.\d',
            r'ETHEL|EDITH|EZTV|TGx|rartv|YIFY|NAISU|SHORTBREHD|iCEBERG|AGLET|TBD|t3nzin|VideoGod|WORLD|CONDITION|SyncUp|GainfulCapedHyraxOfPiety|SuccessfulCrab|EniaHD|APEX|SPARKS|GECKOS|DRACULA|ROVERS|LAZY|DEFLATE|DEMAND|NTb|KiNGS|Cinefeel|TRASHCAN|GalaxyTV|FLUX|HONE|KOGi|mSD|BAMBOOLEZ|MiNX|ION10|PSA|RARBG|YTS|AMIABLE',
            r'REMUX|Complete|UNRATED|UNCUT|PROPER|REPACK|EXTENDED|DIRECTORS?\.?CUT|DC',
            r'Sample|Samples|Trailer|Featurette',
            r'S\d{1,2}E\d{1,2}', r'Season\s*\d+|Episode\s*\d+',
            r'www\..*?\.org|www\..*?\.com|www\..*?\.net',
        ]
        
        for pattern in tech_patterns:
            clean_name = re.sub(pattern, ' ', clean_name, flags=re.IGNORECASE)
        
        clean_name = re.sub(r'[._]', ' ', clean_name)
        clean_name = re.sub(r'\s+', ' ', clean_name).strip()
        clean_name = re.sub(r'[-\s]+$', '', clean_name)
        
        info["title"] = clean_name
        
        search_name = clean_name
        season_marker = re.search(r'\bS\d{1,2}(?:E\d{1,2})?\b', search_name, re.IGNORECASE)
        if season_marker:
            search_name = search_name[:season_marker.start()].strip()
            search_name = re.sub(r'\s+', ' ', search_name).strip()
        info["search_query"] = search_name if search_name else clean_name
        return info

    def extract_season_from_release_info(self, release_info: str) -> Optional[int]:
        if not release_info:
            return None
        
        patterns = [
            r'Season\s*(\d{1,2})',
            r'Season(\d{1,2})',
            r'\bS(\d{1,2})\b',
            r'S(\d{1,2})E\d{1,2}',
        ]
        
        for pattern in patterns:
            match = re.search(pattern, release_info, re.IGNORECASE)
            if match:
                return int(match.group(1))
        
        return None

    def filter_subtitles_by_season(self, subtitles: List[Dict], target_season: str) -> List[Dict]:
        if not target_season:
            return subtitles
        
        target_season_int = int(target_season)
        filtered = []
        
        for sub in subtitles:
            release_info = ' '.join(sub.get('releaseInfo', []))
            sub_season = self.extract_season_from_release_info(release_info)
            
            if sub_season is not None:
                if sub_season == target_season_int:
                    filtered.append(sub)
            else:
                filtered.append(sub)
        
        return filtered

    def extract_and_rename_best(self, zip_path: Path, video_path: Path, file_info: Dict) -> bool:
        """Extract best match (sub1) and rename to match video"""
        extract_dir = video_path.parent / f"_temp_subs_{video_path.stem}"
        
        try:
            with zipfile.ZipFile(zip_path, 'r') as zip_ref:
                all_files = zip_ref.namelist()
                subtitle_files = [f for f in all_files 
                                if any(f.lower().endswith(ext) for ext in SUBTITLE_EXTENSIONS)]
                
                if not subtitle_files:
                    print(f"    [ERROR] No subtitle files found in ZIP")
                    return False
                
                print(f"    [ZIP] ZIP contains {len(subtitle_files)} subtitle file(s)")
                
                extract_dir.mkdir(exist_ok=True)
                zip_ref.extractall(extract_dir)
                
                # Map episodes if multiple files
                if len(subtitle_files) == 1:
                    # Single file - simple rename
                    src = extract_dir / subtitle_files[0]
                    dst = video_path.parent / f"{video_path.stem}.fa{src.suffix}"
                    
                    if dst.exists():
                        backup = dst.with_suffix(dst.suffix + ".backup")
                        dst.rename(backup)
                        print(f"    [BACKUP] Backed up existing: {backup.name}")

                    src.rename(dst)
                    print(f"    [OK] Extracted and renamed to: {dst.name}")
                    return True
                else:
                    # Multiple files - try to find matching episode
                    target_ep = int(file_info["episode"]) if file_info["episode"] else None
                    
                    if target_ep:
                        # Try to find matching episode
                        for sub_file in subtitle_files:
                            basename = os.path.basename(sub_file)
                            # Patterns: E01, EP01, episode01, S01E01, 1x01, etc.
                            patterns = [
                                rf'[eE][pP]?{target_ep:02d}\b',
                                rf'[eE][pP]?\.?{target_ep:02d}',
                                rf'episode[\s\.]?{target_ep:02d}',
                                rf'\b{target_ep:02d}\b',
                                rf'S\d{{1,2}}E{target_ep:02d}',
                            ]
                            
                            if any(re.search(p, basename, re.IGNORECASE) for p in patterns):
                                src = extract_dir / sub_file
                                dst = video_path.parent / f"{video_path.stem}.fa{src.suffix}"
                                
                                if dst.exists():
                                    backup = dst.with_suffix(dst.suffix + ".backup")
                                    dst.rename(backup)
                                
                                src.rename(dst)
                                print(f"    [OK] Matched episode {target_ep}: {dst.name}")
                                return True
                        
                        # Could not match specific episode
                        print(f"    [WARNING] Could not match episode {target_ep} in multi-file ZIP")
                        print(f"    [DIR] Available files: {len(subtitle_files)}")
                        for i, f in enumerate(subtitle_files[:5], 1):
                            print(f"       {i}. {os.path.basename(f)}")
                        return False
                    else:
                        # No episode number, just take first file
                        src = extract_dir / subtitle_files[0]
                        dst = video_path.parent / f"{video_path.stem}.fa{src.suffix}"
                        
                        if dst.exists():
                            backup = dst.with_suffix(dst.suffix + ".backup")
                            dst.rename(backup)
                        
                        src.rename(dst)
                        print(f"    [OK] Extracted first file: {dst.name}")
                        return True
                
        except Exception as e:
            print(f"    [ERROR] Error extracting ZIP: {e}")
            import traceback
            traceback.print_exc()
            return False
            
        finally:
            # Cleanup temp directory
            if extract_dir.exists():
                try:
                    for f in extract_dir.iterdir():
                        f.unlink()
                    extract_dir.rmdir()
                except:
                    pass

    def score_subtitle(self, sub: Dict) -> Tuple[float, int]:
        r = sub.get('rating', {}) or {}
        good = r.get('good', 0)
        total = r.get('total', 1)
        rating_score = good / total if total > 0 else 0
        downloads = sub.get('downloads', 0)
        return (rating_score, downloads)

    def process_video_file(self, video_path: Path, dry_run: bool = False) -> bool:
        print(f"\n[FILE] Processing: {video_path.name}")
        self.stats["scanned"] += 1
        
        file_info = self.parse_filename(video_path.name)
        print(f"  [PARSE] Title='{file_info['title']}', S={file_info['season']}, E={file_info['episode']}")
        
        base_name = video_path.stem
        parent_dir = video_path.parent
        sub_dir = parent_dir / "sub"
        sub_dir.mkdir(exist_ok=True)
        
        # Check if best subtitle already extracted
        best_subtitle = parent_dir / f"{base_name}.fa.srt"
        if best_subtitle.exists():
            print(f"  [INFO] Best subtitle already extracted: {best_subtitle.name}")
            # Still check if we have backup ZIPs
            backup_zips = list(sub_dir.glob(f"{base_name}_sub[2-9]_*.zip"))
            if len(backup_zips) >= self.top_n - 1:
                print(f"  [INFO] {len(backup_zips)} backup ZIPs also present")
                self.stats["skipped"] += 1
                return True
         
        if not file_info['search_query']:
            folder_name = video_path.parent.name
            folder_info = self.parse_filename(folder_name)
            fallback = folder_info['search_query'] or folder_name
            file_info['search_query'] = fallback
            file_info['title'] = fallback
            if not file_info['season'] and folder_info['season']:
                file_info['season'] = folder_info['season']
            print(f"  [FALLBACK] Using folder-derived name: '{fallback}'")
        
        print(f"  [SEARCH] Searching: '{file_info['search_query']}'")
        results = self.api.search_movie(file_info['search_query'])
        
        if not results:
            print(f"  [ERROR] No search results")
            self.stats["errors"] += 1
            return False
        
        print(f"  [COUNT] API returned {len(results)} result(s)")
        
        # Find best match
        best_match = None
        best_score = 0
        for movie in results:
            titles = [movie.get('title', ''), movie.get('alternateTitle', '')]
            for title in titles:
                if title:
                    score = SequenceMatcher(None, title.lower(), file_info['title'].lower()).ratio()

                    # Boost score for TV series when processing episodes
                    if file_info["is_episode"] and movie.get('type') in ['tvseries', 'series']:
                        score += 0.15

                    # For TV episodes, check season match
                    if file_info["is_episode"] and file_info["season"]:
                        movie_season = movie.get('season')
                        if movie_season is not None:
                            target_season = int(file_info["season"])
                            if movie_season == target_season:
                                score += 0.3  # Big boost for exact season match
                            else:
                                score -= 0.5  # Penalty for wrong season

                    if score > best_score and score > 0.6:
                        best_score = score
                        best_match = movie
        
        if not best_match:
            print(f"  [ERROR] No good match found")
            self.stats["errors"] += 1
            return False
        
        movie_id = best_match.get('movieId')
        movie_title = best_match.get('title') or best_match.get('alternateTitle')
        movie_type = best_match.get('type')
        print(f"  [SUCCESS] Found: '{movie_title}' [{movie_type}, ID: {movie_id}]")
        
        # Find Farsi subtitles
        print(f"  [LANG] Searching for Farsi/Persian subtitles...")
        lang_codes = ["farsi_persian", "farsi", "persian", "fa"]
        subtitles = None
        
        for lang in lang_codes:
            print(f"    [TRY] Language code: '{lang}'...")
            subs = self.api.get_subtitles(movie_id, lang)
            if subs and len(subs) > 0:
                print(f"    [FOUND] Found {len(subs)} subtitles with code '{lang}'")
                subtitles = subs
                break
        
        if not subtitles:
            print(f"  [ERROR] No Farsi/Persian subtitles found")
            self.stats["errors"] += 1
            return False
        
        # Filter by season
        print(f"  [FILTER] Filtering for Season {file_info['season']}...")
        season_filtered = self.filter_subtitles_by_season(subtitles, file_info['season'])
        
        if not season_filtered:
            print(f"  [WARNING] No Season {file_info['season']} subtitles found, using all available...")
            season_filtered = subtitles
        
        print(f"  [COUNT] Found {len(season_filtered)} subtitle(s) for Season {file_info['season']}")
        
        # Filter by episode if specified
        filtered = season_filtered
        if file_info["is_episode"] and file_info["season"] and file_info["episode"]:
            s, e = file_info["season"], file_info["episode"]
            ep_matches = []
            for sub in season_filtered:
                rel_info = ' '.join(sub.get('releaseInfo', []))
                commentary = sub.get('commentary', '')
                combined = f"{rel_info} {commentary}"
                
                patterns = [f'S{s}E{e}', f's{s}e{e}', f'{int(s)}x{int(e)}']
                if any(p in combined or p.lower() in combined.lower() for p in patterns):
                    ep_matches.append(sub)
            
            if ep_matches:
                filtered = ep_matches
                print(f"  [MATCH] Episode-specific matches: {len(ep_matches)}")
        if not filtered:
            print(f"  [ERROR] No matching subtitles")
            self.stats["errors"] += 1
            return False
        
        # Sort by score and get top N
        sorted_subs = sorted(filtered, key=self.score_subtitle, reverse=True)
        top_subs = sorted_subs[:self.top_n]
        
        print(f"\n  [DOWNLOAD] Downloading top {len(top_subs)} subtitle(s):")
        
        downloaded_zips = []
        
        for i, sub in enumerate(top_subs, 1):
            sub_id = sub.get('subtitleId')
            release_info = ' '.join(sub.get('releaseInfo', []))
            rating = sub.get('rating', {})
            rating_str = f"{rating.get('good', 0)}/{rating.get('total', 0)}"
            downloads = sub.get('downloads', 0)
            
            # Create filename: video_sub1_releasename.zip, video_sub2_releasename.zip, etc.
            safe_release = re.sub(r'[^\w\-\.]', '_', release_info, flags=re.ASCII)[:50]
            zip_name = f"{base_name}_sub{i}_{safe_release}.zip"
            zip_path = sub_dir / zip_name
            
            is_best = (i == 1)
            action = "EXTRACT" if is_best else "BACKUP"
            
            safe_release = release_info[:50].encode('ascii', 'replace').decode('ascii')
            print(f"\n    [{i}/{len(top_subs)}] [{action}] {safe_release}...")
            print(f"         Rating: {rating_str}, Downloads: {downloads}")
            
            if dry_run:
                print(f"         [DRYRUN] Would save as: sub\\{zip_name}")
                continue
          
            # Check if already exists
            if zip_path.exists():
                print(f"         [INFO] Already exists: {zip_name}")
                downloaded_zips.append(zip_path)
                
                # If it's the best and not yet extracted, extract it
                if is_best and not best_subtitle.exists():
                    print(f"         [EXTRACT] Extracting existing best match...")
                    if self.extract_and_rename_best(zip_path, video_path, file_info):
                        self.stats["downloaded"] += 1
                continue
            
            # Download
            print(f"         [DOWNLOAD] Downloading...")
            if self.api.download_subtitle_zip(str(sub_id), str(zip_path)):
                print(f"         [SUCCESS] Saved: {zip_name}")
                downloaded_zips.append(zip_path)
                
                # If it's the best match, extract and rename
                if is_best:
                    print(f"         [EXTRACT] Extracting best match...")
                    if self.extract_and_rename_best(zip_path, video_path, file_info):
                        self.stats["downloaded"] += 1
            else:
                print(f"         [ERROR] Download failed")
        if len(downloaded_zips) > 0:
            print(f"\n  [SUCCESS] Downloaded {len(downloaded_zips)} ZIP file(s)")
            print(f"     Best (sub1) extracted to: {base_name}.fa.srt")
            backup_count = len(downloaded_zips)
            if backup_count > 1:
                print(f"     Backups in sub\\: {base_name}_sub[2-{backup_count + 1}]_*.zip")
            elif backup_count == 1:
                print(f"     Backups in sub\\: {base_name}_sub2_*.zip")
            self.stats["found"] += 1
        else:
            print(f"\n  [ERROR] Failed to download any subtitles")
            self.stats["errors"] += 1
            return False

    def scan_directory(self, directory: Path, recursive: bool = True, dry_run: bool = False):
        print(f"\n{'='*70}")
        print(f"[SEARCH] Scanning: {directory}")
        print(f"   Mode: Extract best .fa.srt + keep top {self.top_n} ZIPs in sub\\")
        print(f"{'='*70}")
        
        pattern = "**/*" if recursive else "*"
        videos = []
        for ext in VIDEO_EXTENSIONS:
            videos.extend(directory.glob(f"{pattern}{ext}"))
        
        videos = sorted([v for v in videos if 'sample' not in v.name.lower()])
        print(f"Found {len(videos)} video file(s)")
        
        for v in videos:
            try:
                self.process_video_file(v, dry_run)
            except Exception as e:
                print(f"  [EXCEPTION] Error: {e}")
                import traceback
                traceback.print_exc()
                self.stats["errors"] += 1
            time.sleep(0.5)
            time.sleep(0.5)
        
        self.print_stats()

    def print_stats(self):
        print(f"\n{'='*70}")
        print("[STATS] FINAL STATISTICS")
        print(f"{'='*70}")
        print(f"  Scanned:    {self.stats['scanned']}")
        print(f"  Found:      {self.stats['found']}")
        print(f"  Downloaded: {self.stats['downloaded']} (extracted + backups)")
        print(f"  Skipped:    {self.stats['skipped']}")
        print(f"  Errors:     {self.stats['errors']}")
        print(f"{'='*70}")
        print("\n[OUTPUT] Output structure:")
        print("   video.mkv")
        print("   video.fa.srt          <- Best match (extracted)")
        print("   sub\\")
        print("     video_sub1_*.zip    <- Best match (backup)")
        print("     video_sub2_*.zip    <- Alternative 1")
        print("     video_sub3_*.zip    <- Alternative 2")
        print("     ...")


def main():
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    parser = argparse.ArgumentParser(
        description="Download Farsi subtitles - extract best, keep top 5 as ZIP",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python script.py "G:\\1080\\MyShow"
  python script.py "G:\\1080\\MyShow" --top 3
  python script.py "G:\\1080\\MyShow" --dry-run
        """
    )
    parser.add_argument('--directory', default='.', help='Directory to use (default: current)')
    parser.add_argument("--api-key", help="API key")
    parser.add_argument("--no-recursive", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--top", type=int, default=5, help="Keep top N as backup (default: 5)")
    parser.add_argument("--proxy", default=None, help="Proxy URL (e.g. http://127.0.0.1:8080)")
    
    args = parser.parse_args()
    
    api_key = args.api_key or os.environ.get("SUBSOURCE_API_KEY") or API_KEY
    if not api_key:
        print("[ERROR] Error: API key required")
        return 1
    
    directory = Path(args.directory).expanduser().resolve() 
    if not directory.exists():
        print(f"[ERROR] Error: Directory not found: {directory}")
        return 1


    
    downloader = SubtitleDownloader(api_key, top_n=args.top, proxy=args.proxy)
    downloader.scan_directory(directory, not args.no_recursive, args.dry_run)
    return 0


if __name__ == "__main__":
    exit(main())