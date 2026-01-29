use crate::css::State;
use crate::scanner::simd::{SIMD, swar};
use crate::scanner::{CPUID, Scanner};
use crate::xhtml::text_content::TextContent;
use crate::{Element, Query, Save, dbg_print};

#[cfg(target_arch = "x86_64")]
use crate::scanner::simd::x86_64;

use super::element::{Attribute, Attributes, ElementFactory, XHtmlElement};

type Runners<'query> = Vec<SelectionRunner<'query, 'query>>;

use crate::css::{DocumentPosition, SelectionRunner};
use crate::store::Store;

pub struct Runner {}

impl<'a: 'html, 'html: 'query, 'query: 'html> Runner {
    pub fn run(input: &'html str, queries: &'a [Query<'query>]) -> Store<'html, 'query> {
        //let detect = CPUID::detect();
        let detect = CPUID::Other;
        let indexes = match detect {
            CPUID::AVX512BW => {
                dbg_print!("Using AVX512");
                let buffer = x86_64::SIMD512::buffer(input);
                const RATIO_DENOMINATOR: usize = 8;
                let mut out: Vec<u32> = Vec::with_capacity(input.len() / RATIO_DENOMINATOR);
                Scanner::new().scan::<x86_64::SIMD512>(
                    &mut out,
                    0,
                    buffer.as_slice(),
                    buffer.len() - x86_64::SIMD512::BYTES,
                );
                out
            }
            CPUID::Other => {
                dbg_print!("Using SWAR");

                // let (before, bytes, after) = unsafe { input.as_bytes().align_to::<u64>() };

                // let buf_before = {
                //     let mut list = [0u8; 8];
                //     list[..before.len()].copy_from_slice(before);
                //     list
                // };

                // let buf_after = {
                //     let mut list = [0u8; 8];
                //     list[..after.len()].copy_from_slice(after);
                //     list
                // };

                // let mut scanner = Scanner::new();

                // const RATIO_DENOMINATOR: usize = 8;
                // let mut out: Vec<u32> = Vec::with_capacity(input.len() / RATIO_DENOMINATOR);

                // scanner.scan::<swar::SWAR>(&mut out, 0, &buf_before, 8);

                // scanner
                //     .scan_aligned::<swar::SWAR>(&mut out, before.len() as u32, bytes, bytes.len() * 8);

                // scanner
                //     .scan::<swar::SWAR>(&mut out, (before.len() + bytes.len() * 8) as u32, &buf_after, 8);

                let buffer = swar::SWAR::buffer(input);
                const RATIO_DENOMINATOR: usize = 8;
                let mut out: Vec<u32> = Vec::with_capacity(input.len() / RATIO_DENOMINATOR);
                Scanner::new().scan::<swar::SWAR>(
                    &mut out,
                    0,
                    buffer.as_slice(),
                    buffer.len() - swar::SWAR::BYTES,
                );

                out
            }
        };

        let mut factory = ElementFactory::new();

        let bytes = input.as_bytes();

        let mut store = Store::new();

        let mut document_position = DocumentPosition {
            element_depth: 0,
            reader_position: 0, // for inner_html
            text_content_position: usize::MAX,
        };
        let mut selection = SelectionRunner::new(&queries[0]);

        // let mut elements = vec![];
        // while factory.next(bytes, &indexes) {
        //     elements.push(factory.element.clone());
        // }

        store.text_content.start_recording();
        store.text_content.set_start(0);
        while factory.next(bytes, &indexes) {
            //println!("Element {}: {:#?}", factory.index, factory.element);
            let after_end_of_element = factory.element_end + 1;
            store.text_content.push(bytes, factory.element_start);
            store.text_content.set_start(after_end_of_element);

            if !factory.element.closing {
                document_position.reader_position = after_end_of_element;
                selection
                    .next(&factory.element, &document_position, &mut store)
                    .unwrap();
            } else {
                document_position.reader_position = factory.element_start;
                selection.back(
                    &mut store,
                    unsafe { str::from_utf8_unchecked(factory.element.name) },
                    &document_position,
                    bytes,
                );
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
                    <a href="https://hello2.com" active>Hello2</a>

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

        let store = Runner::run(MORE_ADVANCED_BASIC_HTML, queries);

        println!("Elements: {:#?}", store.elements);
        assert!(false, "Missing implementation of test");
    }
}
