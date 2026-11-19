//! Tiny helpers shared by every handler.



pub(crate) fn ci_eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}
