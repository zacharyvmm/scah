mod css;
mod utils;
mod xhtml;

fn main() {
    println!("Hello World");
}

/* mod scrooge {
    pub mod css {
        use crate::xhtml::element::parser::XHtmlElement;

        struct Hook {
            //fsm: SelectorFSM,
            queue: Vec,
            // When going throught the fsm selectors IF it conforms to the fsm then it appended at that state to the back of the list.
        }

        impl Hook {
            fn transition<'b>(element: XHtmlElement<'b>){
                // Check the last FSM ==[if State of FSM is not *DONE* then]==> try next step

                // ----------------------------
                // When the scope ends the FSM needs to *step back* if state is not *DONE*
            }
        }

        struct Selectors {
            hooks: Vec,
        }
        impl Selectors {
            fn transition<'b>(element: XHtmlElement<'b>){
                // This will step throught every single selector FSM
                // In addition, it will add in a list the elements (with the desired information) corresponding to the hook used
            }
        }
    }

    pub mod xhtml {
         
    }
} */