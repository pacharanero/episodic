# Browser smoke test

Prerequisites: the Python `playwright` package and Chromium at `/usr/bin/chromium`. No production workspace or credentials are used. The test creates a temporary synthetic channel and starts the debug binary, then stops it and removes the fixture. Screenshots remain under the ignored `.private/gui-captures/` directory.

```bash
cargo build
npm --prefix gui run build
python gui/e2e/smoke.py
```

Coverage: populated board, long titles, missing record, episode findings, safe recorded-video links, channel modal, filtering, error with stale-data warning, keyboard entry and modal dismissal, reduced motion, phone/tablet/desktop and a half-width reflow viewport. The workspace file bytes must remain unchanged. The half-width check is a 200% reflow proxy, not a claim of manual browser-zoom or full WCAG conformance. Embedded packaging and real-workspace acceptance remain R33 work.
