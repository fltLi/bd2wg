//! 测试模板串格式化

// TODO: Test error.

use std::borrow::Cow;

use template_parse::*;

// fn test_compare(template: &str, expect: &str, map: &[(&str, &'static str)]) {
//     let (parser, errs) = TemplateParser::new(template);
//     assert!(errs.is_empty());
//
//     let (result, errs) = parser.parse(|var| {
//         map.iter()
//             .find(|(k, _)| *k == var)
//             .map(|(_, v)| Cow::Borrowed(*v))
//     });
//     assert!(errs.is_empty());
//     assert_eq!(result, expect.to_string());
// }

fn test_compare<F>(template: &str, expect: &str, map: F)
where
    F: FnMut(&str) -> Option<Cow<'_, str>>,
{
    let (parser, errs) = TemplateParser::new(template);
    assert!(errs.is_empty());

    let (result, errs) = parser.parse(map);
    assert!(errs.is_empty());
    assert_eq!(result, expect.to_string());
}

//////////////// test ////////////////

#[test]
fn test_non_replace() {
    let test = |template| test_compare(template, template, |_| panic!());

    test("");
    test("common template");

    test("${{{ ${{{ ${{{");
    test("fake }}$}} {${");
}

#[test]
fn test_common_replace() {
    test_compare("${var}", "variable", |var| match var {
        "var" => Some(Cow::Owned("variable".to_string())),
        _ => panic!("error input: `{var}`"),
    });

    test_compare("${${var}}}}", "variable}}", |var| match var {
        "${var}" => Some(Cow::Owned("variable".to_string())),
        _ => panic!("error input: `{var}`"),
    });

    test_compare("${first} ${second}", "Anon Tokyo", |var| match var {
        "first" => Some(Cow::Owned("Anon".to_string())),
        "second" => Some(Cow::Owned("Tokyo".to_string())),
        _ => panic!("error input: `{var}`"),
    });
}

#[test]
fn test_regex_replace() {
    test_compare(
        "G${0:^.(.+)?} ${1}, and in case I don't see you, ${0} ${2}, ${0} ${3}, and ${0} ${4}!",
        "Good morning, and in case I don't see you, good afternoon, good evening, and good night!",
        |var| match var {
            "0" => Some(Cow::Owned("good".to_string())),
            "1" => Some(Cow::Owned("morning".to_string())),
            "2" => Some(Cow::Owned("afternoon".to_string())),
            "3" => Some(Cow::Owned("evening".to_string())),
            "4" => Some(Cow::Owned("night".to_string())),
            _ => panic!("error input: `{var}`"),
        },
    );
}
