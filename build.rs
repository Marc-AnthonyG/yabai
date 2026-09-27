use std::collections::HashMap;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_directory = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_directory = PathBuf::from(env::var("OUT_DIR").unwrap());
    let osax_directory = manifest_directory.join("src/osax");

    for dependency in [
        "build.rs",
        "src/osax/payload.m",
        "src/osax/loader.m",
        "src/osax/arm64_payload.m",
        "src/osax/x64_payload.m",
        "src/osax/common.h",
        "src/misc/hashtable.h",
        "assets/Info.plist",
    ] {
        println!("cargo:rerun-if-changed={dependency}");
    }

    run_or_fail_the_build(
        Command::new("xcrun")
            .arg("clang")
            .arg(osax_directory.join("payload.m"))
            .args([
                "-shared",
                "-fPIC",
                "-O3",
                "-mmacosx-version-min=11.0",
                "-arch",
                "x86_64",
                "-arch",
                "arm64e",
            ])
            .arg("-o")
            .arg(out_directory.join("payload"))
            .args([
                "-F/System/Library/PrivateFrameworks",
                "-framework",
                "SkyLight",
                "-framework",
                "Foundation",
                "-framework",
                "Carbon",
            ]),
    );

    run_or_fail_the_build(
        Command::new("xcrun")
            .arg("clang")
            .arg(osax_directory.join("loader.m"))
            .args([
                "-O3",
                "-mmacosx-version-min=11.0",
                "-arch",
                "x86_64",
                "-arch",
                "arm64e",
            ])
            .arg("-o")
            .arg(out_directory.join("loader"))
            .args(["-framework", "Cocoa"]),
    );

    generate_osax_common(&osax_directory.join("common.h"), &out_directory);

    println!("cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=11.0");
    println!("cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks");
    for framework in ["Carbon", "Cocoa", "CoreServices", "CoreVideo", "SkyLight"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    println!(
        "cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{}",
        manifest_directory.join("assets/Info.plist").display()
    );
}

