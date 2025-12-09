# static2jxl - Static Image to JXL Converter
# Makefile for macOS/Linux

CC = clang
CFLAGS = -Wall -Wextra -O3 -pthread
LDFLAGS = -lpthread

TARGET = static2jxl
SRC_DIR = src
BUILD_DIR = build

SRCS = $(SRC_DIR)/main.c
OBJS = $(BUILD_DIR)/main.o

.PHONY: all clean install

all: $(BUILD_DIR) $(TARGET)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(TARGET): $(OBJS)
	$(CC) $(OBJS) -o $(TARGET) $(LDFLAGS)
	@echo "✅ Build complete: $(TARGET)"

$(BUILD_DIR)/main.o: $(SRC_DIR)/main.c $(SRC_DIR)/static2jxl.h
	$(CC) $(CFLAGS) -c $< -o $@

clean:
	rm -rf $(BUILD_DIR) $(TARGET)
	@echo "🧹 Cleaned"

install: $(TARGET)
	cp $(TARGET) /usr/local/bin/
	@echo "📦 Installed to /usr/local/bin/$(TARGET)"

# Run tests
test: $(TARGET)
	@echo "🧪 Running basic tests..."
	./$(TARGET) --help
	@echo "✅ Tests passed"
