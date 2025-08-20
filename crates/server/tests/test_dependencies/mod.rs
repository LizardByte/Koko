// lib imports
use rstest::rstest;

// local imports
use koko::dependencies::get_dependencies;

#[rstest]
#[case("Apache-2.0")]
#[case("BSD-2-Clause")]
#[case("BSD-3-Clause")]
#[case("CC0-1.0")]
#[case("ISC")]
#[case("MIT")]
#[case("MPL-2.0")]
#[case("NCSA")]
#[case("Unicode-3.0")]
#[case("Unlicense")]
#[case("Zlib")]
fn test_individual_license_compatibility(#[case] license: &str) {
    assert!(
        is_license_compatible(license),
        "License '{}' should be compatible",
        license
    );
}

#[rstest]
#[case("GPL-3.0")]
#[case("AGPL-3.0")]
#[case("Custom License")]
#[case("Proprietary")]
fn test_individual_license_incompatibility(#[case] license: &str) {
    assert!(
        !is_license_compatible(license),
        "License '{}' should be incompatible",
        license
    );
}

fn is_license_compatible(license: &str) -> bool {
    let compatible_licenses = vec![
        // compatible: https://www.gnu.org/licenses/license-list.en.html#GPLCompatibleLicenses
        // format: https://spdx.github.io/license-list-data/
        "Apache-2.0",
        "BSD-2-Clause",
        "BSD-3-Clause",
        "CC0-1.0",
        "ISC",
        "MIT",
        "MPL-2.0",
        "NCSA",
        "Unicode-3.0",
        "Unlicense",
        "Zlib",
    ];

    compatible_licenses.iter().any(|&l| license.contains(l))
}

/// Deps that are allowed to have incompatible licenses.
fn dependency_exceptions() -> Vec<&'static str> {
    vec![
        "koko",
        "dlopen2_derive", // https://github.com/OpenByteDev/dlopen2/issues/20
        "ring",           // https://github.com/briansmith/ring/blob/main/LICENSE
    ]
}

#[test]
fn test_dependencies_licenses() {
    let dependencies = get_dependencies().unwrap();

    for package in dependencies {
        if dependency_exceptions().contains(&package.name.as_str()) {
            continue;
        }

        let license = package.license.as_deref().unwrap_or("");
        assert!(
            is_license_compatible(license),
            "License '{}' of package {} is not compatible",
            license,
            package.name
        );
    }
}
