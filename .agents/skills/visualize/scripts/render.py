from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass
from html import escape
from html.parser import HTMLParser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit

_SKILL_DIRECTORY = Path(__file__).resolve().parents[1]
_FRAGMENT_PLACEHOLDER = "<!--__INLINE_VISUALIZATION_FRAGMENT__-->"
_MAX_FRAGMENT_BYTES = 1_000_000
_DOCUMENT_MARKUP = re.compile(r"<!doctype\s|<\s*(?:html|head|body)(?:\s|>)", re.IGNORECASE)
_HOST_API_REFERENCE = re.compile(r"\b(?:window|globalThis)\.openai\b")
_RESOURCE_SOURCES = " ".join(
    (
        "blob:",
        "data:",
        "https://cdnjs.cloudflare.com",
        "https://cdn.jsdelivr.net",
        "https://esm.sh",
        "https://fonts.bunny.net",
        "https://fonts.googleapis.com",
        "https://fonts.gstatic.com",
        "https://unpkg.com",
    ),
)
_FRAME_CSP = "; ".join(
    (
        "default-src 'none'",
        f"script-src 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval' {_RESOURCE_SOURCES}",
        f"style-src 'unsafe-inline' {_RESOURCE_SOURCES}",
        f"img-src {_RESOURCE_SOURCES}",
        f"font-src {_RESOURCE_SOURCES}",
        f"media-src {_RESOURCE_SOURCES}",
        "worker-src blob:",
        "connect-src blob: data:",
        "frame-src 'none'",
        "object-src 'none'",
        "base-uri 'none'",
        "form-action 'none'",
    ),
)
# A srcdoc frame inherits the shell CSP, so the shell must permit resources
# which the stricter inner frame policy may load.
_SHELL_CSP = _FRAME_CSP.replace("frame-src 'none'", "frame-src 'self'")


def _read_fragment(source: Path) -> str:
    if source.is_symlink() or not source.is_file():
        raise ValueError(f"source must be a regular file: {source}")
    if source.stat().st_size > _MAX_FRAGMENT_BYTES:
        raise ValueError("source fragment exceeds the 1 MB size limit")
    fragment = source.read_text(encoding="utf-8")
    if _DOCUMENT_MARKUP.search(fragment):
        raise ValueError("source must be an HTML fragment, not a complete document")
    return fragment


def _default_title(source: Path) -> str:
    words = source.stem.replace("-", " ").replace("_", " ").split()
    return " ".join(word.capitalize() for word in words) or "Visualization"


def calendar_script(fragment: str) -> str:
    """Register the bundled element before a fragment uses it in HTML or JavaScript."""
    if re.search(r"\bviz-calendar\b", fragment, re.IGNORECASE) is None:
        return ""
    source = (_SKILL_DIRECTORY / "assets" / "calendar.js").read_text(encoding="utf-8")
    return "<script>" + source.replace("</script", "<\\/script") + "</script>"


@dataclass(frozen=True)
class _ShareSource:
    title: str
    provider: str | None = None
    type: str | None = None
    date: str | None = None
    description: str | None = None
    url: str | None = None


@dataclass(frozen=True)
class _ShareMetadata:
    version: int
    authorName: str | None
    sources: list[_ShareSource]


class _ShareMetadataParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.blocks: list[str] = []
        self.active = False

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        values = dict(attrs)
        if (
            tag == "script"
            and values.get("id") == "visualization-share-metadata"
            and values.get("type") == "application/json"
        ):
            self.blocks.append("")
            self.active = True

    def handle_endtag(self, tag: str) -> None:
        if tag == "script":
            self.active = False

    def handle_data(self, data: str) -> None:
        if self.active:
            self.blocks[-1] += data


