# Build the blip documentation. Requires uv (https://docs.astral.sh/uv/).

SPHINXOPTS ?= -W --keep-going
SOURCEDIR   = source
BUILDDIR    = _build
SPHINX      = uv run sphinx-build

.PHONY: help html serve clean

help:
	@$(SPHINX) -M help "$(SOURCEDIR)" "$(BUILDDIR)" $(SPHINXOPTS)

html:
	@$(SPHINX) -M html "$(SOURCEDIR)" "$(BUILDDIR)" $(SPHINXOPTS)

serve:
	uv run sphinx-autobuild "$(SOURCEDIR)" "$(BUILDDIR)/html" --open-browser

clean:
	rm -rf "$(BUILDDIR)"

# Catch-all: forward other targets (latexpdf, linkcheck, ...) to sphinx-build.
%:
	@$(SPHINX) -M $@ "$(SOURCEDIR)" "$(BUILDDIR)" $(SPHINXOPTS)
