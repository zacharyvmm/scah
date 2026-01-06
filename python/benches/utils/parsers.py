import os
import re
from html.parser import HTMLParser

try:
    from bs4 import BeautifulSoup
except ImportError:
    BeautifulSoup = None

try:
    import lxml.html
    from lxml import etree
except ImportError:
    lxml = None

try:
    from selectolax.parser import HTMLParser as SelectolaxParser
except ImportError:
    SelectolaxParser = None

try:
    from parsel import Selector
except ImportError:
    Selector = None

try:
    from gazpacho import Soup as GazpachoSoup
except ImportError:
    GazpachoSoup = None

try:
    import onego
except ImportError:
    onego = None

def parse_bs4_htmlparser(html:str, query:str):
    assert(BeautifulSoup)
    soup = BeautifulSoup(html, "html.parser")
    return soup.find_all(query)

def parse_bs4_lxml(html:str, query:str):
    assert(BeautifulSoup)
    assert(lxml)
    soup = BeautifulSoup(html, "lxml")
    return soup.find_all(query)

def parse_selectolax(html:str, query:str):
    assert(SelectolaxParser)
    tree = SelectolaxParser(html)
    return tree.css(query)

def parse_parsel(html:str, query:str):
    assert(Selector)
    selector = Selector(text=html)
    return selector.css(query)

def parse_gazpacho(html:str, query:str):
    assert(GazpachoSoup)
    soup = GazpachoSoup(html)
    return soup.find(query)

def parse_onego(html:str, query:str):
    assert(onego)
    result = onego.parse(html, {query: {}})
    try:
        return result['children'][query]
    except (KeyError, TypeError):
        return []


PARSERS = {
    "BS4 (html.parser)": parse_bs4_htmlparser,
    "BS4 (lxml)": parse_bs4_lxml,
    "Selectolax": parse_selectolax,
    "Parsel": parse_parsel,
    "Gazpacho": parse_gazpacho,
    "OneGo": parse_onego,
}

def get_available_parsers():
    available = {}
    for name, func in PARSERS.items():
        is_avail = True
        if "BS4" in name and BeautifulSoup is None: is_avail = False
        if "lxml" in name and lxml is None: is_avail = False
        if "Selectolax" in name and SelectolaxParser is None: is_avail = False
        if "Parsel" in name and Selector is None: is_avail = False
        if "Gazpacho" in name and GazpachoSoup is None: is_avail = False
        if "OneGo" in name and onego is None: is_avail = False
        
        if is_avail:
            available[name] = func
            
    return available
