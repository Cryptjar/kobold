// The `quote!` macro requires deep recursion.
#![recursion_limit = "196"]
#![warn(clippy::all, clippy::cast_possible_truncation, clippy::unused_self)]

#[cfg(not(test))]
extern crate proc_macro;

#[cfg(test)]
extern crate proc_macro2 as proc_macro;

use proc_macro::TokenStream;

mod branching;
mod component;
mod dom;
mod gen;
mod itertools;
mod parse;
mod syntax;
mod tokenize;

use parse::TokenTreeExt;
use tokenize::prelude::*;

macro_rules! unwrap_err {
    ($expr:expr) => {
        match $expr {
            Ok(dom) => dom,
            Err(err) => return err.tokenize(),
        }
    };
}

#[proc_macro_attribute]
pub fn branching(_: TokenStream, input: TokenStream) -> TokenStream {
    do_branching(input)
}

#[proc_macro_attribute]
pub fn component(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = unwrap_err!(component::args(args));

    unwrap_err!(component::component(args, input))
}

fn do_branching(input: TokenStream) -> TokenStream {
    use branching::Scope;

    unwrap_err!(parse::parse::<Scope>(input)).tokenize()
}

#[allow(clippy::let_and_return)]
#[proc_macro]
pub fn html(mut body: TokenStream) -> TokenStream {
    let mut iter = body.into_iter();

    let first = iter.next();

    body = TokenStream::new();
    body.extend(first.clone());
    body.extend(iter);

    if first.is("match") || first.is("if") {
        return do_branching(body);
    }

    // --

    let nodes = unwrap_err!(dom::parse(body));

    // panic!("{nodes:#?}");

    let transient = gen::generate(nodes);

    let out = transient.tokenize();

    // panic!("{out}");

    out
}
