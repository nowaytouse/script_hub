# jpeg2jxl - High-Performance JPEG to JXL Batch Converter
# Makefile for macOS/Linux

CC = clang
CFLAGS = -O3 -march=native -Wall -Wextra -pedantic -std=c11
LDFLAGS = -lpthread

# macOS specific
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
    CFLAGS += -D_DARWIN_C_SOURCE
endif

TARGET = jpeg2jxl
SRCDIR = src
OBJDIR = obj
BINDIR = bin

SOURCES = $(wildcard $(SRCDIR)/*.c)
OBJECTS = $(SOURCES:$(SRCDIR)/%.c=$(OBJDIR)/%.o)

.PHONY: all clean install

all: $(BINDIR)/$(TARGET)

$(BINDIR)/$(TARGET): $(OBJECTS) | $(BINDIR)
	$(CC) $(OBJECTS) -o $@ $(LDFLAGS)
	@echo "✅ Build complete: $@"

$(OBJDIR)/%.o: $(SRCDIR)/%.c | $(OBJDIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(OBJDIR):
	mkdir -p $(OBJDIR)

$(BINDIR):
	mkdir -p $(BINDIR)

clean:
	rm -rf $(OBJDIR) $(BINDIR)
	@echo "🧹 Cleaned build artifacts"

install: $(BINDIR)/$(TARGET)
	cp $(BINDIR)/$(TARGET) /usr/local/bin/
	@echo "📦 Installed to /usr/local/bin/$(TARGET)"

# Development build with debug symbols
debug: CFLAGS = -g -O0 -Wall -Wextra -pedantic -std=c11 -DDEBUG
debug: clean all
