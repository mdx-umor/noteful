"""Packs the wasm-bindgen output into one HTML file (wasm gzipped + base64)."""
import base64, gzip, sys, pathlib

pkg, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
js = (pkg / "notebook.js").read_text()
wasm = gzip.compress((pkg / "notebook_bg.wasm").read_bytes(), 9)
b64 = base64.b64encode(wasm).decode()
template = sys.argv[3] if len(sys.argv) > 3 else "template.htm"
html = (pathlib.Path(__file__).parent / template).read_text()
runtime = (pathlib.Path(__file__).parent / "runtime.js").read_text()
html = html.replace("/*RUNTIME*/", runtime).replace("/*GLUE*/", js).replace("WASM_B64", b64)
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(html)
print(f"wrote {out} ({out.stat().st_size / 1e6:.1f} MB)")