def _read_share_metadata(fragment: str, metadata_path: Path | None) -> _ShareMetadata:
    parser = _ShareMetadataParser()
    parser.feed(fragment)
    if len(parser.blocks) > 1:
        raise ValueError("a visualization must have at most one share metadata block")
    if metadata_path is not None:
        if (
            metadata_path.is_symlink()
            or not metadata_path.is_file()
            or metadata_path.stat().st_size > 64 * 1024
        ):
            raise ValueError("share metadata must be a regular JSON file under 64 KiB")
        raw = metadata_path.read_text(encoding="utf-8")
    else:
        raw = parser.blocks[0] if parser.blocks else '{"version":1}'
    if len(raw.encode("utf-8")) > 64 * 1024:
        raise ValueError("share metadata exceeds 64 KiB")
    metadata = json.loads(raw)
    if (
        not isinstance(metadata, dict)
        or type(metadata.get("version")) is not int
        or metadata["version"] != 1
    ):
        raise ValueError("share metadata must be an object with version 1")
    author = metadata.get("authorName")
    if "authorName" in metadata:
        if not isinstance(author, str) or not author.strip() or len(author.strip()) > 200:
            raise ValueError("authorName must be a nonempty string under 200 characters")
        author = author.strip()
    sources = metadata.get("sources", [])
    if not isinstance(sources, list) or len(sources) > 100:
        raise ValueError("sources must be an array with at most 100 entries")
    checked_sources: list[_ShareSource] = []
    for source in sources:
        if (
            not isinstance(source, dict)
            or not isinstance(source.get("title"), str)
            or not source["title"].strip()
        ):
            raise ValueError("every source needs its actual title")
        for key, limit in (
            ("title", 300),
            ("provider", 120),
            ("type", 120),
            ("date", 120),
            ("description", 5000),
            ("url", 2000),
        ):
            if key not in source:
                continue
            value = source[key]
            if not isinstance(value, str) or len(value.strip()) > limit:
                raise ValueError(f"source {key} must be a string under {limit} characters")
            source[key] = value.strip()
        if "url" in source:
            url = urlsplit(source["url"])
            if url.scheme != "https" or not url.hostname or url.username or url.password:
                raise ValueError("source URLs must use HTTPS without credentials")
        checked_sources.append(
            _ShareSource(
                title=source["title"],
                provider=source.get("provider"),
                type=source.get("type"),
                date=source.get("date"),
                description=source.get("description"),
                url=source.get("url"),
            )
        )
    return _ShareMetadata(version=1, authorName=author, sources=checked_sources)


def _share_assets(metadata: _ShareMetadata | None, stylesheet: str) -> tuple[str, str]:
    if metadata is None:
        return "", ""
    assets = _SKILL_DIRECTORY / "assets"
    styles = (assets / "share-shell.css").read_text(encoding="utf-8")
    runtime = (
        (assets / "share-shell.js").read_text(encoding="utf-8").replace("</script", r"<\/script")
    )
    configuration = json.dumps(
        {
            "metadata": asdict(metadata),
            "labels": {
                "madeBy": "Made by {author} using {visualize}",
                "madeWith": "Made using {visualize}",
                "sources": "Sources",
                "backToChart": "Back to chart",
                "openSource": "Open source",
            },
        }
    ).replace("<", r"\u003c")
    return (
        f"<style>{stylesheet}\n{styles}</style>",
        f'<script type="application/json" id="visualization-share">{configuration}</script>\n<script>{runtime}</script>',
    )


def _render_document(fragment: str, title: str, metadata: _ShareMetadata | None = None) -> str:
    stylesheet = (_SKILL_DIRECTORY / "assets" / "visualize.css").read_text(
        encoding="utf-8",
    )
    inner_kit = (_SKILL_DIRECTORY / "assets" / "visualize.html").read_text(
        encoding="utf-8",
    )
    host_bridge = (
        (_SKILL_DIRECTORY / "assets" / "standalone-host-bridge.js")
        .read_text(
            encoding="utf-8",
        )
        .replace("</script", r"<\/script")
    )
    shell = (
        (_SKILL_DIRECTORY / "assets" / "standalone-shell.js")
        .read_text(
            encoding="utf-8",
        )
        .replace("</script", r"<\/script")
    )
    inner_html = inner_kit.replace(_FRAGMENT_PLACEHOLDER, calendar_script(fragment) + fragment)
    share_styles, share_runtime = _share_assets(metadata, stylesheet)
    document_title = escape(title)
    frame_html = f"""<!doctype html>
<html lang="en" data-visualize-standalone>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="referrer" content="no-referrer">
<meta http-equiv="Content-Security-Policy" content="{_FRAME_CSP}">
<title>{document_title}</title>
<style>{stylesheet}
html>body{{padding:0}}</style>
</head>
<body>
<script type="application/json" id="codex-visualization-widget-state">__CODEX_VISUALIZATION_WIDGET_STATE__</script>
<script>{host_bridge}</script>
{inner_html}
</body>
</html>
"""
    return f"""<!doctype html>
<html lang="en" data-visualize-standalone>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="referrer" content="no-referrer">
<meta http-equiv="Content-Security-Policy" content="{_SHELL_CSP}">
<title>{document_title}</title>
<style>:root{{color-scheme:light dark;background:light-dark(rgb(255 255 255), rgb(24 24 24))}}html,body{{margin:0}}body{{box-sizing:border-box;padding:1rem;background:inherit}}iframe{{display:block;width:100%;height:calc(100vh - 2rem);margin:0 auto;border:0}}</style>
{share_styles}
</head>
<body>
<iframe id="codex-visualization" sandbox="allow-scripts" referrerpolicy="no-referrer" title="{document_title}" data-srcdoc="{escape(frame_html)}"></iframe>
{share_runtime}
<script>{shell}</script>
</body>
</html>
"""


