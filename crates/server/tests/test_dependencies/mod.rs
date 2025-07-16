use koko::dependencies::get_dependencies;

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
