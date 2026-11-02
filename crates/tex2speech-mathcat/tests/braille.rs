// Upstream MathCAT tests, kept as they are so that upstream changes can be merged by hand; not formatted
// with this workspace's rustfmt settings (rustfmt::skip on the module declarations).
// Upstream test code: the workspace pedantic profile would only add merge conflicts.
#![allow(clippy::all, clippy::pedantic)]
#![allow(non_snake_case)]
#![allow(clippy::needless_return)]

#[rustfmt::skip]
mod common;

#[rustfmt::skip]
mod braille {
    mod Nemeth {
        mod rules;
        mod other;
        mod chemistry;
        mod SRE_Nemeth72;
        mod SRE_NemethBase;
        mod AataNemeth;
    }
}
