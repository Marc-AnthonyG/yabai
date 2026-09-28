set shell := ["bash", "-euo", "pipefail", "-c"]
set lazy

build_directory := "./bin"
documentation_directory := "./doc"
scripts_directory := "./scripts"
assets_directory := "./assets"
examples_directory := "./examples"
archive_directory := "./archive"

yabai_binary := build_directory / "yabai"
manual_page_source := documentation_directory / "yabai.asciidoc"
manual_page := documentation_directory / "yabai.1"
set_icon_script := scripts_directory / "seticon.py"
icon_image := assets_directory / "icon/2x/icon-512px@2x.png"
install_script := scripts_directory / "install.sh"
install_script_version_line := "60"
install_script_expected_hash_line := "61"
codesigning_identity := "yabai-cert"

apple_silicon_target := "aarch64-apple-darwin"
intel_target := "x86_64-apple-darwin"
host_target := `rustc -vV | sed -n 's/^host: //p'`
debug_profile_directory := "debug"
release_profile_directory := "release"
clippy_denied_lint_groups := "-D clippy::correctness -D clippy::suspicious"

alias all := build

[default]
build: (build-universal-binary debug_profile_directory)

install: (build-universal-binary release_profile_directory "--release")

asan: (build-sanitized-host-binary "address")

tsan: (build-sanitized-host-binary "thread")

[private]
build-universal-binary profile_directory *cargo_build_flags: clean-build
    mkdir -p {{ build_directory }}
    cargo build {{ cargo_build_flags }} --target {{ apple_silicon_target }}
    cargo build {{ cargo_build_flags }} --target {{ intel_target }}
    lipo -create -output {{ yabai_binary }} \
        ./target/{{ apple_silicon_target }}/{{ profile_directory }}/yabai \
        ./target/{{ intel_target }}/{{ profile_directory }}/yabai

[private]
build-sanitized-host-binary sanitizer: clean-build
    mkdir -p {{ build_directory }}
    RUSTFLAGS="-Zsanitizer={{ sanitizer }}" cargo +nightly build -Zbuild-std --target {{ host_target }}
    cp ./target/{{ host_target }}/{{ debug_profile_directory }}/yabai {{ yabai_binary }}

check:
    cargo check --target {{ apple_silicon_target }}
    cargo check --target {{ intel_target }}

test:
    cargo test --target {{ apple_silicon_target }}
    cargo test --target {{ intel_target }}

lint:
    cargo clippy --all-targets --target {{ apple_silicon_target }} -- {{ clippy_denied_lint_groups }}
    cargo clippy --all-targets --target {{ intel_target }} -- {{ clippy_denied_lint_groups }}

man:
    asciidoctor -b manpage {{ manual_page_source }} -o {{ manual_page }}

icon:
    python3 {{ set_icon_script }} {{ icon_image }} {{ yabai_binary }}

publish:
    sed -i '' "{{ install_script_version_line }}s/^VERSION=.*/VERSION=\"$({{ yabai_binary }} --version | cut -d "v" -f 2)\"/" {{ install_script }}
    sed -i '' "{{ install_script_expected_hash_line }}s/^EXPECTED_HASH=.*/EXPECTED_HASH=\"$(shasum -a 256 {{ build_directory }}/$({{ yabai_binary }} --version).tar.gz | cut -d " " -f 1)\"/" {{ install_script }}

archive: man install sign icon
    rm -rf {{ archive_directory }}
    mkdir -p {{ archive_directory }}
    cp -r {{ build_directory }} {{ archive_directory }}/
    cp -r {{ documentation_directory }} {{ archive_directory }}/
    cp -r {{ examples_directory }} {{ archive_directory }}/
    tar -cvzf {{ build_directory }}/$({{ yabai_binary }} --version).tar.gz {{ archive_directory }}
    rm -rf {{ archive_directory }}

sign:
    codesign -fs "{{ codesigning_identity }}" {{ yabai_binary }}

clean-build:
    rm -rf {{ build_directory }}

clean: clean-build
    cargo clean
