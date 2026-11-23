#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Exercise the built workbench against an isolated synthetic channel."""

import json
import subprocess
import tempfile
from pathlib import Path

from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[2]
EPS = ROOT / "target/debug/eps"
CAPTURES = ROOT / ".private/gui-captures"
CAPTURES.mkdir(parents=True, exist_ok=True)

with tempfile.TemporaryDirectory(prefix="eps-gui-") as temporary:
    workspace = Path(temporary)
    config = workspace / "eps.toml"
    config.write_text("[channel]\nid='synthetic'\nname='Synthetic production channel'\ntimezone='Europe/London'\n")

    def eps(*args):
        return subprocess.run([str(EPS), "--config", str(config), *args], check=True, capture_output=True, text=True)

    for date, title, stage in [
        ("2026-11-21", "A long episode title about interoperable clinical records and careful human review", "in-progress"),
        ("2026-11-22", "Synthetic planning conversation", "in-planning"),
        ("2026-11-20", "Synthetic published discussion", "published"),
    ]:
        eps("episode", "new", date, title)
        # Use the filesystem rather than depending on the creation report's shape.
        folder = next((workspace / "episodes/in-planning").glob(date.replace("-", ".") + "-*"))
        if stage == "published":
            eps("episode", "set", str(folder), "Video ID=dQw4w9WgXcQ")
        if stage != "in-planning":
            eps("episode", "promote", str(folder), stage, "--force")
    (workspace / "episodes/in-planning/2026.11.23-missing-record").mkdir()
    before = {p.relative_to(workspace): p.read_bytes() for p in workspace.rglob("*") if p.is_file()}
    server = subprocess.Popen([str(EPS), "--config", str(config), "gui", "--assets", str(ROOT / "gui/dist")], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        url = server.stdout.readline().strip()
        assert url.startswith("http://127.0.0.1:"), "server must print a loopback URL"
        with sync_playwright() as playwright:
            browser = playwright.chromium.launch(executable_path="/usr/bin/chromium", args=["--no-sandbox"])
            context = browser.new_context(reduced_motion="reduce")
            page = context.new_page()
            errors = []
            page.on("pageerror", lambda error: errors.append(str(error)))
            page.goto(url)
            page.get_by_role("heading", name="Synthetic production channel").wait_for()
            for width, height, name in [(1440, 900, "desktop"), (768, 1024, "tablet"), (390, 844, "phone"), (720, 450, "reflow")]:
                page.set_viewport_size({"width": width, "height": height})
                assert page.evaluate("document.documentElement.scrollWidth <= innerWidth"), f"overflow at {name}"
                page.screenshot(path=str(CAPTURES / f"{name}.png"), full_page=True)
            page.set_viewport_size({"width": 1440, "height": 900})
            page.keyboard.press("Tab")
            assert page.get_by_role("link", name="Skip to episodes").evaluate("node => node === document.activeElement")
            page.keyboard.press("Enter")
            page.get_by_role("button", name="Channel", exact=True).click()
            page.get_by_role("dialog").wait_for()
            page.get_by_text("No model profiles configured.").wait_for()
            page.keyboard.press("Escape")
            page.get_by_role("textbox", name="Find an episode").fill("published discussion")
            assert page.locator("button.episode").count() == 1
            page.locator("button.episode").click()
            page.get_by_role("heading", name="Artefacts", exact=True).wait_for()
            assert page.get_by_role("link", name="Watch on YouTube").get_attribute("href") == "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
            page.screenshot(path=str(CAPTURES / "episode.png"), full_page=True)
            page.get_by_role("button", name="Episodes", exact=True).click()
            page.get_by_role("textbox", name="Find an episode").fill("no match")
            assert page.locator("button.episode").count() == 0
            page.get_by_role("textbox", name="Find an episode").fill("")
            page.route("**/api/workspace", lambda route: route.fulfill(status=500, body="Unavailable"))
            page.get_by_role("button", name="Refresh", exact=True).click()
            page.get_by_role("alert").wait_for()
            page.screenshot(path=str(CAPTURES / "error.png"), full_page=True)
            page.unroute("**/api/workspace")
            page.route("**/api/workspace", lambda route: route.fulfill(content_type="application/json", body=json.dumps({"channel": {"name": "Empty channel", "id": "empty"}, "models": [], "episodes": [], "stages": []})))
            page.get_by_role("button", name="Refresh", exact=True).click()
            page.get_by_text("No episode folders found.", exact=False).wait_for()
            page.screenshot(path=str(CAPTURES / "empty.png"), full_page=True)
            assert not errors, errors
            browser.close()
        after = {p.relative_to(workspace): p.read_bytes() for p in workspace.rglob("*") if p.is_file()}
        assert before == after, "read-only browser workflow must not alter the workspace"
        print("Browser smoke passed; screenshots saved under .private/gui-captures/")
    finally:
        server.terminate()
        server.wait(timeout=10)
