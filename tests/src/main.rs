use sketch::{html, document, Html, Node, Mountable, Update};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

fn main() {
    fn hello(foo: &'static str, n: u64) -> impl Html {
        html! {
            <div>
                <h1>"Hello "{ foo }"!"</h1>
                <p>{ n }" × 2 = "{ n * 2 }</p>
            </div>
        }
    }

    let bob = hello("Bob", 2);

    let mut rendered = bob.render();

    let body = document().body().expect("document should have a body");
    let body: &Node = body.as_ref();

    body.append_child(&rendered.node()).unwrap();

    let window = sketch::reexport::web_sys::window().expect("should have a window in this context");
    let mut i = 2;
    let a = Closure::wrap(Box::new(move || {
        i += 1;

        static NAMES: &[&str] = &["Bob", "Alice", "Maciej", "World"];

        let name = NAMES[(i as usize / 10) % NAMES.len()];

        rendered.update(hello(name, i));
    }) as Box<dyn FnMut()>);

    window
        .set_interval_with_callback_and_timeout_and_arguments_0(a.as_ref().unchecked_ref(), 100).unwrap();

    a.forget();
}