#[test]
fn ui_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*_fail.rs");
    t.pass("tests/ui/*_ok.rs");
}
