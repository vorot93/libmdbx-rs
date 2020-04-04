# This makefile is for GNU Make, and nowadays provided
# just for compatibility and preservation of traditions.
# Please use CMake in case of any difficulties or problems.
#
# Preprocessor macros (for MDBX_OPTIONS) of interest...
# Note that the defaults should already be correct for most platforms;
# you should not need to change any of these. Read their descriptions
# in README and source code if you do. There may be other macros of interest.
SHELL   := env bash

# install sandbox
SANDBOX ?=

# install prefixes (inside sandbox)
prefix  ?= /usr/local
mandir  ?= $(prefix)/man

# lib/bin suffix for multiarch/biarch, e.g. '.x86_64'
suffix  ?=

CC      ?= gcc
LD      ?= ld
MDBX_OPTIONS ?= -DNDEBUG=1
CFLAGS  ?= -O2 -g -Wall -Werror -Wextra -Wpedantic -ffunction-sections -fPIC -fvisibility=hidden -std=gnu11 -pthread -Wno-error=attributes
# -Wno-tautological-compare

# HINT: Try append '--no-as-needed,-lrt' for ability to built with modern glibc, but then run with the old.
LIBS    ?= $(shell uname | grep -qi SunOS && echo "-lkstat") $(shell uname | grep -qi -e Darwin -e OpenBSD || echo "-lrt")

LDFLAGS ?= $(shell $(LD) --help 2>/dev/null | grep -q -- --gc-sections && echo '-Wl,--gc-sections,-z,relro,-O1')$(shell $(LD) --help 2>/dev/null | grep -q -- -dead_strip && echo '-Wl,-dead_strip')
EXE_LDFLAGS ?= -pthread

################################################################################

UNAME      := $(shell uname -s 2>/dev/null || echo Unknown)
define uname2sosuffix
  case "$(UNAME)" in
    Darwin*|Mach*) echo dylib;;
    CYGWIN*|MINGW*|MSYS*|Windows*) echo dll;;
    *) echo so;;
  esac
endef
SO_SUFFIX  := $(shell $(uname2sosuffix))

HEADERS    := mdbx.h
LIBRARIES  := libmdbx.a libmdbx.$(SO_SUFFIX)
TOOLS      := mdbx_stat mdbx_copy mdbx_dump mdbx_load mdbx_chk
MANPAGES   := mdbx_stat.1 mdbx_copy.1 mdbx_dump.1 mdbx_load.1 mdbx_chk.1

.PHONY: mdbx all install clean test dist check

all: $(LIBRARIES) $(TOOLS)

mdbx: libmdbx.a libmdbx.$(SO_SUFFIX)

tools: $(TOOLS)

strip: all
	strip libmdbx.$(SO_SUFFIX) $(TOOLS)

