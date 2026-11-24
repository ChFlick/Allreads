"""Regenerates the EPUB test fixtures in this folder.

Run from anywhere: python3 make_fixtures.py
Outputs: tiny.epub (simple French book), layout.epub (tricky markup and
creator roles), broken.epub (not a zip, for error handling).
"""
import pathlib
import zipfile

HERE = pathlib.Path(__file__).resolve().parent

CONTAINER = """<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"""

NAV = """<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Table des matières</title></head>
<body>
  <nav epub:type="toc"><ol>
    <li><a href="chapter1.xhtml">Chapitre un</a></li>
    <li><a href="chapter2.xhtml">Chapitre deux</a></li>
  </ol></nav>
</body>
</html>"""


def xhtml(title, body):
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>{title}</title></head>
<body>
{body}
</body>
</html>"""


def opf(title, creators, identifier):
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="id">{identifier}</dc:identifier>
    <dc:title>{title}</dc:title>
{creators}
    <dc:language>fr</dc:language>
    <meta property="dcterms:modified">2026-11-24T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2" href="chapter2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="c1"/>
    <itemref idref="c2"/>
  </spine>
</package>"""


def write_epub(name, files):
    with zipfile.ZipFile(HERE / name, "w") as z:
        z.writestr(zipfile.ZipInfo("mimetype"), "application/epub+zip",
                   compress_type=zipfile.ZIP_STORED)
        for path, body in files.items():
            z.writestr(path, body, compress_type=zipfile.ZIP_DEFLATED)


TINY_FILES = {
    "META-INF/container.xml": CONTAINER,
    "OEBPS/content.opf": opf(
        "Le Petit Test",
        "    <dc:creator>Jean Exemple</dc:creator>",
        "allreads-test-fixture",
    ),
    "OEBPS/nav.xhtml": NAV,
    "OEBPS/chapter1.xhtml": xhtml("Chapitre un", """  <h1>Chapitre un</h1>
  <p>Il était une fois  un   homme   qui lisait.</p>
  <p>Le <i>livre</i> était <b>très</b> long.</p>
  <p>   </p>
  <p>L’homme s’arrêta.</p>"""),
    "OEBPS/chapter2.xhtml": xhtml("Chapitre deux", """  <h2>Chapitre deux</h2>
  <p>La suite commence ici.</p>"""),
}

LAYOUT_FILES = {
    "META-INF/container.xml": CONTAINER,
    "OEBPS/content.opf": opf(
        "Mise en page",
        # The illustrator is listed first, so the author must win on role.
        """    <dc:creator id="c1">Mara Illustratrice</dc:creator>
    <meta refines="#c1" property="role" scheme="marc:relators">ill</meta>
    <dc:creator id="c2">Jules Auteur</dc:creator>
    <meta refines="#c2" property="role" scheme="marc:relators">aut</meta>""",
        "allreads-layout-fixture",
    ),
    "OEBPS/nav.xhtml": NAV,
    "OEBPS/chapter1.xhtml": xhtml("Mise en page", """  Texte nu.
  <div class="chap">
    <div><p>Premier paragraphe &amp; co.</p></div>
    <div>Un<br/>deux</div>
    <script>var x = 1;</script>
    <style>p { color: red; }</style>
    <p>Il dit&#160;: oui.</p>
    <blockquote><p>Citation</p></blockquote>
    <ul><li>Un</li><li>Deux <i>trois</i></li></ul>
    <h3>   </h3>
  </div>"""),
    "OEBPS/chapter2.xhtml": xhtml("Fin", "  <div>Fin.</div>"),
}

if __name__ == "__main__":
    write_epub("tiny.epub", TINY_FILES)
    write_epub("layout.epub", LAYOUT_FILES)
    (HERE / "broken.epub").write_text("this is not a zip archive\n", encoding="utf-8")
    print("fixtures written to", HERE)
