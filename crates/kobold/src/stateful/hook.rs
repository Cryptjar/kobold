// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::future::Future;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::rc::{Rc, Weak};

use wasm_bindgen_futures::spawn_local;

use crate::event::{EventCast, Listener};
use crate::internal::{In, Out};
use crate::stateful::{Inner, ShouldRender};
use crate::View;

/// A hook into some state `S`. A reference to `Hook` is obtained by using the [`stateful`](crate::stateful::stateful)
/// function.
///
/// Hook can be read from though its `Deref` implementation, and it allows for mutations either by [`bind`ing](Hook::bind)
/// closures to it.
#[repr(transparent)]
pub struct Hook<S> {
    inner: Inner<S>,
}

#[repr(transparent)]
pub struct Signal<S> {
    pub(super) weak: Weak<Inner<S>>,
}

impl<S> Signal<S> {
    /// Update the state behind this `Signal`.
    ///
    /// ```
    /// # use kobold::prelude::*;
    /// fn example(count: Signal<i32>) {
    ///     // increment count and trigger a render
    ///     count.update(|count| *count += 1);
    ///
    ///     // increment count if less than 10, only render on change
    ///     count.update(|count| {
    ///         if *count < 10 {
    ///             *count += 1;
    ///             Then::Render
    ///         } else {
    ///             Then::Stop
    ///         }
    ///     })
    /// }
    /// ```
    pub fn update<F, O>(&self, mutator: F)
    where
        F: FnOnce(&mut S) -> O,
        O: ShouldRender,
    {
        if let Some(inner) = self.weak.upgrade() {
            if inner.state.with(mutator).should_render() {
                inner.update()
            }
        }
    }

    /// Same as [`update`](Signal::update), but it never renders updates.
    pub fn update_silent<F>(&self, mutator: F)
    where
        F: FnOnce(&mut S),
    {
        if let Some(inner) = self.weak.upgrade() {
            inner.state.with(mutator);
        }
    }

    /// Replace the entire state with a new value and trigger an update.
    pub fn set(&self, val: S) {
        self.update(move |s| *s = val);
    }
}

impl<S> Clone for Signal<S> {
    fn clone(&self) -> Self {
        Signal {
            weak: self.weak.clone(),
        }
    }
}

impl<S> Hook<S> {
    pub(super) fn new(inner: &Inner<S>) -> &Self {
        unsafe { &*(inner as *const _ as *const Hook<S>) }
    }

    /// Binds a closure to a mutable reference of the state. While this method is public
    /// it's recommended to use the [`bind!`](crate::bind) macro instead.
    pub fn bind<E, F, O>(&self, callback: F) -> Bound<S, F>
    where
        S: 'static,
        E: EventCast,
        F: Fn(&mut S, E) -> O + 'static,
        O: ShouldRender,
    {
        let inner = &self.inner;

        Bound { inner, callback }
    }

    pub fn bind_async<E, F, T>(&self, callback: F) -> impl Listener<E>
    where
        S: 'static,
        E: EventCast,
        F: Fn(Signal<S>, E) -> T + 'static,
        T: Future<Output = ()> + 'static,
    {
        let inner = &self.inner as *const Inner<S>;

        move |e| {
            // ⚠️ Safety:
            // ==========
            //
            // This is fired only as event listener from the DOM, which guarantees that
            // state is not currently borrowed, as events cannot interrupt normal
            // control flow, and `Signal`s cannot borrow state across .await points.
            //
            // This temporary `Rc` will not mess with the `strong_count` value, we only
            // need it to construct a `Weak` reference to `Inner`.
            let rc = ManuallyDrop::new(unsafe { Rc::from_raw(inner) });

            let signal = Signal {
                weak: Rc::downgrade(&*rc),
            };

            spawn_local(callback(signal, e));
        }
    }

    /// Get the value of state if state implements `Copy`. This is equivalent to writing
    /// `**hook` but conveys intent better.
    pub fn get(&self) -> S
    where
        S: Copy,
    {
        **self
    }
}

pub struct Bound<'b, S, F> {
    inner: &'b Inner<S>,
    callback: F,
}

impl<S, F> Bound<'_, S, F> {
    pub fn into_listener<E, O>(self) -> impl Listener<E>
    where
        S: 'static,
        E: EventCast,
        F: Fn(&mut S, E) -> O + 'static,
        O: ShouldRender,
    {
        let Bound { inner, callback } = self;

        let inner = inner as *const Inner<S>;
        let bound = move |e| {
            // ⚠️ Safety:
            // ==========
            //
            // This is fired only as event listener from the DOM, which guarantees that
            // state is not currently borrowed, as events cannot interrupt normal
            // control flow, and `Signal`s cannot borrow state across .await points.
            let inner = unsafe { &*inner };
            let state = unsafe { inner.state.mut_unchecked() };

            if callback(state, e).should_render() {
                inner.update();
            }
        };

        BoundListener {
            bound,
            _unbound: PhantomData::<F>,
        }
    }
}

impl<S, F> Clone for Bound<'_, S, F>
where
    F: Clone,
{
    fn clone(&self) -> Self {
        Bound {
            inner: self.inner,
            callback: self.callback.clone(),
        }
    }
}

impl<S, F> Copy for Bound<'_, S, F> where F: Copy {}

struct BoundListener<B, U> {
    bound: B,
    _unbound: PhantomData<U>,
}

impl<B, U, E> Listener<E> for BoundListener<B, U>
where
    B: Listener<E>,
    E: EventCast,
    Self: 'static,
{
    type Product = B::Product;

    fn build(self, p: In<Self::Product>) -> Out<Self::Product> {
        self.bound.build(p)
    }

    fn update(self, p: &mut Self::Product) {
        // No need to update zero-sized closures.
        //
        // This is a const branch that should be optimized away.
        if std::mem::size_of::<U>() != 0 {
            self.bound.update(p);
        }
    }
}

impl<S> Deref for Hook<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        // ⚠️ Safety:
        // ==========
        //
        // Hook only lives inside the inner closure of `stateful`, and no mutable
        // references to `Inner` are present while it's around.
        unsafe { self.inner.state.ref_unchecked() }
    }
}

impl<'a, V> View for &'a Hook<V>
where
    &'a V: View + 'a,
{
    type Product = <&'a V as View>::Product;

    fn build(self, p: In<Self::Product>) -> Out<Self::Product> {
        (**self).build(p)
    }

    fn update(self, p: &mut Self::Product) {
        (**self).update(p)
    }
}

#[cfg(test)]
mod test {
    use std::cell::UnsafeCell;
    use wasm_bindgen::JsCast;

    use crate::stateful::cell::WithCell;
    use crate::stateful::product::ProductHandler;
    use crate::value::TextProduct;

    use super::*;

    #[test]
    fn bound_callback_is_copy() {
        let inner = Inner {
            state: WithCell::new(0_i32),
            prod: UnsafeCell::new(ProductHandler::mock(
                |_, _| {},
                TextProduct {
                    memo: 0,
                    node: wasm_bindgen::JsValue::UNDEFINED.unchecked_into(),
                },
            )),
        };

        let mock = Bound {
            inner: &inner,
            callback: |state: &mut i32, _: web_sys::Event| {
                *state += 1;
            },
        };

        // Make sure we can copy the mock twice
        drop([mock, mock]);
    }
}
