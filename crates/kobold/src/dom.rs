//! Utilities for mounting elements in the DOM

use std::ops::Deref;

use wasm_bindgen::JsValue;
use web_sys::Node;

use crate::{util, Mountable};

#[derive(Clone)]
pub struct Element {
    kind: Kind,
    pub(crate) node: Node,
}

#[derive(Clone, Copy)]
enum Kind {
    Element,
    Fragment,
}

impl Deref for Element {
    type Target = JsValue;

    fn deref(&self) -> &JsValue {
        &self.node
    }
}

pub struct Fragment {
    el: Element,
    tail: Node,
}

impl Fragment {
    pub fn new() -> Self {
        let node = util::__kobold_fragment();
        let tail = util::__kobold_fragment_decorate(&node);
        Fragment {
            el: Element {
                kind: Kind::Fragment,
                node,
            },
            tail,
        }
    }

    pub fn append(&self, child: &JsValue) {
        util::__kobold_before(&self.tail, child);
    }
}

impl Deref for Fragment {
    type Target = Element;

    fn deref(&self) -> &Element {
        &self.el
    }
}

impl Element {
    pub fn new(node: Node) -> Self {
        Element {
            kind: Kind::Element,
            node,
        }
    }

    pub fn new_text(text: &str) -> Self {
        Self::new(util::__kobold_text_node(text))
    }

    pub fn new_empty() -> Self {
        Self::new(util::__kobold_empty_node())
    }

    pub fn new_fragment_raw(node: Node) -> Self {
        util::__kobold_fragment_decorate(&node);

        Element {
            kind: Kind::Fragment,
            node,
        }
    }

    pub fn set_text(&self, text: &str) {
        util::__kobold_update_text(&self.node, text);
    }

    pub fn anchor(&self) -> &JsValue {
        &self.node
    }

    pub fn js(&self) -> &JsValue {
        &self.node
    }

    pub fn replace_with(&self, new: &JsValue) {
        match self.kind {
            Kind::Element => util::__kobold_replace(&self.node, new),
            Kind::Fragment => util::__kobold_fragment_replace(&self.node, new),
        }
    }

    pub fn unmount(&self) {
        match self.kind {
            Kind::Element => util::__kobold_unmount(&self.node),
            Kind::Fragment => util::__kobold_fragment_unmount(&self.node),
        }
    }
}

impl Mountable for Element {
    fn el(&self) -> &Element {
        self
    }
}

impl Drop for Element {
    fn drop(&mut self) {
        if let Kind::Fragment = self.kind {
            util::__kobold_fragment_drop(&self.node);
        }
    }
}
