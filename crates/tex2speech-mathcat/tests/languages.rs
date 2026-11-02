// Upstream MathCAT tests, kept as they are so that upstream changes can be merged by hand; not formatted
// with this workspace's rustfmt settings (rustfmt::skip on the module declarations).
// Upstream test code: the workspace pedantic profile would only add merge conflicts.
#![allow(clippy::all, clippy::pedantic)]
#![allow(non_snake_case)]
#![allow(
    clippy::doc_lazy_continuation,
    clippy::module_inception,
    clippy::needless_return
)]

#[rustfmt::skip]
mod common;

#[rustfmt::skip]
mod Languages {
    mod intent;
    mod en;
    mod pt;
    mod fr;
}