fn run_or_fail_the_build(command: &mut Command) {
    let output = match command.output() {
        Ok(output) => output,
        Err(error) => panic!("build.rs: could not run {command:?}: {error}"),
    };

    if !output.status.success() {
        panic!(
            "build.rs: {command:?} failed with {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

const SA_OPCODES_NO_LIVE_DAEMON_CODE_SENDS: [&str; 1] = ["SA_OPCODE_WINDOW_FOCUS"];

const OSAX_ATTRIBUTE_NAMES: [&str; 7] = [
    "OSAX_ATTRIB_DOCK_SPACES",
    "OSAX_ATTRIB_DPPM",
    "OSAX_ATTRIB_ADD_SPACE",
    "OSAX_ATTRIB_REM_SPACE",
    "OSAX_ATTRIB_MOV_SPACE",
    "OSAX_ATTRIB_SET_WINDOW",
    "OSAX_ATTRIB_ANIM_TIME",
];

fn generate_osax_common(common_header: &Path, out_directory: &Path) {
    let header_text = match fs::read_to_string(common_header) {
        Ok(header_text) => header_text,
        Err(error) => panic!(
            "build.rs: could not read {}: {error}",
            common_header.display()
        ),
    };

    let defines = parse_defines(&header_text, common_header);
    let opcodes = parse_sa_opcode_enum(&header_text, common_header);

    let socket_path_format = require_string_define(&defines, "SA_SOCKET_PATH_FMT", common_header);
    let socket_buffer_length = require_hexadecimal_define(
        &defines,
        "SA_SOCKET_BUFF_LEN",
        common_header,
    );
    let osax_version = require_string_define(&defines, "OSAX_VERSION", common_header);

    let mut generated = String::new();
    writeln!(
        generated,
        "pub const SA_SOCKET_PATH_FMT: &str = \"{socket_path_format}\";"
    )
    .unwrap();
    writeln!(
        generated,
        "pub const SA_SOCKET_BUFF_LEN: usize = 0x{socket_buffer_length:X};"
    )
    .unwrap();
    writeln!(generated).unwrap();
    writeln!(
        generated,
        "pub const OSAX_VERSION: &str = \"{osax_version}\";"
    )
    .unwrap();
    writeln!(generated).unwrap();

    for attribute_name in OSAX_ATTRIBUTE_NAMES {
        let attribute_value =
            require_hexadecimal_define(&defines, attribute_name, common_header);
        writeln!(
            generated,
            "pub const {attribute_name}: u32 = 0x{attribute_value:02X};"
        )
        .unwrap();
    }
    writeln!(generated).unwrap();

    writeln!(generated, "pub const OSAX_ATTRIB_ALL: u32 = {};", {
        let mut all = String::new();
        for (index, attribute_name) in OSAX_ATTRIBUTE_NAMES.iter().enumerate() {
            if index > 0 {
                all.push_str("\n    | ");
            }
            all.push_str(attribute_name);
        }
        all
    })
    .unwrap();
    writeln!(generated).unwrap();

    writeln!(generated, "#[repr(u8)]").unwrap();
    writeln!(generated, "#[derive(Clone, Copy, PartialEq, Eq)]").unwrap();
    writeln!(generated, "pub enum SaOpcode {{").unwrap();
    for (variant_name, variant_value) in &opcodes {
        writeln!(generated, "    {variant_name} = 0x{variant_value:02X},").unwrap();
    }
    writeln!(generated, "}}").unwrap();

    let generated_path = out_directory.join("osax_common.rs");
    if let Err(error) = fs::write(&generated_path, generated) {
        panic!(
            "build.rs: could not write {}: {error}",
            generated_path.display()
        );
    }
}

fn parse_defines(header_text: &str, common_header: &Path) -> HashMap<String, String> {
    let mut defines = HashMap::new();
    let mut lines = header_text.lines().peekable();

    while let Some(line) = lines.next() {
        let Some(rest) = line.trim_start().strip_prefix("#define") else {
            continue;
        };

        let mut value = rest.trim().to_string();
        while value.ends_with('\\') {
            value.pop();
            let Some(continuation) = lines.next() else {
                panic!(
                    "build.rs: {} ends inside a line continuation",
                    common_header.display()
                );
            };
            value.push_str(continuation.trim());
        }

        let mut parts = value.splitn(2, char::is_whitespace);
        let Some(name) = parts.next() else { continue };
        if name.is_empty() {
            continue;
        }
        let define_value = parts.next().unwrap_or("").trim().to_string();
        defines.insert(name.to_string(), define_value);
    }

    if defines.is_empty() {
        panic!(
            "build.rs: no #define found in {}",
            common_header.display()
        );
    }

    defines
}

fn require_string_define(
    defines: &HashMap<String, String>,
    name: &str,
    common_header: &Path,
) -> String {
    let Some(value) = defines.get(name) else {
        panic!(
            "build.rs: {} does not define {name}",
            common_header.display()
        );
    };

    let Some(text) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        panic!(
            "build.rs: {} defines {name} as {value}, which is not a string literal",
            common_header.display()
        );
    };

    if text.contains('"') || text.contains('\\') {
        panic!(
            "build.rs: {} defines {name} as {value}, which this parser cannot transcribe",
            common_header.display()
        );
    }

    text.to_string()
}

fn require_hexadecimal_define(
    defines: &HashMap<String, String>,
    name: &str,
    common_header: &Path,
) -> u32 {
    let Some(value) = defines.get(name) else {
        panic!(
            "build.rs: {} does not define {name}",
            common_header.display()
        );
    };

    parse_c_integer(value).unwrap_or_else(|| {
        panic!(
            "build.rs: {} defines {name} as {value}, which is not an integer literal",
            common_header.display()
        )
    })
}

fn parse_c_integer(value: &str) -> Option<u32> {
    let value = value.trim();
    match value.strip_prefix("0x").or_else(|| value.strip_prefix("0X")) {
        Some(digits) => u32::from_str_radix(digits, 16).ok(),
        None => value.parse::<u32>().ok(),
    }
}

fn parse_sa_opcode_enum(header_text: &str, common_header: &Path) -> Vec<(String, u8)> {
    let Some(enum_start) = find_sa_opcode_declaration(header_text) else {
        panic!(
            "build.rs: {} does not declare enum sa_opcode",
            common_header.display()
        );
    };

    let body = &header_text[enum_start..];
    let Some(body_start) = body.find('{') else {
        panic!(
            "build.rs: enum sa_opcode in {} has no body",
            common_header.display()
        );
    };
    let Some(body_end) = body.find('}') else {
        panic!(
            "build.rs: enum sa_opcode in {} is not closed",
            common_header.display()
        );
    };
    if body_end < body_start {
        panic!(
            "build.rs: enum sa_opcode in {} is not closed",
            common_header.display()
        );
    }

    let mut opcodes = Vec::new();
    let mut skipped_opcode_names: Vec<String> = Vec::new();
    for entry in body[body_start + 1..body_end].split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }

        let Some((name, value)) = entry.split_once('=') else {
            panic!(
                "build.rs: enum sa_opcode entry {entry} in {} has no explicit value",
                common_header.display()
            );
        };

        let name = name.trim();
        let Some(value) = parse_c_integer(value) else {
            panic!(
                "build.rs: enum sa_opcode entry {entry} in {} has a value this parser cannot read",
                common_header.display()
            );
        };
        let Ok(value) = u8::try_from(value) else {
            panic!(
                "build.rs: enum sa_opcode entry {entry} in {} does not fit in a u8",
                common_header.display()
            );
        };

        let Some(variant_suffix) = name.strip_prefix("SA_OPCODE_") else {
            panic!(
                "build.rs: enum sa_opcode entry {name} in {} is not prefixed SA_OPCODE_",
                common_header.display()
            );
        };

        if SA_OPCODES_NO_LIVE_DAEMON_CODE_SENDS.contains(&name) {
            skipped_opcode_names.push(name.to_string());
            continue;
        }

        opcodes.push((camel_case_from_screaming_snake_case(variant_suffix), value));
    }

    for skipped_name in SA_OPCODES_NO_LIVE_DAEMON_CODE_SENDS {
        if !skipped_opcode_names.iter().any(|name| name == skipped_name) {
            panic!(
                "build.rs: enum sa_opcode in {} no longer declares {skipped_name}",
                common_header.display()
            );
        }
    }

    if opcodes.is_empty() {
        panic!(
            "build.rs: enum sa_opcode in {} is empty",
            common_header.display()
        );
    }

    opcodes
}

fn find_sa_opcode_declaration(header_text: &str) -> Option<usize> {
    let declaration = "enum sa_opcode";
    let mut searched_up_to = 0;

    while let Some(offset) = header_text[searched_up_to..].find(declaration) {
        let declaration_start = searched_up_to + offset;
        let declaration_end = declaration_start + declaration.len();
        let follows = header_text[declaration_end..].chars().next();
        if !follows.is_some_and(|character| character.is_alphanumeric() || character == '_') {
            return Some(declaration_start);
        }
        searched_up_to = declaration_end;
    }

    None
}

fn camel_case_from_screaming_snake_case(name: &str) -> String {
    let mut camel_case = String::with_capacity(name.len());
    for word in name.split('_') {
        let mut characters = word.chars();
        if let Some(first) = characters.next() {
            camel_case.push(first.to_ascii_uppercase());
            for character in characters {
                camel_case.push(character.to_ascii_lowercase());
            }
        }
    }
    camel_case
}
