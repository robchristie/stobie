use super::*;

#[test]
fn a_spec_starts_as_one_spellchecked_line_with_nothing_beneath() {
    let spec = TextFieldSpec::new("Name");
    assert_eq!(spec.label(), "Name");
    assert_eq!(spec.multiline_rows(), None);
    assert!(spec.spellcheck);
    assert_eq!(spec.accessible_description(), None);
}

#[test]
fn the_description_for_assistive_technology_puts_the_error_first() {
    let spec = TextFieldSpec::new("Code")
        .description("Six letters from the invitation.")
        .error(Some("That code has expired."));
    assert_eq!(
        spec.accessible_description().as_deref(),
        Some("That code has expired.\nSix letters from the invitation.")
    );
    // An empty error or description is none.
    let spec = spec.error(Some("")).description("");
    assert_eq!(spec.accessible_description(), None);
    assert_eq!(spec.error(None), spec);
}

#[test]
fn a_password_is_one_line() {
    let spec = TextFieldSpec::new("Password").multiline(3).password();
    assert_eq!(spec.multiline_rows(), None);
    assert_eq!(
        TextFieldSpec::new("Notes").multiline(0).multiline_rows(),
        Some(1)
    );
}
