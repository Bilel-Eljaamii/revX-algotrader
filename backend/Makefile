# Cross-compilation settings
TARGET = aarch64-unknown-linux-gnu

.PHONY: all pi3 pi5 generic pc debug test coverage clean pi3-debug pi5-debug check fmt coverage-report

all: fmt check pc

# Local PC Targets
pc:
	cargo build --release

fmt:
	cargo +nightly fmt 

debug:
	cargo build

test:
	cargo test

coverage:
	cargo llvm-cov test

check:
	cargo check --release

# RPi 3 Targets
pi3:
	RUSTFLAGS="-C target-cpu=cortex-a53" cargo build --target $(TARGET) --release

pi3-debug:
	cargo build --target $(TARGET)

# RPi 5 Targets
pi5:
	RUSTFLAGS="-C target-cpu=cortex-a76" cargo build --target $(TARGET) --release

pi5-debug:
	cargo build --target $(TARGET)

# Generic: Runs on Pi 3, 4, and 5 (Safe fallback)
generic:
	cargo build --target $(TARGET) --release

clean:
	cargo clean