def render(
    fragment_path: Path,
    title: str | None = None,
    *,
    share: bool = False,
    metadata_path: Path | None = None,
) -> str:
    fragment = _read_fragment(fragment_path)
    metadata = _read_share_metadata(fragment, metadata_path) if share else None
    return _render_document(fragment, title or _default_title(fragment_path), metadata)


def export_html(
    source: Path,
    destination: Path,
    *,
    title: str | None = None,
    force: bool = False,
    share: bool = False,
    metadata_path: Path | None = None,
) -> bool:
    if destination.is_symlink():
        raise ValueError(f"destination cannot be a symbolic link: {destination}")
    if source.resolve() == destination.resolve() or (
        destination.exists() and source.samefile(destination)
    ):
        raise ValueError("source and destination must be different files")

    fragment = _read_fragment(source)
    metadata = _read_share_metadata(fragment, metadata_path) if share else None
    document = _render_document(fragment, title or _default_title(source), metadata)
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("w" if force else "x", encoding="utf-8", newline="\n") as output:
        output.write(document)
    return _HOST_API_REFERENCE.search(fragment) is not None


def serve(document: str, port: int) -> None:
    encoded_document = document.encode("utf-8")

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            if self.path not in ("/", "/index.html"):
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(encoded_document)))
            self.end_headers()
            self.wfile.write(encoded_document)

        def log_message(self, format: str, *args: object) -> None:
            pass

    server = ThreadingHTTPServer(("127.0.0.1", port), Handler)
    print(f"http://127.0.0.1:{server.server_port}/", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Render an inline visualization fragment as standalone HTML.",
    )
    parser.add_argument("fragment", type=Path, help="absolute fragment HTML path")
    parser.add_argument(
        "destination",
        type=Path,
        nargs="?",
        help="optional output HTML path",
    )
    parser.add_argument(
        "--title",
        help="document title; defaults to the fragment file name",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="replace the destination if it already exists",
    )
    parser.add_argument(
        "--serve",
        action="store_true",
        help="serve the rendered visualization locally",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=0,
        help="local serve port; defaults to any free port",
    )
    parser.add_argument(
        "--share",
        action="store_true",
        help="add the shared-page card, attribution, and Sources view",
    )
    parser.add_argument(
        "--metadata",
        type=Path,
        help="optional version 1 share metadata JSON; otherwise read the fragment's explicit metadata block",
    )
    args = parser.parse_args(argv)
    if args.metadata is not None and not args.share:
        parser.error("--metadata requires --share")
    if args.serve and args.destination is not None:
        parser.error("destination cannot be used with --serve")
    if args.destination is not None and args.destination.exists() and not args.force:
        parser.error("destination already exists; pass --force to replace it")

    try:
        if args.destination is not None:
            references_host_api = export_html(
                args.fragment,
                args.destination,
                title=args.title,
                force=args.force,
                share=args.share,
                metadata_path=args.metadata,
            )
            if references_host_api:
                print(
                    "warning: the source references window.openai; verify APIs beyond "
                    "widgetState/setWidgetState before hosting",
                    file=sys.stderr,
                )
            print(args.destination)
        else:
            document = render(
                args.fragment, args.title, share=args.share, metadata_path=args.metadata
            )
            if args.serve:
                serve(document, args.port)
            else:
                sys.stdout.write(document)
    except (OSError, UnicodeError, ValueError) as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
