use koko::dependencies::get_dependencies;

fn is_license_compatible_with_agplv3(license: &str) -> bool {
    let compatible_licenses = vec![
        // compatible: https://www.gnu.org/licenses/license-list.en.html#GPLCompatibleLicenses
        // format: https://spdx.github.io/license-list-data/
        "AGPL-3.0-only",
        "AGPL-3.0-or-later",
        "Apache-2.0",
        "BSD-2-Clause",
        "BSD-3-Clause",
        "CC0-1.0",
        "GPL-3.0-only",
        "GPL-3.0-or-later",
        "ISC",
        "LGPL-3.0-only",
        "LGPL-3.0-or-later",
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
        "ring", // https://github.com/briansmith/ring/blob/main/LICENSE
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
            is_license_compatible_with_agplv3(license),
            "License {} of package {} is not compatible with AGPLv3",
            license,
            package.name
        );
    }
}
