import pytest
from utils.parsers import get_available_parsers

PARSERS = get_available_parsers()

@pytest.mark.parametrize("name", PARSERS.keys())
def test_spec_benchmark(benchmark, spec_html_content, name):
    lib = PARSERS[name]
    benchmark.group = "WhatWG HTML spec"
    benchmark.name = name

    # result = benchmark.pedantic(lib, args=(spec_html_content, "a"), rounds=25, iterations=1)
    result = benchmark(lib, spec_html_content, "a")

    assert result is not None
