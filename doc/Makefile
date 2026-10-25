# Build the blip documentation. Requires uv (https://docs.astral.sh/uv/).

SPHINXOPTS ?= -W --keep-going
SOURCEDIR   = source
BUILDDIR    = _build
SPHINX      = uv run sphinx-build

.PHONY: help html serve clean diagrams screenshots

help:
	@$(SPHINX) -M help "$(SOURCEDIR)" "$(BUILDDIR)" $(SPHINXOPTS)

html:
	@$(SPHINX) -M html "$(SOURCEDIR)" "$(BUILDDIR)" $(SPHINXOPTS)

serve:
	uv run sphinx-autobuild "$(SOURCEDIR)" "$(BUILDDIR)/html" --open-browser

clean:
	rm -rf "$(BUILDDIR)"

# Export the draw.io diagrams to SVG next to their sources, in the light colours of
# blip and, mapped by palette.py, in the dark ones (*-dark.svg). Needs the draw.io
# desktop app (drawio) on the PATH; the SVGs are committed, so building the
# documentation does not.
DRAWIO   ?= drawio
PYTHON   ?= python3
PALETTE   = $(SOURCEDIR)/diagrams/palette.py
DIAGRAMS  = $(wildcard $(SOURCEDIR)/diagrams/*.drawio)

diagrams: $(DIAGRAMS:.drawio=.svg) $(DIAGRAMS:.drawio=-dark.svg)

%-dark.svg: %.drawio $(PALETTE)
	$(PYTHON) $(PALETTE) dark "$<" "$*.dark.tmp.drawio"
	$(DRAWIO) --export --format svg --output "$@" "$*.dark.tmp.drawio"
	rm -f "$*.dark.tmp.drawio"
	$(PYTHON) $(PALETTE) fonts "$@"

%.svg: %.drawio $(PALETTE)
	$(DRAWIO) --export --format svg --output "$@" "$<"
	$(PYTHON) $(PALETTE) fonts "$@"

# Render the screenshots of blip to source/screenshots from the demo recording and
# layout in blip/tests/data, in both themes, with wgpu; see
# blip/src/app/screenshots.rs. Times show in UTC. optipng, if installed, makes the
# PNGs smaller.
OPTIPNG ?= $(shell command -v optipng)

screenshots:
	cd .. && TZ=UTC cargo test -p blip --release --features screenshots -- --ignored --test-threads 1 screenshots
	$(if $(OPTIPNG),$(OPTIPNG) -quiet -o2 $(SOURCEDIR)/screenshots/*.png)

# Catch-all: forward other targets (latexpdf, linkcheck, ...) to sphinx-build.
%:
	@$(SPHINX) -M $@ "$(SOURCEDIR)" "$(BUILDDIR)" $(SPHINXOPTS)
