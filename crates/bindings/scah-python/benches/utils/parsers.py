import os
import re
from io import BytesIO

from html.parser import HTMLParser
from bs4 import BeautifulSoup
import lxml.html
from selectolax.parser import HTMLParser as SelectolaxParser
from parsel import Selector
from gazpacho import Soup as GazpachoSoup
import scah

def parse_bs4_htmlparser(html:str, query:str):
    soup = BeautifulSoup(html, "html.parser")
    return soup.find_all(query)

def parse_bs4_lxml(html:str, query:str):
    soup = BeautifulSoup(html, "lxml")
    return soup.find_all(query)

def parse_lxml(html:str, query:str):
    tree = lxml.html.fromstring(html)
    return tree.cssselect(query)

def parse_selectolax(html:str, query:str):
    tree = SelectolaxParser(html)
    return tree.css(query)

def parse_parsel(html:str, query:str):
    selector = Selector(text=html)
    return selector.css(query)

def parse_gazpacho(html:str, query:str):
    soup = GazpachoSoup(html)
    return soup.find(query, mode='all')

def parse_scah(html: str, query:str):
    q = scah.Query.all(query, scah.Save.all()).build()
    store = scah.parse(html, [q])
    return store.get(query)


PARSERS = {
    "Scah": parse_scah,
    # "BS4 (html.parser)": parse_bs4_htmlparser,
    "lxml": parse_lxml,
    "BS4 (lxml)": parse_bs4_lxml,
    "Selectolax": parse_selectolax,
    "Parsel": parse_parsel,
    "Gazpacho": parse_gazpacho,
}

def get_available_parsers():
    return PARSERS
