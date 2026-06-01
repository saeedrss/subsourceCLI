import time
from typing import Dict, List, Optional

import requests

API_BASE_URL = "https://api.subsource.net/api/v1"
REQUEST_DELAY = 1.0


class SubSourceAPI:
    def __init__(self, api_key: str, proxy: str = None):
        self.api_key = api_key
        self.proxy = proxy
        self.headers = {
            "X-API-Key": api_key,
            "Accept": "application/json",
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
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
                proxies=self._proxies(),
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
                "limit": 100,
            }

            response = requests.get(
                f"{API_BASE_URL}/subtitles",
                headers=self.headers,
                params=params,
                timeout=30,
                proxies=self._proxies(),
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
                proxies=self._proxies(),
            )
            response.raise_for_status()

            with open(output_path, "wb") as f:
                f.write(response.content)
            return True
        except Exception as e:
            print(f"  Error downloading: {e}")
            return False
