FRAMEWORK_PATH = -F/System/Library/PrivateFrameworks
BUILD_PATH     = ./bin
DOC_PATH       = ./doc
SCRIPT_PATH    = ./scripts
ASSET_PATH     = ./assets
SMP_PATH       = ./examples
ARCH_PATH      = ./archive
OSAX_SRC       = ./src/osax/payload_bin.c ./src/osax/loader_bin.c
OSAX_PATH      = ./src/osax
HOST_TARGET    = $(shell rustc -vV | sed -n 's/^host: //p')
ARM_TARGET     = aarch64-apple-darwin
X64_TARGET     = x86_64-apple-darwin

CARGO          = cargo
CARGO_FLAGS    =
PROFILE_DIR    = debug

.PHONY: all asan tsan install man icon archive publish sign clean-build clean

all: clean-build $(BUILD_PATH)/yabai

install: CARGO_FLAGS = --release
install: PROFILE_DIR = release
install: clean-build $(BUILD_PATH)/yabai

asan: CARGO       = RUSTFLAGS="-Zsanitizer=address" cargo +nightly
asan: CARGO_FLAGS = -Zbuild-std --target $(HOST_TARGET)
asan: clean-build $(BUILD_PATH)/yabai-host

tsan: CARGO       = RUSTFLAGS="-Zsanitizer=thread" cargo +nightly
tsan: CARGO_FLAGS = -Zbuild-std --target $(HOST_TARGET)
tsan: clean-build $(BUILD_PATH)/yabai-host

$(OSAX_SRC): $(OSAX_PATH)/loader.m $(OSAX_PATH)/payload.m
	xcrun clang $(OSAX_PATH)/payload.m -shared -fPIC -O3 -mmacosx-version-min=11.0 -arch x86_64 -arch arm64e -o $(OSAX_PATH)/payload $(FRAMEWORK_PATH) -framework SkyLight -framework Foundation -framework Carbon
	xcrun clang $(OSAX_PATH)/loader.m -O3 -mmacosx-version-min=11.0 -arch x86_64 -arch arm64e -o $(OSAX_PATH)/loader -framework Cocoa
	xxd -i -a $(OSAX_PATH)/payload $(OSAX_PATH)/payload_bin.c
	xxd -i -a $(OSAX_PATH)/loader $(OSAX_PATH)/loader_bin.c
	rm -f $(OSAX_PATH)/payload
	rm -f $(OSAX_PATH)/loader

man:
	asciidoctor -b manpage $(DOC_PATH)/yabai.asciidoc -o $(DOC_PATH)/yabai.1

icon:
	python3 $(SCRIPT_PATH)/seticon.py $(ASSET_PATH)/icon/2x/icon-512px@2x.png $(BUILD_PATH)/yabai

publish:
	sed -i '' "60s/^VERSION=.*/VERSION=\"$(shell $(BUILD_PATH)/yabai --version | cut -d "v" -f 2)\"/" $(SCRIPT_PATH)/install.sh
	sed -i '' "61s/^EXPECTED_HASH=.*/EXPECTED_HASH=\"$(shell shasum -a 256 $(BUILD_PATH)/$(shell $(BUILD_PATH)/yabai --version).tar.gz | cut -d " " -f 1)\"/" $(SCRIPT_PATH)/install.sh

archive: man install sign icon
	rm -rf $(ARCH_PATH)
	mkdir -p $(ARCH_PATH)
	cp -r $(BUILD_PATH) $(ARCH_PATH)/
	cp -r $(DOC_PATH) $(ARCH_PATH)/
	cp -r $(SMP_PATH) $(ARCH_PATH)/
	tar -cvzf $(BUILD_PATH)/$(shell $(BUILD_PATH)/yabai --version).tar.gz $(ARCH_PATH)
	rm -rf $(ARCH_PATH)

sign:
	codesign -fs "yabai-cert" $(BUILD_PATH)/yabai

$(BUILD_PATH)/yabai:
	mkdir -p $(BUILD_PATH)
	$(CARGO) build $(CARGO_FLAGS) --target $(ARM_TARGET)
	$(CARGO) build $(CARGO_FLAGS) --target $(X64_TARGET)
	lipo -create -output $@ \
	    ./target/$(ARM_TARGET)/$(PROFILE_DIR)/yabai \
	    ./target/$(X64_TARGET)/$(PROFILE_DIR)/yabai

$(BUILD_PATH)/yabai-host:
	mkdir -p $(BUILD_PATH)
	$(CARGO) build $(CARGO_FLAGS)
	cp ./target/$(HOST_TARGET)/$(PROFILE_DIR)/yabai $(BUILD_PATH)/yabai

clean-build:
	rm -rf $(BUILD_PATH)

clean: clean-build
	cargo clean