clean:
	rm -rf $(TOOLS) mdbx_test @* *.[ao] *.[ls]o *~ tmp.db/* \
		*.gcov *.log *.err src/*.o test/*.o mdbx_example dist \
		config.h src/elements/config.h src/elements/version.c *.tar*

libmdbx.a: mdbx-static.o
	$(AR) rs $@ $?

libmdbx.$(SO_SUFFIX): mdbx-dylib.o
	$(CC) $(CFLAGS) $^ -pthread -shared $(LDFLAGS) $(LIBS) -o $@


################################################################################
# Amalgamated source code, i.e. distributed after `make dists`
MAN_SRCDIR := man1/

config.h: mdbx.c $(lastword $(MAKEFILE_LIST))
	(echo '#define MDBX_BUILD_TIMESTAMP "$(shell date +%Y-%m-%dT%H:%M:%S%z)"' \
	&& echo '#define MDBX_BUILD_FLAGS "$(CFLAGS) $(LDFLAGS) $(LIBS)"' \
	&& echo '#define MDBX_BUILD_COMPILER "$(shell set -o pipefail; $(CC) --version | head -1 || echo 'Please use GCC or CLANG compatible compiler')"' \
	&& echo '#define MDBX_BUILD_TARGET "$(shell set -o pipefail; LC_ALL=C $(CC) -v 2>&1 | grep -i '^Target:' | cut -d ' ' -f 2- || echo 'Please use GCC or CLANG compatible compiler')"' \
	) > $@

mdbx-dylib.o: config.h mdbx.c $(lastword $(MAKEFILE_LIST))
	$(CC) $(CFLAGS) $(MDBX_OPTIONS) '-DMDBX_CONFIG_H="config.h"' -DLIBMDBX_EXPORTS=1 -c mdbx.c -o $@

mdbx-static.o: config.h mdbx.c $(lastword $(MAKEFILE_LIST))
	$(CC) $(CFLAGS) $(MDBX_OPTIONS) '-DMDBX_CONFIG_H="config.h"' -ULIBMDBX_EXPORTS -c mdbx.c -o $@

mdbx_%:	mdbx_%.c libmdbx.a
	$(CC) $(CFLAGS) $(MDBX_OPTIONS) '-DMDBX_CONFIG_H="config.h"' $^ $(EXE_LDFLAGS) $(LIBS) -o $@

install: $(LIBRARIES) $(TOOLS) $(HEADERS)
	mkdir -p $(SANDBOX)$(prefix)/bin$(suffix) \
		&& cp -t $(SANDBOX)$(prefix)/bin$(suffix) $(TOOLS) && \
	mkdir -p $(SANDBOX)$(prefix)/lib$(suffix) \
		&& cp -t $(SANDBOX)$(prefix)/lib$(suffix) $(LIBRARIES) && \
	mkdir -p $(SANDBOX)$(prefix)/include \
		&& cp -t $(SANDBOX)$(prefix)/include $(HEADERS) && \
	mkdir -p $(SANDBOX)$(mandir)/man1 \
		&& cp -t $(SANDBOX)$(mandir)/man1 $(addprefix $(MAN_SRCDIR), $(MANPAGES))

################################################################################
# Benchmarking by ioarena

IOARENA ?= $(shell \
  (test -x ../ioarena/@BUILD/src/ioarena && echo ../ioarena/@BUILD/src/ioarena) || \
  (test -x ../../@BUILD/src/ioarena && echo ../../@BUILD/src/ioarena) || \
  (test -x ../../src/ioarena && echo ../../src/ioarena) || which ioarena)
NN	?= 25000000

ifneq ($(wildcard $(IOARENA)),)

.PHONY: bench clean-bench re-bench

clean-bench:
	rm -rf bench-*.txt _ioarena/*

re-bench: clean-bench bench

define bench-rule
bench-$(1)_$(2).txt: $(3) $(IOARENA) $(lastword $(MAKEFILE_LIST))
	LD_LIBRARY_PATH="./:$$$${LD_LIBRARY_PATH}" \
		$(IOARENA) -D $(1) -B crud -m nosync -n $(2) \
		| tee $$@ | grep throughput && \
	LD_LIBRARY_PATH="./:$$$${LD_LIBRARY_PATH}" \
		$(IOARENA) -D $(1) -B get,iterate -m sync -r 4 -n $(2) \
		| tee -a $$@ | grep throughput \
	|| mv -f $$@ $$@.error

endef

$(eval $(call bench-rule,mdbx,$(NN),libmdbx.$(SO_SUFFIX)))

$(eval $(call bench-rule,sophia,$(NN)))
$(eval $(call bench-rule,leveldb,$(NN)))
$(eval $(call bench-rule,rocksdb,$(NN)))
$(eval $(call bench-rule,wiredtiger,$(NN)))
$(eval $(call bench-rule,forestdb,$(NN)))
$(eval $(call bench-rule,lmdb,$(NN)))
$(eval $(call bench-rule,nessdb,$(NN)))
$(eval $(call bench-rule,sqlite3,$(NN)))
$(eval $(call bench-rule,ejdb,$(NN)))
$(eval $(call bench-rule,vedisdb,$(NN)))
$(eval $(call bench-rule,dummy,$(NN)))

$(eval $(call bench-rule,debug,10))

bench: bench-mdbx_$(NN).txt

.PHONY: bench-debug

bench-debug: bench-debug_10.txt

bench-quartet: bench-mdbx_$(NN).txt bench-lmdb_$(NN).txt bench-rocksdb_$(NN).txt bench-wiredtiger_$(NN).txt

endif
