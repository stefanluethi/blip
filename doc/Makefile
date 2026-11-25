# Build the blip documentation. Requires uv (https://docs.astral.sh/uv/).

SPHINXOPTS ?= -W --keep-going
SOURCEDIR   = source
BUILDDIR    = _build
SPHINX      = uv run sphinx-build

.PHONY: help html linkcheck serve clean screenshots

help:
	@$(SPHINX) -M help "$(SOURCEDIR)" "$(BUILDDIR)" $(SPHINXOPTS)

html:
	@$(SPHINX) -M html "$(SOURCEDIR)" "$(BUILDDIR)" $(SPHINXOPTS)

# Check the links to other sites; needs the internet. Run it before each release.
# It fails on broken links; without -W, as needflow warns for builders other than
# html.
linkcheck:
	@$(SPHINX) -M linkcheck "$(SOURCEDIR)" "$(BUILDDIR)" --keep-going

serve:
	uv run sphinx-autobuild "$(SOURCEDIR)" "$(BUILDDIR)/html" --open-browser

clean:
	rm -rf "$(BUILDDIR)"

# Render the screenshots of blip to source/screenshots from the demo recording and
# preset in blip/tests/data, in both themes, with wgpu; see
# blip/src/app/screenshots.rs. Times show in UTC. optipng, if installed, makes the
# PNGs smaller.
OPTIPNG ?= $(shell command -v optipng)
# Where the PNGs go; set it to render them elsewhere, e.g. to compare them with the
# committed ones without changing them.
SCREENSHOTS_DIR ?= $(SOURCEDIR)/screenshots

screenshots:
	cd .. && TZ=UTC BLIP_SCREENSHOTS_DIR="$(abspath $(SCREENSHOTS_DIR))" cargo test -p blip --release --features screenshots -- --ignored --test-threads 1 screenshots
	$(if $(OPTIPNG),$(OPTIPNG) -quiet -o2 "$(abspath $(SCREENSHOTS_DIR))"/*.png)

# Catch-all: forward other targets (latexpdf, linkcheck, ...) to sphinx-build.
%:
	@$(SPHINX) -M $@ "$(SOURCEDIR)" "$(BUILDDIR)" $(SPHINXOPTS)
