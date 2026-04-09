import pytest
from pathlib import Path

@pytest.fixture(scope="session")
def spec_html_content():
    path = (
        Path(__file__).resolve().parents[4]
        / "benches"
        / "bench_data"
        / "html.spec.whatwg.org.html"
    )
    if path.exists():
        return path.read_text()
    pytest.skip(f"Spec file not found at {path}")

@pytest.fixture(scope="session")
def html_content():
    element_number = 10_000
    return f"""<html><body>{"".join(f"<div class='container'><a href='#'>Link {i}</a></div>" for i in range(element_number))}</body></html>"""
