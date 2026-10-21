#!/usr/bin/env python3
"""Generate and validate llms.txt against a Docusaurus site's built pages.

Uses only Python's standard library. The homepage's canonical URL and title
identify the site; sitemap.xml supplies the public routes. No source document
names, sidebar assumptions, or external network requests are required.
"""

from __future__ import annotations

import argparse
from html.parser import HTMLParser
from pathlib import Path
import re
import sys
from urllib.parse import unquote, urljoin, urlsplit
import xml.etree.ElementTree as ET


class Page(HTMLParser):
    def __init__(self, path: Path):
        super().__init__(convert_charrefs=True)
        self.title = ""
        self.canonical = ""
        self.description = ""
        self.ids: set[str] = set()
        self.links: list[str] = []
        self.in_title = False
        self.feed(path.read_text(encoding="utf-8"))

    def handle_starttag(self, tag, attributes):
        attrs = dict(attributes)
        if attrs.get("id"):
            self.ids.add(attrs["id"])
        if tag == "title":
            self.in_title = True
        elif tag == "link" and "canonical" in attrs.get("rel", "").split():
            self.canonical = attrs.get("href", "")
        elif tag == "meta" and attrs.get("name") == "description":
            self.description = attrs.get("content", "")
        elif tag == "a" and attrs.get("href"):
            self.links.append(attrs["href"])
            if attrs.get("name"):
                self.ids.add(attrs["name"])

    def handle_endtag(self, tag):
        if tag == "title":
            self.in_title = False

    def handle_data(self, data):
        if self.in_title:
            self.title += data


def page_path(site: Path, base: str, url: str) -> Path:
    root = urlsplit(base)
    target = urlsplit(url)
    if (target.scheme, target.netloc) != (root.scheme, root.netloc):
        raise ValueError(f"URL belongs to another origin: {url}")
    base_path = unquote(root.path).rstrip("/")
    path = unquote(target.path)
    if path != base_path and not path.startswith(base_path + "/"):
        raise ValueError(f"URL is outside the docs base: {url}")
    relative = path[len(base_path):].lstrip("/")
    if ".." in Path(relative).parts or "\\" in relative:
        raise ValueError(f"Unsafe route: {url}")
    candidates = [site / relative, site / relative / "index.html"]
    if relative:
        candidates.append(site / (relative.rstrip("/") + ".html"))
    for candidate in candidates:
        if candidate.is_file() and candidate.resolve().is_relative_to(site.resolve()):
            return candidate
    raise ValueError(f"Missing built page or asset: {url}")


def clean_text(text: str) -> str:
    return " ".join(text.split())


def markdown_label(text: str) -> str:
    return clean_text(text).replace("\\", "\\\\").replace("[", "\\[").replace("]", "\\]")


def validate_site(site: Path, write_index: bool = False) -> list[str]:
    errors: list[str] = []
    try:
        home = Page(site / "index.html")
        base = home.canonical.rstrip("/") + "/"
        parsed_base = urlsplit(base)
        if parsed_base.scheme not in {"https", "http"} or not parsed_base.netloc:
            raise ValueError("Homepage needs an absolute HTTP(S) canonical URL")
        title = clean_text(home.title.rsplit(" | ", 1)[-1])
        if not title or title.lower() in {"undefined", "null"}:
            raise ValueError("Homepage needs a defined site title")
        tree = ET.parse(site / "sitemap.xml")
        urls = [clean_text(node.text or "") for node in tree.findall(".//{*}loc")]
        if not urls or any(not url for url in urls):
            raise ValueError("Sitemap must contain nonempty document URLs")
        if len(urls) != len(set(urls)):
            raise ValueError("Sitemap contains duplicate URLs")
    except (OSError, ValueError, ET.ParseError) as exc:
        return [f"{site}: {exc}"]

    pages: dict[Path, Page] = {}
    entries: list[tuple[str, str]] = []
    for url in sorted(urls):
        try:
            path = page_path(site, base, url)
            page = pages.setdefault(path, Page(path))
            if not page.title.strip():
                raise ValueError(f"Missing page title: {url}")
            if page.canonical.rstrip("/") != url.rstrip("/"):
                raise ValueError(f"Canonical URL disagrees with sitemap: {url}")
            label = page.title.removesuffix(" | " + title)
            entries.append((markdown_label(label), url))
        except (OSError, ValueError) as exc:
            errors.append(str(exc))

    # Validate rendered links, including same-page and cross-page fragments.
    # Other project sites and external services remain outside this check.
    for path, page in list(pages.items()):
        for href in page.links:
            resolved = urljoin(page.canonical, href)
            target = urlsplit(resolved)
            root = urlsplit(base)
            if (target.scheme, target.netloc) != (root.scheme, root.netloc):
                continue
            base_path = unquote(root.path).rstrip("/")
            target_path = unquote(target.path)
            if target_path != base_path and not target_path.startswith(base_path + "/"):
                continue
            try:
                destination = page_path(site, base, resolved)
                if target.fragment and destination.suffix == ".html":
                    target_page = pages.get(destination)
                    if target_page is None:
                        target_page = Page(destination)
                        pages[destination] = target_page
                    if unquote(target.fragment) not in target_page.ids:
                        raise ValueError(f"Missing fragment: {resolved}")
            except (OSError, ValueError) as exc:
                errors.append(f"{path.relative_to(site)} -> {href}: {exc}")

    index = site / "llms.txt"
    if write_index and not errors:
        description = clean_text(home.description)
        header = f"# {title}\n\n" + (f"> {description}\n\n" if description else "")
        content = header + "## Documentation\n\n"
        content += "".join(f"- [{label}]({url})\n" for label, url in entries)
        index.write_text(content, encoding="utf-8")
    if not errors:
        try:
            text = index.read_text(encoding="utf-8")
            if text.splitlines()[0] != f"# {title}":
                raise ValueError("llms.txt title does not match the built site")
            links = re.findall(r"^- \[(?:\\.|[^\]])+\]\(([^)]*)\)$", text, re.MULTILINE)
            if len(links) != len(urls) or set(links) != set(urls):
                raise ValueError("llms.txt must link exactly once to every sitemap page")
        except (OSError, ValueError, IndexError) as exc:
            errors.append(f"{index}: {exc}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("build_dir", type=Path)
    parser.add_argument("--write-index", action="store_true")
    args = parser.parse_args()
    sitemaps = sorted(args.build_dir.rglob("sitemap.xml"))
    if not sitemaps:
        print(f"No sitemap.xml found under {args.build_dir}", file=sys.stderr)
        return 1
    errors = []
    for sitemap in sitemaps:
        problems = validate_site(sitemap.parent, args.write_index)
        errors.extend(problems)
        if not problems:
            print(f"Validated docs pages, links, fragments, and llms.txt: {sitemap.parent}")
    for error in errors:
        print(error, file=sys.stderr)
    return bool(errors)


if __name__ == "__main__":
    raise SystemExit(main())
