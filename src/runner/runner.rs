use crate::scanner::simd::{SIMD, swar};
use crate::scanner::{CPUID, Scanner};
use crate::{Query, Save};

#[cfg(target_arch = "x86_64")]
use crate::scanner::simd::x86_64;

use super::element::{Attribute, Attributes, Element, XHtmlElement};

type Runners<'query, E> = Vec<SelectionRunner<'query, 'query, E>>;

use crate::css::SelectionRunner;
use crate::store::{RustStore, Store};

struct Runner {}

impl<'html, 'query> Runner {
    pub(crate) fn run(input: &str, queries: &[Query<'query>]) -> Vec<XHtmlElement<'html>> {
        // let runners = queries
        //         .iter()
        //         .map(|query| SelectionRunner::new(query))
        //         .collect::<Runners<'query, S::E>>();

        let indexes = match CPUID::detect() {
            CPUID::AVX512BW => {
                println!("Using AVX512");
                Scanner::new().scan::<x86_64::SIMD512>(input)
            }
            CPUID::Other => {
                println!("Using SWAR");
                Scanner::new().scan::<swar::SWAR>(input)
            }
        };

        let mut factory = Element::new();

        let bytes = input.as_bytes();
        while factory.next(bytes, &indexes) {
            println!("Element {}: {:#?}", factory.index, factory.element);
        }

        vec![]
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
        let query = Query::first("a", Save::all()).build();
        let queries = &[query];

        Runner::run(MORE_ADVANCED_BASIC_HTML, queries);
    }
}
