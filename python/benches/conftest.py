import pytest
import os

@pytest.fixture(scope="session")
def spec_html_content():
    path = "/home/zmm/Music/html.spec.whatwg.index.html"
    if os.path.exists(path):
        with open(path, 'r') as f:
            return f.read()
    pytest.skip(f"Spec file not found at {path}")

@pytest.fixture(scope="session")
def html_content():
    file_path = os.getenv("BENCHMARK_HTML_FILE")
    if file_path and os.path.exists(file_path):
        with open(file_path, 'r') as f:
            return f.read()
            
    element_number = 5000
    return f"""<html><body>{"".join(f"<div class='container'><a href='#'>Link {i}</a></div>" for i in range(element_number))}</body></html>"""
