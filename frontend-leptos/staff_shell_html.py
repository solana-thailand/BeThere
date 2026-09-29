"""Write staff-shell.html, the Trunk target of the staff build (.issues/169).

It is index.html with the stylesheet links replaced by every file in styles/,
in sorted order. index.html links only the attendee sheets; the
`style-NN-*.staff.css` files hold rules that match nothing the attendee app
renders. The numeric prefix is the cascade order, and a `.staff.css` file
sorts right after the sheet it was split from, so the staff shell sees the
same cascade as before the split. `tests/staff_shell_split.rs` pins
index.html's list to the sorted non-staff files.

Usage: python3 staff_shell_html.py   (writes staff-shell.html beside index.html)
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent
LINK_RE = re.compile(r'^([ \t]*)<link data-trunk rel="css" href="styles/[^"]+" />\n', re.M)


def main() -> int:
    html = (ROOT / "index.html").read_text()
    links = list(LINK_RE.finditer(html))
    if not links:
        print("staff_shell_html: no stylesheet links in index.html", file=sys.stderr)
        return 1
    first, last = links[0], links[-1]
    between = html[first.start() : last.end()]
    if len(LINK_RE.findall(between)) != len(links) or LINK_RE.sub("", between).strip():
        print("staff_shell_html: the stylesheet links in index.html must be one block", file=sys.stderr)
        return 1
    indent = first.group(1)
    sheets = sorted(p.name for p in (ROOT / "styles").glob("style-*.css"))
    block = "".join(f'{indent}<link data-trunk rel="css" href="styles/{s}" />\n' for s in sheets)
    (ROOT / "staff-shell.html").write_text(html[: first.start()] + block + html[last.end() :])
    print(f"🧩 staff-shell.html: {len(sheets)} stylesheets ({len(sheets) - len(links)} staff-only)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
