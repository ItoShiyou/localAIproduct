"""Read-only checks for the minutes static product pages; no dependencies."""
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parent


class Page(HTMLParser):
    def __init__(self, path):
        super().__init__()
        self.path, self.ids, self.refs = path, set(), []
        self.h1, self.preview = 0, False
        self.feed(path.read_text(encoding="utf-8"))

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if "id" in attrs:
            assert attrs["id"] not in self.ids, f"Duplicate id: {self.path}"
            self.ids.add(attrs["id"])
        self.h1 += tag == "h1"
        if tag == "meta" and attrs.get("name") == "robots":
            self.preview = "noindex" in attrs.get("content", "")
        for key in ("href", "src"):
            if key in attrs:
                self.refs.append(attrs[key])


pages = [Page(path) for path in ROOT.rglob("*.html")]
for page in pages:
    assert page.h1 == 1 and not page.preview, f"Heading/publication metadata: {page.path}"
    for ref in page.refs:
        url = urlsplit(ref)
        if url.scheme or url.netloc:
            continue
        target = page.path if not url.path else page.path.parent / unquote(url.path)
        if target.is_dir():
            target = target / "index.html"
        assert target.is_file(), f"Broken resource: {page.path}: {ref}"
        if url.fragment and target.suffix == ".html":
            assert url.fragment in Page(target).ids, f"Broken anchor: {ref}"
    print(f"PASS {page.path.relative_to(ROOT)}")
print(f"Verified {len(pages)} pages; local resources, anchors, h1, publication metadata.")
