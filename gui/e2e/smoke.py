#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Exercise the built workbench against an isolated synthetic channel."""

import json
import subprocess
import tempfile
import urllib.request
from pathlib import Path

from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[2]
EPS = ROOT / "target/debug/eps"
CAPTURES = ROOT / ".private/gui-captures"
CAPTURES.mkdir(parents=True, exist_ok=True)

with tempfile.TemporaryDirectory(prefix="eps-gui-") as temporary:
    workspace = Path(temporary)
    config = workspace / "eps.toml"
    config.write_text("""[channel]
id = 'synthetic'
name = 'Synthetic production channel'
timezone = 'Europe/London'

[youtube]
channel_id = 'UCsyntheticsyntheticsynt'

[recording.vdo_ninja]
guest_url = 'https://vdo.ninja/?room=synthetic-room&push=guest'

[archive.forum]
topic_url = 'https://forum.invalid/t/archive/1'

[metadata.social.bluesky-draft]
max_characters = 300
""")

    def image(path, size, colours):
        path.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["magick", "-size", size, f"gradient:{colours}", str(path)], check=True)

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
            draft = workspace / "draft.txt"
            draft.write_text("A synthetic conversation about careful review. #synthetic\nhttps://www.youtube.com/watch?v=dQw4w9WgXcQ\n")
            eps("episode", "set-draft", str(folder), "Bluesky", "--from-file", str(draft))
            draft.unlink()
        if stage != "in-planning":
            base = folder.name
            image(folder / f"{base}.thumbnail.png", "1280x720", "#09695d-#1f2933")
            for index, colours in enumerate(["#285c89-#f7f8f8", "#855600-#fff4dc", "#a52f35-#fceef0"], 1):
                image(folder / f"{base}.frames" / f"frame-{index:02}.jpg", "640x360", colours)
        if stage != "in-planning":
            eps("episode", "promote", str(folder), stage, "--force")
    (workspace / "episodes/in-planning/2026.11.23-missing-record").mkdir()
    before = {p.relative_to(workspace): p.read_bytes() for p in workspace.rglob("*") if p.is_file()}
    server = subprocess.Popen([str(EPS), "--config", str(config), "gui", "--assets", str(ROOT / "gui/dist")], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        url = server.stdout.readline().strip()
        assert url.startswith("http://127.0.0.1:"), "server must print a loopback URL"
        origin, token = url.split("#", 1)

        def current_fingerprint():
            request = urllib.request.Request(origin + "api/fingerprint", headers={"Authorization": f"Bearer {token}"})
            with urllib.request.urlopen(request) as response:
                return json.load(response)["fingerprint"]
        with sync_playwright() as playwright:
            browser = playwright.chromium.launch(executable_path="/usr/bin/chromium", args=["--no-sandbox"])
            context = browser.new_context(reduced_motion="reduce")
            page = context.new_page()
            errors = []
            page.on("pageerror", lambda error: errors.append(str(error)))
            page.goto(url)
            page.get_by_role("heading", name="Synthetic production channel").wait_for()
            # Thumbnails load through the token and must actually decode.
            page.wait_for_function("() => document.querySelectorAll('article.episode .card-thumbnail img').length === 2")
            page.wait_for_function("() => [...document.querySelectorAll('article.episode img')].every(img => img.complete && img.naturalWidth > 0)")
            card = page.locator("article.episode", has_text="Synthetic published discussion")
            assert card.locator("a[data-kind=youtube_watch]").get_attribute("href") == "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
            assert card.locator("a[data-kind=youtube_studio]").get_attribute("href") == "https://studio.youtube.com/video/dQw4w9WgXcQ/edit"
            assert page.get_by_role("link", name="Channel in YouTube Studio").get_attribute("href") == "https://studio.youtube.com/channel/UCsyntheticsyntheticsynt"
            assert page.locator(".milestone-strip").count() == 3

            def assert_colour_contrast():
                failures = page.evaluate("""() => {
                    const rgb = value => value.match(/[\\d.]+/g).slice(0, 3).map(Number);
                    const luminance = values => values.map(v => {
                        v /= 255;
                        return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
                    }).reduce((sum, v, i) => sum + v * [0.2126, 0.7152, 0.0722][i], 0);
                    const failures = [];
                    for (const node of document.querySelectorAll('.stage h3, .status, .muted, .brand, footer, input')) {
                        const style = getComputedStyle(node);
                        let background = style.backgroundColor;
                        let parent = node.parentElement;
                        while (background === 'rgba(0, 0, 0, 0)' && parent) {
                            background = getComputedStyle(parent).backgroundColor;
                            parent = parent.parentElement;
                        }
                        const colours = [style.color];
                        if (node.matches('input')) colours.push(getComputedStyle(node, '::placeholder').color);
                        for (const colour of colours) {
                            const a = luminance(rgb(colour)), b = luminance(rgb(background));
                            const ratio = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
                            if (ratio < 4.5) failures.push(`${node.textContent || 'input'}: ${ratio.toFixed(2)}`);
                        }
                    }
                    return failures;
                }""")
                assert not failures, f"Text contrast below 4.5:1: {failures}"

            assert_colour_contrast()
            for width, height, name in [(1440, 900, "desktop"), (768, 1024, "tablet"), (390, 844, "phone"), (720, 450, "reflow")]:
                page.set_viewport_size({"width": width, "height": height})
                assert page.evaluate("document.documentElement.scrollWidth <= innerWidth"), f"overflow at {name}"
                page.screenshot(path=str(CAPTURES / f"{name}.png"), full_page=True)
            page.set_viewport_size({"width": 1440, "height": 900})
            page.locator("article.episode").first.hover()
            assert_colour_contrast()
            page.screenshot(path=str(CAPTURES / "hover.png"), full_page=True)
            page.mouse.move(0, 0)
            cdp = context.new_cdp_session(page)
            for deficiency in ["deuteranopia", "protanopia", "tritanopia", "achromatopsia"]:
                cdp.send("Emulation.setEmulatedVisionDeficiency", {"type": deficiency})
                page.screenshot(path=str(CAPTURES / f"{deficiency}.png"), full_page=True)
            cdp.send("Emulation.setEmulatedVisionDeficiency", {"type": "none"})
            page.keyboard.press("Tab")
            assert page.get_by_role("link", name="Skip to episodes").evaluate("node => node === document.activeElement")
            page.keyboard.press("Enter")
            page.get_by_role("button", name="Channel", exact=True).click()
            page.get_by_role("dialog").wait_for()
            page.get_by_text("No model profiles configured.").wait_for()
            page.keyboard.press("Escape")
            page.get_by_role("textbox", name="Find an episode").fill("published discussion")
            assert page.locator("article.episode").count() == 1
            page.locator(".card-open").click()
            page.get_by_role("heading", name="Artefacts", exact=True).wait_for()
            assert page.get_by_role("link", name="Watch on YouTube").first.get_attribute("href") == "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
            assert page.locator(".milestone-list li").count() == 11
            page.locator(".frames").scroll_into_view_if_needed()
            page.wait_for_function("() => document.querySelectorAll('.frames img').length === 3 && [...document.querySelectorAll('.frames img, .thumbnail-large img')].every(img => img.naturalWidth > 0)")
            compose = page.get_by_role("link", name="Compose on Bluesky")
            assert compose.get_attribute("href").startswith("https://bsky.app/intent/compose?text=A%20synthetic%20conversation")
            page.get_by_text("/ 300 characters", exact=False).wait_for()
            assert_colour_contrast()
            page.screenshot(path=str(CAPTURES / "episode.png"), full_page=True)
            page.set_viewport_size({"width": 390, "height": 844})
            assert page.evaluate("document.documentElement.scrollWidth <= innerWidth"), "episode overflows on a phone"
            page.screenshot(path=str(CAPTURES / "episode-phone.png"), full_page=True)
            page.set_viewport_size({"width": 1440, "height": 900})
            page.get_by_role("button", name="Enlarge frame-02.jpg").click()
            page.get_by_role("dialog").locator("img").wait_for()
            page.keyboard.press("Escape")
            page.get_by_role("button", name="Preview Show notes").click()
            page.get_by_role("dialog").locator("pre.preview").wait_for()
            page.screenshot(path=str(CAPTURES / "preview.png"))
            page.keyboard.press("Escape")
            page.get_by_role("button", name="Episodes", exact=True).click()
            page.get_by_role("textbox", name="Find an episode").fill("no match")
            assert page.locator("article.episode").count() == 0
            page.get_by_role("textbox", name="Find an episode").fill("")

            # Everything above was read-only. Now change the workspace from
            # outside the browser and expect the board to follow.
            after = {p.relative_to(workspace): p.read_bytes() for p in workspace.rglob("*") if p.is_file()}
            assert before == after, "read-only browser workflow must not alter the workspace"
            eps("episode", "new", "2026-11-24", "Arrived while watching")
            page.get_by_role("button", name="Arrived while watching").wait_for(timeout=15000)
            page.route("**/api/workspace", lambda route: route.fulfill(status=500, body="Unavailable"))
            page.get_by_role("button", name="Refresh", exact=True).click()
            page.get_by_role("alert").wait_for()
            page.screenshot(path=str(CAPTURES / "error.png"), full_page=True)
            page.unroute("**/api/workspace")
            page.route("**/api/workspace", lambda route: route.fulfill(content_type="application/json", body=json.dumps({"channel": {"name": "Empty channel", "id": "empty"}, "channel_links": [], "models": [], "episodes": [], "stages": [], "fingerprint": current_fingerprint()})))
            page.get_by_role("button", name="Refresh", exact=True).click()
            page.get_by_text("No episode folders found.", exact=False).wait_for()
            page.screenshot(path=str(CAPTURES / "empty.png"), full_page=True)
            assert not errors, errors
            browser.close()
        print("Browser smoke passed; screenshots saved under .private/gui-captures/")
    finally:
        server.terminate()
        server.wait(timeout=10)
