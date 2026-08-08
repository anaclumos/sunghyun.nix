#!/usr/bin/env python3
"""Export Cursor usage CSV by opening the dashboard CSV API in the logged-in browser.

Used by cho.sh scripts/cursor-usage-push.ts, locally (default browser) and over
ssh (a named browser on a remote Mac via CURSOR_USAGE_BROWSER).

Accounts covered by CURSOR_MANAGEMENT_KEY (Team Admin API) are NOT exported here.

Opens:
  https://cursor.com/api/dashboard/export-usage-events-csv
    ?startDate=<ms>&endDate=<ms>&strategy=tokens

The browser attaches the live WorkosCursorSessionToken cookie and downloads the
CSV to ~/Downloads — no cua AX clicks, no cookie scrape (Keychain-locked over
SSH). Copies the file to /tmp/cursor-usage-latest.csv (shell-safe), deletes the
Downloads original, and prints that absolute path on the last stdout line.
The caller (cursor-usage-push.ts) deletes the stable path only after ingest is
confirmed — not after read — so a failed POST keeps the retry source.

Env:
  CURSOR_USAGE_DAYS     trailing UTC days (default 30); same window as Admin API
  CURSOR_USAGE_BROWSER  optional macOS app name for `open -a` (e.g. "Google Chrome")
"""

from __future__ import annotations

import os
import subprocess
import sys
import time
from datetime import datetime, timedelta, timezone
from pathlib import Path
from urllib.parse import urlencode

DOWNLOAD_WAIT_S = 45
STABLE_PATH = Path("/tmp/cursor-usage-latest.csv")
DEFAULT_DAYS = 30


def delete_path(path: Path) -> None:
    try:
        path.unlink()
    except FileNotFoundError:
        return
    except OSError as exc:
        print(f"warning: could not delete {path}: {exc}", file=sys.stderr)


def trailing_window_ms(days: int) -> tuple[int, int]:
    """UTC inclusive trailing window of `days` calendar dates ending today.

    days=30 → today and the prior 29 midnights (30 dates), matching
    scripts/cursor-usage-push.ts.
    """
    now = datetime.now(timezone.utc)
    today = now.replace(hour=0, minute=0, second=0, microsecond=0)
    starting = today - timedelta(days=days - 1)
    ending = today + timedelta(days=1) - timedelta(milliseconds=1)
    return int(starting.timestamp() * 1000), int(ending.timestamp() * 1000)


def export_url(days: int) -> str:
    start_ms, end_ms = trailing_window_ms(days)
    query = urlencode(
        {
            "endDate": str(end_ms),
            "startDate": str(start_ms),
            "strategy": "tokens",
        }
    )
    return f"https://cursor.com/api/dashboard/export-usage-events-csv?{query}"


def open_url(url: str) -> None:
    browser = (os.environ.get("CURSOR_USAGE_BROWSER") or "").strip()
    if browser:
        subprocess.check_call(["open", "-a", browser, url])
        return
    subprocess.check_call(["open", url])


def newest_usage_csv(after_ts: float) -> Path | None:
    downloads = Path.home() / "Downloads"
    cands: list[tuple[float, Path]] = []
    for path in downloads.glob("*.csv"):
        try:
            st = path.stat()
        except OSError:
            continue
        if st.st_mtime < after_ts:
            continue
        if path.name.endswith(".crdownload"):
            continue
        name = path.name.lower()
        # Dashboard API names files usage-events-YYYY-MM-DD.csv (and " (N)" dupes).
        if "usage" in name or "team-usage" in name or "events" in name:
            cands.append((st.st_mtime, path))
    if not cands:
        for path in downloads.glob("*.csv"):
            try:
                st = path.stat()
            except OSError:
                continue
            if st.st_mtime >= after_ts and not path.name.endswith(".crdownload"):
                cands.append((st.st_mtime, path))
    if not cands:
        return None
    cands.sort(reverse=True)
    return cands[0][1]


def export_once(days: int) -> Path:
    url = export_url(days)
    after = time.time()
    print(f"opening {url}", file=sys.stderr)
    open_url(url)
    deadline = after + DOWNLOAD_WAIT_S
    while time.time() < deadline:
        time.sleep(0.4)
        path = newest_usage_csv(after)
        if path is None:
            continue
        # Chrome may still be writing.
        if path.with_suffix(path.suffix + ".crdownload").exists():
            continue
        # Copy to a shell-safe path — "usage-events-… (1).csv" breaks remote ssh cat
        # under zsh NOMATCH when the push script fetches the file. Then delete the
        # Downloads original so repeated builds do not pile up CSVs.
        STABLE_PATH.write_bytes(path.read_bytes())
        if path.resolve() != STABLE_PATH.resolve():
            delete_path(path)
        print(STABLE_PATH.resolve())
        return STABLE_PATH
    raise RuntimeError(
        "timed out waiting for usage CSV download in ~/Downloads "
        "(is the browser logged into cursor.com on this host?)"
    )


def parse_days() -> int:
    raw = os.environ.get("CURSOR_USAGE_DAYS")
    if raw is None or raw.strip() == "":
        return DEFAULT_DAYS
    try:
        days = int(raw.strip())
    except ValueError as exc:
        raise SystemExit(
            f"CURSOR_USAGE_DAYS must be a positive integer (got {raw!r})"
        ) from exc
    if days <= 0:
        raise SystemExit(f"CURSOR_USAGE_DAYS must be a positive integer (got {days})")
    return days


def main() -> int:
    days = parse_days()
    path = export_once(days)
    print(f"exported {path.name} ({path.stat().st_size} bytes)", file=sys.stderr)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as exc:
        print(f"open(1) failed: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
