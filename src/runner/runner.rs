use crate::scanner::simd::{SIMD, swar};
use crate::scanner::{CPUID, Scanner};
use crate::{Element, Query, Save, dbg_print};

#[cfg(target_arch = "x86_64")]
use crate::scanner::simd::x86_64;

use super::element::{Attribute, Attributes, ElementFactory, XHtmlElement};

type Runners<'query, E> = Vec<SelectionRunner<'query, 'query, E>>;

use crate::css::{DocumentPosition, SelectionRunner};
use crate::store::{RustStore, Store};

pub struct Runner {}

impl<'html: 'query, 'query: 'html> Runner {
    pub fn run(input: &'html str, queries: &[Query<'query>]) -> RustStore<'html, 'query> {
        let indexes = match CPUID::detect() {
            CPUID::AVX512BW => {
                dbg_print!("Using AVX512");
                Scanner::new().scan::<x86_64::SIMD512>(input)
            }
            CPUID::Other => {
                dbg_print!("Using SWAR");
                Scanner::new().scan::<swar::SWAR>(input)
            }
        };

        let mut factory = ElementFactory::new();

        let bytes = input.as_bytes();

        let mut store = RustStore::new(());
        let document_position = DocumentPosition {
            element_depth: 0,
            reader_position: 0, // for inner_html
            text_content_position: usize::MAX,
        };
        let mut selection = SelectionRunner::<usize>::new(&queries[0]);
        while factory.next(bytes, &indexes) {
            //println!("Element {}: {:#?}", factory.index, factory.element);
            if !factory.element.closing {
                selection
                    .next(&factory.element, &document_position, &mut store)
                    .unwrap();
            }
        }

        store
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MORE_ADVANCED_BASIC_HTML: &str = r#"
        <html>
            <h1>Hello World</h1>
            <main>
                <section>
                    <a href="https://hello.com">Hello</a>
                    <div>
                        <a href="https://world.com">World</a>
                    </div>
                </section>
            </main>

            <main>
                <section>
                    <a href="https://hello2.com">Hello2</a>

                    <div>
                        <a href="https://world2.com">World2</a>
                        <div>
                            <a href="https://world3.com">World3</a>
                        </div>
                    </div>
                </section>
            </main>
        </html>
        "#;

    #[test]
    fn test_single_element_query() {
        let query = Query::all("a", Save::all()).build();
        let queries = &[query];

        let arena = Runner::run(MORE_ADVANCED_BASIC_HTML, queries).arena;

        println!("Arena: {:#?}", arena);
    }
}
